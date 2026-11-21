use baileys_rs::types::WaMessage;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

const MAX_CACHED_MESSAGES: usize = 256;
const MAX_DELETED_PER_CHAT: usize = 5;

pub struct SnipeStore {
    cache: Mutex<(HashMap<String, WaMessage>, VecDeque<String>)>,
    deleted: Mutex<HashMap<String, VecDeque<WaMessage>>>,
}

impl Default for SnipeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SnipeStore {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new((
                HashMap::with_capacity(64),
                VecDeque::with_capacity(64),
            )),
            deleted: Mutex::new(HashMap::new()),
        }
    }

    pub fn record_message(&self, msg: &WaMessage) {
        let Some(id) = msg.key.id.clone() else {
            return;
        };
        if let Ok(mut guard) = self.cache.lock() {
            let (ref mut map, ref mut order) = *guard;
            if !map.contains_key(&id) {
                while map.len() >= MAX_CACHED_MESSAGES {
                    if let Some(old) = order.pop_front() {
                        map.remove(&old);
                    } else {
                        break;
                    }
                }
                order.push_back(id.clone());
            }
            map.insert(id, msg.clone());
        }
    }

    pub fn record_revoked(&self, chat_jid: &str, msg_id: &str) {
        let cached = self
            .cache
            .lock()
            .ok()
            .and_then(|g| g.0.get(msg_id).cloned());
        if let Some(msg) = cached {
            if let Ok(mut guard) = self.deleted.lock() {
                let entry = guard
                    .entry(chat_jid.to_string())
                    .or_insert_with(|| VecDeque::with_capacity(MAX_DELETED_PER_CHAT));
                if entry.len() >= MAX_DELETED_PER_CHAT {
                    entry.pop_front();
                }
                entry.push_back(msg);
            }
        }
    }

    pub fn latest_deleted(&self, chat_jid: &str) -> Option<WaMessage> {
        self.deleted
            .lock()
            .ok()
            .and_then(|g| g.get(chat_jid).and_then(|q| q.back().cloned()))
    }
}
