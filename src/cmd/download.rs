use crate::config::{ALYA_API, DEFAULT_BOT_NAME, DELIRIUS_API, EVOGB_API, FAA_API};
use crate::context::CommandContext;
use crate::ui::{box_section, footer_tag, fyt_bold};
use serde_json::Value;

const MAX_MEDIA_BYTES: usize = 35 * 1024 * 1024;
const BROWSER_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

async fn fetch_json(ctx: &CommandContext, url: &str, params: &[(&str, &str)]) -> Result<Value, String> {
    let resp = ctx
        .state
        .http
        .get(url)
        .header("User-Agent", BROWSER_UA)
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

fn looks_like_error_payload(mime: &str, bytes: &[u8]) -> bool {
    let low_mime = mime.to_ascii_lowercase();
    if low_mime.contains("text/html")
        || low_mime.contains("application/json")
        || low_mime.contains("text/plain")
        || low_mime.contains("application/xml")
    {
        return true;
    }
    let head = &bytes[..bytes.len().min(64)];
    head.starts_with(b"<!DOCTYPE")
        || head.starts_with(b"<html")
        || head.starts_with(b"<HTML")
        || head.starts_with(b"{\"")
}

async fn download_media_bytes(
    ctx: &CommandContext,
    url: &str,
    referer: Option<&str>,
    max_bytes: usize,
) -> Result<(Vec<u8>, String), String> {
    let mut req = ctx
        .state
        .http
        .get(url)
        .header("User-Agent", BROWSER_UA)
        .header("Accept", "*/*");
    if let Some(r) = referer {
        req = req.header("Referer", r);
    }
    let resp = req.send().await.map_err(|e| format!("red: {}", e))?;
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
    if bytes.is_empty() {
        return Err("Archivo vacio recibido del servidor.".to_string());
    }
    if bytes.len() > max_bytes {
        return Err(format!(
            "Archivo supera el limite de {:.0} MB.",
            max_bytes as f64 / (1024.0 * 1024.0)
        ));
    }
    if looks_like_error_payload(&mime, &bytes) {
        return Err(format!("El servidor devolvio {} en vez de media.", mime));
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
        for nested in ["data", "result", "dl", "download", "resultados", "media", "video", "audio"] {
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

fn fix_tikwm_url(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() {
        return None;
    }
    if t.starts_with("http://") || t.starts_with("https://") {
        Some(t.to_string())
    } else if t.starts_with('/') {
        Some(format!("https://www.tikwm.com{}", t))
    } else {
        None
    }
}

fn is_yt_url(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    low.contains("youtube.com/") || low.contains("youtu.be/")
}

fn scrape_first_yt_video_id(html: &str) -> Option<String> {
    let marker = "\"videoId\":\"";
    let mut rest = html;
    while let Some(pos) = rest.find(marker) {
        let after = &rest[pos + marker.len()..];
        if let Some(end) = after.find('"') {
            let id = &after[..end];
            if id.len() == 11
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(id.to_string());
            }
        }
        rest = after;
    }
    None
}

async fn resolve_yt_url_and_title(ctx: &CommandContext, query: &str) -> Result<(String, String), String> {
    if is_yt_url(query) {
        return Ok((query.to_string(), "YouTube".to_string()));
    }
    let alya = ALYA_API.base_url.trim_end_matches('/');
    let key = ALYA_API.api_key.unwrap_or("");
    if let Ok(json) = fetch_json(
        ctx,
        &format!("{}/search/youtube", alya),
        &[("query", query), ("key", key)],
    )
    .await
    {
        if let Some(first) = json
            .get("data")
            .and_then(Value::as_array)
            .or_else(|| json.get("result").and_then(Value::as_array))
            .or_else(|| json.as_array())
            .and_then(|arr| arr.first())
        {
            if let Some(url) = find_first_url(first, &["url", "link"]) {
                let title = find_str(first, &["title", "name"]).unwrap_or_else(|| query.to_string());
                return Ok((url, title));
            }
        }
    }
    if let Ok(resp) = ctx
        .state
        .http
        .get("https://www.youtube.com/results")
        .header("User-Agent", BROWSER_UA)
        .query(&[("search_query", query), ("hl", "es")])
        .send()
        .await
    {
        if let Ok(html) = resp.text().await {
            if let Some(vid) = scrape_first_yt_video_id(&html) {
                return Ok((format!("https://youtu.be/{}", vid), query.to_string()));
            }
        }
    }
    Err("No se encontraron resultados en YouTube.".to_string())
}

async fn download_yt_with_fallbacks(
    ctx: &CommandContext,
    yt_url: &str,
    want_video: bool,
) -> Result<(Vec<u8>, String), String> {
    let alya = ALYA_API.base_url.trim_end_matches('/');
    let alya_key = ALYA_API.api_key.unwrap_or("");
    let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
    let faa = FAA_API.base_url.trim_end_matches('/');
    let evogb = EVOGB_API.base_url.trim_end_matches('/');
    let evogb_key = EVOGB_API.api_key.unwrap_or("");

    let endpoints: Vec<(String, Vec<(&str, &str)>)> = if want_video {
        vec![
            (format!("{}/dl/ytmp4", alya), vec![("url", yt_url), ("key", alya_key)]),
            (format!("{}/dl/ytdlmp4", alya), vec![("url", yt_url), ("apikey", alya_key)]),
            (format!("{}/download/ytmp4", delirius), vec![("url", yt_url)]),
            (format!("{}/faa/ytmp4", faa), vec![("url", yt_url)]),
            (format!("{}/api/download/ytmp4", evogb), vec![("url", yt_url), ("apikey", evogb_key)]),
        ]
    } else {
        vec![
            (format!("{}/dl/ytmp3", alya), vec![("url", yt_url), ("key", alya_key)]),
            (format!("{}/dl/ytdlmp3", alya), vec![("url", yt_url), ("apikey", alya_key)]),
            (format!("{}/dl/fastytmp3", alya), vec![("url", yt_url), ("apikey", alya_key)]),
            (format!("{}/download/ytmp3", delirius), vec![("url", yt_url)]),
            (format!("{}/faa/ytmp3", faa), vec![("url", yt_url)]),
        ]
    };

    let url_keys = ["dl", "download", "mp4", "mp3", "url", "link"];
    let mut last_err = "Sin respuesta de servidores de descarga.".to_string();

    for (ep, params) in endpoints {
        if let Ok(json) = fetch_json(ctx, &ep, &params).await {
            if let Some(dl_url) = find_first_url(&json, &url_keys) {
                match download_media_bytes(ctx, &dl_url, None, MAX_MEDIA_BYTES).await {
                    Ok((bytes, _)) => return Ok((bytes, dl_url)),
                    Err(e) => last_err = e,
                }
            }
        }
    }
    Err(last_err)
}

fn extract_attr_href_near(html: &str, marker: &str) -> Option<String> {
    let pos = html.find(marker)?;
    let start = pos.saturating_sub(250);
    let end = (pos + 450).min(html.len());
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
        "play" | "ytmp3" | "yta" | "playaudio" | "playmp3" | "ytmusic" | "y2mate"
        | "docplay" | "ytdmp3" | "dmp3" | "dyta" | "docaudio" => {
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
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section(
                    "🎵",
                    "YOUTUBE AUDIO",
                    &[
                        format!("> {} › {}", fyt_bold("Titulo"), title),
                        format!("> {} › {}", fyt_bold("Enlace"), yt_url),
                        "> Descargando audio...".to_string(),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            ))
            .await;
            match download_yt_with_fallbacks(ctx, &yt_url, false).await {
                Ok((bytes, _)) => {
                    let as_doc = matches!(
                        ctx.cmd_name.as_str(),
                        "docplay" | "ytdmp3" | "dmp3" | "dyta" | "docaudio"
                    );
                    let send_res = if as_doc {
                        let fname = format!(
                            "{}.mp3",
                            title.replace('/', "_").chars().take(60).collect::<String>()
                        );
                        ctx.send_document_bytes(bytes, &fname, "audio/mpeg", Some(format!("*{}*", title)))
                            .await
                    } else {
                        ctx.send_audio_bytes(bytes, "audio/mpeg", false).await
                    };
                    if let Err(err) = send_res {
                        ctx.reply_text(format!("Error enviando audio: {}", err))
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error descargando audio: {}", err)).await,
            }
            true
        }
        "play2" | "ytmp4" | "ytv" | "mp4" | "playvideo" | "video"
        | "docplay2" | "dytmp4" | "dmp4" | "dytv" | "docvideo" => {
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
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section(
                    "🎬",
                    "YOUTUBE VIDEO",
                    &[
                        format!("> {} › {}", fyt_bold("Titulo"), title),
                        format!("> {} › {}", fyt_bold("Enlace"), yt_url),
                        "> Descargando y optimizando H.264...".to_string(),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            ))
            .await;
            match download_yt_with_fallbacks(ctx, &yt_url, true).await {
                Ok((bytes, _)) => {
                    let as_doc = matches!(
                        ctx.cmd_name.as_str(),
                        "docplay2" | "dytmp4" | "dmp4" | "dytv" | "docvideo"
                    );
                    let send_res = if as_doc {
                        let fname = format!(
                            "{}.mp4",
                            title.replace('/', "_").chars().take(60).collect::<String>()
                        );
                        ctx.send_document_bytes(bytes, &fname, "video/mp4", Some(format!("*{}*", title)))
                            .await
                    } else {
                        ctx.send_video_bytes(bytes, Some(format!("*{}*", title)), "video/mp4")
                            .await
                    };
                    if let Err(err) = send_res {
                        ctx.reply_text(format!("Error enviando video: {}", err))
                            .await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error descargando video: {}", err)).await,
            }
            true
        }
        "tiktok" | "tk" | "tt" | "ttv" | "ttdl" | "tkmp4" | "tiktok2" | "dtk" | "dtt" | "dttv"
        | "doctiktok" | "tt2" | "ttaudio" | "tta" | "tka" | "tkmusic" | "doctta" | "dtta"
        | "dtka" | "docttaudio" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}tiktok <enlace o busqueda>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let want_audio = matches!(
                ctx.cmd_name.as_str(),
                "ttaudio" | "tta" | "tka" | "tkmusic" | "doctta" | "dtta" | "dtka" | "docttaudio"
            );
            let as_doc = matches!(
                ctx.cmd_name.as_str(),
                "tiktok2" | "dtk" | "dtt" | "dttv" | "doctiktok" | "tt2" | "doctta" | "dtta" | "dtka" | "docttaudio"
            );

            let tt_url = if query.contains("tiktok.com/") {
                query.to_string()
            } else {
                let alya = ALYA_API.base_url.trim_end_matches('/');
                let key = ALYA_API.api_key.unwrap_or("");
                let from_alya = fetch_json(
                    ctx,
                    &format!("{}/search/tiktok", alya),
                    &[("query", query), ("key", key)],
                )
                .await
                .ok()
                .and_then(|j| find_first_url(&j, &["url", "play", "link"]));
                if let Some(u) = from_alya {
                    u
                } else {
                    match fetch_json(
                        ctx,
                        "https://www.tikwm.com/api/feed/search",
                        &[("keywords", query), ("count", "3")],
                    )
                    .await
                    {
                        Ok(j) => match j.pointer("/data/videos/0/play").and_then(Value::as_str).and_then(fix_tikwm_url) {
                            Some(u) => u,
                            None => {
                                ctx.reply_text("No se encontraron videos en TikTok para esa busqueda.")
                                    .await;
                                return true;
                            }
                        },
                        Err(err) => {
                            ctx.reply_text(format!("Error buscando en TikTok: {}", err))
                                .await;
                            return true;
                        }
                    }
                }
            };

            let tikwm = fetch_json(
                ctx,
                "https://www.tikwm.com/api/",
                &[("url", &tt_url), ("hd", "1")],
            )
            .await
            .ok();

            let title = tikwm
                .as_ref()
                .and_then(|j| find_str(j, &["title"]))
                .unwrap_or_else(|| "TikTok".to_string());
            let author = tikwm
                .as_ref()
                .and_then(|j| {
                    j.pointer("/data/author/nickname")
                        .or_else(|| j.pointer("/data/author/unique_id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "TikTok".to_string());

            if want_audio {
                let music = tikwm.as_ref().and_then(|j| {
                    j.pointer("/data/music_info/play")
                        .or_else(|| j.pointer("/data/music"))
                        .and_then(Value::as_str)
                        .and_then(fix_tikwm_url)
                });
                let Some(music_url) = music else {
                    ctx.reply_text("No se encontro pista de audio en este TikTok.")
                        .await;
                    return true;
                };
                match download_media_bytes(ctx, &music_url, Some("https://www.tikwm.com/"), MAX_MEDIA_BYTES).await {
                    Ok((bytes, _)) => {
                        let res = if as_doc {
                            ctx.send_document_bytes(bytes, "tiktok_audio.mp3", "audio/mpeg", Some(title))
                                .await
                        } else {
                            ctx.send_audio_bytes(bytes, "audio/mpeg", false).await
                        };
                        if let Err(err) = res {
                            ctx.reply_text(format!("Error enviando audio TikTok: {}", err))
                                .await;
                        }
                    }
                    Err(err) => ctx.reply_text(format!("Error bajando audio TikTok: {}", err)).await,
                }
                return true;
            }

            if let Some(ref j) = tikwm {
                let images: Vec<String> = j
                    .pointer("/data/images")
                    .and_then(Value::as_array)
                    .map(|arr| {
                        arr.iter()
                            .filter_map(Value::as_str)
                            .filter_map(fix_tikwm_url)
                            .take(10)
                            .collect()
                    })
                    .unwrap_or_default();
                if !images.is_empty() {
                    let cap = format!(
                        "{}\n\n{}",
                        box_section(
                            "🖼️",
                            "TIKTOK SLIDESHOW",
                            &[
                                format!("> {} › {}", fyt_bold("Autor"), author),
                                format!("> {} › {}", fyt_bold("Titulo"), title.chars().take(180).collect::<String>()),
                                format!("> {} › {} fotos", fyt_bold("Album"), images.len()),
                            ],
                        ),
                        footer_tag(DEFAULT_BOT_NAME)
                    );
                    for (idx, img_url) in images.iter().enumerate() {
                        if let Ok((bytes, mime)) = download_media_bytes(ctx, img_url, None, MAX_MEDIA_BYTES).await {
                            let c = if idx == 0 { Some(cap.clone()) } else { None };
                            let _ = ctx.send_image_bytes(bytes, c, Vec::new(), &mime).await;
                        }
                    }
                    if let Some(music_url) = j
                        .pointer("/data/music_info/play")
                        .or_else(|| j.pointer("/data/music"))
                        .and_then(Value::as_str)
                        .and_then(fix_tikwm_url)
                    {
                        if let Ok((ab, _)) = download_media_bytes(ctx, &music_url, None, MAX_MEDIA_BYTES).await {
                            let _ = ctx.send_audio_bytes(ab, "audio/mpeg", false).await;
                        }
                    }
                    return true;
                }
            }

            let mut candidates: Vec<String> = Vec::new();
            if let Some(ref j) = tikwm {
                for ptr in ["/data/play", "/data/hdplay", "/data/wmplay"] {
                    if let Some(u) = j.pointer(ptr).and_then(Value::as_str).and_then(fix_tikwm_url) {
                        if !candidates.contains(&u) {
                            candidates.push(u);
                        }
                    }
                }
            }

            if candidates.is_empty() {
                let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
                if let Ok(dj) = fetch_json(ctx, &format!("{}/download/tiktok", delirius), &[("url", &tt_url)]).await {
                    if let Some(u) = find_first_url(&dj, &["play", "hdplay", "url", "dl"]) {
                        candidates.push(u);
                    }
                }
            }

            if candidates.is_empty() {
                ctx.reply_text("No se encontro video descargable en ese enlace de TikTok.")
                    .await;
                return true;
            }

            let mut downloaded: Option<Vec<u8>> = None;
            let mut last_err = "No se pudo descargar el video.".to_string();
            for cand in &candidates {
                match download_media_bytes(ctx, cand, Some("https://www.tikwm.com/"), MAX_MEDIA_BYTES).await {
                    Ok((bytes, _)) => {
                        downloaded = Some(bytes);
                        break;
                    }
                    Err(e) => last_err = e,
                }
            }

            let Some(video_bytes) = downloaded else {
                ctx.reply_text(format!("Error bajando video TikTok: {}", last_err))
                    .await;
                return true;
            };

            let cap = format!(
                "{}\n\n{}",
                box_section(
                    "🎵",
                    "TIKTOK VIDEO",
                    &[
                        format!("> {} › {}", fyt_bold("Autor"), author),
                        format!("> {} › {}", fyt_bold("Titulo"), title.chars().take(200).collect::<String>()),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            );
            let res = if as_doc {
                ctx.send_document_bytes(video_bytes, "tiktok.mp4", "video/mp4", Some(cap))
                    .await
            } else {
                ctx.send_video_bytes(video_bytes, Some(cap), "video/mp4")
                    .await
            };
            if let Err(err) = res {
                ctx.reply_text(format!("Error enviando video TikTok: {}", err))
                    .await;
            }
            true
        }
        "ig" | "instagram" => {
            if !query.contains("instagram.com/") && !query.contains("instagr.am/") {
                ctx.reply_text(format!("Uso: `{}ig <enlace de Instagram>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
            let alya = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let mut sent = false;

            if let Ok(json) = fetch_json(ctx, &format!("{}/download/instagramv2", delirius), &[("url", query)]).await {
                let items = json
                    .pointer("/data/download")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if let Some(vurl) = items
                    .iter()
                    .find(|i| i.get("type").and_then(Value::as_str) == Some("video"))
                    .and_then(|i| i.get("url").and_then(Value::as_str))
                {
                    if let Ok((bytes, _)) = download_media_bytes(ctx, vurl, None, MAX_MEDIA_BYTES).await {
                        if ctx.send_video_bytes(bytes, Some("Instagram".to_string()), "video/mp4").await.is_ok() {
                            sent = true;
                        }
                    }
                } else if let Some(iurl) = items
                    .iter()
                    .find(|i| i.get("type").and_then(Value::as_str) == Some("image"))
                    .and_then(|i| i.get("url").and_then(Value::as_str))
                {
                    if let Ok((bytes, mime)) = download_media_bytes(ctx, iurl, None, MAX_MEDIA_BYTES).await {
                        if ctx.send_image_bytes(bytes, Some("Instagram".to_string()), Vec::new(), &mime).await.is_ok() {
                            sent = true;
                        }
                    }
                }
            }

            if !sent {
                if let Ok(json) = fetch_json(ctx, &format!("{}/dl/instagram", alya), &[("url", query), ("key", key)]).await {
                    if let Some(u) = find_first_url(&json, &["url", "dl", "video", "image"]) {
                        if let Ok((bytes, mime)) = download_media_bytes(ctx, &u, None, MAX_MEDIA_BYTES).await {
                            if mime.contains("video") {
                                sent = ctx.send_video_bytes(bytes, Some("Instagram".to_string()), "video/mp4").await.is_ok();
                            } else {
                                sent = ctx.send_image_bytes(bytes, Some("Instagram".to_string()), Vec::new(), &mime).await.is_ok();
                            }
                        }
                    }
                }
            }

            if !sent {
                ctx.reply_text("No se pudo descargar ese contenido de Instagram.")
                    .await;
            }
            true
        }
        "fb" | "facebook" | "fbdl" | "facebookdl" | "fbvideo" | "fbv" | "fbreels" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}fb <enlace de Facebook>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let alya = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
            let mut vurl: Option<String> = None;

            if let Ok(json) = fetch_json(ctx, &format!("{}/dl/facebook", alya), &[("url", query), ("key", key)]).await {
                vurl = find_first_url(&json, &["hd", "url", "dl", "sd", "link"]);
            }
            if vurl.is_none() {
                if let Ok(json) = fetch_json(ctx, &format!("{}/download/facebook", delirius), &[("url", query)]).await {
                    vurl = find_first_url(&json, &["hd", "sd", "url", "dl"]);
                }
            }
            let Some(dl) = vurl else {
                ctx.reply_text("No se encontro video descargable en Facebook.")
                    .await;
                return true;
            };
            match download_media_bytes(ctx, &dl, Some("https://www.facebook.com/"), MAX_MEDIA_BYTES).await {
                Ok((bytes, _)) => {
                    if let Err(err) = ctx.send_video_bytes(bytes, Some("Facebook".to_string()), "video/mp4").await {
                        ctx.reply_text(format!("Error enviando video FB: {}", err)).await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error bajando FB ({}):\n{}", err, dl)).await,
            }
            true
        }
        "spotify" | "splay" | "sp" | "spdl" | "spotifydoc" | "docsplay" | "dsp" | "dspdl" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}spotify <cancion o enlace>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let alya = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let is_url = query.contains("open.spotify.com/");
            let (ep, param_k, param_v) = if is_url {
                ("/dl/spotify", "url", query.split('?').next().unwrap_or(query))
            } else {
                ("/dl/spotifyplay", "query", query)
            };
            match fetch_json(
                ctx,
                &format!("{}{}", alya, ep),
                &[(param_k, param_v), ("key", key)],
            )
            .await
            {
                Ok(json) => {
                    let title = find_str(&json, &["title", "name"]).unwrap_or_else(|| "Spotify".to_string());
                    if let Some(dl) = find_first_url(&json, &["mp3", "dl", "download", "url"]) {
                        ctx.reply_text(format!("Descargando Spotify: *{}*", title)).await;
                        match download_media_bytes(ctx, &dl, None, MAX_MEDIA_BYTES).await {
                            Ok((bytes, _)) => {
                                let as_doc = matches!(ctx.cmd_name.as_str(), "spotifydoc" | "docsplay" | "dsp" | "dspdl");
                                let res = if as_doc {
                                    let fname = format!("{}.mp3", title.replace('/', "_").chars().take(60).collect::<String>());
                                    ctx.send_document_bytes(bytes, &fname, "audio/mpeg", Some(format!("*{}*", title))).await
                                } else {
                                    ctx.send_audio_bytes(bytes, "audio/mpeg", false).await
                                };
                                if let Err(err) = res {
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
        "sc" | "scplay" | "scdl" | "soundcloud" | "dsc" | "dscplay" | "dscdl" | "docsoundcloud" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}sc <cancion o enlace SoundCloud>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let as_doc = matches!(ctx.cmd_name.as_str(), "dsc" | "dscplay" | "dscdl" | "docsoundcloud");
            let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
            let alya = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");

            let sc_url = if query.contains("soundcloud.com/") {
                query.to_string()
            } else {
                let s = fetch_json(ctx, &format!("{}/search/soundcloud", delirius), &[("q", query), ("limit", "3")])
                    .await
                    .ok()
                    .and_then(|j| find_first_url(&j, &["link", "url"]));
                match s {
                    Some(u) => u,
                    None => {
                        ctx.reply_text("No se encontraron pistas en SoundCloud.").await;
                        return true;
                    }
                }
            };

            let mut dl_info: Option<(String, String)> = None;
            if let Ok(j) = fetch_json(ctx, &format!("{}/download/soundcloud", delirius), &[("url", &sc_url)]).await {
                if let Some(u) = find_first_url(&j, &["dl", "download", "url", "mp3"]) {
                    let t = find_str(&j, &["title", "name"]).unwrap_or_else(|| "SoundCloud".to_string());
                    dl_info = Some((u, t));
                }
            }
            if dl_info.is_none() {
                if let Ok(j) = fetch_json(ctx, &format!("{}/dl/soundcloud", alya), &[("url", &sc_url), ("key", key)]).await {
                    if let Some(u) = find_first_url(&j, &["dl", "download", "url", "mp3"]) {
                        let t = find_str(&j, &["title", "name"]).unwrap_or_else(|| "SoundCloud".to_string());
                        dl_info = Some((u, t));
                    }
                }
            }

            let Some((dl_url, title)) = dl_info else {
                ctx.reply_text("No se pudo obtener el enlace descargable de SoundCloud.")
                    .await;
                return true;
            };
            ctx.reply_text(format!("Descargando SoundCloud: *{}*", title)).await;
            match download_media_bytes(ctx, &dl_url, None, MAX_MEDIA_BYTES).await {
                Ok((bytes, _)) => {
                    let res = if as_doc {
                        let fname = format!("{}.mp3", title.replace('/', "_").chars().take(60).collect::<String>());
                        ctx.send_document_bytes(bytes, &fname, "audio/mpeg", Some(format!("*{}*", title))).await
                    } else {
                        ctx.send_audio_bytes(bytes, "audio/mpeg", false).await
                    };
                    if let Err(err) = res {
                        ctx.reply_text(format!("Error enviando SoundCloud: {}", err)).await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error bajando SoundCloud: {}", err)).await,
            }
            true
        }
        "x" | "twitter" | "tw" | "xdl" => {
            if !query.contains("x.com/") && !query.contains("twitter.com/") {
                ctx.reply_text(format!("Uso: `{}x <enlace de X/Twitter>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let alya = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let delirius = DELIRIUS_API.base_url.trim_end_matches('/');
            let mut vurl: Option<String> = None;

            if let Ok(json) = fetch_json(ctx, &format!("{}/dl/twitter", alya), &[("url", query), ("key", key)]).await {
                vurl = find_first_url(&json, &["url", "hd", "sd", "dl"]);
            }
            if vurl.is_none() {
                if let Ok(json) = fetch_json(ctx, &format!("{}/download/twitterdl", delirius), &[("url", query)]).await {
                    vurl = find_first_url(&json, &["url", "hd", "sd", "dl"]);
                }
            }
            let Some(dl) = vurl else {
                ctx.reply_text("No se encontro video en ese enlace de X.")
                    .await;
                return true;
            };
            match download_media_bytes(ctx, &dl, None, MAX_MEDIA_BYTES).await {
                Ok((bytes, _)) => {
                    if let Err(err) = ctx.send_video_bytes(bytes, Some("X / Twitter".to_string()), "video/mp4").await {
                        ctx.reply_text(format!("Error enviando video X: {}", err)).await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error bajando X ({}):\n{}", err, dl)).await,
            }
            true
        }
        "apk" | "apkdl" | "apkd" | "apks" | "apkdownload" | "androidapp" | "app" => {
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
                    match download_media_bytes(ctx, &dl, None, MAX_MEDIA_BYTES).await {
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
                .header("User-Agent", BROWSER_UA)
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
            match download_media_bytes(ctx, &dl_url, Some(query), MAX_MEDIA_BYTES).await {
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
        "ssweb" | "ss" | "webss" => {
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
                        if looks_like_error_payload(&mime, &bytes) {
                            ctx.reply_text("La API de captura no devolvio una imagen valida.").await;
                        } else if let Err(err) = ctx
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
