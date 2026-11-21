use crate::context::CommandContext;
use baileys_rs::defaults::MediaType;
use baileys_rs::proto::message::{ExtendedTextMessage, ImageMessage, StickerMessage, VideoMessage};
use baileys_rs::types::WaMessage;
use baileys_rs::utils::{
    download_content_from_message, normalize_message_content, unix_timestamp_seconds,
    DownloadableMessage, MediaDownloadOptions,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::Value;
use std::borrow::Borrow;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static STICKER_SEQ: AtomicU64 = AtomicU64::new(0);

enum StickerInput {
    Image(ImageMessage),
    Video(VideoMessage),
}

fn find_sticker_input(msg: &WaMessage) -> Option<StickerInput> {
    let proto = msg.message.as_ref()?;
    let norm = normalize_message_content(proto);
    if let Some(ref img) = norm.image_message {
        let inner: &ImageMessage = Borrow::borrow(img);
        return Some(StickerInput::Image(inner.clone()));
    }
    if let Some(ref vid) = norm.video_message {
        let inner: &VideoMessage = Borrow::borrow(vid);
        return Some(StickerInput::Video(inner.clone()));
    }
    if let Some(ref ext) = norm.extended_text_message {
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        if let Some(ref c) = inner.context_info {
            if let Some(ref quoted_msg) = c.quoted_message {
                let q_norm = normalize_message_content(quoted_msg);
                if let Some(ref q_img) = q_norm.image_message {
                    let q_inner: &ImageMessage = Borrow::borrow(q_img);
                    return Some(StickerInput::Image(q_inner.clone()));
                }
                if let Some(ref q_vid) = q_norm.video_message {
                    let q_inner: &VideoMessage = Borrow::borrow(q_vid);
                    return Some(StickerInput::Video(q_inner.clone()));
                }
            }
        }
    }
    None
}

fn find_quoted_sticker(msg: &WaMessage) -> Option<StickerMessage> {
    let proto = msg.message.as_ref()?;
    let norm = normalize_message_content(proto);
    if let Some(ref st) = norm.sticker_message {
        let inner: &StickerMessage = Borrow::borrow(st);
        return Some(inner.clone());
    }
    if let Some(ref ext) = norm.extended_text_message {
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        if let Some(ref c) = inner.context_info {
            if let Some(ref quoted_msg) = c.quoted_message {
                let q_norm = normalize_message_content(quoted_msg);
                if let Some(ref q_st) = q_norm.sticker_message {
                    let q_inner: &StickerMessage = Borrow::borrow(q_st);
                    return Some(q_inner.clone());
                }
            }
        }
    }
    None
}

async fn encode_webp(raw_bytes: &[u8], animated: bool, tmp_dir: &Path) -> Result<Vec<u8>, String> {
    let _ = tokio::fs::create_dir_all(tmp_dir).await;
    let seq = STICKER_SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = format!("{}_{}_{}", std::process::id(), unix_timestamp_seconds(), seq);
    let ext = if animated { "mp4" } else { "img" };
    let in_path = tmp_dir.join(format!("s_in_{}.{}", stamp, ext));
    let out_path = tmp_dir.join(format!("s_out_{}.webp", stamp));

    tokio::fs::write(&in_path, raw_bytes)
        .await
        .map_err(|e| format!("write tmp: {}", e))?;

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.args(["-y", "-loglevel", "error", "-threads", "4", "-i"])
        .arg(&in_path);

    if animated {
        cmd.args([
            "-t",
            "6",
            "-vf",
            "fps=12,scale=512:512:force_original_aspect_ratio=decrease:flags=fast_bilinear,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000",
            "-c:v",
            "libwebp",
            "-loop",
            "0",
            "-preset",
            "default",
            "-an",
            "-vsync",
            "0",
            "-quality",
            "60",
        ]);
    } else {
        cmd.args([
            "-vf",
            "scale=512:512:force_original_aspect_ratio=decrease:flags=fast_bilinear,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000",
            "-c:v",
            "libwebp",
            "-quality",
            "72",
            "-compression_level",
            "0",
        ]);
    }

    let ok = cmd
        .arg(&out_path)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let result = if ok {
        tokio::fs::read(&out_path)
            .await
            .map_err(|e| format!("read webp: {}", e))
    } else if !animated {
        Ok(raw_bytes.to_vec())
    } else {
        Err("ffmpeg no pudo convertir el video a sticker animado.".to_string())
    };

    tokio::spawn(async move {
        let _ = tokio::fs::remove_file(in_path).await;
        let _ = tokio::fs::remove_file(out_path).await;
    });

    result
}

async fn decode_webp_to_png(raw_bytes: &[u8], tmp_dir: &Path) -> Result<Vec<u8>, String> {
    let _ = tokio::fs::create_dir_all(tmp_dir).await;
    let seq = STICKER_SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = format!("{}_{}_{}", std::process::id(), unix_timestamp_seconds(), seq);
    let in_path = tmp_dir.join(format!("toimg_in_{}.webp", stamp));
    let out_path = tmp_dir.join(format!("toimg_out_{}.png", stamp));

    tokio::fs::write(&in_path, raw_bytes)
        .await
        .map_err(|e| format!("write tmp: {}", e))?;

    let ok = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(&in_path)
        .args(["-frames:v", "1"])
        .arg(&out_path)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let result = if ok {
        tokio::fs::read(&out_path)
            .await
            .map_err(|e| format!("read png: {}", e))
    } else {
        Err("ffmpeg no pudo convertir el sticker a imagen.".to_string())
    };

    tokio::spawn(async move {
        let _ = tokio::fs::remove_file(in_path).await;
        let _ = tokio::fs::remove_file(out_path).await;
    });

    result
}

async fn fetch_api_media_bytes(ctx: &CommandContext, url: &str, params: &[(&str, &str)]) -> Result<Vec<u8>, String> {
    let resp = ctx
        .state
        .http
        .get(url)
        .query(params)
        .send()
        .await
        .map_err(|e| format!("red: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("bytes: {}", e))?;
    if let Ok(text) = std::str::from_utf8(&bytes) {
        let trimmed = text.trim();
        if trimmed.starts_with('{') {
            if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
                for k in ["image", "result", "data", "sticker", "url"] {
                    if let Some(s) = json.get(k).and_then(Value::as_str) {
                        if s.starts_with("http://") || s.starts_with("https://") {
                            let r2 = ctx
                                .state
                                .http
                                .get(s)
                                .send()
                                .await
                                .map_err(|e| format!("red2: {}", e))?;
                            return r2
                                .bytes()
                                .await
                                .map(|b| b.to_vec())
                                .map_err(|e| format!("bytes2: {}", e));
                        }
                        let clean = s.split(',').next_back().unwrap_or(s);
                        if let Ok(decoded) = BASE64.decode(clean) {
                            return Ok(decoded);
                        }
                    }
                }
            }
            return Err("La API no devolvio un archivo valido.".to_string());
        }
    }
    Ok(bytes.to_vec())
}

fn split_two_emojis(raw: &str) -> Option<(String, String)> {
    let clean = raw.trim();
    if clean.is_empty() {
        return None;
    }
    if let Some((a, b)) = clean.split_once('+') {
        let ea = a.trim();
        let eb = b.trim();
        if !ea.is_empty() && !eb.is_empty() {
            return Some((ea.to_string(), eb.to_string()));
        }
    }
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.len() >= 2 {
        return Some((parts[0].to_string(), parts[1].to_string()));
    }
    let mut clusters: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut join_next = false;
    for ch in clean.chars() {
        if ch == '\u{200d}' {
            cur.push(ch);
            join_next = true;
        } else if ch == '\u{fe0f}' || ch == '\u{fe0e}' || ('\u{1f3fb}'..='\u{1f3ff}').contains(&ch) {
            cur.push(ch);
        } else if join_next {
            cur.push(ch);
            join_next = false;
        } else {
            if !cur.is_empty() {
                clusters.push(cur);
            }
            cur = ch.to_string();
        }
    }
    if !cur.is_empty() {
        clusters.push(cur);
    }
    if clusters.len() >= 2 {
        Some((clusters[0].clone(), clusters[1].clone()))
    } else {
        None
    }
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "s" | "sticker" | "stiker" => {
            let Some(input) = find_sticker_input(&ctx.message) else {
                ctx.reply_text(format!(
                    "Envia o responde a una imagen o video con `{}s`.",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };

            let t0 = Instant::now();
            let (dl_msg, media_type, animated) = match input {
                StickerInput::Image(img) => (
                    DownloadableMessage {
                        media_key: img.media_key,
                        direct_path: img.direct_path,
                        url: img.url,
                    },
                    MediaType::Image,
                    false,
                ),
                StickerInput::Video(vid) => (
                    DownloadableMessage {
                        media_key: vid.media_key,
                        direct_path: vid.direct_path,
                        url: vid.url,
                    },
                    MediaType::Video,
                    true,
                ),
            };
            let dl_opts = MediaDownloadOptions {
                timeout_ms: Some(12_000),
                ..Default::default()
            };

            let raw_bytes = match download_content_from_message(&dl_msg, media_type, &dl_opts).await {
                Ok(b) => b,
                Err(err) => {
                    ctx.reply_text(format!("Error descargando media: {}", err))
                        .await;
                    return true;
                }
            };

            let sticker_bytes = match encode_webp(&raw_bytes, animated, &ctx.state.config.cache_dir).await {
                Ok(b) => b,
                Err(err) => {
                    ctx.reply_text(format!("Error convirtiendo a WebP: {}", err))
                        .await;
                    return true;
                }
            };

            if let Err(err) = ctx.send_sticker_bytes(sticker_bytes, animated).await {
                ctx.state
                    .debug
                    .record_error("sticker_send", format!("{}", err))
                    .await;
                ctx.reply_text(format!("Error enviando sticker: {}", err))
                    .await;
            } else {
                ctx.state
                    .debug
                    .record_timing(
                        "sticker",
                        format!("{:.0} ms", t0.elapsed().as_secs_f64() * 1000.0),
                    )
                    .await;
            }
            true
        }
        "setmeta" | "metasticker" | "sticker-meta" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let cur_pack = u.sticker_pack_name.as_deref().unwrap_or("Aura Reed");
            let cur_author = u.sticker_pack_author.as_deref().unwrap_or("Aura Reed");
            let raw = ctx.text.trim();
            if raw.is_empty() {
                ctx.reply_text(format!(
                    "*Sticker Metadata*\nPack: *{}*\nAutor: *{}*\nUso: `{}setmeta Pack | Autor`",
                    cur_pack, cur_author, ctx.used_prefix
                ))
                .await;
                return true;
            }
            let Some((p, a)) = raw.split_once('|') else {
                ctx.reply_text(format!(
                    "Formato invalido. Usa `{}setmeta Pack | Autor`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            let pack = p.trim();
            let author = a.trim();
            if pack.is_empty() || author.is_empty() {
                ctx.reply_text("Tanto el pack como el autor deben tener texto.")
                    .await;
                return true;
            }
            ctx.state.db.set_sticker_meta(&ctx.sender, pack, author);
            ctx.reply_text(format!(
                "Metadata guardada.\nPack: *{}*\nAutor: *{}*",
                pack, author
            ))
            .await;
            true
        }
        "brat" => {
            let text = if !ctx.text.trim().is_empty() {
                ctx.text.trim().to_string()
            } else {
                ctx.quoted_text().unwrap_or_default()
            };
            if text.is_empty() {
                ctx.reply_text(format!("Uso: `{}brat <texto>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = crate::config::ALYA_API.base_url.trim_end_matches('/');
            let url = format!("{}/tools/brat", base);
            let key = crate::config::ALYA_API.api_key.unwrap_or("");
            match fetch_api_media_bytes(ctx, &url, &[("text", &text), ("key", key)]).await {
                Ok(raw) => match encode_webp(&raw, false, &ctx.state.config.cache_dir).await {
                    Ok(webp) => {
                        if let Err(err) = ctx.send_sticker_bytes(webp, false).await {
                            ctx.reply_text(format!("Error enviando brat: {}", err))
                                .await;
                        }
                    }
                    Err(err) => ctx.reply_text(format!("Error WebP: {}", err)).await,
                },
                Err(err) => ctx.reply_text(format!("Error generando brat: {}", err)).await,
            }
            true
        }
        "bratv" | "bratvid" | "bratvideo" | "bratstickervideo" => {
            let text = if !ctx.text.trim().is_empty() {
                ctx.text.trim().to_string()
            } else {
                ctx.quoted_text().unwrap_or_default()
            };
            if text.is_empty() {
                ctx.reply_text(format!("Uso: `{}bratv <texto>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = crate::config::FAA_API.base_url.trim_end_matches('/');
            let url = format!("{}/faa/bratvid", base);
            match fetch_api_media_bytes(ctx, &url, &[("text", &text)]).await {
                Ok(raw) => match encode_webp(&raw, true, &ctx.state.config.cache_dir).await {
                    Ok(webp) => {
                        if let Err(err) = ctx.send_sticker_bytes(webp, true).await {
                            ctx.reply_text(format!("Error enviando bratv: {}", err))
                                .await;
                        }
                    }
                    Err(err) => ctx.reply_text(format!("Error WebP animado: {}", err)).await,
                },
                Err(err) => ctx.reply_text(format!("Error generando bratv: {}", err)).await,
            }
            true
        }
        "emojimix" | "ekitchen" | "emojikitchen" => {
            let Some((e1, e2)) = split_two_emojis(&ctx.text) else {
                ctx.reply_text(format!(
                    "Envia dos emojis. Ejemplo: `{}emojimix 😀+🔥`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            let base = crate::config::ALYA_API.base_url.trim_end_matches('/');
            let url = format!("{}/tools/emojimix", base);
            let key = crate::config::ALYA_API.api_key.unwrap_or("");
            match fetch_api_media_bytes(
                ctx,
                &url,
                &[("emoji1", &e1), ("emoji2", &e2), ("key", key)],
            )
            .await
            {
                Ok(raw) => match encode_webp(&raw, false, &ctx.state.config.cache_dir).await {
                    Ok(webp) => {
                        if let Err(err) = ctx.send_sticker_bytes(webp, false).await {
                            ctx.reply_text(format!("Error enviando emojimix: {}", err))
                                .await;
                        }
                    }
                    Err(err) => ctx.reply_text(format!("Error WebP: {}", err)).await,
                },
                Err(err) => ctx.reply_text(format!("Error en emojimix: {}", err)).await,
            }
            true
        }
        "emojivid" | "emoji" | "emoji-video" | "emojivideo" => {
            let em = ctx.text.trim();
            if em.is_empty() {
                ctx.reply_text(format!("Envia un emoji. Ejemplo: `{}emoji 🔥`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = crate::config::ALYA_API.base_url.trim_end_matches('/');
            let url = format!("{}/whatsapp/emoji", base);
            let key = crate::config::ALYA_API.api_key.unwrap_or("");
            match fetch_api_media_bytes(ctx, &url, &[("emoji", em), ("key", key)]).await {
                Ok(raw) => {
                    let is_webp = raw.len() >= 12 && &raw[0..4] == b"RIFF" && &raw[8..12] == b"WEBP";
                    let out = if is_webp {
                        Ok(raw)
                    } else {
                        encode_webp(&raw, true, &ctx.state.config.cache_dir).await
                    };
                    match out {
                        Ok(webp) => {
                            if let Err(err) = ctx.send_sticker_bytes(webp, true).await {
                                ctx.reply_text(format!("Error enviando sticker emoji: {}", err)).await;
                            }
                        }
                        Err(err) => ctx.reply_text(format!("Error convirtiendo emoji: {}", err)).await,
                    }
                }
                Err(err) => ctx.reply_text(format!("Error generando emoji animado: {}", err)).await,
            }
            true
        }
        "qc" => {
            let text = if !ctx.text.trim().is_empty() {
                ctx.text.trim().to_string()
            } else {
                ctx.quoted_text().unwrap_or_default()
            };
            if text.is_empty() {
                ctx.reply_text(format!("Uso: `{}qc <texto>` o responde a un mensaje.", ctx.used_prefix))
                    .await;
                return true;
            }
            let display = ctx
                .user
                .name
                .as_deref()
                .or(ctx.user.username.as_deref())
                .or(ctx.push_name.as_deref())
                .unwrap_or("Usuario");
            let payload = serde_json::json!({
                "type": "quote",
                "format": "png",
                "backgroundColor": "#1b1429",
                "width": 512,
                "height": 768,
                "scale": 2,
                "messages": [{
                    "entities": [],
                    "avatar": true,
                    "from": {
                        "id": 1,
                        "name": display,
                        "photo": { "url": "https://i.pinimg.com/736x/af/a3/24/afa324dff15091f93624bb470d60a592.jpg" }
                    },
                    "text": text,
                    "replyMessage": {}
                }]
            });
            match ctx.state.http.post("https://bot.lyo.su/quote/generate").json(&payload).send().await {
                Ok(r) if r.status().is_success() => {
                    if let Ok(json) = r.json::<Value>().await {
                        if let Some(b64) = json.pointer("/result/image").and_then(Value::as_str) {
                            if let Ok(png) = BASE64.decode(b64) {
                                match encode_webp(&png, false, &ctx.state.config.cache_dir).await {
                                    Ok(webp) => {
                                        if let Err(err) = ctx.send_sticker_bytes(webp, false).await {
                                            ctx.reply_text(format!("Error enviando qc: {}", err)).await;
                                        }
                                        return true;
                                    }
                                    Err(err) => {
                                        ctx.reply_text(format!("Error WebP qc: {}", err)).await;
                                        return true;
                                    }
                                }
                            }
                        }
                    }
                    ctx.reply_text("No se pudo generar la cita.").await;
                }
                Ok(r) => ctx.reply_text(format!("API QC HTTP {}", r.status().as_u16())).await,
                Err(err) => ctx.reply_text(format!("Error en QC: {}", err)).await,
            }
            true
        }
        "spack" | "packs" | "stickerly" | "stickersearch" | "buscars" => {
            let q = ctx.text.trim();
            if q.is_empty() {
                ctx.reply_text(format!("Uso: `{}spack <tema>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = crate::config::ALYA_API.base_url.trim_end_matches('/');
            let key = crate::config::ALYA_API.api_key.unwrap_or("");
            let search = ctx
                .state
                .http
                .get(format!("{}/stickerly/search", base))
                .query(&[("query", q), ("key", key)])
                .send()
                .await;
            let Ok(r) = search else {
                ctx.reply_text("Error consultando Stickerly.").await;
                return true;
            };
            let Ok(sj) = r.json::<Value>().await else {
                ctx.reply_text("Respuesta invalida de Stickerly.").await;
                return true;
            };
            let pack_url = sj
                .get("data")
                .or_else(|| sj.get("result"))
                .and_then(Value::as_array)
                .and_then(|arr| {
                    arr.iter().find(|it| !it.get("isPaid").and_then(Value::as_bool).unwrap_or(false))
                })
                .and_then(|it| it.get("url").and_then(Value::as_str))
                .map(str::to_string);
            let Some(purl) = pack_url else {
                ctx.reply_text("No encontre packs gratuitos para esa busqueda.").await;
                return true;
            };
            let detail = ctx
                .state
                .http
                .get(format!("{}/stickerly/detail", base))
                .query(&[("url", purl.as_str()), ("key", key)])
                .send()
                .await;
            let Ok(dr) = detail else {
                ctx.reply_text("Error obteniendo detalle del pack.").await;
                return true;
            };
            let Ok(dj) = dr.json::<Value>().await else {
                ctx.reply_text("Detalle invalido del pack.").await;
                return true;
            };
            let stickers = dj
                .pointer("/data/stickers")
                .or_else(|| dj.pointer("/result/stickers"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if stickers.is_empty() {
                ctx.reply_text("El pack no contiene stickers descargables.").await;
                return true;
            }
            ctx.reply_text(format!("Enviando 5 stickers del pack para *{}*...", q)).await;
            for item in stickers.iter().take(5) {
                let u = item
                    .get("imageUrl")
                    .or_else(|| item.get("url"))
                    .or_else(|| item.get("image"))
                    .and_then(Value::as_str);
                if let Some(su) = u {
                    if let Ok(resp) = ctx.state.http.get(su).send().await {
                        if let Ok(b) = resp.bytes().await {
                            let is_webp = b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP";
                            let webp = if is_webp {
                                Ok(b.to_vec())
                            } else {
                                encode_webp(&b, false, &ctx.state.config.cache_dir).await
                            };
                            if let Ok(wb) = webp {
                                let _ = ctx.send_sticker_bytes(wb, false).await;
                            }
                        }
                    }
                }
            }
            true
        }
        "toimg" | "img" | "toimage" => {
            let Some(st) = find_quoted_sticker(&ctx.message) else {
                ctx.reply_text(format!(
                    "Responde a un sticker con `{}toimg`.",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            let dl_msg = DownloadableMessage {
                media_key: st.media_key,
                direct_path: st.direct_path,
                url: st.url,
            };
            let dl_opts = MediaDownloadOptions {
                timeout_ms: Some(10_000),
                ..Default::default()
            };
            let raw = match download_content_from_message(&dl_msg, MediaType::Sticker, &dl_opts).await {
                Ok(b) => b,
                Err(err) => {
                    ctx.reply_text(format!("Error descargando sticker: {}", err))
                        .await;
                    return true;
                }
            };
            match decode_webp_to_png(&raw, &ctx.state.config.cache_dir).await {
                Ok(png) => {
                    if let Err(err) = ctx.send_image_bytes(png, None, Vec::new(), "image/png").await {
                        ctx.reply_text(format!("Error enviando imagen: {}", err))
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error convirtiendo: {}", err)).await,
            }
            true
        }
        _ => false,
    }
}
