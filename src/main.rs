mod cmd;
mod config;
mod context;
mod db;
mod debug;
mod handler;
mod lang;
mod snipe;
mod ui;

use baileys_rs::signal::LibSignalRepository;
use baileys_rs::socket::{MessagesRecvSocket, MessagesSocket, RecvEvent, SubBotEvent, SubBotManager, WaSocket};
use baileys_rs::types::{
    AuthenticationCreds, ConnectionState, DisconnectReason, SocketConfig, SocketEvent, WaPresence,
};
use baileys_rs::utils::{
    extract_call_telemetry, fetch_latest_wa_web_version, get_next_pre_keys_node,
    unix_timestamp_seconds, use_multi_file_auth_state, Browsers,
};
use config::{AppConfig, DEFAULT_BOT_NAME, DEFAULT_BOT_VERSION};
use context::BotState;
use db::AuraDb;
use debug::DebugState;
use qrcode::render::unicode;
use qrcode::QrCode;
use serde::{Deserialize, Serialize};
use snipe::SnipeStore;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{Mutex, Semaphore};

#[derive(Serialize, Deserialize)]
struct CachedWaVersion {
    version: [u32; 3],
    updated_at: u64,
}

async fn resolve_wa_version(data_dir: &Path) -> [u32; 3] {
    let cache_path = data_dir.join("wa_version.json");
    let now = unix_timestamp_seconds();
    if let Ok(bytes) = tokio::fs::read(&cache_path).await {
        if let Ok(cached) = serde_json::from_slice::<CachedWaVersion>(&bytes) {
            if now.saturating_sub(cached.updated_at) < 86_400 {
                return cached.version;
            }
        }
    }
    let fetched = fetch_latest_wa_web_version(Some(2_500)).await;
    if fetched.is_latest {
        let _ = tokio::fs::create_dir_all(data_dir).await;
        if let Ok(json) = serde_json::to_vec(&CachedWaVersion {
            version: fetched.version,
            updated_at: now,
        }) {
            let _ = tokio::fs::write(&cache_path, json).await;
        }
    }
    fetched.version
}

fn render_qr_terminal(qr_data: &str) {
    if let Ok(code) = QrCode::new(qr_data.as_bytes()) {
        let rendered = code
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Dark)
            .light_color(unicode::Dense1x2::Light)
            .quiet_zone(true)
            .build();
        println!("\nEscanea este QR en WhatsApp -> Dispositivos vinculados:\n");
        println!("{}", rendered);
    }
}

async fn save_creds_atomic(
    auth_dir: &Path,
    creds: &AuthenticationCreds,
    lock: &Mutex<()>,
) -> Result<(), std::io::Error> {
    let _guard = lock.lock().await;
    tokio::fs::create_dir_all(auth_dir).await?;
    let bytes = serde_json::to_vec(creds)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let tmp = auth_dir.join("creds.json.tmp");
    let dst = auth_dir.join("creds.json");
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, &dst).await
}

async fn activate_session(
    messages: &Arc<MessagesSocket>,
    state: &Arc<BotState>,
    activated: &AtomicBool,
    auth_dir: &Path,
    connect_started: Instant,
) {
    if activated.swap(true, Ordering::SeqCst) {
        return;
    }

    let mut creds_now = messages.socket.creds().await;
    if creds_now.first_unuploaded_pre_key_id <= 1 && !auth_dir.join("pre-key-1.json").exists() {
        if let Ok(pk) = get_next_pre_keys_node(&messages.auth, 30).await {
            if messages.socket.query(pk.node, Some(4_000)).await.is_ok() {
                creds_now.first_unuploaded_pre_key_id = pk.update.first_unuploaded_pre_key_id;
                creds_now.next_pre_key_id = pk.update.next_pre_key_id;
                let _ = save_creds_atomic(auth_dir, &creds_now, &state.creds_lock).await;
            }
        }
    }

    let _ = tokio::join!(
        messages.socket.send_passive_iq(true),
        messages
            .socket
            .send_presence_update(WaPresence::Available, None)
    );

    let warm = Arc::clone(messages);
    tokio::spawn(async move {
        let _ = warm.refresh_media_conn(false).await;
    });

    println!(
        "[{}] conectado como {:?} en {:.0} ms",
        DEFAULT_BOT_NAME,
        creds_now.me.as_ref().map(|m| &m.id),
        connect_started.elapsed().as_secs_f64() * 1000.0
    );
}

