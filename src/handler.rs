use crate::cmd;
use crate::config::is_default_owner;
use crate::context::{BotState, CommandContext};
use crate::db::GroupRecord;
use baileys_rs::proto::message::{ExtendedTextMessage, ImageMessage, ProtocolMessage};
use baileys_rs::proto::Message;
use baileys_rs::socket::MessagesSocket;
use baileys_rs::types::WaMessage;
use baileys_rs::utils::{
    normalize_message_content, AnyMessageContent, MessageGenerationOptions,
};
use std::borrow::Borrow;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BAD_WORDS: &[&str] = &[
    "puto", "puta", "cabron", "cabrón", "pendejo", "pendeja", "idiota", "estupido", "estúpido",
    "imbecil", "imbécil", "mierda", "joder", "coño", "carajo", "verga", "chingar", "chinga",
    "pinche", "culero", "culera", "marica", "maricon", "maricón", "hijueputa", "hijo de puta",
    "malparido", "malparida", "gonorrea", "hdp", "hdspm", "ptm", "ctm",
];

pub fn extract_message_text(msg: &Message) -> Option<String> {
    let norm = normalize_message_content(msg);
    if let Some(ref text) = norm.conversation {
        if !text.is_empty() {
            return Some(text.clone());
        }
    }
    if let Some(ref ext) = norm.extended_text_message {
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        if let Some(ref text) = inner.text {
            if !text.is_empty() {
                return Some(text.clone());
            }
        }
    }
    if let Some(ref img) = norm.image_message {
        let inner: &ImageMessage = Borrow::borrow(img);
        if let Some(ref cap) = inner.caption {
            if !cap.is_empty() {
                return Some(cap.clone());
            }
        }
    }
    None
}

pub fn parse_command(raw: &str, prefixes: &[String]) -> Option<(String, String, Vec<String>, String)> {
    let trimmed = raw.trim();
    for p in prefixes {
        if let Some(rest) = trimmed.strip_prefix(p.as_str()) {
            let rest_trimmed = rest.trim_start();
            if rest_trimmed.is_empty() {
                return None;
            }
            let mut parts = rest_trimmed.splitn(2, char::is_whitespace);
            let cmd_name = parts.next().unwrap_or("").to_ascii_lowercase();
            let text = parts.next().unwrap_or("").trim().to_string();
            let args: Vec<String> = text.split_whitespace().map(str::to_string).collect();
            return Some((p.clone(), cmd_name, args, text));
        }
    }
    None
}

fn contains_wa_link(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    low.contains("chat.whatsapp.com/") || low.contains("whatsapp.com/channel/")
}

fn contains_bad_word(text: &str) -> bool {
    let low = text.to_ascii_lowercase();
    for word in BAD_WORDS {
        if word.contains(' ') {
            if low.contains(word) {
                return true;
            }
        } else if low
            .split(|c: char| !c.is_alphanumeric())
            .any(|tok| tok == *word)
        {
            return true;
        }
    }
    false
}

fn check_spam_burst(state: &BotState, group_jid: &str, sender: &str) -> bool {
    let key = format!("{}::{}", group_jid, sender);
    let now = Instant::now();
    let Ok(mut map) = state.spam_tracker.lock() else {
        return false;
    };
    if map.len() > 512 {
        map.clear();
    }
    let q = map.entry(key).or_insert_with(|| VecDeque::with_capacity(8));
    while let Some(front) = q.front() {
        if now.duration_since(*front) > Duration::from_secs(5) {
            q.pop_front();
        } else {
            break;
        }
    }
    q.push_back(now);
    q.len() >= 6
}

async fn delete_offending(
    messages: &Arc<MessagesSocket>,
    from: &str,
    bot_jid: &Option<String>,
    message: &WaMessage,
) {
    let opts = MessageGenerationOptions {
        user_jid: bot_jid.clone(),
        ..Default::default()
    };
    let _ = messages
        .send_message(from, AnyMessageContent::Delete(message.key.clone()), opts)
        .await;
}

async fn run_group_moderation(
    state: &Arc<BotState>,
    messages: &Arc<MessagesSocket>,
    message: &WaMessage,
    from: &str,
    sender: &str,
    bot_jid: &Option<String>,
    group: &GroupRecord,
    raw_text: Option<&str>,
) -> bool {
    if state.db.is_user_muted(from, sender) {
        delete_offending(messages, from, bot_jid, message).await;
        return true;
    }

    if let Some(txt) = raw_text {
        if group.antilink && contains_wa_link(txt) {
            delete_offending(messages, from, bot_jid, message).await;
            return true;
        }
        if group.anti_toxic && contains_bad_word(txt) {
            delete_offending(messages, from, bot_jid, message).await;
            return true;
        }
    }

    if group.anti_spam && check_spam_burst(state, from, sender) {
        delete_offending(messages, from, bot_jid, message).await;
        return true;
    }

    false
}

