use crate::config::DEFAULT_BOT_NAME;
use crate::context::CommandContext;
use crate::ui::{box_section, footer_tag, jid_tag};
use baileys_rs::defaults::MediaType;
use baileys_rs::proto::message::{AudioMessage, ExtendedTextMessage, ImageMessage, VideoMessage};
use baileys_rs::utils::{
    download_content_from_message, normalize_message_content, unix_timestamp_seconds,
    DownloadableMessage, MediaDownloadOptions,
};
use std::borrow::Borrow;
use std::sync::atomic::{AtomicU64, Ordering};

static UTIL_SEQ: AtomicU64 = AtomicU64::new(0);

enum ViewOnceTarget {
    Image(ImageMessage),
    Video(VideoMessage),
    Audio(AudioMessage),
}

fn find_quoted_media(ctx: &CommandContext) -> Option<ViewOnceTarget> {
    let proto = ctx.message.message.as_ref()?;
    let norm = normalize_message_content(proto);
    let ext = norm.extended_text_message.as_ref()?;
    let inner: &ExtendedTextMessage = Borrow::borrow(ext);
    let q = inner.context_info.as_ref()?.quoted_message.as_ref()?;
    let q_norm = normalize_message_content(q);
    if let Some(ref img) = q_norm.image_message {
        let i: &ImageMessage = Borrow::borrow(img);
        return Some(ViewOnceTarget::Image(i.clone()));
    }
    if let Some(ref vid) = q_norm.video_message {
        let v: &VideoMessage = Borrow::borrow(vid);
        return Some(ViewOnceTarget::Video(v.clone()));
    }
    if let Some(ref aud) = q_norm.audio_message {
        let a: &AudioMessage = Borrow::borrow(aud);
        return Some(ViewOnceTarget::Audio(a.clone()));
    }
    None
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "lid" | "jid" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            ctx.reply_with_mentions(
                format!(
                    "{}\n\n{}",
                    box_section(
                        "🆔",
                        "IDENTIFICADORES",
                        &[
                            format!("> Chat › `{}`", ctx.from),
                            format!("> Usuario › `{}`", target),
                            format!("> LID DB › `{}`", u.lid.as_deref().unwrap_or("N/A")),
                            format!("> Rol › `{}`", u.role),
                        ],
                    ),
                    footer_tag(DEFAULT_BOT_NAME)
                ),
                vec![target],
            )
            .await;
            true
        }
        "read" => {
            let Some(media) = find_quoted_media(ctx) else {
                ctx.reply_text("Responde a una imagen, video o audio (incluidos los de una sola vista) con `.read`.")
                    .await;
                return true;
            };
            let dl_opts = MediaDownloadOptions {
                timeout_ms: Some(15_000),
                ..Default::default()
            };
            match media {
                ViewOnceTarget::Image(img) => {
                    let dl = DownloadableMessage {
                        media_key: img.media_key,
                        direct_path: img.direct_path,
                        url: img.url,
                    };
                    match download_content_from_message(&dl, MediaType::Image, &dl_opts).await {
                        Ok(bytes) => {
                            let mime = img.mimetype.as_deref().unwrap_or("image/jpeg");
                            if let Err(err) = ctx.send_image_bytes(bytes, img.caption, Vec::new(), mime).await {
                                ctx.reply_text(format!("Error reenviando imagen: {}", err)).await;
                            }
                        }
                        Err(err) => ctx.reply_text(format!("Error descargando imagen: {}", err)).await,
                    }
                }
                ViewOnceTarget::Video(vid) => {
                    let dl = DownloadableMessage {
                        media_key: vid.media_key,
                        direct_path: vid.direct_path,
                        url: vid.url,
                    };
                    match download_content_from_message(&dl, MediaType::Video, &dl_opts).await {
                        Ok(bytes) => {
                            let mime = vid.mimetype.as_deref().unwrap_or("video/mp4");
                            if let Err(err) = ctx.send_video_bytes(bytes, vid.caption, mime).await {
                                ctx.reply_text(format!("Error reenviando video: {}", err)).await;
                            }
                        }
                        Err(err) => ctx.reply_text(format!("Error descargando video: {}", err)).await,
                    }
                }
                ViewOnceTarget::Audio(aud) => {
                    let dl = DownloadableMessage {
                        media_key: aud.media_key,
                        direct_path: aud.direct_path,
                        url: aud.url,
                    };
                    match download_content_from_message(&dl, MediaType::Audio, &dl_opts).await {
                        Ok(bytes) => {
                            let mime = aud.mimetype.as_deref().unwrap_or("audio/ogg; codecs=opus");
                            if let Err(err) = ctx.send_audio_bytes(bytes, mime, aud.ptt.unwrap_or(false)).await {
                                ctx.reply_text(format!("Error reenviando audio: {}", err)).await;
                            }
                        }
                        Err(err) => ctx.reply_text(format!("Error descargando audio: {}", err)).await,
                    }
                }
            }
            true
        }
        "compress" | "comprimir" | "optvideo" => {
            let Some(ViewOnceTarget::Video(vid)) = find_quoted_media(ctx) else {
                ctx.reply_text(format!("Responde a un video con `{}compress`.", ctx.used_prefix))
                    .await;
                return true;
            };
            let dl = DownloadableMessage {
                media_key: vid.media_key,
                direct_path: vid.direct_path,
                url: vid.url,
            };
            let dl_opts = MediaDownloadOptions {
                timeout_ms: Some(20_000),
                ..Default::default()
            };
            let raw = match download_content_from_message(&dl, MediaType::Video, &dl_opts).await {
                Ok(b) => b,
                Err(err) => {
                    ctx.reply_text(format!("Error descargando video: {}", err))
                        .await;
                    return true;
                }
            };
            let orig_mb = raw.len() as f64 / (1024.0 * 1024.0);
            let _ = tokio::fs::create_dir_all(&ctx.state.config.cache_dir).await;
            let seq = UTIL_SEQ.fetch_add(1, Ordering::Relaxed);
            let stamp = format!("{}_{}_{}", std::process::id(), unix_timestamp_seconds(), seq);
            let in_p = ctx.state.config.cache_dir.join(format!("cmp_in_{}.mp4", stamp));
            let out_p = ctx.state.config.cache_dir.join(format!("cmp_out_{}.mp4", stamp));
            if tokio::fs::write(&in_p, &raw).await.is_err() {
                ctx.reply_text("Error escribiendo temporal.").await;
                return true;
            }
            let ok = tokio::process::Command::new("ffmpeg")
                .args(["-y", "-loglevel", "error", "-i"])
                .arg(&in_p)
                .args([
                    "-c:v",
                    "libx264",
                    "-crf",
                    "30",
                    "-preset",
                    "veryfast",
                    "-vf",
                    "scale='min(720,iw)':-2",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "96k",
                    "-movflags",
                    "+faststart",
                ])
                .arg(&out_p)
                .output()
                .await
                .map(|o| o.status.success())
                .unwrap_or(false);
            let compressed = if ok {
                tokio::fs::read(&out_p).await.ok()
            } else {
                None
            };
            tokio::spawn(async move {
                let _ = tokio::fs::remove_file(in_p).await;
                let _ = tokio::fs::remove_file(out_p).await;
            });
            match compressed {
                Some(bytes) => {
                    let new_mb = bytes.len() as f64 / (1024.0 * 1024.0);
                    let cap = format!("Video comprimido: {:.2} MB -> {:.2} MB", orig_mb, new_mb);
                    if let Err(err) = ctx.send_video_bytes(bytes, Some(cap), "video/mp4").await {
                        ctx.reply_text(format!("Error enviando video: {}", err)).await;
                    }
                }
                None => ctx.reply_text("No se pudo comprimir el video con ffmpeg.").await,
            }
            true
        }
        "report" | "bug" | "sug" | "sugerencia" => {
            let msg_txt = ctx.text.trim();
            if msg_txt.is_empty() {
                ctx.reply_text(format!("Uso: `{}{} <descripcion>`", ctx.used_prefix, ctx.cmd_name))
                    .await;
                return true;
            }
            ctx.state
                .debug
                .record_error("user_report", format!("from={} user={} msg={}", ctx.from, ctx.sender, msg_txt))
                .await;
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section("✅", "REPORTE REGISTRADO", &[format!("> Mensaje › {}", msg_txt)]),
                footer_tag(DEFAULT_BOT_NAME)
            ))
            .await;
            true
        }
        "get" | "fetch" => {
            let raw = ctx.args.first().map(|s| s.trim()).unwrap_or("");
            if raw.is_empty() {
                ctx.reply_text(format!("Uso: `{}get <url>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let url = if raw.starts_with("http://") || raw.starts_with("https://") {
                raw.to_string()
            } else {
                format!("https://{}", raw)
            };
            match ctx.state.http.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let ct = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("unknown")
                        .to_string();
                    let body = resp.text().await.unwrap_or_default();
                    let preview: String = body.chars().take(1800).collect();
                    ctx.reply_text(format!(
                        "*HTTP {}* ({})\n\n```{}```",
                        status.as_u16(),
                        ct,
                        preview
                    ))
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error HTTP: {}", err)).await;
                }
            }
            true
        }
        "setrole" | "setuserrole" | "rol" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let target = ctx.target_jid();
            let role = ctx
                .args
                .iter()
                .find(|a| !a.starts_with('@'))
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if target == ctx.sender
                || !matches!(role.as_str(), "user" | "premium" | "mod" | "coowner" | "owner")
            {
                ctx.reply_text(format!(
                    "Uso: `{}setrole @usuario <user|premium|mod|coowner|owner>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            ctx.state
                .db
                .update_user_field(&target, "role", Some(&role));
            ctx.reply_with_mentions(
                format!("Rol de @{} actualizado a *{}*.", jid_tag(&target), role),
                vec![target],
            )
            .await;
            true
        }
        "banuser" | "unbanuser" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde a otro usuario.").await;
                return true;
            }
            let banned = ctx.cmd_name == "banuser";
            ctx.state.db.set_user_banned(&target, banned);
            ctx.reply_with_mentions(
                format!(
                    "@{} ha sido *{}*.",
                    jid_tag(&target),
                    if banned { "baneado" } else { "desbaneado" }
                ),
                vec![target],
            )
            .await;
            true
        }
        _ => false,
    }
}
