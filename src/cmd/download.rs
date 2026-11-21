use crate::config::{ALYA_API, DELIRIUS_API, FAA_API};
use crate::context::CommandContext;
use serde_json::Value;

const MAX_MEDIA_BYTES: usize = 30 * 1024 * 1024;

async fn fetch_json(ctx: &CommandContext, url: &str, params: &[(&str, &str)]) -> Result<Value, String> {
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
    resp.json::<Value>()
        .await
        .map_err(|e| format!("json: {}", e))
}

async fn download_bytes_capped(ctx: &CommandContext, url: &str, max_bytes: usize) -> Result<(Vec<u8>, String), String> {
    let resp = ctx
        .state
        .http
        .get(url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 Chrome/126.0.0.0 Mobile Safari/537.36",
        )
        .send()
        .await
        .map_err(|e| format!("red: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    if let Some(len) = resp.content_length() {
        if (len as usize) > max_bytes {
            return Err(format!(
                "Archivo muy pesado ({:.1} MB, limite {:.0} MB).",
                len as f64 / (1024.0 * 1024.0),
                max_bytes as f64 / (1024.0 * 1024.0)
            ));
        }
    }
    let mime = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .split(';')
        .next()
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| format!("bytes: {}", e))?;
    if bytes.len() > max_bytes {
        return Err(format!(
            "Archivo supera el limite de {:.0} MB.",
            max_bytes as f64 / (1024.0 * 1024.0)
        ));
    }
    Ok((bytes.to_vec(), mime))
}

fn find_first_url(val: &Value, keys: &[&str]) -> Option<String> {
    if let Some(s) = val.as_str() {
        if s.starts_with("http://") || s.starts_with("https://") {
            return Some(s.to_string());
        }
    }
    if let Some(obj) = val.as_object() {
        for k in keys {
            if let Some(v) = obj.get(*k) {
                if let Some(s) = v.as_str() {
                    if s.starts_with("http://") || s.starts_with("https://") {
                        return Some(s.to_string());
                    }
                }
            }
        }
        for nested in ["data", "result", "dl", "download", "resultados"] {
            if let Some(sub) = obj.get(nested) {
                if let Some(found) = find_first_url(sub, keys) {
                    return Some(found);
                }
            }
        }
    }
    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(found) = find_first_url(item, keys) {
                return Some(found);
            }
        }
    }
    None
}

