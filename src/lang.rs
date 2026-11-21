use std::path::Path;

pub const SUPPORTED_LANGS: &[(&str, &str)] = &[
    ("es", "Espanol"),
    ("en", "English"),
    ("br", "Portugues (Brasil)"),
    ("cn", "Chinese"),
    ("cz", "Cestina"),
    ("de", "Deutsch"),
    ("fr", "Francais"),
    ("gr", "Greek"),
    ("hi", "Hindi"),
    ("hu", "Magyar"),
    ("id", "Bahasa Indonesia"),
    ("it", "Italiano"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
    ("nl", "Nederlands"),
    ("ph", "Filipino"),
    ("pl", "Polski"),
    ("ro", "Romana"),
    ("ru", "Russian"),
    ("sv", "Svenska"),
    ("th", "Thai"),
    ("uk", "Ukrainian"),
    ("vn", "Tieng Viet"),
];

pub fn is_supported(code: &str) -> bool {
    SUPPORTED_LANGS.iter().any(|(c, _)| *c == code)
}

pub fn lang_name(code: &str) -> &'static str {
    SUPPORTED_LANGS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| *n)
        .unwrap_or("Espanol")
}

pub fn resolve_lang<'a>(user_lang: &'a str, group_lang: Option<&'a str>) -> &'a str {
    if is_supported(user_lang) {
        return user_lang;
    }
    if let Some(gl) = group_lang {
        if is_supported(gl) {
            return gl;
        }
    }
    "es"
}

pub fn tr(data_dir: &Path, lang: &str, key: &str) -> String {
    let custom_path = data_dir.join("langs").join(format!("{}.lang.json", lang));
    if let Ok(raw) = std::fs::read_to_string(&custom_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(found) = val.get(key).and_then(serde_json::Value::as_str) {
                return found.to_string();
            }
        }
    }
    match (lang, key) {
        ("en", "group_only") => "This command only works in groups.",
        ("br", "group_only") => "Este comando so funciona em grupos.",
        (_, "group_only") => "Este comando solo funciona en grupos.",
        ("en", "nsfw_disabled") => "NSFW content is disabled in this group. An admin can enable it with `nsfw on`.",
        ("br", "nsfw_disabled") => "O conteudo NSFW esta desativado neste grupo. Um admin pode ativar com `nsfw on`.",
        (_, "nsfw_disabled") => "El contenido NSFW esta desactivado en este grupo. Un admin puede activarlo con `nsfw on`.",
        ("en", "need_bot_admin") => "I need to be a group admin to run this action.",
        ("br", "need_bot_admin") => "Preciso ser administrador do grupo para executar esta acao.",
        (_, "need_bot_admin") => "Necesito ser administrador del grupo para ejecutar esta accion.",
        ("en", "lang_header") => "Language Settings",
        ("br", "lang_header") => "Configuracao de Idioma",
        (_, "lang_header") => "Configuracion de Idioma",
        _ => key,
    }
    .to_string()
}
