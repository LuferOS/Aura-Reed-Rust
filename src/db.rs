use crate::config::is_default_owner;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Mutex;

const MAX_USER_CACHE: usize = 512;
const MAX_GROUP_CACHE: usize = 256;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub jid: String,
    pub lid: Option<String>,
    pub username: Option<String>,
    pub name: Option<String>,
    pub genre: String,
    pub birth_date: Option<String>,
    pub description: String,
    pub marriage_to: Option<String>,
    pub role: String,
    pub is_banned: bool,
    pub coins: i64,
    pub bank: i64,
    pub aura_points: i64,
    pub aura_level: i64,
    pub sticker_pack_name: Option<String>,
    pub sticker_pack_author: Option<String>,
    pub cmds_used_count: i64,
    pub user_lang: String,
    pub data_json: String,
}


#[derive(Debug, Clone)]
pub struct GachaDrawRecord {
    pub message_id: String,
    pub drawer_jid: String,
    pub character_id: String,
    pub image_id: i64,
    pub expires_at: u64,
    pub claimed_by: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GachaHaremEntry {
    pub character_id: String,
    pub image_id: i64,
    pub owner_jid: String,
    pub claimed_at: u64,
}

#[derive(Debug, Clone)]
pub struct GachaMarketEntry {
    pub id: i64,
    pub seller_jid: String,
    pub character_id: String,
    pub image_id: i64,
    pub price: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupWarnEntry {
    pub reason: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRecord {
    pub jid: String,
    pub group_name: Option<String>,
    pub antilink: bool,
    pub anti_calls: bool,
    pub anti_toxic: bool,
    pub anti_spam: bool,
    pub anti_status: bool,
    pub only_admin: bool,
    pub bot_online: bool,
    pub prefix: Option<String>,
    pub group_lang: Option<String>,
    pub user_warns_json: String,
    pub muted_users_json: String,
    pub data_json: String,
}

struct BoundedCache<V> {
    map: HashMap<String, V>,
    order: VecDeque<String>,
    cap: usize,
}

impl<V: Clone> BoundedCache<V> {
    fn new(cap: usize) -> Self {
        Self {
            map: HashMap::with_capacity(cap.min(64)),
            order: VecDeque::with_capacity(cap.min(64)),
            cap,
        }
    }

    fn get(&self, key: &str) -> Option<V> {
        self.map.get(key).cloned()
    }

    fn insert(&mut self, key: String, val: V) {
        if !self.map.contains_key(&key) {
            while self.map.len() >= self.cap {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                } else {
                    break;
                }
            }
            self.order.push_back(key.clone());
        }
        self.map.insert(key, val);
    }

    fn remove(&mut self, key: &str) {
        self.map.remove(key);
    }
}

pub struct AuraDb {
    conn: Mutex<Connection>,
    users_cache: Mutex<BoundedCache<UserRecord>>,
    groups_cache: Mutex<BoundedCache<GroupRecord>>,
}

const USER_SELECT_SQL: &str = "SELECT jid, lid, username, name, COALESCE(genre, 'Unknown'), birth_date, COALESCE(description, 'Sin descripcion'), marriage_to, COALESCE(role, 'user'), is_banned, coins, bank, aura_points, aura_level, stickerPackName, stickerPackAuthor, cmdsUsedCount, COALESCE(UserLang, 'es'), COALESCE(data, '{}') FROM users";

fn map_user_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserRecord> {
    Ok(UserRecord {
        jid: row.get(0)?,
        lid: row.get(1)?,
        username: row.get(2)?,
        name: row.get(3)?,
        genre: row.get(4)?,
        birth_date: row.get(5)?,
        description: row.get(6)?,
        marriage_to: row.get(7)?,
        role: row.get(8)?,
        is_banned: row.get::<_, i64>(9)? != 0,
        coins: row.get(10)?,
        bank: row.get(11)?,
        aura_points: row.get(12)?,
        aura_level: row.get(13)?,
        sticker_pack_name: row.get(14)?,
        sticker_pack_author: row.get(15)?,
        cmds_used_count: row.get(16)?,
        user_lang: row.get(17)?,
        data_json: row.get(18)?,
    })
}

impl AuraDb {
    pub fn open(data_dir: &Path) -> Result<Self, rusqlite::Error> {
        let _ = std::fs::create_dir_all(data_dir);
        let db_path = data_dir.join("Aura.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA busy_timeout = 10000;
             PRAGMA cache_size = -4000;
             PRAGMA temp_store = MEMORY;

             CREATE TABLE IF NOT EXISTS users (
                jid TEXT PRIMARY KEY,
                lid TEXT,
                username TEXT,
                phone_number TEXT,
                role TEXT DEFAULT 'user',
                is_banned INTEGER DEFAULT 0,
                self INTEGER DEFAULT 0,
                coins INTEGER DEFAULT 100000,
                bank INTEGER DEFAULT 10000,
                marriage_to TEXT DEFAULT NULL,
                name TEXT DEFAULT NULL,
                genre TEXT DEFAULT 'Unknown',
                birth_date TEXT DEFAULT NULL,
                description TEXT DEFAULT 'Sin descripcion',
                aura_points INTEGER DEFAULT 0,
                aura_level INTEGER DEFAULT 1,
                stickerPackAuthor TEXT DEFAULT NULL,
                stickerPackName TEXT DEFAULT NULL,
                cmdsUsedCount INTEGER DEFAULT 0,
                UserLang TEXT DEFAULT 'es',
                data TEXT DEFAULT '{}'
             );

             CREATE TABLE IF NOT EXISTS groups (
                jid TEXT PRIMARY KEY,
                group_id TEXT,
                group_name TEXT,
                antilink INTEGER DEFAULT 0,
                antiCalls INTEGER DEFAULT 0,
                antiToxic INTEGER DEFAULT 0,
                antiSpam INTEGER DEFAULT 0,
                antiStatus INTEGER DEFAULT 0,
                onlyAdmin INTEGER DEFAULT 0,
                botOnline INTEGER DEFAULT 1,
                prefix TEXT DEFAULT NULL,
                topCmdUsers TEXT DEFAULT '[]',
                userWarns TEXT DEFAULT '{}',
                Muted_Users TEXT DEFAULT '[]',
                catBlocked TEXT DEFAULT '[\"nsfw\"]',
                GroupLang TEXT DEFAULT NULL,
                data TEXT DEFAULT '{}'
             );

             CREATE TABLE IF NOT EXISTS bots (
                jid TEXT PRIMARY KEY,
                bot_id TEXT,
                bot_name TEXT,
                phone_number TEXT,
                lid TEXT,
                groups TEXT DEFAULT '[]',
                isMain INTEGER DEFAULT 0,
                status TEXT DEFAULT 'offline',
                modPrefix TEXT DEFAULT NULL,
                modSelf INTEGER DEFAULT 0,
                currency TEXT DEFAULT 'AuraCoins',
                currencySymbol TEXT DEFAULT 'C',
                currentBanner TEXT DEFAULT NULL,
                currentAudio TEXT DEFAULT NULL,
                bot_type TEXT DEFAULT 'Principal',
                data TEXT DEFAULT '{}'
             );

             CREATE TABLE IF NOT EXISTS gacha_draws (
                group_jid TEXT NOT NULL,
                message_id TEXT NOT NULL,
                drawer_jid TEXT NOT NULL,
                character_id TEXT NOT NULL,
                image_id INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                claimed_by TEXT DEFAULT NULL,
                PRIMARY KEY (group_jid, message_id)
             );

             CREATE TABLE IF NOT EXISTS gacha_harem (
                group_jid TEXT NOT NULL,
                character_id TEXT NOT NULL,
                image_id INTEGER NOT NULL,
                owner_jid TEXT NOT NULL,
                claimed_at INTEGER NOT NULL,
                PRIMARY KEY (group_jid, character_id, image_id)
             );

             CREATE TABLE IF NOT EXISTS gacha_votes (
                group_jid TEXT NOT NULL,
                character_id TEXT NOT NULL,
                voter_jid TEXT NOT NULL,
                voted_at INTEGER NOT NULL,
                PRIMARY KEY (group_jid, character_id, voter_jid)
             );

             CREATE TABLE IF NOT EXISTS gacha_market (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_jid TEXT NOT NULL,
                seller_jid TEXT NOT NULL,
                character_id TEXT NOT NULL,
                image_id INTEGER NOT NULL,
                price INTEGER NOT NULL,
                created_at INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS gacha_favorites (
                group_jid TEXT NOT NULL,
                user_jid TEXT NOT NULL,
                character_id TEXT NOT NULL,
                image_id INTEGER NOT NULL,
                PRIMARY KEY (group_jid, user_jid)
             );",
        )?;

        Ok(Self {
            conn: Mutex::new(conn),
            users_cache: Mutex::new(BoundedCache::new(MAX_USER_CACHE)),
            groups_cache: Mutex::new(BoundedCache::new(MAX_GROUP_CACHE)),
        })
    }

    pub fn ensure_user(&self, jid: &str, username: Option<&str>) -> UserRecord {
        if let Ok(cache) = self.users_cache.lock() {
            if let Some(u) = cache.get(jid) {
                return u;
            }
        }

        let default_role = if is_default_owner(jid) { "owner" } else { "user" };
        let sql = format!("{} WHERE jid = ?1", USER_SELECT_SQL);
        let record = if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO users (jid, username, role) VALUES (?1, ?2, ?3)",
                params![jid, username, default_role],
            );
            conn.query_row(&sql, params![jid], map_user_row)
                .optional()
                .ok()
                .flatten()
        } else {
            None
        };

