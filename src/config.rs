#![allow(dead_code)]
use std::path::{Path, PathBuf};

pub const DEFAULT_BOT_NAME: &str = "AURA REED";
pub const DEFAULT_BOT_VERSION: &str = "3.1.1-beta";
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Eco,
    Perf,
}

impl RuntimeMode {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "eco" | "low" | "bajo" | "bajoconsumo" | "ahorro" | "lite" => Some(Self::Eco),
            "perf" | "performance" | "rendimiento" | "turbo" | "fast" | "alto" => Some(Self::Perf),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eco => "eco",
            Self::Perf => "perf",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Eco => "Bajo Consumo (ECO)",
            Self::Perf => "Rendimiento (PERF)",
        }
    }

    pub fn is_perf(self) -> bool {
        matches!(self, Self::Perf)
    }
}

pub fn load_saved_mode(data_dir: &Path) -> Option<RuntimeMode> {
    let raw = std::fs::read_to_string(data_dir.join("runtime_mode.txt")).ok()?;
    RuntimeMode::parse(&raw)
}

pub fn save_runtime_mode(data_dir: &Path, mode: RuntimeMode) {
    let _ = std::fs::create_dir_all(data_dir);
    let _ = std::fs::write(data_dir.join("runtime_mode.txt"), mode.as_str());
}

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
    pub mode: RuntimeMode,
}

fn default_threads_for_mode(mode: RuntimeMode) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    match mode {
        RuntimeMode::Eco => 1,
        RuntimeMode::Perf => cores.clamp(2, 4),
    }
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
        let mut mode = std::env::var("AURA_MODE")
            .ok()
            .and_then(|v| RuntimeMode::parse(&v))
            .or_else(|| load_saved_mode(&data_dir))
            .unwrap_or(RuntimeMode::Eco);

        let mut custom_threads = std::env::var("AURA_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n >= 1);

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
                "--mode" if i + 1 < args.len() => {
                    if let Some(m) = RuntimeMode::parse(&args[i + 1]) {
                        mode = m;
                    }
                    i += 2;
                }
                "--eco" => {
                    mode = RuntimeMode::Eco;
                    i += 1;
                }
                "--perf" => {
                    mode = RuntimeMode::Perf;
                    i += 1;
                }
                "--threads" if i + 1 < args.len() => {
                    if let Ok(n) = args[i + 1].parse::<usize>() {
                        if n >= 1 {
                            custom_threads = Some(n);
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

        let worker_threads = custom_threads.unwrap_or_else(|| default_threads_for_mode(mode));

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
            mode,
        }
    }
}

pub fn is_default_owner(jid: &str) -> bool {
    let bare = jid.split(':').next().unwrap_or(jid);
    DEFAULT_OWNERS.iter().any(|(lid, owner_jid)| {
        bare == *lid || owner_jid.is_some_and(|oj| bare == oj)
    })
}
