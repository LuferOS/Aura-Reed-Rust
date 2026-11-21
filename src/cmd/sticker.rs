use crate::context::CommandContext;
use baileys_rs::defaults::MediaType;
use baileys_rs::proto::message::{ExtendedTextMessage, ImageMessage, StickerMessage};
use baileys_rs::proto::Message;
use baileys_rs::types::WaMessage;
use baileys_rs::utils::{
    download_content_from_message, encrypted_stream, normalize_message_content,
    unix_timestamp_seconds, upload_media_to_server, AnyMessageContent, DownloadableMessage,
    MediaDownloadOptions, MessageGenerationOptions, WaMediaUpload,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sha2::{Digest, Sha256};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static STICKER_SEQ: AtomicU64 = AtomicU64::new(0);

fn find_image_message(msg: &WaMessage) -> Option<ImageMessage> {
    let proto = msg.message.as_ref()?;
    let norm = normalize_message_content(proto);
    if let Some(ref img) = norm.image_message {
        let inner: &ImageMessage = Borrow::borrow(img);
        return Some(inner.clone());
    }
    if let Some(ref ext) = norm.extended_text_message {
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        if let Some(ref c) = inner.context_info {
            if let Some(ref quoted_msg) = c.quoted_message {
                let q_norm = normalize_message_content(quoted_msg);
                if let Some(ref q_img) = q_norm.image_message {
                    let q_inner: &ImageMessage = Borrow::borrow(q_img);
                    return Some(q_inner.clone());
                }
            }
        }
    }
    None
}

async fn encode_webp(raw_bytes: &[u8], tmp_dir: &Path) -> Result<Vec<u8>, String> {
    let seq = STICKER_SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = format!("{}_{}_{}", std::process::id(), unix_timestamp_seconds(), seq);
    let in_path = tmp_dir.join(format!("s_in_{}.img", stamp));
    let out_path = tmp_dir.join(format!("s_out_{}.webp", stamp));

    tokio::fs::write(&in_path, raw_bytes)
        .await
        .map_err(|e| format!("write tmp: {}", e))?;

    let ok = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-threads",
            "4",
            "-i",
        ])
        .arg(&in_path)
        .args([
            "-vf",
            "scale=512:512:force_original_aspect_ratio=decrease:flags=fast_bilinear,pad=512:512:(ow-iw)/2:(oh-ih)/2:color=0x00000000",
            "-c:v",
            "libwebp",
            "-quality",
            "72",
            "-compression_level",
            "0",
        ])
        .arg(&out_path)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let result = if ok {
        tokio::fs::read(&out_path)
            .await
            .map_err(|e| format!("read webp: {}", e))
    } else {
        Ok(raw_bytes.to_vec())
    };

    tokio::spawn(async move {
        let _ = tokio::fs::remove_file(in_path).await;
        let _ = tokio::fs::remove_file(out_path).await;
    });

    result
}

pub async fn run(ctx: &CommandContext) -> bool {
    if !matches!(ctx.cmd_name.as_str(), "s" | "sticker" | "stiker") {
        return false;
    }

    let Some(img) = find_image_message(&ctx.message) else {
        ctx.reply_text(format!(
            "Envia o responde a una imagen con `{}s`.",
            ctx.used_prefix
        ))
        .await;
        return true;
    };

    let t0 = Instant::now();
    let dl_msg = DownloadableMessage {
        media_key: img.media_key.clone(),
        direct_path: img.direct_path.clone(),
        url: img.url.clone(),
    };
    let dl_opts = MediaDownloadOptions {
        timeout_ms: Some(8_000),
        ..Default::default()
    };

    let raw_bytes = match download_content_from_message(&dl_msg, MediaType::Image, &dl_opts).await {
        Ok(b) => b,
        Err(err) => {
            ctx.reply_text(format!("Error descargando imagen: {}", err))
                .await;
            return true;
        }
    };

    let _ = tokio::fs::create_dir_all(&ctx.state.config.cache_dir).await;
    let sticker_bytes = match encode_webp(&raw_bytes, &ctx.state.config.cache_dir).await {
        Ok(b) => b,
        Err(err) => {
            ctx.reply_text(format!("Error convirtiendo a WebP: {}", err))
                .await;
            return true;
        }
    };

    let sha_res = tokio::task::spawn_blocking(move || {
        let mut h = Sha256::new();
        h.update(&sticker_bytes);
        (h.finalize().to_vec(), sticker_bytes)
    })
    .await;

    let (sha_vec, final_bytes) = match sha_res {
        Ok(v) => v,
        Err(err) => {
            ctx.reply_text(format!("Error interno: {}", err)).await;
            return true;
        }
    };

    let enc = match encrypted_stream(&WaMediaUpload::Bytes(final_bytes), MediaType::Sticker, false)
        .await
    {
        Ok(e) => e,
        Err(err) => {
            ctx.reply_text(format!("Error cifrando sticker: {}", err))
                .await;
            return true;
        }
    };

    let enc_len = tokio::fs::metadata(&enc.enc_file_path)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    let b64 = BASE64.encode(&enc.file_enc_sha256);
    let upload_res = upload_media_to_server(
        &enc.enc_file_path,
        MediaType::Sticker,
        &b64,
        &ctx.messages,
        &[],
        &HashMap::new(),
        Some(8_000),
    )
    .await;

    let enc_path = enc.enc_file_path.clone();
    let orig_path = enc.original_file_path.clone();
    tokio::spawn(async move {
        let _ = tokio::fs::remove_file(enc_path).await;
        if let Some(p) = orig_path {
            let _ = tokio::fs::remove_file(p).await;
        }
    });

    let up = match upload_res {
        Ok(u) => u,
        Err(err) => {
            ctx.reply_text(format!("Error subiendo sticker: {}", err))
                .await;
            return true;
        }
    };

    let sticker = StickerMessage {
        url: Some(up.media_url),
        direct_path: Some(up.direct_path),
        media_key: Some(enc.media_key),
        file_enc_sha256: Some(enc.file_enc_sha256),
        file_sha256: Some(sha_vec),
        file_length: Some(enc_len),
        mimetype: Some("image/webp".to_string()),
        width: Some(512),
        height: Some(512),
        is_animated: Some(false),
        ..Default::default()
    };

    let opts = MessageGenerationOptions {
        user_jid: ctx.bot_jid.clone(),
        message_id: None,
        timestamp: None,
        quoted: Some(Box::new(ctx.message.clone())),
        ephemeral_expiration: None,
        mentions: Vec::new(),
        view_once: false,
        edit: None,
        context_info: None,
        background_color: None,
        font: None,
    };

    if let Err(err) = ctx
        .messages
        .send_message(
            &ctx.from,
            AnyMessageContent::Raw(Message {
                sticker_message: Some(sticker.into()),
                ..Default::default()
            }),
            opts,
        )
        .await
    {
        ctx.state
            .debug
            .record_error("sticker_send", format!("{}", err))
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
