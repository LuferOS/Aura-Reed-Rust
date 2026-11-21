use crate::config::ALYA_API;
use crate::context::CommandContext;
use serde_json::Value;

async fn alya_get(ctx: &CommandContext, path: &str, query: &str) -> Result<Value, String> {
    let base = ALYA_API.base_url.trim_end_matches('/');
    let url = format!("{}{}", base, path);
    let mut req = ctx.state.http.get(&url).query(&[("query", query)]);
    if let Some(key) = ALYA_API.api_key {
        req = req.query(&[("key", key)]);
    }
    let resp = req.send().await.map_err(|e| format!("red: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("json: {}", e))
}

fn str_field<'a>(obj: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(Value::as_str) {
            let t = s.trim();
            if !t.is_empty() {
                return Some(t);
            }
        }
    }
    None
}

fn extract_items(val: &Value) -> Vec<&Value> {
    if let Some(arr) = val.as_array() {
        return arr.iter().collect();
    }
    for k in ["data", "result", "results", "videos", "tracks", "items"] {
        if let Some(arr) = val.get(k).and_then(Value::as_array) {
            return arr.iter().collect();
        }
    }
    if let Some(data) = val.get("data") {
        for k in ["results", "videos", "tracks", "items", "list"] {
            if let Some(arr) = data.get(k).and_then(Value::as_array) {
                return arr.iter().collect();
            }
        }
    }
    Vec::new()
}

pub async fn run(ctx: &CommandContext) -> bool {
    let query = ctx.text.trim();
    match ctx.cmd_name.as_str() {
        "ytsearch" | "yts" | "plays" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}yts <busqueda>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match alya_get(ctx, "/search/youtube", query).await {
                Ok(json) => {
                    let items = extract_items(&json);
                    if items.is_empty() {
                        ctx.reply_text("No se encontraron resultados en YouTube.")
                            .await;
                        return true;
                    }
                    let lines = items
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, it)| {
                            let title = str_field(it, &["title", "name"]).unwrap_or("Sin titulo");
                            let dur = str_field(it, &["duration", "timestamp"]).unwrap_or("-");
                            let url = str_field(it, &["url", "link"]).unwrap_or("");
                            format!("{}. *{}* ({})\n   {}", i + 1, title, dur, url)
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    ctx.reply_text(format!("*Resultados YouTube*\n\n{}", lines))
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error buscando en YouTube: {}", err))
                        .await;
                }
            }
            true
        }
        "ttsearch" | "tiktoksearch" | "tts" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}tts <busqueda>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match alya_get(ctx, "/search/tiktok", query).await {
                Ok(json) => {
                    let items = extract_items(&json);
                    if items.is_empty() {
                        ctx.reply_text("No se encontraron videos en TikTok.").await;
                        return true;
                    }
                    let lines = items
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, it)| {
                            let title = str_field(it, &["title", "desc", "caption"])
                                .unwrap_or("Video TikTok");
                            let author = str_field(it, &["author", "username"]).unwrap_or("-");
                            let url = str_field(it, &["url", "play", "link"]).unwrap_or("");
                            format!("{}. *{}*\n   Autor: {} | {}", i + 1, title, author, url)
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    ctx.reply_text(format!("*Resultados TikTok*\n\n{}", lines))
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error buscando en TikTok: {}", err))
                        .await;
                }
            }
            true
        }
        "spsearch" | "spotifysearch" | "sps" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}sps <cancion>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match alya_get(ctx, "/search/spotify", query).await {
                Ok(json) => {
                    let items = extract_items(&json);
                    if items.is_empty() {
                        ctx.reply_text("No se encontraron canciones en Spotify.")
                            .await;
                        return true;
                    }
                    let lines = items
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, it)| {
                            let title = str_field(it, &["title", "name"]).unwrap_or("Cancion");
                            let artist = str_field(it, &["artist", "artists", "author"])
                                .unwrap_or("Desconocido");
                            let url = str_field(it, &["url", "link"]).unwrap_or("");
                            format!("{}. *{}* — {}\n   {}", i + 1, title, artist, url)
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    ctx.reply_text(format!("*Resultados Spotify*\n\n{}", lines))
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error buscando en Spotify: {}", err))
                        .await;
                }
            }
            true
        }
        "pin" | "pinterest" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}pin <consulta>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match alya_get(ctx, "/search/pinterest", query).await {
                Ok(json) => {
                    let items = extract_items(&json);
                    if items.is_empty() {
                        ctx.reply_text("No se encontraron imagenes en Pinterest.")
                            .await;
                        return true;
                    }
                    let lines = items
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, it)| {
                            let url = it
                                .as_str()
                                .or_else(|| str_field(it, &["image", "url", "hd", "link"]))
                                .unwrap_or("");
                            format!("{}. {}", i + 1, url)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    ctx.reply_text(format!("*Pinterest ({})*\n{}", query, lines))
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error buscando en Pinterest: {}", err))
                        .await;
                }
            }
            true
        }
        "scsearch" | "scbuscar" | "scb" | "scs" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}scsearch <cancion>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let delirius = crate::config::DELIRIUS_API.base_url.trim_end_matches('/');
            let resp = ctx
                .state
                .http
                .get(format!("{}/search/soundcloud", delirius))
                .query(&[("q", query), ("limit", "5")])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let json: Value = r.json().await.unwrap_or(Value::Null);
                    let items = extract_items(&json);
                    if items.is_empty() {
                        ctx.reply_text("No se encontraron pistas en SoundCloud.").await;
                        return true;
                    }
                    let lines = items
                        .iter()
                        .take(5)
                        .enumerate()
                        .map(|(i, it)| {
                            let title = str_field(it, &["title", "name"]).unwrap_or("Pista");
                            let author = str_field(it, &["artist", "author", "user"]).unwrap_or("-");
                            let url = str_field(it, &["link", "url"]).unwrap_or("");
                            format!("{}. *{}* — {}\n   {}", i + 1, title, author, url)
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    ctx.reply_text(format!("*Resultados SoundCloud*\n\n{}", lines)).await;
                }
                Ok(r) => ctx.reply_text(format!("Error SoundCloud: HTTP {}", r.status().as_u16())).await,
                Err(err) => ctx.reply_text(format!("Error SoundCloud: {}", err)).await,
            }
            true
        }
        "lrc" | "lyrics" | "letra" => {
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}lrc <cancion>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match alya_get(ctx, "/tools/lyrics", query).await {
                Ok(json) => {
                    let first = extract_items(&json).into_iter().next().unwrap_or(&json);
                    let title = str_field(first, &["title", "name", "track"]).unwrap_or(query);
                    let artist = str_field(first, &["artist", "author"]).unwrap_or("");
                    let lyrics = str_field(first, &["lyrics", "plainLyrics", "text", "result"])
                        .or_else(|| {
                            json.get("data")
                                .and_then(|d| str_field(d, &["lyrics", "plainLyrics", "text"]))
                        })
                        .unwrap_or("No se encontro la letra.");
                    let clipped: String = lyrics.chars().take(3200).collect();
                    let header = if artist.is_empty() {
                        format!("*{}*", title)
                    } else {
                        format!("*{}* — {}", title, artist)
                    };
                    ctx.reply_text(format!("{}\n\n{}", header, clipped)).await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error buscando letra: {}", err))
                        .await;
                }
            }
            true
        }
        _ => false,
    }
}
