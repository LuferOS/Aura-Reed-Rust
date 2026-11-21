use crate::config::AppConfig;
use crate::db::{AuraDb, UserRecord};
use crate::debug::DebugState;
use crate::snipe::SnipeStore;
use baileys_rs::defaults::MediaType;
use baileys_rs::proto::message::{AudioMessage, DocumentMessage, ExtendedTextMessage, ImageMessage, StickerMessage, VideoMessage};
use baileys_rs::proto::{ContextInfo, Message};
use baileys_rs::socket::{MessagesSocket, SubBotManager};
use baileys_rs::types::{GroupMetadata, GroupParticipant, WaMessage};
use baileys_rs::utils::{
    encrypted_stream, normalize_message_content, upload_media_to_server, AnyMessageContent,
    MessageGenerationOptions, WaMediaUpload,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use sha2::{Digest, Sha256};
use std::borrow::Borrow;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};

const MAX_GROUP_META_CACHE: usize = 64;
const GROUP_META_TTL_SECS: u64 = 300;

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
    pub db: Arc<AuraDb>,
    pub snipe: Arc<SnipeStore>,
    pub debug: Arc<DebugState>,
    pub subbots: Arc<SubBotManager>,
    pub http: reqwest::Client,
    pub cmd_semaphore: Arc<Semaphore>,
    pub creds_lock: Arc<Mutex<()>>,
    pub proposals: std::sync::Mutex<HashMap<String, PendingProposal>>,
    pub spam_tracker: std::sync::Mutex<HashMap<String, VecDeque<Instant>>>,
    pub group_meta_cache: std::sync::Mutex<HashMap<String, (GroupMetadata, Instant)>>,
    pub hangman_games: std::sync::Mutex<HashMap<String, HangmanGame>>,
}

impl BotState {
    pub fn build_http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(8)
            .http2_adaptive_window(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
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

pub struct CommandContext {
    pub state: Arc<BotState>,
    pub messages: Arc<MessagesSocket>,
    pub message: WaMessage,
    pub from: String,
    pub sender: String,
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
        let opts = MessageGenerationOptions {
            user_jid: self.bot_jid.clone(),
            message_id: None,
            timestamp: None,
            quoted: Some(Box::new(self.message.clone())),
            ephemeral_expiration: None,
            mentions,
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

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Image,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(10_000),
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

        let context_info = if mentions.is_empty() {
            None
        } else {
            Some(
                ContextInfo {
                    mentioned_jid: mentions,
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
            file_length: Some(enc_len),
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

        self.messages
            .send_message(
                &self.from,
                AnyMessageContent::Raw(Message {
                    image_message: Some(img.into()),
                    ..Default::default()
                }),
                opts,
            )
            .await
            .map(|_| ())
            .map_err(|e| format!("send: {}", e))
    }

    pub fn target_jid(&self) -> String {
        if let Some(ref proto) = self.message.message {
            let norm = normalize_message_content(proto);
            if let Some(ref ext) = norm.extended_text_message {
                let inner: &ExtendedTextMessage = Borrow::borrow(ext);
                if let Some(ref ci) = inner.context_info {
                    if let Some(first) = ci.mentioned_jid.first() {
                        if !first.is_empty() {
                            return first.clone();
                        }
                    }
                    if let Some(ref part) = ci.participant {
                        if !part.is_empty() {
                            return part.clone();
                        }
                    }
                }
            }
        }
        for arg in &self.args {
            if let Some(stripped) = arg.strip_prefix('@') {
                let digits: String = stripped.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 8 {
                    return format!("{}@s.whatsapp.net", digits);
                }
            }
        }
        self.sender.clone()
    }

    pub async fn send_image_bytes_returning_id(
        &self,
        bytes: Vec<u8>,
        caption: Option<String>,
        mentions: Vec<String>,
        mimetype: &str,
    ) -> Result<Option<String>, String> {
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

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Image,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(12_000),
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
        let context_info = if mentions.is_empty() {
            None
        } else {
            Some(
                ContextInfo {
                    mentioned_jid: mentions,
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
            file_length: Some(enc_len),
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
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&bytes);
            (h.finalize().to_vec(), bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Video, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
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
        let vid = VideoMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(enc_len),
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

    pub async fn send_audio_bytes(
        &self,
        bytes: Vec<u8>,
        mimetype: &str,
        ptt: bool,
    ) -> Result<(), String> {
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

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Audio,
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
        let audio = AudioMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(enc_len),
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

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Document,
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
        let doc = DocumentMessage {
            url: Some(up.media_url),
            direct_path: Some(up.direct_path),
            media_key: Some(enc.media_key),
            file_enc_sha256: Some(enc.file_enc_sha256),
            file_sha256: Some(sha_vec),
            file_length: Some(enc_len),
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

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Sticker,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(12_000),
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
            file_length: Some(enc_len),
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

    pub async fn send_gif_video_bytes(
        &self,
        bytes: Vec<u8>,
        caption: Option<String>,
        mentions: Vec<String>,
    ) -> Result<(), String> {
        let sha_res = tokio::task::spawn_blocking(move || {
            let mut h = Sha256::new();
            h.update(&bytes);
            (h.finalize().to_vec(), bytes)
        })
        .await
        .map_err(|e| format!("sha: {}", e))?;

        let (sha_vec, raw) = sha_res;
        let enc = encrypted_stream(&WaMediaUpload::Bytes(raw), MediaType::Video, false)
            .await
            .map_err(|e| format!("encrypt: {}", e))?;

        let enc_len = tokio::fs::metadata(&enc.enc_file_path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        let b64 = BASE64.encode(&enc.file_enc_sha256);
        let upload_res = upload_media_to_server(
            &enc.enc_file_path,
            MediaType::Video,
            &b64,
            &self.messages,
            &[],
            &HashMap::new(),
            Some(15_000),
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
        let context_info = if mentions.is_empty() {
            None
        } else {
            Some(
                ContextInfo {
                    mentioned_jid: mentions,
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
            file_length: Some(enc_len),
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
