use crate::config::{save_runtime_mode, AppConfig, RuntimeMode};
use crate::db::{AuraDb, UserRecord};
use crate::debug::DebugState;
use crate::snipe::SnipeStore;
use baileys_rs::defaults::MediaType;
use baileys_rs::proto::message::{AudioMessage, DocumentMessage, ExtendedTextMessage, ImageMessage, StickerMessage, VideoMessage};
use baileys_rs::proto::{ContextInfo, Message};
use baileys_rs::socket::{MessagesSocket, SubBotManager};
use baileys_rs::types::{GroupMetadata, GroupParticipant, WaMessage};
use baileys_rs::utils::{
    encrypted_stream, normalize_message_content, unix_timestamp_seconds, upload_media_to_server, AnyMessageContent,
    MessageGenerationOptions, WaMediaUpload,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sha2::{Digest, Sha256};
use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

const MAX_GROUP_META_CACHE: usize = 24;
const GROUP_META_TTL_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct GachaTradeOffer {
    pub from_jid: String,
    pub to_jid: String,
    pub offer_char: String,
    pub offer_img: i64,
    pub want_char: String,
    pub want_img: i64,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub struct HangmanGame {
    pub word: String,
    pub guessed: Vec<char>,
    pub wrong: u8,
    pub started_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PendingProposal {
    pub kind: String,
    pub from: String,
    pub to: String,
    pub created_at: Instant,
}

pub struct BotState {
    pub config: AppConfig,
    pub started_at: Instant,
    pub wa_version: [u32; 3],
    pub processed_msgs: AtomicU64,
    pub mode_perf: AtomicBool,
    pub db: Arc<AuraDb>,
    pub snipe: Arc<SnipeStore>,
    pub debug: Arc<DebugState>,
    pub subbots: Arc<SubBotManager>,
    pub http: reqwest::Client,
    pub cmd_semaphore: Arc<Semaphore>,
    pub ffmpeg_sem: Arc<Semaphore>,
    pub creds_lock: Arc<Mutex<()>>,
    pub proposals: std::sync::Mutex<HashMap<String, PendingProposal>>,
    pub spam_tracker: std::sync::Mutex<HashMap<String, VecDeque<Instant>>>,
    pub group_meta_cache: std::sync::Mutex<HashMap<String, (GroupMetadata, Instant)>>,
    pub hangman_games: std::sync::Mutex<HashMap<String, HangmanGame>>,
    pub group_activity: std::sync::Mutex<HashMap<String, HashMap<String, u32>>>,
    pub gacha_trades: std::sync::Mutex<HashMap<String, GachaTradeOffer>>,
}

impl BotState {
    pub fn build_http_client(perf: bool) -> reqwest::Client {
        let (max_idle, idle_secs) = if perf { (6, 45) } else { (1, 10) };
        reqwest::Client::builder()
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(idle_secs))
            .pool_max_idle_per_host(max_idle)
            .http2_adaptive_window(true)
            .connect_timeout(Duration::from_secs(12))
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    }

    pub fn current_mode(&self) -> RuntimeMode {
        if self.mode_perf.load(Ordering::Relaxed) {
            RuntimeMode::Perf
        } else {
            RuntimeMode::Eco
        }
    }

    pub fn set_runtime_mode(&self, mode: RuntimeMode) {
        let perf = mode.is_perf();
        self.mode_perf.store(perf, Ordering::Relaxed);
        self.db.apply_mode(perf);
        self.snipe.set_capacity(if perf { 96 } else { 24 });
        if let Ok(mut gm) = self.group_meta_cache.lock() {
            let cap = if perf { 48 } else { 12 };
            if gm.len() > cap {
                gm.clear();
            }
            gm.shrink_to_fit();
        }
        if let Ok(mut st) = self.spam_tracker.lock() {
            if !perf {
                st.clear();
                st.shrink_to_fit();
            }
        }
        if let Ok(mut ga) = self.group_activity.lock() {
            if !perf && ga.len() > 8 {
                ga.clear();
                ga.shrink_to_fit();
            }
        }
        save_runtime_mode(&self.config.data_dir, mode);
    }

    pub async fn get_group_metadata(
        &self,
        messages: &Arc<MessagesSocket>,
        jid: &str,
        force_refresh: bool,
    ) -> Option<GroupMetadata> {
        if !force_refresh {
            if let Ok(cache) = self.group_meta_cache.lock() {
                if let Some((meta, ts)) = cache.get(jid) {
                    if ts.elapsed().as_secs() < GROUP_META_TTL_SECS {
                        return Some(meta.clone());
                    }
                }
            }
        }
        let fetched = messages.socket.group_metadata(jid).await.ok()?;
        if let Ok(mut cache) = self.group_meta_cache.lock() {
            if cache.len() >= MAX_GROUP_META_CACHE {
                cache.clear();
            }
            cache.insert(jid.to_string(), (fetched.clone(), Instant::now()));
        }
        Some(fetched)
    }


    pub fn record_group_activity(&self, group_jid: &str, user_jid: &str) {
        if let Ok(mut map) = self.group_activity.lock() {
            if map.len() > 32 && !map.contains_key(group_jid) {
                map.clear();
            }
            let group_map = map.entry(group_jid.to_string()).or_default();
            if group_map.len() > 256 && !group_map.contains_key(user_jid) {
                return;
            }
            *group_map.entry(user_jid.to_string()).or_insert(0) += 1;
        }
    }

    pub fn top_group_activity(&self, group_jid: &str, limit: usize) -> Vec<(String, u32)> {
        let Ok(map) = self.group_activity.lock() else {
            return Vec::new();
        };
        let Some(group_map) = map.get(group_jid) else {
            return Vec::new();
        };
        let mut list: Vec<(String, u32)> = group_map
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        list.sort_by(|a, b| b.1.cmp(&a.1));
        list.truncate(limit);
        list
    }

    pub fn invalidate_group_metadata(&self, jid: &str) {
        if let Ok(mut cache) = self.group_meta_cache.lock() {
            cache.remove(jid);
        }
    }
}

pub fn same_bare_jid(a: &str, b: &str) -> bool {
    let ba = a.split(':').next().unwrap_or(a);
    let bb = b.split(':').next().unwrap_or(b);
    if ba == bb {
        return true;
    }
    let ua = ba.split('@').next().unwrap_or(ba);
    let ub = bb.split('@').next().unwrap_or(bb);
    !ua.is_empty() && ua == ub
}

pub fn participant_matches(p: &GroupParticipant, target: &str) -> bool {
    same_bare_jid(&p.id, target)
        || p.lid.as_deref().is_some_and(|l| same_bare_jid(l, target))
        || p.phone_number.as_deref().is_some_and(|n| same_bare_jid(n, target))
}

pub fn is_participant_admin(meta: &GroupMetadata, target: &str) -> bool {
    meta.participants
        .iter()
        .any(|p| participant_matches(p, target) && p.admin.is_some())
}

#[allow(dead_code)]
pub struct PreparedVideo {
    pub bytes: Vec<u8>,
    pub thumbnail: Option<Vec<u8>>,
    pub seconds: Option<u32>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

async fn prepare_video_for_whatsapp(
    bytes: Vec<u8>,
    cache_dir: &std::path::Path,
    ffmpeg_sem: &Arc<Semaphore>,
) -> Result<PreparedVideo, String> {
    if bytes.len() < 128 {
        return Err("Archivo de video vacio o corrupto.".to_string());
    }
    let head = &bytes[..bytes.len().min(64)];
    if head.starts_with(b"<!DOCTYPE")
        || head.starts_with(b"<html")
        || head.starts_with(b"<HTML")
        || head.starts_with(b"{")
    {
        return Err("El servidor devolvio una pagina de error en lugar del video.".to_string());
    }

    let Ok(_permit) = ffmpeg_sem.clone().acquire_owned().await else {
        return Ok(PreparedVideo {
            bytes,
            thumbnail: None,
            seconds: None,
            width: None,
            height: None,
        });
    };

    let _ = tokio::fs::create_dir_all(cache_dir).await;
    let stamp = format!(
        "{}_{}_{}",
        std::process::id(),
        unix_timestamp_seconds(),
        bytes.len()
    );
    let in_path = cache_dir.join(format!("vid_in_{}.mp4", stamp));
    let out_path = cache_dir.join(format!("vid_out_{}.mp4", stamp));
    let thumb_path = cache_dir.join(format!("vid_th_{}.jpg", stamp));

    if tokio::fs::write(&in_path, &bytes).await.is_err() {
        return Ok(PreparedVideo {
            bytes,
            thumbnail: None,
            seconds: None,
            width: None,
            height: None,
        });
    }

    let probe_out = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,codec_name,pix_fmt,width,height:format=duration",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(&in_path)
        .output()
        .await
        .ok();

    let mut vcodec = String::new();
    let mut pix_fmt = String::new();
    let mut acodec = String::new();
    let mut has_audio = false;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut seconds: Option<u32> = None;

    if let Some(ref po) = probe_out {
        let text = String::from_utf8_lossy(&po.stdout);
        let mut cur_type = String::new();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                match k.trim() {
                    "codec_type" => {
                        cur_type = v.trim().to_string();
                        if cur_type == "audio" {
                            has_audio = true;
                        }
                    }
                    "codec_name" => {
                        if cur_type == "video" && vcodec.is_empty() {
                            vcodec = v.trim().to_ascii_lowercase();
                        } else if cur_type == "audio" && acodec.is_empty() {
                            acodec = v.trim().to_ascii_lowercase();
                        }
                    }
                    "pix_fmt" if cur_type == "video" && pix_fmt.is_empty() => {
                        pix_fmt = v.trim().to_ascii_lowercase();
                    }
                    "width" if cur_type == "video" && width.is_none() => {
                        width = v.trim().parse::<u32>().ok();
                    }
                    "height" if cur_type == "video" && height.is_none() => {
                        height = v.trim().parse::<u32>().ok();
                    }
                    "duration" if seconds.is_none() => {
                        seconds = v
                            .trim()
                            .parse::<f64>()
                            .ok()
                            .filter(|d| d.is_finite() && *d > 0.0)
                            .map(|d| d.round().max(1.0) as u32);
                    }
                    _ => {}
                }
            }
        }
    }

    let even_dims = width.zip(height).is_some_and(|(w, h)| w > 0 && h > 0 && w % 2 == 0 && h % 2 == 0);
    let already_compatible = vcodec == "h264"
        && (pix_fmt.is_empty() || pix_fmt == "yuv420p")
        && (!has_audio || acodec == "aac")
        && even_dims;

    let conv_ok;
    if already_compatible {
        conv_ok = tokio::process::Command::new("ffmpeg")
            .args(["-y", "-loglevel", "error", "-i"])
            .arg(&in_path)
            .args(["-c", "copy", "-movflags", "+faststart"])
            .arg(&out_path)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
    } else {
        let mut cmd = tokio::process::Command::new("ffmpeg");
        cmd.args([
            "-y",
            "-loglevel",
            "error",
            "-threads",
            "2",
            "-i",
        ])
        .arg(&in_path)
        .args([
            "-map",
            "0:v:0",
            "-map",
            "0:a?",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "24",
            "-pix_fmt",
            "yuv420p",
            "-vf",
            "scale=trunc(iw/2)*2:trunc(ih/2)*2",
        ]);
        if has_audio && acodec == "aac" {
            cmd.args(["-c:a", "copy"]);
        } else {
            cmd.args(["-c:a", "aac", "-b:a", "128k", "-ar", "44100", "-ac", "2"]);
        }
        conv_ok = cmd
            .args(["-movflags", "+faststart"])
            .arg(&out_path)
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
    }

    let final_src = if conv_ok { &out_path } else { &in_path };

    let _ = tokio::process::Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(final_src)
        .args(["-frames:v", "1", "-vf", "scale=240:-1", "-q:v", "7"])
        .arg(&thumb_path)
        .output()
        .await;

    let final_bytes = if conv_ok {
        tokio::fs::read(&out_path).await.unwrap_or(bytes)
    } else {
        bytes
    };
    let thumbnail = tokio::fs::read(&thumb_path).await.ok().filter(|t| !t.is_empty());

    tokio::spawn(async move {
        let _ = tokio::fs::remove_file(in_path).await;
        let _ = tokio::fs::remove_file(out_path).await;
        let _ = tokio::fs::remove_file(thumb_path).await;
    });

    Ok(PreparedVideo {
        bytes: final_bytes,
        thumbnail,
        seconds,
        width,
        height,
    })
}

pub struct CommandContext {
    pub state: Arc<BotState>,
    pub messages: Arc<MessagesSocket>,
    pub message: WaMessage,
    pub from: String,
    pub sender: String,
    pub push_name: Option<String>,
    pub bot_jid: Option<String>,
    pub is_group: bool,
    pub is_owner: bool,
    pub user: UserRecord,
    pub used_prefix: String,
    pub cmd_name: String,
    pub args: Vec<String>,
    pub text: String,
    pub received_at: Instant,
}

impl CommandContext {
    pub async fn reply_text(&self, text: impl Into<String>) {
        self.reply_with_mentions(text, Vec::new()).await;
    }

    pub async fn reply_with_mentions(&self, text: impl Into<String>, mentions: Vec<String>) {
        let clean_mentions: Vec<String> = mentions
            .into_iter()
            .map(|m| crate::ui::normalize_jid(&m))
            .collect();
        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            message_id: None,
            timestamp: None,
            quoted: Some(Box::new(self.message.clone())),
            ephemeral_expiration: None,
            mentions: clean_mentions,
            view_once: false,
            edit: None,
            context_info: None,
            background_color: None,
            font: None,
        };
        if let Err(err) = self
            .messages
            .send_message(&self.from, AnyMessageContent::Text(text.into()), opts)
            .await
        {
            self.state
                .debug
                .record_error("reply_text", format!("{}: {}", self.from, err))
                .await;
        }
    }

    pub async fn group_metadata(&self, force_refresh: bool) -> Option<GroupMetadata> {
        if !self.is_group {
            return None;
        }
        self.state
            .get_group_metadata(&self.messages, &self.from, force_refresh)
            .await
    }

    pub fn invalidate_group_cache(&self) {
        self.state.invalidate_group_metadata(&self.from);
    }

    pub async fn admin_status(&self) -> (bool, bool) {
        let sender_priv = self.is_owner
            || matches!(self.user.role.as_str(), "owner" | "coowner" | "mod");
        let Some(meta) = self.group_metadata(false).await else {
            return (sender_priv, false);
        };
        let sender_admin = sender_priv || is_participant_admin(&meta, &self.sender);
        let bot_admin = self
            .bot_jid
            .as_deref()
            .is_some_and(|b| is_participant_admin(&meta, b));
        (sender_admin, bot_admin)
    }

    pub async fn send_image_bytes(
        &self,
        bytes: Vec<u8>,
        caption: Option<String>,
        mentions: Vec<String>,
        mimetype: &str,
    ) -> Result<(), String> {
        self.send_image_bytes_returning_id(bytes, caption, mentions, mimetype)
            .await
            .map(|_| ())
    }

    pub async fn send_image_bytes_returning_id(
        &self,
        bytes: Vec<u8>,
        caption: Option<String>,
        mentions: Vec<String>,
        mimetype: &str,
    ) -> Result<Option<String>, String> {
        let raw_len = bytes.len() as u64;
        if raw_len == 0 {
            return Err("Imagen vacia.".to_string());
        }
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&bytes);
            (h.finalize().to_vec(), bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Image, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Image,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(20_000),
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

        let up = upload_res.map_err(|e| format!("upload: {}", e))?;
        let clean_mentions: Vec<String> = mentions
            .into_iter()
            .map(|m| crate::ui::normalize_jid(&m))
            .collect();
        let context_info = if clean_mentions.is_empty() {
            None
        } else {
            Some(
                ContextInfo {
                    mentioned_jid: clean_mentions,
                    ..Default::default()
                }
                .into(),
            )
        };

        let img = ImageMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(raw_len),
            mimetype: Some(mimetype.to_string()),
            caption,
            context_info,
            ..Default::default()
        };

        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            quoted: Some(Box::new(self.message.clone())),
            ..Default::default()
        };

        let sent = self
            .messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    image_message: Some(img.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map_err(|e| format!("send: {}", e))?;
        Ok(sent.key.id)
    }

    pub async fn send_video_bytes(
        &self,
        bytes: Vec<u8>,
        caption: Option<String>,
        mimetype: &str,
    ) -> Result<(), String> {
        let prep = prepare_video_for_whatsapp(
            bytes,
            &self.state.config.cache_dir,
            &self.state.ffmpeg_sem,
        )
        .await?;
        let raw_len = prep.bytes.len() as u64;
        let raw_bytes = prep.bytes;
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&raw_bytes);
            (h.finalize().to_vec(), raw_bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Video, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Video,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(45_000),
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

        let up = upload_res.map_err(|e| format!("upload: {}", e))?;
        let vid = VideoMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(raw_len),
            mimetype: Some(mimetype.to_string()),
            caption,
            ..Default::default()
        };

        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            quoted: Some(Box::new(self.message.clone())),
            ..Default::default()
        };

        self.messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    video_message: Some(vid.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("send: {}", e))
    }

    pub async fn send_gif_video_bytes(
        &self,
        bytes: Vec<u8>,
        caption: Option<String>,
        mentions: Vec<String>,
    ) -> Result<(), String> {
        let prep = prepare_video_for_whatsapp(
            bytes,
            &self.state.config.cache_dir,
            &self.state.ffmpeg_sem,
        )
        .await?;
        let raw_len = prep.bytes.len() as u64;
        let raw_bytes = prep.bytes;
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&raw_bytes);
            (h.finalize().to_vec(), raw_bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Video, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Video,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(25_000),
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

        let up = upload_res.map_err(|e| format!("upload: {}", e))?;
        let clean_mentions: Vec<String> = mentions
            .into_iter()
            .map(|m| crate::ui::normalize_jid(&m))
            .collect();
        let context_info = if clean_mentions.is_empty() {
            None
        } else {
            Some(
                ContextInfo {
                    mentioned_jid: clean_mentions,
                    ..Default::default()
                }
                .into(),
            )
        };
        let vid = VideoMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(raw_len),
            mimetype: Some("video/mp4".to_string()),
            caption,
            context_info,
            ..Default::default()
        };

        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            quoted: Some(Box::new(self.message.clone())),
            ..Default::default()
        };

        self.messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    video_message: Some(vid.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("send: {}", e))
    }

    pub async fn send_audio_bytes(
        &self,
        bytes: Vec<u8>,
        mimetype: &str,
        ptt: bool,
    ) -> Result<(), String> {
        let raw_len = bytes.len() as u64;
        if raw_len == 0 {
            return Err("Audio vacio.".to_string());
        }
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&bytes);
            (h.finalize().to_vec(), bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Audio, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Audio,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(30_000),
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

        let up = upload_res.map_err(|e| format!("upload: {}", e))?;
        let audio = AudioMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(raw_len),
            mimetype: Some(mimetype.to_string()),
            ptt: Some(ptt),
            ..Default::default()
        };

        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            quoted: Some(Box::new(self.message.clone())),
            ..Default::default()
        };

        self.messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    audio_message: Some(audio.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("send: {}", e))
    }

    pub async fn send_document_bytes(
        &self,
        bytes: Vec<u8>,
        file_name: &str,
        mimetype: &str,
        caption: Option<String>,
    ) -> Result<(), String> {
        let raw_len = bytes.len() as u64;
        if raw_len == 0 {
            return Err("Documento vacio.".to_string());
        }
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&bytes);
            (h.finalize().to_vec(), bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Document, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Document,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(45_000),
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

        let up = upload_res.map_err(|e| format!("upload: {}", e))?;
        let doc = DocumentMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(raw_len),
            mimetype: Some(mimetype.to_string()),
            file_name: Some(file_name.to_string()),
            caption,
            ..Default::default()
        };

        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            quoted: Some(Box::new(self.message.clone())),
            ..Default::default()
        };

        self.messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    document_message: Some(doc.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("send: {}", e))
    }

    pub async fn send_sticker_bytes(&self, bytes: Vec<u8>, animated: bool) -> Result<(), String> {
        let raw_len = bytes.len() as u64;
        if raw_len == 0 {
            return Err("Sticker vacio.".to_string());
        }
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&bytes);
            (h.finalize().to_vec(), bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Sticker, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Sticker,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(20_000),
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

        let up = upload_res.map_err(|e| format!("upload: {}", e))?;
        let sticker = StickerMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(raw_len),
            mimetype: Some("image/webp".to_string()),
            width: Some(512),
            height: Some(512),
            is_animated: Some(animated),
            ..Default::default()
        };

        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            quoted: Some(Box::new(self.message.clone())),
            ..Default::default()
        };

        self.messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    sticker_message: Some(sticker.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("send: {}", e))
    }

    pub fn mentioned_jids(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(ref proto) = self.message.message {
            let norm = normalize_message_content(proto);
            if let Some(ref ext) = norm.extended_text_message {
                let inner: &ExtendedTextMessage = Borrow::borrow(ext);
                if let Some(ref ci) = inner.context_info {
                    for m in &ci.mentioned_jid {
                        if !m.is_empty() {
                            let clean = crate::ui::normalize_jid(m);
                            if !out.contains(&clean) {
                                out.push(clean);
                            }
                        }
                    }
                    if out.is_empty() {
                        if let Some(ref part) = ci.participant {
                            if !part.is_empty() {
                                out.push(crate::ui::normalize_jid(part));
                            }
                        }
                    }
                }
            }
        }
        if out.is_empty() {
            for arg in &self.args {
                if let Some(stripped) = arg.strip_prefix('@') {
                    let digits: String = stripped.chars().filter(|c| c.is_ascii_digit()).collect();
                    if digits.len() >= 8 {
                        let jid = format!("{}@s.whatsapp.net", digits);
                        if !out.contains(&jid) {
                            out.push(jid);
                        }
                    }
                }
            }
        }
        out
    }

    pub fn target_jid(&self) -> String {
        if let Some(first) = self.mentioned_jids().into_iter().next() {
            return first;
        }
        self.sender.clone()
    }

    pub fn quoted_stanza_id(&self) -> Option<String> {
        let proto = self.message.message.as_ref()?;
        let norm = normalize_message_content(proto);
        let ext = norm.extended_text_message.as_ref()?;
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        inner.context_info.as_ref()?.stanza_id.clone()
    }

    pub fn quoted_text(&self) -> Option<String> {
        let proto = self.message.message.as_ref()?;
        let norm = normalize_message_content(proto);
        let ext = norm.extended_text_message.as_ref()?;
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        let q = inner.context_info.as_ref()?.quoted_message.as_ref()?;
        crate::handler::extract_message_text(q)
    }
}
