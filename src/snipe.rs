use baileys_rs::types::WaMessage;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const MAX_DELETED_PER_CHAT: usize = 3;

pub struct SnipeStore {
    max_cached: AtomicUsize,
    cache: Mutex<(HashMap<String, WaMessage>, VecDeque<String>)>,
    deleted: Mutex<HashMap<String, VecDeque<WaMessage>>>,
}

impl Default for SnipeStore {
    fn default() -> Self {
        Self::new(24)
    }
}

impl SnipeStore {
    pub fn new(cap: usize) -> Self {
        let cap = cap.max(8);
        Self {
            max_cached: AtomicUsize::new(cap),
            cache: Mutex::new((
                HashMap::with_capacity(cap.min(16)),
                VecDeque::with_capacity(cap.min(16)),
            )),
            deleted: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_capacity(&self, new_cap: usize) {
        let cap = new_cap.max(8);
        self.max_cached.store(cap, Ordering::Relaxed);
        if let Ok(mut guard) = self.cache.lock() {
            let (ref mut map, ref mut order) = *guard;
            while map.len() > cap {
                if let Some(old) = order.pop_front() {
                    map.remove(&old);
                } else {
                    map.clear();
                    break;
                }
            }
            map.shrink_to_fit();
            order.shrink_to_fit();
        }
    }

    pub fn record_message(&self, msg: &WaMessage) {
        let Some(id) = msg.key.id.clone() else {
            return;
        };
        let cap = self.max_cached.load(Ordering::Relaxed);
        if let Ok(mut guard) = self.cache.lock() {
            let (ref mut map, ref mut order) = *guard;
            if !map.contains_key(&id) {
                while map.len() >= cap {
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
                if guard.len() > 32 && !guard.contains_key(chat_jid) {
                    guard.clear();
                }
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