fn spawn_subbot_event_loop(state: Arc<BotState>) {
    let mut rx = state.subbots.subscribe();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(SubBotEvent::PairingCode { session_id, code }) => {
                    let formatted = if code.len() == 8 {
                        format!("{}-{}", &code[..4], &code[4..])
                    } else {
                        code
                    };
                    println!("[SubBot {}] Pairing Code: {}", session_id, formatted);
                }
                Ok(SubBotEvent::Connected { session_id, jid, .. }) => {
                    println!("[SubBot {}] conectado como {:?}", session_id, jid);
                }
                Ok(SubBotEvent::Message {
                    session_id,
                    message,
                    is_offline,
                }) => {
                    if let Some(sock) = state.subbots.get_messages_socket(&session_id).await {
                        let st = Arc::clone(&state);
                        tokio::spawn(async move {
                            Box::pin(handler::handle_message(st, sock, message, is_offline)).await;
                        });
                    }
                }
                Ok(SubBotEvent::Call {
                    session_id,
                    telemetry,
                }) => {
                    state.debug.log(
                        "subbot_call",
                        format!(
                            "sub={} from={:?} type={} endpoints={:?}",
                            session_id, telemetry.from, telemetry.event_type, telemetry.endpoints
                        ),
                    );
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => break,
            }
        }
    });
}

