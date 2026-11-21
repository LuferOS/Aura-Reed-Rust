#![allow(dead_code)]
use std::path::PathBuf;

pub const DEFAULT_BOT_NAME: &str = "AURA REED";
pub const DEFAULT_BOT_VERSION: &str = "2.5.0";
pub const DEFAULT_BOT_AUTHOR: &str = "Powered By: LuferOS";
pub const CHANNEL_ID: &str = "120363410945728976@newsletter";

pub const DEFAULT_PREFIXES: &[&str] = &[".", "#", "/", "!", "-", "%", "$"];

pub const DEFAULT_OWNERS: &[(&str, Option<&str>)] = &[
    ("249907738841305@lid", Some("573132801341@s.whatsapp.net")),
    ("37031996583942@lid", Some("50672373785@s.whatsapp.net")),
    ("207563673047206@lid", Some("380970121849@s.whatsapp.net")),
    ("276772692222150@lid", Some("525538176300@s.whatsapp.net")),
    ("40763551416477@lid", None),
];

#[derive(Debug, Clone)]
pub struct ApiEndpoint {
    pub base_url: &'static str,
    pub api_key: Option<&'static str>,
}

pub const ALYA_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://api.alyacore.xyz/",
    api_key: Some("AURA-BOT-JERIELB"),
};

pub const LEMPI_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://api.lempi.lat/",
    api_key: Some("OBOE-AERETHIX"),
};

pub const DELIRIUS_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://api.delirius.online/",
    api_key: None,
};

pub const FAA_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://api-faa.my.id/",
    api_key: None,
};

pub const NEKOS_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://nekos.best/api/v2",
    api_key: None,
};

pub const EVOGB_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://api.evogb.org",
    api_key: Some("evogb-4lhGsnmK"),
};

pub const POKE_API: ApiEndpoint = ApiEndpoint {
    base_url: "https://pokeapi.co/api/v2",
    api_key: None,
};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub main_session_dir: PathBuf,
    pub sub_sessions_dir: PathBuf,
    pub prem_sessions_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub pair_phone: Option<String>,
    pub prefixes: Vec<String>,
    pub worker_threads: usize,
    pub debug_enabled: bool,
    pub start_subbots: bool,
}

fn detected_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .max(4)
}

impl AppConfig {
    pub fn from_env_and_args() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut main_session_dir = root.join("sessions").join("main");
        let sub_sessions_dir = root.join("sessions").join("subs");
        let prem_sessions_dir = root.join("sessions").join("prem");
        let data_dir = root.join("data");
        let cache_dir = std::env::var("GLOBAL_CUSTOM_TMP")
            .map(PathBuf::from)
            .unwrap_or_else(|_| root.join("cache"));

        let mut pair_phone = std::env::var("PAIR_NUMBER").ok();
        let mut prefixes: Vec<String> = DEFAULT_PREFIXES.iter().map(|s| (*s).to_string()).collect();
        let mut worker_threads = std::env::var("AURA_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or_else(detected_cores);
        let mut debug_enabled = std::env::var("AURA_DEBUG")
            .ok()
            .is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes" | "on"));
        let start_subbots = std::env::var("DISABLE_SUBBOTS")
            .map(|v| v != "true")
            .unwrap_or(true);

        if let Ok(env_prefix) = std::env::var("AURA_PREFIX") {
            let parsed: Vec<String> = env_prefix
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !parsed.is_empty() {
                prefixes = parsed;
            }
        }

        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--auth" if i + 1 < args.len() => {
                    main_session_dir = PathBuf::from(&args[i + 1]);
                    i += 2;
                }
                "--pair" if i + 1 < args.len() => {
                    pair_phone = Some(args[i + 1].clone());
                    i += 2;
                }
                "--threads" if i + 1 < args.len() => {
                    if let Ok(n) = args[i + 1].parse::<usize>() {
                        if n >= 1 {
                            worker_threads = n;
                        }
                    }
                    i += 2;
                }
                "--debug" => {
                    debug_enabled = true;
                    i += 1;
                }
                _ => i += 1,
            }
        }

        Self {
            main_session_dir,
            sub_sessions_dir,
            prem_sessions_dir,
            data_dir,
            cache_dir,
            pair_phone: pair_phone
                .map(|p| p.chars().filter(|c| c.is_ascii_digit()).collect::<String>())
                .filter(|p| !p.is_empty()),
            prefixes,
            worker_threads,
            debug_enabled,
            start_subbots,
        }
    }
}

pub fn is_default_owner(jid: &str) -> bool {
    let bare = jid.split(':').next().unwrap_or(jid);
    DEFAULT_OWNERS.iter().any(|(lid, owner_jid)| {
        bare == *lid || owner_jid.is_some_and(|oj| bare == oj)
    })
}
