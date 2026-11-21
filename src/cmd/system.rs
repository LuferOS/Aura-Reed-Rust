use crate::config::{DEFAULT_BOT_AUTHOR, DEFAULT_BOT_NAME, DEFAULT_BOT_VERSION};
use crate::context::CommandContext;
use crate::ui::{box_section, footer_tag, format_duration, fyt_bold};
use std::sync::atomic::Ordering;
use std::time::Duration;

struct ProcessStats {
    rss_mb: f64,
    hwm_mb: f64,
    threads: u32,
}

struct HostMem {
    total_gb: f64,
    avail_gb: f64,
    used_pct: f64,
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

fn read_host_mem() -> HostMem {
    let mut total_kb = 0u64;
    let mut avail_kb = 0u64;
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                total_kb = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail_kb = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(0);
            }
        }
    }
    let total_gb = total_kb as f64 / (1024.0 * 1024.0);
    let avail_gb = avail_kb as f64 / (1024.0 * 1024.0);
    let used_pct = if total_kb > 0 {
        ((total_kb.saturating_sub(avail_kb)) as f64 / total_kb as f64) * 100.0
    } else {
        0.0
    };
    HostMem {
        total_gb,
        avail_gb,
        used_pct,
    }
}

fn read_host_uptime() -> Duration {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .map(Duration::from_secs_f64)
        .unwrap_or_default()
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "ping" | "p" | "pong" | "lat" => {
            let local_ms = ctx.received_at.elapsed().as_secs_f64() * 1000.0;
            let wa_ping = ctx.messages.socket.ping_ms(Some(2_500)).await.ok();
            let (badge, state_txt) = match wa_ping {
                Some(ms) if ms < 100.0 => ("🟢 Excelente", "Sistema estable"),
                Some(ms) if ms < 300.0 => ("🟡 Buena", "Operacion normal"),
                Some(ms) if ms < 500.0 => ("🟠 Moderada", "Estabilizando"),
                Some(_) => ("🔴 Alta", "Latencia elevada"),
                None => ("⚪ Sin respuesta", "Timeout WA"),
            };
            let wa_txt = wa_ping
                .map(|ms| format!("{:.2} ms", ms))
                .unwrap_or_else(|| "N/A".to_string());
            let body = format!(
                "{}\n\n{}",
                box_section(
                    "⚡",
                    "RESULTADO DE VELOCIDAD",
                    &[
                        format!("> {} › {}", fyt_bold("Ping WA"), wa_txt),
                        format!("> {} › {:.3} ms", fyt_bold("Local Rust"), local_ms),
                        format!("> {} › {}", fyt_bold("Estado"), badge),
                        format!("> {} › {}", fyt_bold("Sistema"), state_txt),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_text(body).await;
            true
        }
        "ram" => {
            let stats = read_process_stats();
            let host = read_host_mem();
            let snap = ctx.state.debug.snapshot().await;
            let body = format!(
                "{}\n\n{}",
                box_section(
                    "🧠",
                    "MEMORIA",
                    &[
                        format!("> {} › {:.2} MB", fyt_bold("Bot usa"), stats.rss_mb),
                        format!("> {} › {:.2} MB", fyt_bold("Pico HWM"), stats.hwm_mb),
                        format!(
                            "> {} › {:.2} GB / {:.2} GB ({:.1}%)",
                            fyt_bold("Host"),
                            host.total_gb - host.avail_gb,
                            host.total_gb,
                            host.used_pct
                        ),
                        format!("> {} › {:.2} GB", fyt_bold("Libre"), host.avail_gb),
                        format!(
                            "> {} › {} | Workers: {} | Tareas: {}",
                            fyt_bold("Hilos"),
                            stats.threads,
                            ctx.state.config.worker_threads,
                            snap.active_tasks
                        ),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_text(body).await;
            true
        }
        "info" | "infobot" | "botinfo" | "system" | "sys" | "aura" => {
            let local_ms = ctx.received_at.elapsed().as_secs_f64() * 1000.0;
            let wa_ping = ctx.messages.socket.ping_ms(Some(2_500)).await.ok();
            let stats = read_process_stats();
            let host = read_host_mem();
            let host_up = read_host_uptime();
            let snap = ctx.state.debug.snapshot().await;
            let subbots = ctx.state.subbots.active_count().await;
            let msgs = ctx.state.processed_msgs.load(Ordering::Relaxed);
            let wa_txt = wa_ping
                .map(|ms| format!("{:.2} ms", ms))
                .unwrap_or_else(|| "N/A".to_string());

            let sec_perf = box_section(
                "⚡",
                DEFAULT_BOT_NAME,
                &[
                    format!(
                        "> {} › v{} (Rust Native)",
                        fyt_bold("Version"),
                        DEFAULT_BOT_VERSION
                    ),
                    format!("> {} › {}", fyt_bold("Autor"), DEFAULT_BOT_AUTHOR),
                    format!("> {} › {} | Local: {:.3} ms", fyt_bold("Ping"), wa_txt, local_ms),
                    format!(
                        "> {} › {} | Comandos: {}",
                        fyt_bold("Mensajes"),
                        msgs,
                        snap.total_commands
                    ),
                    format!("> {} › {}", fyt_bold("SubBots"), subbots),
                ],
            );

            let sec_mem = box_section(
                "🧠",
                "MEMORIA",
                &[
                    format!("> {} › {:.2} MB (pico {:.2} MB)", fyt_bold("Bot usa"), stats.rss_mb, stats.hwm_mb),
                    format!("> {} › {:.2} GB", fyt_bold("Total host"), host.total_gb),
                    format!("> {} › {:.2} GB", fyt_bold("Libre"), host.avail_gb),
                ],
            );

            let sec_up = box_section(
                "⏱️",
                "UPTIME",
                &[
                    format!("> {} › {}", fyt_bold("Bot activo"), format_duration(ctx.state.started_at.elapsed())),
                    format!("> {} › {}", fyt_bold("Host encendido"), format_duration(host_up)),
                ],
            );

            let sec_env = box_section(
                "⚙️",
                "ENTORNO",
                &[
                    format!("> {} › Rust 2021 ({})", fyt_bold("Runtime"), std::env::consts::ARCH),
                    format!("> {} › {}", fyt_bold("PID"), std::process::id()),
                    format!("> {} › {} | Workers: {}", fyt_bold("Hilos"), stats.threads, ctx.state.config.worker_threads),
                    format!(
                        "> {} › {}.{}.{}",
                        fyt_bold("WA Web"),
                        ctx.state.wa_version[0],
                        ctx.state.wa_version[1],
                        ctx.state.wa_version[2]
                    ),
                    format!(
                        "> {} › {} | Errores: {}",
                        fyt_bold("Debug"),
                        if snap.enabled { "ON" } else { "OFF" },
                        snap.total_errors
                    ),
                ],
            );

            let body = format!(
                "{}\n\n{}\n\n{}\n\n{}\n\n{}",
                sec_perf,
                sec_mem,
                sec_up,
                sec_env,
                footer_tag("REED SYSTEM")
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
                        "{}\n\n*Tiempos*\n{}\n\n*Errores*\n{}",
                        box_section(
                            "🛠️",
                            "DEBUG",
                            &[
                                format!("> Estado › {}", if snap.enabled { "ON" } else { "OFF" }),
                                format!("> Lag socket/recv › {}/{}", snap.socket_lagged, snap.recv_lagged),
                                format!("> Activas › {} | Errores › {}", snap.active_tasks, snap.total_errors),
                            ],
                        ),
                        perf,
                        errs
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
            let body = format!(
                "{}\n\n*Disponibles:*\n{}\n\n{}",
                box_section(
                    "🌐",
                    &title.to_uppercase(),
                    &[
                        format!("> Activo › {} (`{}`)", crate::lang::lang_name(active), active),
                        format!("> Usuario › `{}` | Grupo › `{}`", ctx.user.user_lang, group_lang.as_deref().unwrap_or("auto")),
                        format!("> Cambiar › `{}idioma <cod>` | `{}setgrouplang <cod>`", ctx.used_prefix, ctx.used_prefix),
                    ],
                ),
                codes,
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_text(body).await;
            true
        }
        "snipe" => {
            if let Some(msg) = ctx.state.snipe.latest_deleted(&ctx.from) {
                let preview = msg
                    .message
                    .as_ref()
                    .and_then(crate::handler::extract_message_text)
                    .unwrap_or_else(|| "[contenido multimedia]".to_string());
                let body = format!(
                    "{}\n\n{}",
                    box_section("🕵️", "SNIPE", &[preview]),
                    footer_tag(DEFAULT_BOT_NAME)
                );
                ctx.reply_text(body).await;
            } else {
                ctx.reply_text("No hay mensajes eliminados recientes en este chat.")
                    .await;
            }
            true
        }
        _ => false,
    }
}