        let user = record.unwrap_or_else(|| UserRecord {
            jid: jid.to_string(),
            lid: None,
            username: username.map(str::to_string),
            name: None,
            genre: "Unknown".to_string(),
            birth_date: None,
            description: "Sin descripcion".to_string(),
            marriage_to: None,
            role: default_role.to_string(),
            is_banned: false,
            coins: 100_000,
            bank: 10_000,
            aura_points: 0,
            aura_level: 1,
            sticker_pack_name: None,
            sticker_pack_author: None,
            cmds_used_count: 0,
            user_lang: "es".to_string(),
            data_json: "{}".to_string(),
        });

        if let Ok(mut cache) = self.users_cache.lock() {
            cache.insert(jid.to_string(), user.clone());
        }
        user
    }

    pub fn invalidate_user(&self, jid: &str) {
        if let Ok(mut cache) = self.users_cache.lock() {
            cache.remove(jid);
        }
    }

    pub fn invalidate_group(&self, jid: &str) {
        if let Ok(mut cache) = self.groups_cache.lock() {
            cache.remove(jid);
        }
    }

    pub fn increment_cmd_count(&self, jid: &str) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE users SET cmdsUsedCount = cmdsUsedCount + 1 WHERE jid = ?1",
                params![jid],
            );
        }
        self.invalidate_user(jid);
    }

    pub fn update_user_economy(&self, jid: &str, coins: i64, bank: i64, data_json: &str) {
        let _ = self.ensure_user(jid, None);
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE users SET coins = ?1, bank = ?2, data = ?3 WHERE jid = ?4",
                params![coins.max(0), bank.max(0), data_json, jid],
            );
        }
        self.invalidate_user(jid);
    }

    pub fn add_user_aura(&self, jid: &str, delta_xp: i64) {
        let u = self.ensure_user(jid, None);
        let new_points = (u.aura_points + delta_xp).max(0);
        let new_level = (new_points / 500) + 1;
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE users SET aura_points = ?1, aura_level = ?2 WHERE jid = ?3",
                params![new_points, new_level, jid],
            );
        }
        self.invalidate_user(jid);
    }

    pub fn update_user_field(&self, jid: &str, field: &str, value: Option<&str>) {
        let _ = self.ensure_user(jid, None);
        let sql = match field {
            "name" => "UPDATE users SET name = ?1 WHERE jid = ?2",
            "description" => "UPDATE users SET description = ?1 WHERE jid = ?2",
            "genre" => "UPDATE users SET genre = ?1 WHERE jid = ?2",
            "birth_date" => "UPDATE users SET birth_date = ?1 WHERE jid = ?2",
            "marriage_to" => "UPDATE users SET marriage_to = ?1 WHERE jid = ?2",
            "UserLang" => "UPDATE users SET UserLang = ?1 WHERE jid = ?2",
            "role" => "UPDATE users SET role = ?1 WHERE jid = ?2",
            _ => return,
        };
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(sql, params![value, jid]);
        }
        self.invalidate_user(jid);
    }

    pub fn set_user_banned(&self, jid: &str, banned: bool) {
        let _ = self.ensure_user(jid, None);
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE users SET is_banned = ?1 WHERE jid = ?2",
                params![if banned { 1i64 } else { 0i64 }, jid],
            );
        }
        self.invalidate_user(jid);
    }

    pub fn top_users_by_balance(&self, limit: usize) -> Vec<UserRecord> {
        let sql = format!("{} ORDER BY (coins + bank) DESC LIMIT ?1", USER_SELECT_SQL);
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(&sql) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![limit as i64], map_user_row) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    pub fn top_users_by_aura(&self, limit: usize, offset: usize) -> Vec<UserRecord> {
        let sql = format!(
            "{} ORDER BY aura_points DESC LIMIT ?1 OFFSET ?2",
            USER_SELECT_SQL
        );
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(&sql) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![limit as i64, offset as i64], map_user_row) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    pub fn ensure_group(&self, jid: &str) -> GroupRecord {
        if let Ok(cache) = self.groups_cache.lock() {
            if let Some(g) = cache.get(jid) {
                return g;
            }
        }

        let record = if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO groups (jid, group_id) VALUES (?1, ?1)",
                params![jid],
            );
            conn.query_row(
                "SELECT jid, group_name, antilink, antiCalls, antiToxic, antiSpam, antiStatus, onlyAdmin, botOnline, prefix, GroupLang, COALESCE(userWarns, '{}'), COALESCE(Muted_Users, '[]'), COALESCE(data, '{}') FROM groups WHERE jid = ?1",
                params![jid],
                |row| {
                    Ok(GroupRecord {
                        jid: row.get(0)?,
                        group_name: row.get(1)?,
                        antilink: row.get::<_, i64>(2)? != 0,
                        anti_calls: row.get::<_, i64>(3)? != 0,
                        anti_toxic: row.get::<_, i64>(4)? != 0,
                        anti_spam: row.get::<_, i64>(5)? != 0,
                        anti_status: row.get::<_, i64>(6)? != 0,
                        only_admin: row.get::<_, i64>(7)? != 0,
                        bot_online: row.get::<_, i64>(8)? != 0,
                        prefix: row.get(9)?,
                        group_lang: row.get(10)?,
                        user_warns_json: row.get(11)?,
                        muted_users_json: row.get(12)?,
                        data_json: row.get(13)?,
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
        } else {
            None
        };

        let group = record.unwrap_or_else(|| GroupRecord {
            jid: jid.to_string(),
            group_name: None,
            antilink: false,
            anti_calls: false,
            anti_toxic: false,
            anti_spam: false,
            anti_status: false,
            only_admin: false,
            bot_online: true,
            prefix: None,
            group_lang: None,
            user_warns_json: "{}".to_string(),
            muted_users_json: "[]".to_string(),
            data_json: "{}".to_string(),
        });

        if let Ok(mut cache) = self.groups_cache.lock() {
            cache.insert(jid.to_string(), group.clone());
        }
        group
    }

    pub fn set_group_flag(&self, jid: &str, column: &str, enabled: bool) {
        let _ = self.ensure_group(jid);
        let sql = match column {
            "antilink" => "UPDATE groups SET antilink = ?1 WHERE jid = ?2",
            "antiCalls" => "UPDATE groups SET antiCalls = ?1 WHERE jid = ?2",
            "antiToxic" => "UPDATE groups SET antiToxic = ?1 WHERE jid = ?2",
            "antiSpam" => "UPDATE groups SET antiSpam = ?1 WHERE jid = ?2",
            "antiStatus" => "UPDATE groups SET antiStatus = ?1 WHERE jid = ?2",
            "onlyAdmin" => "UPDATE groups SET onlyAdmin = ?1 WHERE jid = ?2",
            "botOnline" => "UPDATE groups SET botOnline = ?1 WHERE jid = ?2",
            _ => return,
        };
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(sql, params![if enabled { 1i64 } else { 0i64 }, jid]);
        }
        self.invalidate_group(jid);
    }

    pub fn set_group_prefix(&self, jid: &str, prefix: Option<&str>) {
        let _ = self.ensure_group(jid);
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET prefix = ?1 WHERE jid = ?2",
                params![prefix, jid],
            );
        }
        self.invalidate_group(jid);
    }

    pub fn set_group_lang(&self, jid: &str, lang: &str) {
        let _ = self.ensure_group(jid);
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET GroupLang = ?1 WHERE jid = ?2",
                params![lang, jid],
            );
        }
        self.invalidate_group(jid);
    }

    pub fn get_group_warn_limit(&self, jid: &str) -> usize {
        let g = self.ensure_group(jid);
        serde_json::from_str::<serde_json::Value>(&g.data_json)
            .ok()
            .and_then(|v| v.get("warnLimit").and_then(|x| x.as_u64()))
            .map(|n| n.max(1) as usize)
            .unwrap_or(3)
    }

    pub fn set_group_warn_limit(&self, jid: &str, limit: usize) {
        let g = self.ensure_group(jid);
        let mut obj = serde_json::from_str::<serde_json::Value>(&g.data_json)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        obj.insert("warnLimit".to_string(), serde_json::Value::from(limit as u64));
        let json = serde_json::to_string(&obj).unwrap_or_else(|_| "{}".to_string());
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET data = ?1 WHERE jid = ?2",
                params![json, jid],
            );
        }
        self.invalidate_group(jid);
    }

    pub fn get_group_warns(&self, group_jid: &str, target_jid: &str) -> Vec<GroupWarnEntry> {
        let g = self.ensure_group(group_jid);
        let map: HashMap<String, Vec<GroupWarnEntry>> =
            serde_json::from_str(&g.user_warns_json).unwrap_or_default();
        map.get(target_jid).cloned().unwrap_or_default()
    }

    pub fn add_group_warn(&self, group_jid: &str, target_jid: &str, entry: GroupWarnEntry) -> usize {
        let g = self.ensure_group(group_jid);
        let mut map: HashMap<String, Vec<GroupWarnEntry>> =
            serde_json::from_str(&g.user_warns_json).unwrap_or_default();
        let list = map.entry(target_jid.to_string()).or_default();
        list.push(entry);
        let count = list.len();
        let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET userWarns = ?1 WHERE jid = ?2",
                params![json, group_jid],
            );
        }
        self.invalidate_group(group_jid);
        count
    }

    pub fn remove_last_warn(&self, group_jid: &str, target_jid: &str) -> Option<usize> {
        let g = self.ensure_group(group_jid);
        let mut map: HashMap<String, Vec<GroupWarnEntry>> =
            serde_json::from_str(&g.user_warns_json).unwrap_or_default();
        let list = map.get_mut(target_jid)?;
        if list.is_empty() {
            return None;
        }
        list.pop();
        let remaining = list.len();
        let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET userWarns = ?1 WHERE jid = ?2",
                params![json, group_jid],
            );
        }
        self.invalidate_group(group_jid);
        Some(remaining)
    }

    pub fn is_user_muted(&self, group_jid: &str, target_jid: &str) -> bool {
        let g = self.ensure_group(group_jid);
        let list: Vec<String> = serde_json::from_str(&g.muted_users_json).unwrap_or_default();
        let bare = target_jid.split(':').next().unwrap_or(target_jid);
        list.iter().any(|u| u.split(':').next().unwrap_or(u) == bare)
    }

    pub fn set_user_muted(&self, group_jid: &str, target_jid: &str, muted: bool) {
        let g = self.ensure_group(group_jid);
        let mut list: Vec<String> = serde_json::from_str(&g.muted_users_json).unwrap_or_default();
        let bare = target_jid.split(':').next().unwrap_or(target_jid);
        list.retain(|u| u.split(':').next().unwrap_or(u) != bare);
        if muted {
            list.push(bare.to_string());
        }
        let json = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET Muted_Users = ?1 WHERE jid = ?2",
                params![json, group_jid],
            );
        }
        self.invalidate_group(group_jid);
    }


    pub fn get_group_data_map(&self, jid: &str) -> serde_json::Map<String, serde_json::Value> {
        let g = self.ensure_group(jid);
        serde_json::from_str::<serde_json::Value>(&g.data_json)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    pub fn set_group_data_field(&self, jid: &str, key: &str, val: serde_json::Value) {
        let mut map = self.get_group_data_map(jid);
        map.insert(key.to_string(), val);
        let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET data = ?1 WHERE jid = ?2",
                params![json, jid],
            );
        }
        self.invalidate_group(jid);
    }

    pub fn clear_group_warns(&self, group_jid: &str, target_jid: &str) {
        let g = self.ensure_group(group_jid);
        let mut map: HashMap<String, Vec<GroupWarnEntry>> =
            serde_json::from_str(&g.user_warns_json).unwrap_or_default();
        map.remove(target_jid);
        let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE groups SET userWarns = ?1 WHERE jid = ?2",
                params![json, group_jid],
            );
        }
        self.invalidate_group(group_jid);
    }


    pub fn set_sticker_meta(&self, jid: &str, pack: &str, author: &str) {
        let _ = self.ensure_user(jid, None);
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "UPDATE users SET stickerPackName = ?1, stickerPackAuthor = ?2 WHERE jid = ?3",
                params![pack, author, jid],
            );
        }
        self.invalidate_user(jid);
    }

    pub fn save_gacha_draw(
        &self,
        group_jid: &str,
        message_id: &str,
        drawer_jid: &str,
        character_id: &str,
        image_id: i64,
        now: u64,
        ttl_secs: u64,
    ) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "DELETE FROM gacha_draws WHERE group_jid = ?1 AND expires_at < ?2",
                params![group_jid, now as i64],
            );
            let _ = conn.execute(
                "INSERT OR REPLACE INTO gacha_draws (group_jid, message_id, drawer_jid, character_id, image_id, created_at, expires_at, claimed_by) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
                params![
                    group_jid,
                    message_id,
                    drawer_jid,
                    character_id,
                    image_id,
                    now as i64,
                    (now + ttl_secs) as i64
                ],
            );
        }
    }

    pub fn find_gacha_draw_by_msg(&self, group_jid: &str, message_id: &str) -> Option<GachaDrawRecord> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT message_id, drawer_jid, character_id, image_id, expires_at, claimed_by FROM gacha_draws WHERE group_jid = ?1 AND message_id = ?2",
            params![group_jid, message_id],
            |row| {
                Ok(GachaDrawRecord {
                    message_id: row.get(0)?,
                    drawer_jid: row.get(1)?,
                    character_id: row.get(2)?,
                    image_id: row.get(3)?,
                    expires_at: row.get::<_, i64>(4)? as u64,
                    claimed_by: row.get(5)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn find_pending_gacha_draw_by_char(
        &self,
        group_jid: &str,
        character_id: &str,
        image_id: Option<i64>,
        now: u64,
    ) -> Option<GachaDrawRecord> {
        let conn = self.conn.lock().ok()?;
        if let Some(img) = image_id {
            conn.query_row(
                "SELECT message_id, drawer_jid, character_id, image_id, expires_at, claimed_by FROM gacha_draws WHERE group_jid = ?1 AND character_id = ?2 AND image_id = ?3 AND claimed_by IS NULL AND expires_at >= ?4 ORDER BY created_at DESC LIMIT 1",
                params![group_jid, character_id, img, now as i64],
                |row| {
                    Ok(GachaDrawRecord {
                        message_id: row.get(0)?,
                        drawer_jid: row.get(1)?,
                        character_id: row.get(2)?,
                        image_id: row.get(3)?,
                        expires_at: row.get::<_, i64>(4)? as u64,
                        claimed_by: row.get(5)?,
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
        } else {
            conn.query_row(
                "SELECT message_id, drawer_jid, character_id, image_id, expires_at, claimed_by FROM gacha_draws WHERE group_jid = ?1 AND character_id = ?2 AND claimed_by IS NULL AND expires_at >= ?3 ORDER BY created_at DESC LIMIT 1",
                params![group_jid, character_id, now as i64],
                |row| {
                    Ok(GachaDrawRecord {
                        message_id: row.get(0)?,
                        drawer_jid: row.get(1)?,
                        character_id: row.get(2)?,
                        image_id: row.get(3)?,
                        expires_at: row.get::<_, i64>(4)? as u64,
                        claimed_by: row.get(5)?,
                    })
                },
            )
            .optional()
            .ok()
            .flatten()
        }
    }

    pub fn claim_gacha_card(
        &self,
        group_jid: &str,
        message_id: &str,
        character_id: &str,
        image_id: i64,
        owner_jid: &str,
        now: u64,
    ) -> bool {
        let Ok(conn) = self.conn.lock() else {
            return false;
        };
        let inserted = conn
            .execute(
                "INSERT OR IGNORE INTO gacha_harem (group_jid, character_id, image_id, owner_jid, claimed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![group_jid, character_id, image_id, owner_jid, now as i64],
            )
            .unwrap_or(0);
        if inserted > 0 {
            let _ = conn.execute(
                "UPDATE gacha_draws SET claimed_by = ?1 WHERE group_jid = ?2 AND message_id = ?3",
                params![owner_jid, group_jid, message_id],
            );
            true
        } else {
            false
        }
    }

    pub fn get_group_claimed_pairs(&self, group_jid: &str) -> Vec<(String, i64)> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT character_id, image_id FROM gacha_harem WHERE group_jid = ?1",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![group_jid], |r| Ok((r.get(0)?, r.get(1)?))) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    pub fn get_user_harem(&self, group_jid: &str, owner_jid: &str) -> Vec<GachaHaremEntry> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT character_id, image_id, owner_jid, claimed_at FROM gacha_harem WHERE group_jid = ?1 AND owner_jid = ?2 ORDER BY claimed_at DESC",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![group_jid, owner_jid], |r| {
            Ok(GachaHaremEntry {
                character_id: r.get(0)?,
                image_id: r.get(1)?,
                owner_jid: r.get(2)?,
                claimed_at: r.get::<_, i64>(3)? as u64,
            })
        }) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    pub fn get_group_harem_all(&self, group_jid: &str) -> Vec<GachaHaremEntry> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT character_id, image_id, owner_jid, claimed_at FROM gacha_harem WHERE group_jid = ?1",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![group_jid], |r| {
            Ok(GachaHaremEntry {
                character_id: r.get(0)?,
                image_id: r.get(1)?,
                owner_jid: r.get(2)?,
                claimed_at: r.get::<_, i64>(3)? as u64,
            })
        }) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    pub fn add_gacha_vote(&self, group_jid: &str, character_id: &str, voter_jid: &str, now: u64) -> Result<i64, u64> {
        let Ok(conn) = self.conn.lock() else {
            return Ok(0);
        };
        let prev: Option<i64> = conn
            .query_row(
                "SELECT voted_at FROM gacha_votes WHERE group_jid = ?1 AND character_id = ?2 AND voter_jid = ?3",
                params![group_jid, character_id, voter_jid],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten();
        if let Some(ts) = prev {
            let elapsed = now.saturating_sub(ts as u64);
            if elapsed < 43_200 {
                return Err(43_200 - elapsed);
            }
        }
        let _ = conn.execute(
            "INSERT OR REPLACE INTO gacha_votes (group_jid, character_id, voter_jid, voted_at) VALUES (?1, ?2, ?3, ?4)",
            params![group_jid, character_id, voter_jid, now as i64],
        );
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM gacha_votes WHERE group_jid = ?1 AND character_id = ?2",
                params![group_jid, character_id],
                |r| r.get(0),
            )
            .unwrap_or(1);
        Ok(cnt)
    }

    pub fn get_group_vote_counts(&self, group_jid: &str) -> HashMap<String, i64> {
        let mut map = HashMap::new();
        let Ok(conn) = self.conn.lock() else {
            return map;
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT character_id, COUNT(*) FROM gacha_votes WHERE group_jid = ?1 GROUP BY character_id",
        ) else {
            return map;
        };
        if let Ok(rows) = stmt.query_map(params![group_jid], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))) {
            for (k, v) in rows.filter_map(Result::ok) {
                map.insert(k, v);
            }
        }
        map
    }

    pub fn add_gacha_market(
        &self,
        group_jid: &str,
        seller_jid: &str,
        character_id: &str,
        image_id: i64,
        price: i64,
        now: u64,
    ) -> Option<i64> {
        let conn = self.conn.lock().ok()?;
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM gacha_market WHERE group_jid = ?1 AND character_id = ?2 AND image_id = ?3",
                params![group_jid, character_id, image_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if exists > 0 {
            return None;
        }
        conn.execute(
            "INSERT INTO gacha_market (group_jid, seller_jid, character_id, image_id, price, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![group_jid, seller_jid, character_id, image_id, price, now as i64],
        )
        .ok()?;
        Some(conn.last_insert_rowid())
    }

    pub fn list_gacha_market(&self, group_jid: &str) -> Vec<GachaMarketEntry> {
        let Ok(conn) = self.conn.lock() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, seller_jid, character_id, image_id, price FROM gacha_market WHERE group_jid = ?1 ORDER BY id ASC LIMIT 30",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map(params![group_jid], |r| {
            Ok(GachaMarketEntry {
                id: r.get(0)?,
                seller_jid: r.get(1)?,
                character_id: r.get(2)?,
                image_id: r.get(3)?,
                price: r.get(4)?,
            })
        }) else {
            return Vec::new();
        };
        rows.filter_map(Result::ok).collect()
    }

    pub fn get_gacha_market_item(&self, group_jid: &str, sale_id: i64) -> Option<GachaMarketEntry> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT id, seller_jid, character_id, image_id, price FROM gacha_market WHERE group_jid = ?1 AND id = ?2",
            params![group_jid, sale_id],
            |r| {
                Ok(GachaMarketEntry {
                    id: r.get(0)?,
                    seller_jid: r.get(1)?,
                    character_id: r.get(2)?,
                    image_id: r.get(3)?,
                    price: r.get(4)?,
                })
            },
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn remove_gacha_market(&self, group_jid: &str, sale_id: i64, seller_jid: Option<&str>) -> bool {
        let Ok(conn) = self.conn.lock() else {
            return false;
        };
        let affected = if let Some(seller) = seller_jid {
            conn.execute(
                "DELETE FROM gacha_market WHERE group_jid = ?1 AND id = ?2 AND seller_jid = ?3",
                params![group_jid, sale_id, seller],
            )
            .unwrap_or(0)
        } else {
            conn.execute(
                "DELETE FROM gacha_market WHERE group_jid = ?1 AND id = ?2",
                params![group_jid, sale_id],
            )
            .unwrap_or(0)
        };
        affected > 0
    }

    pub fn transfer_gacha_card(
        &self,
        group_jid: &str,
        character_id: &str,
        image_id: i64,
        from_jid: &str,
        to_jid: &str,
        now: u64,
    ) -> bool {
        let Ok(conn) = self.conn.lock() else {
            return false;
        };
        let affected = conn
            .execute(
                "UPDATE gacha_harem SET owner_jid = ?1, claimed_at = ?2 WHERE group_jid = ?3 AND character_id = ?4 AND image_id = ?5 AND owner_jid = ?6",
                params![to_jid, now as i64, group_jid, character_id, image_id, from_jid],
            )
            .unwrap_or(0);
        if affected > 0 {
            let _ = conn.execute(
                "DELETE FROM gacha_market WHERE group_jid = ?1 AND character_id = ?2 AND image_id = ?3",
                params![group_jid, character_id, image_id],
            );
            true
        } else {
            false
        }
    }

    pub fn delete_gacha_card(&self, group_jid: &str, character_id: &str, image_id: i64, owner_jid: &str) -> bool {
        let Ok(conn) = self.conn.lock() else {
            return false;
        };
        let affected = conn
            .execute(
                "DELETE FROM gacha_harem WHERE group_jid = ?1 AND character_id = ?2 AND image_id = ?3 AND owner_jid = ?4",
                params![group_jid, character_id, image_id, owner_jid],
            )
            .unwrap_or(0);
        if affected > 0 {
            let _ = conn.execute(
                "DELETE FROM gacha_market WHERE group_jid = ?1 AND character_id = ?2 AND image_id = ?3",
                params![group_jid, character_id, image_id],
            );
            true
        } else {
            false
        }
    }

    pub fn set_gacha_favorite(&self, group_jid: &str, user_jid: &str, character_id: &str, image_id: i64) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO gacha_favorites (group_jid, user_jid, character_id, image_id) VALUES (?1, ?2, ?3, ?4)",
                params![group_jid, user_jid, character_id, image_id],
            );
        }
    }

    pub fn get_gacha_favorite(&self, group_jid: &str, user_jid: &str) -> Option<(String, i64)> {
        let conn = self.conn.lock().ok()?;
        conn.query_row(
            "SELECT character_id, image_id FROM gacha_favorites WHERE group_jid = ?1 AND user_jid = ?2",
            params![group_jid, user_jid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .ok()
        .flatten()
    }

    pub fn checkpoint(&self) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);");
        }
    }
}