pub async fn handle_message(
    state: Arc<BotState>,
    messages: Arc<MessagesSocket>,
    message: WaMessage,
    is_offline: bool,
) {
    if is_offline {
        return;
    }

    let Some(ref proto_msg) = message.message else {
        return;
    };

    let norm_ev = normalize_message_content(proto_msg);
    if let Some(ref proto_ev) = norm_ev.protocol_message {
        let pm: &ProtocolMessage = Borrow::borrow(proto_ev);
        if pm.r#type == Some(0) {
            if let Some(ref k) = pm.key {
                if let (Some(chat), Some(id)) = (
                    k.remote_jid.as_deref().or(message.key.remote_jid.as_deref()),
                    k.id.as_deref(),
                ) {
                    state.snipe.record_revoked(chat, id);
                }
            }
            return;
        }
    }

    state.snipe.record_message(&message);

    let Some(from) = message.key.remote_jid.clone() else {
        return;
    };
    let raw_text_opt = extract_message_text(proto_msg);

    let creds = messages.socket.creds().await;
    let bot_jid = creds.me.as_ref().map(|m| crate::ui::normalize_jid(&m.id));
    let from_me = message.key.from_me == Some(true);
    let raw_sender = if from_me {
        bot_jid.clone().unwrap_or_else(|| from.clone())
    } else {
        message
            .key
            .participant
            .clone()
            .unwrap_or_else(|| from.clone())
    };
    let sender = crate::ui::normalize_jid(&raw_sender);
    let push_name = message
        .push_name
        .clone()
        .or_else(|| if from_me { creds.me.as_ref().and_then(|m| m.name.clone()) } else { None });
    if let Some(ref pn) = push_name {
        state.db.sync_username(&sender, pn);
    }
    let user = state.db.ensure_user(&sender, push_name.as_deref());
    let is_owner = user.role == "owner" || is_default_owner(&sender);
    let is_group = from.ends_with("@g.us");

    let prefixes = if is_group {
        state.record_group_activity(&from, &sender);
        let g = state.db.ensure_group(&from);
        if !from_me
            && !is_owner
            && run_group_moderation(
                &state,
                &messages,
                &message,
                &from,
                &sender,
                &bot_jid,
                &g,
                raw_text_opt.as_deref(),
            )
            .await
        {
            return;
        }
        if !g.bot_online && !is_owner {
            return;
        }
        if let Some(gp) = g.prefix {
            vec![gp]
        } else {
            state.config.prefixes.clone()
        }
    } else {
        state.config.prefixes.clone()
    };

    let Some(raw_text) = raw_text_opt else {
        return;
    };
    let Some((used_prefix, cmd_name, args, text)) = parse_command(&raw_text, &prefixes) else {
        if !from_me {
            let _ = cmd::games::try_hangman_direct_guess(
                &state,
                &messages,
                &message,
                &from,
                &sender,
                &bot_jid,
                &raw_text,
            )
            .await;
        }
        return;
    };

    if user.is_banned && !is_owner {
        return;
    }

    if is_group {
        let gmap = state.db.get_group_data_map(&from);
        let chat_banned = gmap
            .get("chatBanned")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if chat_banned
            && !is_owner
            && !matches!(cmd_name.as_str(), "unbanchat" | "desbanearchat" | "unmutechat")
        {
            return;
        }
        let self_mode = gmap
            .get("self")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if self_mode && !from_me && !is_owner {
            return;
        }
    }

    if is_group && !is_owner && !matches!(user.role.as_str(), "owner" | "coowner" | "mod") {
        let g = state.db.ensure_group(&from);
        if g.only_admin {
            let allowed = state
                .get_group_metadata(&messages, &from, false)
                .await
                .is_some_and(|meta| crate::context::is_participant_admin(&meta, &sender));
            if !allowed {
                return;
            }
        }
    }

    let received_at = Instant::now();
    let ctx = CommandContext {
        state: Arc::clone(&state),
        messages,
        message,
        from,
        sender: sender.clone(),
        push_name,
        bot_jid,
        is_group,
        is_owner,
        user,
        used_prefix,
        cmd_name: cmd_name.clone(),
        args,
        text,
        received_at,
    };

    let cat_open = |cat: &str| !is_group || !state.db.is_category_blocked(&ctx.from, cat);
    let handled = if cmd::system::run(&ctx).await {
        true
    } else if cmd::socket::run(&ctx).await {
        true
    } else if cmd::group::run(&ctx).await {
        true
    } else if cat_open("sticker") && cmd::sticker::run(&ctx).await {
        true
    } else if cat_open("ai") && cmd::ai::run(&ctx).await {
        true
    } else if cat_open("economy") && cmd::economy::run(&ctx).await {
        true
    } else if cat_open("profile") && cmd::profile::run(&ctx).await {
        true
    } else if cat_open("search") && cmd::search::run(&ctx).await {
        true
    } else if cat_open("download") && cmd::download::run(&ctx).await {
        true
    } else if cat_open("gacha") && cmd::gacha::run(&ctx).await {
        true
    } else if cat_open("games") && cmd::games::run(&ctx).await {
        true
    } else if cat_open("stalk") && cmd::stalk::run(&ctx).await {
        true
    } else if cat_open("funy") && cmd::funy::run(&ctx).await {
        true
    } else if cat_open("utils") && cmd::utils::run(&ctx).await {
        true
    } else if cat_open("interaccion") {
        cmd::interaccion::run(&ctx).await
    } else {
        false
    };

    if handled {
        state
            .debug
            .total_commands
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        state.db.increment_cmd_count(&sender);
        state.debug.log(
            "cmd",
            format!(
                "{} ok en {:.2} ms",
                cmd_name,
                received_at.elapsed().as_secs_f64() * 1000.0
            ),
        );
    }
}