fn find_str(val: &Value, keys: &[&str]) -> Option<String> {
    if let Some(obj) = val.as_object() {
        for k in keys {
            if let Some(s) = obj.get(*k).and_then(Value::as_str) {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
        for nested in ["data", "result"] {
            if let Some(sub) = obj.get(nested) {
                if let Some(s) = find_str(sub, keys) {
                    return Some(s);
                }
            }
        }
    }
    None
}

fn is_yt_url(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    low.contains("youtube.com/") || low.contains("youtu.be/")
}

async fn resolve_yt_url_and_title(ctx: &CommandContext, query: &str) -> Result<(String, String), String> {
    if is_yt_url(query) {
        return Ok((query.to_string(), "YouTube".to_string()));
    }
    let alya = ALYA_API.base_url.trim_end_matches('/');
    let key = ALYA_API.api_key.unwrap_or("");
    let json = fetch_json(
        ctx,
        &format!("{}/search/youtube", alya),
        &[("query", query), ("key", key)],
    )
    .await?;
    let first = json
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| json.get("result").and_then(Value::as_array))
        .or_else(|| json.as_array())
        .and_then(|arr| arr.first())
        .ok_or_else(|| "No se encontraron resultados en YouTube.".to_string())?;
    let url = find_first_url(first, &["url", "link"])
        .ok_or_else(|| "Resultado sin enlace de YouTube.".to_string())?;
    let title = find_str(first, &["title", "name"]).unwrap_or_else(|| query.to_string());
    Ok((url, title))
}

async fn resolve_yt_download_link(ctx: &CommandContext, yt_url: &str, want_video: bool) -> Result<String, String> {
    let alya = ALYA_API.base_url.trim_end_matches('/');
    let alya_key = ALYA_API.api_key.unwrap_or("");
    let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
    let faa = FAA_API.base_url.trim_end_matches('/');

    let endpoints: Vec<(String, Vec<(&str, &str)>)> = if want_video {
        vec![
            (format!("{}/dl/ytmp4", alya), vec![("url", yt_url), ("key", alya_key)]),
            (format!("{}/download/ytmp4", delirius), vec![("url", yt_url)]),
            (format!("{}/faa/ytmp4", faa), vec![("url", yt_url)]),
        ]
    } else {
        vec![
            (format!("{}/dl/ytmp3", alya), vec![("url", yt_url), ("apikey", alya_key)]),
            (format!("{}/dl/ytdlmp3", alya), vec![("url", yt_url), ("apikey", alya_key)]),
            (format!("{}/download/ytmp3", delirius), vec![("url", yt_url)]),
            (format!("{}/faa/ytmp3", faa), vec![("url", yt_url)]),
        ]
    };

    let url_keys = ["dl", "download", "mp4", "mp3", "url", "link"];
    for (ep, params) in endpoints {
        if let Ok(json) = fetch_json(ctx, &ep, &params).await {
            if let Some(dl) = find_first_url(&json, &url_keys) {
                return Ok(dl);
            }
        }
    }
    Err("Ningun servidor de YouTube devolvio un enlace descargable.".to_string())
}

fn extract_attr_href_near(html: &str, marker: &str) -> Option<String> {
    let pos = html.find(marker)?;
    let start = pos.saturating_sub(250);
    let end = (pos + 400).min(html.len());
    let slice = &html[start..end];
    let href_pos = slice.find("href=\"")?;
    let rest = &slice[href_pos + 6..];
    let close = rest.find('"')?;
    let link = &rest[..close];
    if link.starts_with("http://") || link.starts_with("https://") {
        Some(link.to_string())
    } else {
        None
    }
}

pub async fn run(ctx: &CommandContext) -> bool {
    let query = ctx.text.trim();
    match ctx.cmd_name.as_str() {
        "play" | "ytmp3" | "yta" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}play <cancion o enlace YouTube>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let (yt_url, title) = match resolve_yt_url_and_title(ctx, query).await {
                Ok(v) => v,
                Err(err) => {
                    ctx.reply_text(format!("Error buscando en YouTube: {}", err))
                        .await;
                    return true;
                }
            };
            ctx.reply_text(format!("Descargando audio: *{}*\n{}", title, yt_url))
                .await;
            match resolve_yt_download_link(ctx, &yt_url, false).await {
                Ok(dl_url) => match download_bytes_capped(ctx, &dl_url, MAX_MEDIA_BYTES).await {
                    Ok((bytes, _)) => {
                        if let Err(err) = ctx.send_audio_bytes(bytes, "audio/mpeg", false).await {
                            ctx.reply_text(format!("Error enviando audio: {}", err))
                                .await;
                        }
                    }
                    Err(err) => {
                        ctx.reply_text(format!("No pude bajar el audio ({}). Enlace:\n{}", err, dl_url))
                            .await;
                    }
                },
                Err(err) => ctx.reply_text(format!("Error en descarga: {}", err)).await,
            }
            true
        }
        "play2" | "ytmp4" | "ytv" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}play2 <video o enlace YouTube>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let (yt_url, title) = match resolve_yt_url_and_title(ctx, query).await {
                Ok(v) => v,
                Err(err) => {
                    ctx.reply_text(format!("Error buscando en YouTube: {}", err))
                        .await;
                    return true;
                }
            };
            ctx.reply_text(format!("Descargando video: *{}*\n{}", title, yt_url))
                .await;
            match resolve_yt_download_link(ctx, &yt_url, true).await {
                Ok(dl_url) => match download_bytes_capped(ctx, &dl_url, MAX_MEDIA_BYTES).await {
                    Ok((bytes, _)) => {
                        if let Err(err) = ctx
                            .send_video_bytes(bytes, Some(format!("*{}*", title)), "video/mp4")
                            .await
                        {
                            ctx.reply_text(format!("Error enviando video: {}", err))
                                .await;
                        }
                    }
                    Err(err) => {
                        ctx.reply_text(format!("No pude bajar el video ({}). Enlace:\n{}", err, dl_url))
                            .await;
                    }
                },
                Err(err) => ctx.reply_text(format!("Error en descarga: {}", err)).await,
            }
            true
        }
        "tiktok" | "tt" | "ttdl" | "tiktok2" | "ttaudio" | "tta" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}tiktok <enlace o busqueda>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let want_audio = matches!(ctx.cmd_name.as_str(), "ttaudio" | "tta");
            let tt_url = if query.contains("tiktok.com/") {
                query.to_string()
            } else {
                let alya = ALYA_API.base_url.trim_end_matches('/');
                let key = ALYA_API.api_key.unwrap_or("");
                match fetch_json(
                    ctx,
                    &format!("{}/search/tiktok", alya),
                    &[("query", query), ("key", key)],
                )
                .await
                {
                    Ok(j) => match find_first_url(&j, &["url", "play", "link"]) {
                        Some(u) => u,
                        None => {
                            ctx.reply_text("No se encontraron videos en TikTok.").await;
                            return true;
                        }
                    },
                    Err(err) => {
                        ctx.reply_text(format!("Error buscando en TikTok: {}", err))
                            .await;
                        return true;
                    }
                }
            };

            let json = match fetch_json(ctx, "https://www.tikwm.com/api/", &[("url", &tt_url), ("hd", "1")]).await {
                Ok(j) => j,
                Err(err) => {
                    ctx.reply_text(format!("Error consultando TikWM: {}", err))
                        .await;
                    return true;
                }
            };

            let title = find_str(&json, &["title"]).unwrap_or_else(|| "TikTok".to_string());
            if want_audio {
                let Some(music) = json
                    .pointer("/data/music_info/play")
                    .and_then(Value::as_str)
                    .or_else(|| json.pointer("/data/music").and_then(Value::as_str))
                    .map(str::to_string)
                else {
                    ctx.reply_text("No se encontro pista de audio en este TikTok.")
                        .await;
                    return true;
                };
                match download_bytes_capped(ctx, &music, MAX_MEDIA_BYTES).await {
                    Ok((bytes, _)) => {
                        if let Err(err) = ctx.send_audio_bytes(bytes, "audio/mpeg", false).await {
                            ctx.reply_text(format!("Error enviando audio: {}", err))
                                .await;
                        }
                    }
                    Err(err) => ctx.reply_text(format!("Error bajando audio: {}", err)).await,
                }
                return true;
            }

            let media_url = json
                .pointer("/data/hdplay")
                .and_then(Value::as_str)
                .filter(|s| s.starts_with("http"))
                .or_else(|| json.pointer("/data/play").and_then(Value::as_str))
                .map(str::to_string);

            let Some(video_url) = media_url else {
                ctx.reply_text("No se encontro video descargable en ese enlace de TikTok.")
                    .await;
                return true;
            };

            match download_bytes_capped(ctx, &video_url, MAX_MEDIA_BYTES).await {
                Ok((bytes, _)) => {
                    let cap: String = title.chars().take(240).collect();
                    if let Err(err) = ctx.send_video_bytes(bytes, Some(cap), "video/mp4").await {
                        ctx.reply_text(format!("Error enviando video: {}", err))
                            .await;
                    }
                }
                Err(err) => {
                    ctx.reply_text(format!("No pude bajar el video ({}). Enlace:\n{}", err, video_url))
                        .await;
                }
            }
            true
        }
        "ig" | "instagram" => {
            if !query.contains("instagram.com/") && !query.contains("instagr.am/") {
                ctx.reply_text(format!("Uso: `{}ig <enlace de Instagram>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = DELIRIUS_API.base_url.trim_end_matches('/');
            match fetch_json(ctx, &format!("{}/download/instagramv2", base), &[("url", query)]).await {
                Ok(json) => {
                    let items = json
                        .pointer("/data/download")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let vid = items
                        .iter()
                        .find(|i| i.get("type").and_then(Value::as_str) == Some("video"))
                        .and_then(|i| i.get("url").and_then(Value::as_str));
                    let img = items
                        .iter()
                        .find(|i| i.get("type").and_then(Value::as_str) == Some("image"))
                        .and_then(|i| i.get("url").and_then(Value::as_str));
                    if let Some(vurl) = vid {
                        match download_bytes_capped(ctx, vurl, MAX_MEDIA_BYTES).await {
                            Ok((bytes, _)) => {
                                if let Err(err) = ctx.send_video_bytes(bytes, Some("Instagram".to_string()), "video/mp4").await {
                                    ctx.reply_text(format!("Error enviando video IG: {}", err)).await;
                                }
                            }
                            Err(err) => ctx.reply_text(format!("Error bajando IG ({}):\n{}", err, vurl)).await,
                        }
                    } else if let Some(iurl) = img {
                        match download_bytes_capped(ctx, iurl, MAX_MEDIA_BYTES).await {
                            Ok((bytes, mime)) => {
                                if let Err(err) = ctx.send_image_bytes(bytes, Some("Instagram".to_string()), Vec::new(), &mime).await {
                                    ctx.reply_text(format!("Error enviando imagen IG: {}", err)).await;
                                }
                            }
                            Err(err) => ctx.reply_text(format!("Error bajando imagen IG: {}", err)).await,
                        }
                    } else {
                        ctx.reply_text("No se encontro contenido descargable en ese enlace de Instagram.")
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error consultando Instagram: {}", err)).await,
            }
            true
        }
        "fb" | "facebook" | "fbdl" | "fbvideo" | "fbreels" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}fb <enlace de Facebook>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            match fetch_json(
                ctx,
                &format!("{}/dl/facebook", base),
                &[("url", query), ("key", key)],
            )
            .await
            {
                Ok(json) => {
                    if let Some(vurl) = find_first_url(&json, &["hd", "url", "dl", "sd", "link"]) {
                        match download_bytes_capped(ctx, &vurl, MAX_MEDIA_BYTES).await {
                            Ok((bytes, _)) => {
                                if let Err(err) = ctx.send_video_bytes(bytes, Some("Facebook".to_string()), "video/mp4").await {
                                    ctx.reply_text(format!("Error enviando video FB: {}", err)).await;
                                }
                            }
                            Err(err) => ctx.reply_text(format!("Error bajando FB ({}):\n{}", err, vurl)).await,
                        }
                    } else {
                        ctx.reply_text("No se encontro video descargable en Facebook.")
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error consultando Facebook: {}", err)).await,
            }
            true
        }
        "spotify" | "splay" | "sp" | "spdl" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}spotify <cancion o enlace>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let is_url = query.contains("open.spotify.com/");
            let (ep, param_k, param_v) = if is_url {
                ("/dl/spotify", "url", query.split('?').next().unwrap_or(query))
            } else {
                ("/dl/spotifyplay", "query", query)
            };
            match fetch_json(
                ctx,
                &format!("{}{}", base, ep),
                &[(param_k, param_v), ("key", key)],
            )
            .await
            {
                Ok(json) => {
                    let title = find_str(&json, &["title", "name"]).unwrap_or_else(|| "Spotify".to_string());
                    if let Some(dl) = find_first_url(&json, &["mp3", "dl", "download", "url"]) {
                        ctx.reply_text(format!("Descargando Spotify: *{}*", title)).await;
                        match download_bytes_capped(ctx, &dl, MAX_MEDIA_BYTES).await {
                            Ok((bytes, _)) => {
                                if let Err(err) = ctx.send_audio_bytes(bytes, "audio/mpeg", false).await {
                                    ctx.reply_text(format!("Error enviando audio Spotify: {}", err)).await;
                                }
                            }
                            Err(err) => ctx.reply_text(format!("Error bajando Spotify ({}):\n{}", err, dl)).await,
                        }
                    } else {
                        ctx.reply_text("No se encontro audio descargable en Spotify.")
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error consultando Spotify: {}", err)).await,
            }
            true
        }
        "x" | "twitter" | "tw" => {
            if !query.contains("x.com/") && !query.contains("twitter.com/") {
                ctx.reply_text(format!("Uso: `{}x <enlace de X/Twitter>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            match fetch_json(
                ctx,
                &format!("{}/dl/twitter", base),
                &[("url", query), ("key", key)],
            )
            .await
            {
                Ok(json) => {
                    if let Some(vurl) = find_first_url(&json, &["url", "hd", "sd", "dl"]) {
                        match download_bytes_capped(ctx, &vurl, MAX_MEDIA_BYTES).await {
                            Ok((bytes, _)) => {
                                if let Err(err) = ctx.send_video_bytes(bytes, Some("X / Twitter".to_string()), "video/mp4").await {
                                    ctx.reply_text(format!("Error enviando video X: {}", err)).await;
                                }
                            }
                            Err(err) => ctx.reply_text(format!("Error bajando X ({}):\n{}", err, vurl)).await,
                        }
                    } else {
                        ctx.reply_text("No se encontro video en ese enlace de X.")
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error consultando X: {}", err)).await,
            }
            true
        }
        "apk" | "apkdl" | "app" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}apk <nombre de la app>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let base = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            match fetch_json(
                ctx,
                &format!("{}/search/apk", base),
                &[("query", query), ("key", key)],
            )
            .await
            {
                Ok(json) => {
                    let name = find_str(&json, &["name", "title"]).unwrap_or_else(|| query.to_string());
                    let Some(dl) = find_first_url(&json, &["dl", "download", "url"]) else {
                        ctx.reply_text("No se encontro una app descargable.").await;
                        return true;
                    };
                    let safe_name = format!("{}.apk", name.replace('/', "_"));
                    match download_bytes_capped(ctx, &dl, MAX_MEDIA_BYTES).await {
                        Ok((bytes, _)) => {
                            if let Err(err) = ctx
                                .send_document_bytes(
                                    bytes,
                                    &safe_name,
                                    "application/vnd.android.package-archive",
                                    Some(format!("*{}*", name)),
                                )
                                .await
                            {
                                ctx.reply_text(format!("Error enviando APK: {}", err)).await;
                            }
                        }
                        Err(err) => {
                            ctx.reply_text(format!("*{}*\n{}\nEnlace directo:\n{}", name, err, dl))
                                .await;
                        }
                    }
                }
                Err(err) => ctx.reply_text(format!("Error buscando APK: {}", err)).await,
            }
            true
        }
        "mediafire" | "mf" | "md" => {
            if !query.contains("mediafire.com/file/") {
                ctx.reply_text(format!("Uso: `{}mf <enlace de MediaFire>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let resp = match ctx
                .state
                .http
                .get(query)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AuraReedBot/2.5")
                .send()
                .await
            {
                Ok(r) => r,
                Err(err) => {
                    ctx.reply_text(format!("Error consultando MediaFire: {}", err))
                        .await;
                    return true;
                }
            };
            let html = resp.text().await.unwrap_or_default();
            let Some(dl_url) = extract_attr_href_near(&html, "downloadButton") else {
                ctx.reply_text("No pude extraer el enlace directo de MediaFire.")
                    .await;
                return true;
            };
            let fname = dl_url
                .rsplit('/')
                .next()
                .unwrap_or("archivo.bin")
                .split('?')
                .next()
                .unwrap_or("archivo.bin")
                .to_string();
            match download_bytes_capped(ctx, &dl_url, MAX_MEDIA_BYTES).await {
                Ok((bytes, mime)) => {
                    if let Err(err) = ctx
                        .send_document_bytes(bytes, &fname, &mime, Some(format!("MediaFire: *{}*", fname)))
                        .await
                    {
                        ctx.reply_text(format!("Error enviando archivo: {}", err))
                            .await;
                    }
                }
                Err(err) => {
                    ctx.reply_text(format!("*{}*\n{}\nDescarga directa:\n{}", fname, err, dl_url))
                        .await;
                }
            }
            true
        }
        "ssweb" | "ss" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}ssweb <url>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let target_url = if query.starts_with("http://") || query.starts_with("https://") {
                query.to_string()
            } else {
                format!("https://{}", query)
            };
            let base = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let resp = ctx
                .state
                .http
                .get(format!("{}/tools/ssweb", base))
                .query(&[("url", target_url.as_str()), ("device", "pc"), ("key", key)])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let mime = r
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("image/png")
                        .to_string();
                    if let Ok(bytes) = r.bytes().await {
                        if let Err(err) = ctx
                            .send_image_bytes(bytes.to_vec(), Some(format!("Captura: {}", target_url)), Vec::new(), &mime)
                            .await
                        {
                            ctx.reply_text(format!("Error enviando captura: {}", err))
                                .await;
                        }
                    }
                }
                Ok(r) => ctx.reply_text(format!("Error ssweb: HTTP {}", r.status().as_u16())).await,
                Err(err) => ctx.reply_text(format!("Error ssweb: {}", err)).await,
            }
            true
        }
        _ => false,
    }
}
