use crate::config::{ALYA_API, NEKOS_API};
use crate::context::CommandContext;
use crate::lang::{resolve_lang, tr};
use baileys_rs::utils::unix_timestamp_seconds;
use serde_json::Value;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_REACTION_BYTES: usize = 15 * 1024 * 1024;
const R34_USER_ID: &str = "6679412";
const R34_API_KEY: &str = "2faa230764f8b4c823f54b2022fd240d2f9fa4a4e6fee5f89e76d0ca2fbf586967e3ccc14c5fa298239c87ffd8ae7256afd6ca49928b58ef2002ad1004c0da28";

static REACT_SEQ: AtomicU64 = AtomicU64::new(0);
static REACT_RNG: AtomicU64 = AtomicU64::new(0x94d0_49bb_1331_11eb);

struct BuiltinReaction {
    cmd: &'static str,
    self_text: &'static str,
    target_text: &'static str,
    nekos_ep: Option<&'static str>,
    waifu_ep: Option<&'static str>,
    nsfw: bool,
}

const BUILTIN_REACTIONS: &[BuiltinReaction] = &[
    BuiltinReaction { cmd: "hug", self_text: "se abraza a si mismo/a", target_text: "le dio un fuerte abrazo a", nekos_ep: Some("hug"), waifu_ep: Some("hug"), nsfw: false },
    BuiltinReaction { cmd: "kiss", self_text: "manda un beso al aire", target_text: "beso a", nekos_ep: Some("kiss"), waifu_ep: Some("kiss"), nsfw: false },
    BuiltinReaction { cmd: "slap", self_text: "se dio una bofetada", target_text: "le dio una bofetada a", nekos_ep: Some("slap"), waifu_ep: Some("slap"), nsfw: false },
    BuiltinReaction { cmd: "pat", self_text: "se da palmaditas", target_text: "acaricio la cabeza de", nekos_ep: Some("pat"), waifu_ep: Some("pat"), nsfw: false },
    BuiltinReaction { cmd: "poke", self_text: "se pica la mejilla", target_text: "esta molestando a", nekos_ep: Some("poke"), waifu_ep: Some("poke"), nsfw: false },
    BuiltinReaction { cmd: "bite", self_text: "se mordio la lengua", target_text: "mordio a", nekos_ep: Some("bite"), waifu_ep: Some("bite"), nsfw: false },
    BuiltinReaction { cmd: "blush", self_text: "se sonrojo", target_text: "se sonrojo por", nekos_ep: Some("blush"), waifu_ep: Some("blush"), nsfw: false },
    BuiltinReaction { cmd: "cry", self_text: "esta llorando", target_text: "llora por culpa de", nekos_ep: Some("cry"), waifu_ep: Some("cry"), nsfw: false },
    BuiltinReaction { cmd: "cuddle", self_text: "se acurruca solo/a", target_text: "se acurruco con", nekos_ep: Some("cuddle"), waifu_ep: Some("cuddle"), nsfw: false },
    BuiltinReaction { cmd: "dance", self_text: "esta bailando", target_text: "baila con", nekos_ep: Some("dance"), waifu_ep: Some("dance"), nsfw: false },
    BuiltinReaction { cmd: "happy", self_text: "esta muy feliz", target_text: "esta feliz por", nekos_ep: Some("happy"), waifu_ep: Some("happy"), nsfw: false },
    BuiltinReaction { cmd: "kill", self_text: "se autoelimino", target_text: "acabo con", nekos_ep: None, waifu_ep: Some("kill"), nsfw: false },
    BuiltinReaction { cmd: "laugh", self_text: "se esta riendo a carcajadas", target_text: "se rie de", nekos_ep: Some("laugh"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "lick", self_text: "se lame los labios", target_text: "lamio a", nekos_ep: None, waifu_ep: Some("lick"), nsfw: false },
    BuiltinReaction { cmd: "punch", self_text: "golpea el aire", target_text: "le dio un golpe a", nekos_ep: Some("punch"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "wink", self_text: "guino un ojo", target_text: "le guino un ojo a", nekos_ep: Some("wink"), waifu_ep: Some("wink"), nsfw: false },
    BuiltinReaction { cmd: "wave", self_text: "saluda a todos", target_text: "saluda a", nekos_ep: Some("wave"), waifu_ep: Some("wave"), nsfw: false },
    BuiltinReaction { cmd: "smile", self_text: "sonrie", target_text: "le sonrie a", nekos_ep: Some("smile"), waifu_ep: Some("smile"), nsfw: false },
    BuiltinReaction { cmd: "smug", self_text: "pone cara presumida", target_text: "mira con superioridad a", nekos_ep: Some("smug"), waifu_ep: Some("smug"), nsfw: false },
    BuiltinReaction { cmd: "highfive", self_text: "choca los cinco al aire", target_text: "choco los cinco con", nekos_ep: Some("highfive"), waifu_ep: Some("highfive"), nsfw: false },
    BuiltinReaction { cmd: "handhold", self_text: "entrelaza sus manos", target_text: "tomo de la mano a", nekos_ep: Some("handhold"), waifu_ep: Some("handhold"), nsfw: false },
    BuiltinReaction { cmd: "bonk", self_text: "se dio un bonk", target_text: "le dio un bonk a", nekos_ep: None, waifu_ep: Some("bonk"), nsfw: false },
    BuiltinReaction { cmd: "yeet", self_text: "salio volando", target_text: "lanzo por los aires a", nekos_ep: Some("yeet"), waifu_ep: Some("yeet"), nsfw: false },
    BuiltinReaction { cmd: "angry", self_text: "esta furioso/a", target_text: "esta furioso/a con", nekos_ep: Some("angry"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "bored", self_text: "esta aburrido/a", target_text: "se aburre con", nekos_ep: Some("bored"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "clap", self_text: "esta aplaudiendo", target_text: "le aplaude a", nekos_ep: Some("clap"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "facepalm", self_text: "se llevo la mano a la cara", target_text: "hace facepalm por", nekos_ep: Some("facepalm"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "pout", self_text: "hace puchero", target_text: "le hace puchero a", nekos_ep: Some("pout"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "sleep", self_text: "se quedo dormido/a", target_text: "se durmio sobre", nekos_ep: Some("sleep"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "think", self_text: "esta pensando profundamente", target_text: "piensa en", nekos_ep: Some("think"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "tickle", self_text: "se hace cosquillas", target_text: "le hace cosquillas a", nekos_ep: Some("tickle"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "feed", self_text: "esta comiendo", target_text: "le dio de comer a", nekos_ep: Some("feed"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "cookie", self_text: "esta comiendo una galleta", target_text: "le invito una galleta a", nekos_ep: Some("feed"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "love", self_text: "irradia amor", target_text: "esta enamorado/a de", nekos_ep: Some("hug"), waifu_ep: Some("kiss"), nsfw: false },
    BuiltinReaction { cmd: "sad", self_text: "esta triste", target_text: "esta triste por", nekos_ep: Some("cry"), waifu_ep: Some("cry"), nsfw: false },
    BuiltinReaction { cmd: "shy", self_text: "se puso timido/a", target_text: "se pone timido/a con", nekos_ep: Some("blush"), waifu_ep: Some("blush"), nsfw: false },
    BuiltinReaction { cmd: "run", self_text: "salio corriendo", target_text: "huye de", nekos_ep: Some("run"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "shrug", self_text: "se encoge de hombros", target_text: "se encoge de hombros ante", nekos_ep: Some("shrug"), waifu_ep: None, nsfw: false },
    BuiltinReaction { cmd: "spank", self_text: "se nalgueo", target_text: "le dio unas nalgadas a", nekos_ep: None, waifu_ep: Some("waifu"), nsfw: true },
    BuiltinReaction { cmd: "blowjob", self_text: "hace un oral", target_text: "le hizo un oral a", nekos_ep: None, waifu_ep: Some("blowjob"), nsfw: true },
    BuiltinReaction { cmd: "fuck", self_text: "se prendio solo/a", target_text: "se la paso intenso con", nekos_ep: None, waifu_ep: Some("waifu"), nsfw: true },
    BuiltinReaction { cmd: "yuri", self_text: "disfruta una escena yuri", target_text: "comparte un momento yuri con", nekos_ep: None, waifu_ep: Some("neko"), nsfw: true },
    BuiltinReaction { cmd: "undress", self_text: "se quita la ropa", target_text: "le quito la ropa a", nekos_ep: None, waifu_ep: Some("waifu"), nsfw: true },
    BuiltinReaction { cmd: "cum", self_text: "llego al climax", target_text: "se vino encima de", nekos_ep: None, waifu_ep: Some("waifu"), nsfw: true },
    BuiltinReaction { cmd: "anal", self_text: "experimenta solo/a", target_text: "tuvo accion intensa con", nekos_ep: None, waifu_ep: Some("waifu"), nsfw: true },
];

fn next_rand() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut x = REACT_RNG.fetch_add(now ^ 0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

fn lookup_json_reaction(data_dir: &Path, filename: &str, cmd: &str) -> Option<(String, String, Vec<String>)> {
    let path = data_dir.join(filename);
    let raw = std::fs::read_to_string(path).ok()?;
    let val: Value = serde_json::from_str(&raw).ok()?;
    let entry = val.get(cmd)?;
    let self_txt = entry.get("self").and_then(Value::as_str)?.to_string();
    let target_txt = entry.get("target").and_then(Value::as_str)?.to_string();
    let videos = entry
        .get("videos")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .filter(|s| s.starts_with("http"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some((self_txt, target_txt, videos))
}

async fn gif_to_mp4(raw_bytes: &[u8], tmp_dir: &Path) -> Result<Vec<u8>, String> {
    let _ = tokio::fs::create_dir_all(tmp_dir).await;
    let seq = REACT_SEQ.fetch_add(1, Ordering::Relaxed);
    let stamp = format!("{}_{}_{}", std::process::id(), unix_timestamp_seconds(), seq);
    let in_path = tmp_dir.join(format!("r_in_{}.gif", stamp));
    let out_path = tmp_dir.join(format!("r_out_{}.mp4", stamp));

    tokio::fs::write(&in_path, raw_bytes)
        .await
        .map_err(|e| format!("write: {}", e))?;

    let ok = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(&in_path)
        .args([
            "-movflags",
            "faststart",
            "-pix_fmt",
            "yuv420p",
            "-vf",
            "scale=trunc(iw/2)*2:trunc(ih/2)*2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-an",
        ])
        .arg(&out_path)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    let res = if ok {
        tokio::fs::read(&out_path)
            .await
            .map_err(|e| format!("read: {}", e))
    } else {
        Err("ffmpeg gif->mp4 fallo".to_string())
    };

    tokio::spawn(async move {
        let _ = tokio::fs::remove_file(in_path).await;
        let _ = tokio::fs::remove_file(out_path).await;
    });

    res
}

async fn fetch_reaction_media_url(ctx: &CommandContext, b: &BuiltinReaction) -> Option<String> {
    if !b.nsfw {
        if let Some(ep) = b.nekos_ep {
            let url = format!("{}/{}", NEKOS_API.base_url.trim_end_matches('/'), ep);
            if let Ok(resp) = ctx.state.http.get(&url).send().await {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(u) = json
                        .pointer("/results/0/url")
                        .and_then(Value::as_str)
                    {
                        return Some(u.to_string());
                    }
                }
            }
        }
    }
    if let Some(ep) = b.waifu_ep {
        let mode = if b.nsfw { "nsfw" } else { "sfw" };
        let url = format!("https://api.waifu.pics/{}/{}", mode, ep);
        if let Ok(resp) = ctx.state.http.get(&url).send().await {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(u) = json.get("url").and_then(Value::as_str) {
                    return Some(u.to_string());
                }
            }
        }
    }
    None
}

async fn send_reaction_media(
    ctx: &CommandContext,
    media_url: &str,
    caption: String,
    mentions: Vec<String>,
) {
    let Ok(resp) = ctx.state.http.get(media_url).send().await else {
        ctx.reply_with_mentions(caption, mentions).await;
        return;
    };
    if !resp.status().is_success() {
        ctx.reply_with_mentions(caption, mentions).await;
        return;
    }
    let mime = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/gif")
        .split(';')
        .next()
        .unwrap_or("image/gif")
        .to_string();
    let Ok(bytes) = resp.bytes().await else {
        ctx.reply_with_mentions(caption, mentions).await;
        return;
    };
    if bytes.len() > MAX_REACTION_BYTES {
        ctx.reply_with_mentions(caption, mentions).await;
        return;
    }
    if mime.contains("mp4") || media_url.ends_with(".mp4") {
        if ctx
            .send_gif_video_bytes(bytes.to_vec(), Some(caption.clone()), mentions.clone())
            .await
            .is_err()
        {
            ctx.reply_with_mentions(caption, mentions).await;
        }
    } else if mime.contains("gif") || media_url.ends_with(".gif") {
        match gif_to_mp4(&bytes, &ctx.state.config.cache_dir).await {
            Ok(mp4) => {
                if ctx
                    .send_gif_video_bytes(mp4, Some(caption.clone()), mentions.clone())
                    .await
                    .is_err()
                {
                    ctx.reply_with_mentions(caption, mentions).await;
                }
            }
            Err(_) => {
                let _ = ctx
                    .send_image_bytes(bytes.to_vec(), Some(caption), mentions, &mime)
                    .await;
            }
        }
    } else if ctx
        .send_image_bytes(bytes.to_vec(), Some(caption.clone()), mentions.clone(), &mime)
        .await
        .is_err()
    {
        ctx.reply_with_mentions(caption, mentions).await;
    }
}

fn check_nsfw_allowed(ctx: &CommandContext) -> bool {
    if !ctx.is_group {
        return true;
    }
    !ctx.state.db.is_category_blocked(&ctx.from, "nsfw")
}

pub async fn run(ctx: &CommandContext) -> bool {
    let lang = resolve_lang(&ctx.user.user_lang, None);
    match ctx.cmd_name.as_str() {
        "r34" | "rule34" => {
            if !check_nsfw_allowed(ctx) {
                ctx.reply_text(tr(&ctx.state.config.data_dir, lang, "nsfw_disabled"))
                    .await;
                return true;
            }
            let query = ctx.text.trim();
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}r34 <tags>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let resp = ctx
                .state
                .http
                .get("https://api.rule34.xxx/index.php")
                .query(&[
                    ("page", "dapi"),
                    ("s", "post"),
                    ("q", "index"),
                    ("json", "1"),
                    ("limit", "40"),
                    ("tags", query),
                    ("api_key", R34_API_KEY),
                    ("user_id", R34_USER_ID),
                ])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let posts: Vec<Value> = r.json().await.unwrap_or_default();
                    let urls: Vec<String> = posts
                        .iter()
                        .filter_map(|p| {
                            p.get("sample_url")
                                .or_else(|| p.get("file_url"))
                                .and_then(Value::as_str)
                                .filter(|u| {
                                    u.ends_with(".jpg")
                                        || u.ends_with(".jpeg")
                                        || u.ends_with(".png")
                                        || u.ends_with(".webp")
                                })
                                .map(str::to_string)
                        })
                        .collect();
                    if urls.is_empty() {
                        ctx.reply_text(format!("Sin resultados en Rule34 para `{}`.", query))
                            .await;
                        return true;
                    }
                    let picked = &urls[(next_rand() as usize) % urls.len()];
                    send_reaction_media(ctx, picked, format!("*Rule34*: `{}`", query), Vec::new()).await;
                }
                Ok(r) => ctx.reply_text(format!("Rule34 HTTP {}", r.status().as_u16())).await,
                Err(err) => ctx.reply_text(format!("Error consultando Rule34: {}", err)).await,
            }
            true
        }
        "xnxx" | "xn" | "xvideos" | "xv" | "xvid" => {
            if !check_nsfw_allowed(ctx) {
                ctx.reply_text(tr(&ctx.state.config.data_dir, lang, "nsfw_disabled"))
                    .await;
                return true;
            }
            let query = ctx.text.trim();
            if query.is_empty() {
                ctx.reply_text(format!("Uso: `{}{} <busqueda o url>`", ctx.used_prefix, ctx.cmd_name))
                    .await;
                return true;
            }
            let is_xn = matches!(ctx.cmd_name.as_str(), "xnxx" | "xn");
            let base = ALYA_API.base_url.trim_end_matches('/');
            let key = ALYA_API.api_key.unwrap_or("");
            let is_url = query.starts_with("http://") || query.starts_with("https://");
            let (ep, k) = if is_url {
                (if is_xn { "/nsfw/dl/xnxx" } else { "/nsfw/dl/xvideos" }, "url")
            } else {
                (if is_xn { "/nsfw/search/xnxx" } else { "/nsfw/search/xvideos" }, "query")
            };
            let resp = ctx
                .state
                .http
                .get(format!("{}{}", base, ep))
                .query(&[(k, query), ("key", key)])
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => {
                    let json: Value = r.json().await.unwrap_or(Value::Null);
                    if is_url {
                        let dl = json
                            .pointer("/resultado/videos/high")
                            .or_else(|| json.pointer("/resultado/videos/low"))
                            .or_else(|| json.get("download"))
                            .or_else(|| json.get("url"))
                            .and_then(Value::as_str);
                        if let Some(vurl) = dl {
                            ctx.reply_text(format!("*Descarga lista:*\n{}", vurl)).await;
                        } else {
                            ctx.reply_text("No se encontro enlace de video descargable.").await;
                        }
                    } else {
                        let items = json
                            .get("resultados")
                            .or_else(|| json.get("data"))
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        if items.is_empty() {
                            ctx.reply_text("No se encontraron resultados.").await;
                            return true;
                        }
                        let lines = items
                            .iter()
                            .take(5)
                            .enumerate()
                            .map(|(i, it)| {
                                let title = it.get("title").and_then(Value::as_str).unwrap_or("Video");
                                let dur = it.get("duration").and_then(Value::as_str).unwrap_or("-");
                                let url = it.get("url").and_then(Value::as_str).unwrap_or("");
                                format!("{}. *{}* ({})\n   {}", i + 1, title, dur, url)
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n");
                        ctx.reply_text(format!("*Resultados*\n\n{}", lines)).await;
                    }
                }
                Ok(r) => ctx.reply_text(format!("Error API NSFW: HTTP {}", r.status().as_u16())).await,
                Err(err) => ctx.reply_text(format!("Error API NSFW: {}", err)).await,
            }
            true
        }
        _ => {
            let cmd = ctx.cmd_name.as_str();
            let custom_sfw = lookup_json_reaction(&ctx.state.config.data_dir, "reacctiones.json", cmd);
            let custom_nsfw = if custom_sfw.is_none() {
                lookup_json_reaction(&ctx.state.config.data_dir, "reacctiones2.json", cmd)
            } else {
                None
            };
            let builtin = BUILTIN_REACTIONS.iter().find(|r| r.cmd == cmd);

            let (is_nsfw, self_txt, target_txt, json_videos) = if let Some((s, t, v)) = custom_sfw {
                (false, s, t, v)
            } else if let Some((s, t, v)) = custom_nsfw {
                (true, s, t, v)
            } else if let Some(b) = builtin {
                (b.nsfw, b.self_text.to_string(), b.target_text.to_string(), Vec::new())
            } else {
                return false;
            };

            if is_nsfw && !check_nsfw_allowed(ctx) {
                ctx.reply_text(tr(&ctx.state.config.data_dir, lang, "nsfw_disabled"))
                    .await;
                return true;
            }

            let target = ctx.target_jid();
            let (caption, mentions) = if target == ctx.sender {
                (
                    format!("@{} {}", jid_tag(&ctx.sender), self_txt),
                    vec![ctx.sender.clone()],
                )
            } else {
                (
                    format!("@{} {} @{}", jid_tag(&ctx.sender), target_txt, jid_tag(&target)),
                    vec![ctx.sender.clone(), target],
                )
            };

            let media_url = if !json_videos.is_empty() {
                Some(json_videos[(next_rand() as usize) % json_videos.len()].clone())
            } else if let Some(b) = builtin {
                fetch_reaction_media_url(ctx, b).await
            } else {
                None
            };

            if let Some(url) = media_url {
                send_reaction_media(ctx, &url, caption, mentions).await;
            } else {
                ctx.reply_with_mentions(caption, mentions).await;
            }
            true
        }
    }
}