async fn run_bot(cfg: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
    let _ = tokio::fs::create_dir_all(&cfg.cache_dir).await;
    let _ = tokio::fs::create_dir_all(&cfg.data_dir).await;
    let _ = tokio::fs::create_dir_all(&cfg.sub_sessions_dir).await;
    let _ = tokio::fs::create_dir_all(&cfg.prem_sessions_dir).await;

    let wa_version = resolve_wa_version(&cfg.data_dir).await;
    let db = Arc::new(AuraDb::open(&cfg.data_dir)?);
    let is_perf = cfg.mode.is_perf();
    db.apply_mode(is_perf);

    let state = Arc::new(BotState {
        config: cfg.clone(),
        started_at: Instant::now(),
        wa_version,
        processed_msgs: AtomicU64::new(0),
        mode_perf: AtomicBool::new(is_perf),
        db: Arc::clone(&db),
        snipe: Arc::new(SnipeStore::new(if is_perf { 96 } else { 24 })),
        debug: Arc::new(DebugState::new(cfg.debug_enabled)),
        subbots: Arc::new(SubBotManager::new(if is_perf { 256 } else { 64 })),
        http: BotState::build_http_client(is_perf),
        cmd_semaphore: Arc::new(Semaphore::new(if is_perf { 24 } else { 4 })),
        ffmpeg_sem: Arc::new(Semaphore::new(if is_perf { 2 } else { 1 })),
        creds_lock: Arc::new(Mutex::new(())),
        proposals: std::sync::Mutex::new(std::collections::HashMap::new()),
        spam_tracker: std::sync::Mutex::new(std::collections::HashMap::new()),
        group_meta_cache: std::sync::Mutex::new(std::collections::HashMap::new()),
        hangman_games: std::sync::Mutex::new(std::collections::HashMap::new()),
        group_activity: std::sync::Mutex::new(std::collections::HashMap::new()),
        gacha_trades: std::sync::Mutex::new(std::collections::HashMap::new()),
    });

    if cfg.start_subbots {
        spawn_subbot_event_loop(Arc::clone(&state));
    }

    let db_maint = Arc::clone(&db);
    tokio::spawn(async move {
        let maint_secs = if is_perf { 300 } else { 90 };
        let mut interval = tokio::time::interval(Duration::from_secs(maint_secs));
        loop {
            interval.tick().await;
            db_maint.maintenance();
        }
    });

    println!(
        "{} Rust v{} | mode={} | WA Web {}.{}.{} | workers={} | debug={}",
        DEFAULT_BOT_NAME,
        DEFAULT_BOT_VERSION,
        cfg.mode.as_str(),
        wa_version[0],
        wa_version[1],
        wa_version[2],
        cfg.worker_threads,
        if cfg.debug_enabled { "ON" } else { "OFF" }
    );

    let mut backoff_ms: u64 = 250;

    loop {
        let connect_started = Instant::now();
        let auth = Arc::new(use_multi_file_auth_state(&cfg.main_session_dir).await?);
        let registered = auth.creds.registered;

        let mut sock_cfg = SocketConfig::default();
        sock_cfg.version = wa_version;
        sock_cfg.browser = Browsers::ubuntu("Chrome");
        sock_cfg.mark_online_on_connect = true;
        sock_cfg.connect_timeout_ms = 8_000;
        sock_cfg.default_query_timeout_ms = 8_000;

        let socket = match WaSocket::connect(sock_cfg, auth.creds.clone()).await {
            Ok(s) => s,
            Err(err) => {
                state
                    .debug
                    .record_error("connect", format!("{}", err))
                    .await;
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(5_000);
                continue;
            }
        };

        let signal_repo = Arc::new(LibSignalRepository::new(Arc::clone(&auth), None));
        let messages = Arc::new(MessagesSocket::new(
            socket,
            Arc::clone(&auth),
            Arc::clone(&signal_repo),
        ));
        let recv = Arc::new(MessagesRecvSocket::new(Arc::clone(&messages)));

        let mut socket_rx = messages.socket.subscribe();
        let mut recv_rx = recv.subscribe();
        let activated = Arc::new(AtomicBool::new(false));
        let pairing_mode = cfg.pair_phone.is_some();

        if registered {
            let m = Arc::clone(&messages);
            let st = Arc::clone(&state);
            let act = Arc::clone(&activated);
            let dir = cfg.main_session_dir.clone();
            tokio::spawn(async move {
                for _ in 0..40 {
                    if m.socket.is_open() {
                        activate_session(&m, &st, &act, &dir, connect_started).await;
                        return;
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                activate_session(&m, &st, &act, &dir, connect_started).await;
            });
        } else if let Some(ref phone) = cfg.pair_phone {
            let m = Arc::clone(&messages);
            let dbg = Arc::clone(&state.debug);
            let phone_clone = phone.clone();
            tokio::spawn(async move {
                for _ in 0..50 {
                    if m.socket.is_open() {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
                match m.socket.request_pairing_code(&phone_clone, None).await {
                    Ok(code) => {
                        let formatted = if code.len() == 8 {
                            format!("{}-{}", &code[..4], &code[4..])
                        } else {
                            code
                        };
                        println!("\nPairing Code: {}\n", formatted);
                    }
                    Err(err) => {
                        dbg.record_error("pairing", format!("{}", err)).await;
                    }
                }
            });
        }

        let msg_sock = Arc::clone(&messages);
        let st_recv = Arc::clone(&state);
        let recv_task = tokio::spawn(async move {
            loop {
                match recv_rx.recv().await {
                    Ok(RecvEvent::MessageUpsert {
                        message,
                        is_offline,
                    }) => {
                        st_recv.processed_msgs.fetch_add(1, Ordering::Relaxed);
                        let sock_c = Arc::clone(&msg_sock);
                        let st_c = Arc::clone(&st_recv);
                        tokio::spawn(async move {
                            let Ok(_permit) = st_c.cmd_semaphore.clone().acquire_owned().await else {
                                return;
                            };
                            st_c.debug.active_tasks.fetch_add(1, Ordering::Relaxed);
                                                        let _ = tokio::time::timeout(
                                Duration::from_secs(180),
                                Box::pin(handler::handle_message(st_c.clone(), sock_c, message, is_offline)),
                            )
                            .await;
                            st_c.debug.active_tasks.fetch_sub(1, Ordering::Relaxed);
                        });
                    }
                    Ok(_) => {}
                    Err(RecvError::Lagged(skipped)) => {
                        st_recv
                            .debug
                            .recv_lagged
                            .fetch_add(skipped, Ordering::Relaxed);
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        });

        let mut reconnect = true;

        loop {
            match socket_rx.recv().await {
                Ok(SocketEvent::ConnectionUpdate(upd)) => {
                    if let Some(qr) = upd.qr {
                        if !pairing_mode {
                            render_qr_terminal(&qr);
                        }
                    }
                    if let Some(ConnectionState::Open) = upd.connection {
                        backoff_ms = 250;
                        let m = Arc::clone(&messages);
                        let st = Arc::clone(&state);
                        let act = Arc::clone(&activated);
                        let dir = cfg.main_session_dir.clone();
                        tokio::spawn(async move {
                            activate_session(&m, &st, &act, &dir, connect_started).await;
                        });
                    }
                    if let Some(ConnectionState::Close) = upd.connection {
                        let code = upd.last_disconnect.as_ref().and_then(|d| d.status_code);
                        state
                            .debug
                            .record_error("disconnect", format!("codigo={:?}", code))
                            .await;
                        if !pairing_mode
                            && (code == Some(DisconnectReason::LoggedOut as u16)
                                || code == Some(401))
                        {
                            let _ = tokio::fs::remove_dir_all(&cfg.main_session_dir).await;
                            reconnect = false;
                        }
                        break;
                    }
                }
                Ok(SocketEvent::CredsUpdate(creds)) => {
                    let dir = cfg.main_session_dir.clone();
                    let lock = Arc::clone(&state.creds_lock);
                    let dbg = Arc::clone(&state.debug);
                    tokio::spawn(async move {
                        if let Err(err) = save_creds_atomic(&dir, &creds, &lock).await {
                            dbg.record_error("creds_save", format!("{}", err)).await;
                        }
                    });
                }
                Ok(SocketEvent::Node(node)) => {
                    if let Some(tel) = extract_call_telemetry(&node) {
                        state.debug.log(
                            "call",
                            format!(
                                "from={:?} type={} endpoints={:?}",
                                tel.from, tel.event_type, tel.endpoints
                            ),
                        );
                    }
                    if registered && !activated.load(Ordering::SeqCst) {
                        let m = Arc::clone(&messages);
                        let st = Arc::clone(&state);
                        let act = Arc::clone(&activated);
                        let dir = cfg.main_session_dir.clone();
                        tokio::spawn(async move {
                            activate_session(&m, &st, &act, &dir, connect_started).await;
                        });
                    }
                    let r = Arc::clone(&recv);
                    tokio::spawn(async move {
                        let _ = r.process_node(&node).await;
                    });
                }
                Err(RecvError::Lagged(skipped)) => {
                    state
                        .debug
                        .socket_lagged
                        .fetch_add(skipped, Ordering::Relaxed);
                }
                Err(RecvError::Closed) => break,
            }
        }

        recv_task.abort();
        if !reconnect {
            break;
        }
        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
        backoff_ms = (backoff_ms * 2).min(5_000);
    }

    db.checkpoint();
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = AppConfig::from_env_and_args();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(cfg.worker_threads)
        .max_blocking_threads(if cfg.mode.is_perf() { 8 } else { 2 })
        .thread_stack_size(1024 * 1024)
        .enable_all()
        .build()?;

    runtime.block_on(run_bot(cfg))
}
