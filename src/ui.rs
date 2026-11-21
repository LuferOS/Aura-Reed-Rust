use std::time::Duration;

pub fn fyt_bold(input: &str) -> String {
    let mut out = String::with_capacity(input.len() * 4);
    for ch in input.chars() {
        let mapped = match ch {
            'a'..='z' => char::from_u32(0x1D41A + (ch as u32 - 'a' as u32)).unwrap_or(ch),
            'A'..='Z' => char::from_u32(0x1D400 + (ch as u32 - 'A' as u32)).unwrap_or(ch),
            '0'..='9' => char::from_u32(0x1D7CE + (ch as u32 - '0' as u32)).unwrap_or(ch),
            _ => ch,
        };
        out.push(mapped);
    }
    out
}

pub fn normalize_jid(jid: &str) -> String {
    let trimmed = jid.trim();
    if let Some((user, domain)) = trimmed.split_once('@') {
        let bare_user = user.split(':').next().unwrap_or(user);
        format!("{}@{}", bare_user, domain)
    } else {
        trimmed.split(':').next().unwrap_or(trimmed).to_string()
    }
}

pub fn jid_tag(jid: &str) -> String {
    let norm = normalize_jid(jid);
    norm.split('@').next().unwrap_or(&norm).to_string()
}

pub fn format_number(n: i64) -> String {
    let neg = n < 0;
    let s = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 2);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    let rev: String = out.chars().rev().collect();
    if neg {
        format!("-{}", rev)
    } else {
        rev
    }
}

pub fn format_coins(n: i64) -> String {
    format!("₡{} AuraCoins", format_number(n))
}

pub fn format_duration(dur: Duration) -> String {
    let secs = dur.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

pub fn box_section(icon: &str, title: &str, lines: &[String]) -> String {
    let header = if icon.is_empty() {
        format!("╭〔 {} 〕━━⬣", fyt_bold(title))
    } else {
        format!("╭〔 {} {} 〕⬣", icon, fyt_bold(title))
    };
    let mut body = String::with_capacity(header.len() + lines.len() * 40 + 32);
    body.push_str(&header);
    for line in lines {
        body.push('\n');
        body.push_str("┃ ");
        body.push_str(line);
    }
    body.push_str("\n╰━━━━━━━━━━━━⬣");
    body
}

pub fn footer_tag(label: &str) -> String {
    format!("╰〔 ⚡ {} 〕⬣", fyt_bold(label))
}
