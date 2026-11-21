use crate::config::{DEFAULT_BOT_AUTHOR, DEFAULT_BOT_NAME, DEFAULT_BOT_VERSION};
use crate::context::CommandContext;
use std::sync::atomic::Ordering;
use std::time::Duration;

struct ProcessStats {
    rss_mb: f64,
    hwm_mb: f64,
    threads: u32,
}

fn read_process_stats() -> ProcessStats {
    let mut rss_kb = 0u64;
    let mut hwm_kb = 0u64;
    let mut threads = 0u32;

    if let Ok(content) = std::fs::read_to_string("/proc/self/status") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                rss_kb = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("VmHWM:") {
                hwm_kb = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("Threads:") {
                threads = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or(0);
            }
        }
    }

    ProcessStats {
        rss_mb: rss_kb as f64 / 1024.0,
        hwm_mb: hwm_kb as f64 / 1024.0,
        threads,
    }
}

fn format_uptime(dur: Duration) -> String {
    let secs = dur.as_secs();
    format!(
        "{:02}h {:02}m {:02}s",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "ping" | "p" | "pong" | "lat" => {
            let wa_ping = ctx.messages.socket.ping_ms(Some(2_500)).await.ok();
            let local_ms = ctx.received_at.elapsed().as_secs_f64() * 1000.0;
            let status = match wa_ping {
                Some(ms) if ms < 100.0 => "Excelente",
                Some(ms) if ms < 300.0 => "Buena",
                Some(ms) if ms < 500.0 => "Moderada",
                Some(_) => "Alta",
                None => "Sin respuesta",
            };
            let wa_txt = wa_ping
                .map(|ms| format!("{:.2} ms", ms))
                .unwrap_or_else(|| "N/A".to_string());
            let body = format!(
                "*{bot} — Pong*\nPing WA: {wa} ({st})\nLocal: {loc:.3} ms",
                bot = DEFAULT_BOT_NAME,
                wa = wa_txt,
                st = status,
                loc = local_ms
            );
            ctx.reply_text(body).await;
            true
        }
        "ram" => {
            let stats = read_process_stats();
            let snap = ctx.state.debug.snapshot().await;
            let body = format!(
                "*{bot} — Memoria*\nVmRSS: {rss:.2} MB\nVmHWM: {hwm:.2} MB\nWorkers: {wrk} | Hilos: {thr}\nTareas activas: {act}",
                bot = DEFAULT_BOT_NAME,
                rss = stats.rss_mb,
                hwm = stats.hwm_mb,
                wrk = ctx.state.config.worker_threads,
                thr = stats.threads,
                act = snap.active_tasks
            );
            ctx.reply_text(body).await;
            true
        }
        "info" | "infobot" | "aura" => {
            let wa_ping = ctx.messages.socket.ping_ms(Some(2_500)).await.ok();
            let local_ms = ctx.received_at.elapsed().as_secs_f64() * 1000.0;
            let stats = read_process_stats();
            let snap = ctx.state.debug.snapshot().await;
            let subbots = ctx.state.subbots.active_count().await;
            let msgs = ctx.state.processed_msgs.load(Ordering::Relaxed);
            let wa_txt = wa_ping
                .map(|ms| format!("{:.2} ms", ms))
                .unwrap_or_else(|| "N/A".to_string());
            let body = format!(
                "*{bot}* v{ver} (Rust)\n\
                {author}\n\n\
                *Rendimiento*\n\
                - Ping WA: {wa}\n\
                - Local: {loc:.3} ms\n\
                - Uptime: {up}\n\
                - Mensajes: {msgs} | Comandos: {cmds}\n\
                - SubBots activos: {subs}\n\n\
                *Sistema*\n\
                - RAM: {rss:.2} MB (pico {hwm:.2} MB)\n\
                - Hilos: {thr} | Workers: {wrk}\n\
                - WA Web: {w0}.{w1}.{w2}\n\
                - Debug: {dbg} | Errores: {errs}",
                bot = DEFAULT_BOT_NAME,
                ver = DEFAULT_BOT_VERSION,
                author = DEFAULT_BOT_AUTHOR,
                wa = wa_txt,
                loc = local_ms,
                up = format_uptime(ctx.state.started_at.elapsed()),
                msgs = msgs,
                cmds = snap.total_commands,
                subs = subbots,
                rss = stats.rss_mb,
                hwm = stats.hwm_mb,
                thr = stats.threads,
                wrk = ctx.state.config.worker_threads,
                w0 = ctx.state.wa_version[0],
                w1 = ctx.state.wa_version[1],
                w2 = ctx.state.wa_version[2],
                dbg = if snap.enabled { "ON" } else { "OFF" },
                errs = snap.total_errors
            );
            ctx.reply_text(body).await;
            true
        }
        "debug" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            match ctx.text.trim().to_ascii_lowercase().as_str() {
                "on" => {
                    ctx.state.debug.set_enabled(true);
                    ctx.reply_text("Debug: *ON*").await;
                }
                "off" => {
                    ctx.state.debug.set_enabled(false);
                    ctx.reply_text("Debug: *OFF*").await;
                }
                "clear" => {
                    ctx.state.debug.clear().await;
                    ctx.reply_text("Historial de debug limpiado.").await;
                }
                _ => {
                    let snap = ctx.state.debug.snapshot().await;
                    let errs = if snap.last_errors.is_empty() {
                        "  (sin errores)".to_string()
                    } else {
                        snap.last_errors
                            .iter()
                            .rev()
                            .take(5)
                            .map(|e| format!("  - {e}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    let perf = if snap.last_timings.is_empty() {
                        "  (sin tiempos)".to_string()
                    } else {
                        snap.last_timings
                            .iter()
                            .rev()
                            .take(5)
                            .map(|t| format!("  - {t}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    ctx.reply_text(format!(
                        "*Debug* ({state})\nLag socket/recv: {sl}/{rl}\nActivas: {act} | Errores: {err}\n\n*Tiempos*\n{perf}\n\n*Errores*\n{errs}",
                        state = if snap.enabled { "ON" } else { "OFF" },
                        sl = snap.socket_lagged,
                        rl = snap.recv_lagged,
                        act = snap.active_tasks,
                        err = snap.total_errors,
                        perf = perf,
                        errs = errs
                    ))
                    .await;
                }
            }
            true
        }
        "lang" | "langs" | "idiomas" => {
            let group_lang = if ctx.is_group {
                ctx.state.db.ensure_group(&ctx.from).group_lang
            } else {
                None
            };
            let active = crate::lang::resolve_lang(&ctx.user.user_lang, group_lang.as_deref());
            let title = crate::lang::tr(&ctx.state.config.data_dir, active, "lang_header");
            let codes = crate::lang::SUPPORTED_LANGS
                .iter()
                .map(|(c, n)| format!("`{}` ({})", c, n))
                .collect::<Vec<_>>()
                .join(", ");
            ctx.reply_text(format!(
                "*{}*\nActivo: *{}* ({})\nUsuario: `{}` | Grupo: `{}`\n\nCambia con `{}idioma <codigo>` o `{}setgrouplang <codigo>`\n\n*Disponibles:*\n{}",
                title,
                crate::lang::lang_name(active),
                active,
                ctx.user.user_lang,
                group_lang.as_deref().unwrap_or("auto"),
                ctx.used_prefix,
                ctx.used_prefix,
                codes
            ))
            .await;
            true
        }
        "snipe" => {
            if let Some(msg) = ctx.state.snipe.latest_deleted(&ctx.from) {
                let preview = msg
                    .message
                    .as_ref()
                    .and_then(crate::handler::extract_message_text)
                    .unwrap_or_else(|| "[contenido multimedia]".to_string());
                ctx.reply_text(format!("*Ultimo mensaje borrado:*\n{}", preview))
                    .await;
            } else {
                ctx.reply_text("No hay mensajes eliminados recientes en este chat.")
                    .await;
            }
            true
        }
        _ => false,
    }
}
