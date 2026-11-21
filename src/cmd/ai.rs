use crate::config::{ALYA_API, DEFAULT_BOT_NAME};
use crate::context::CommandContext;
use crate::ui::{box_section, footer_tag};
use baileys_rs::utils::{AiRichBuilder, MessageGenerationOptions};
use serde_json::Value;
use std::time::Instant;

fn escape_html(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 16);
    for ch in raw.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

fn build_compact_ai_html(title: &str, prompt: &str, answer: &str, secs: f64) -> String {
    format!(
        "<style>*{{box-sizing:border-box;margin:0;padding:0}}body{{font-family:system-ui,sans-serif;background:#070312;color:#f5f3ff;padding:10px}}.c{{border:1px solid #4c1d95;border-radius:16px;background:#110724;overflow:hidden}}.h{{padding:10px 14px;border-bottom:1px solid #2e1065;display:flex;justify-content:space-between;font-weight:800;font-size:13px;color:#c084fc}}.p{{margin:10px 14px;padding:8px 10px;border-left:3px solid #ec4899;background:#0a0417;border-radius:8px;font-size:11.5px;color:#ddd6fe}}.r{{padding:0 14px 14px;font-size:12.5px;line-height:1.5;white-space:pre-wrap;word-break:break-word;color:#f8fafc}}</style><div class=\"c\"><div class=\"h\"><span>{t}</span><span>{s:.2}s</span></div><div class=\"p\">{p}</div><div class=\"r\">{a}</div></div>",
        t = escape_html(title),
        s = secs,
        p = escape_html(&prompt.chars().take(240).collect::<String>()),
        a = escape_html(&answer.chars().take(3000).collect::<String>())
    )
}

fn extract_ai_text(val: &Value) -> Option<String> {
    if let Some(s) = val.as_str() {
        let t = s.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Some(obj) = val.as_object() {
        for field in ["result", "text", "response", "message", "reply", "answer"] {
            if let Some(s) = obj.get(field).and_then(Value::as_str) {
                let t = s.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
        if let Some(data) = obj.get("data") {
            return extract_ai_text(data);
        }
    }
    None
}

async fn ask_alya(ctx: &CommandContext, endpoint: &str, prompt: &str) -> Result<String, String> {
    let base = ALYA_API.base_url.trim_end_matches('/');
    let url = format!("{}{}", base, endpoint);
    let mut req = ctx.state.http.get(&url).query(&[("text", prompt)]);
    if let Some(key) = ALYA_API.api_key {
        req = req.query(&[("key", key)]);
    }
    let resp = req.send().await.map_err(|e| format!("red: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let json: Value = resp.json().await.map_err(|e| format!("json: {}", e))?;
    extract_ai_text(&json).ok_or_else(|| "La API no devolvio una respuesta valida.".to_string())
}

pub async fn run(ctx: &CommandContext) -> bool {
    let (title, path) = match ctx.cmd_name.as_str() {
        "chatgpt" | "ia" | "gpt" => ("ChatGPT AI", "/ai/chatgpt"),
        "gemini" | "gia" | "gai" | "genai" => ("Gemini AI", "/ai/gemini"),
        "deepseek" | "dia" | "spt" => ("DeepSeek AI", "/ai/deepseek"),
        "grok" | "kia" | "gkai" => ("Grok AI", "/ai/grok"),
        "copilot" | "copi" | "cpt" => ("Copilot AI", "/ai/copilot"),
        _ => return false,
    };

    let prompt = ctx.text.trim();
    if prompt.is_empty() {
        ctx.reply_text(format!(
            "Escribe un mensaje para *{}*. Ejemplo: `{}{} hola`",
            title, ctx.used_prefix, ctx.cmd_name
        ))
        .await;
        return true;
    }

    let t0 = Instant::now();
    match ask_alya(ctx, path, prompt).await {
        Ok(answer) => {
            let secs = t0.elapsed().as_secs_f64();
            let mut sent_rich = false;
            if ctx.state.current_mode().is_perf() {
                let html = build_compact_ai_html(title, prompt, &answer, secs);
                let builder = AiRichBuilder::new()
                    .title(format!("{} // AURA RUST", title.to_uppercase()))
                    .footer(format!("{:.2}s | Modo PERF", secs))
                    .disclaimer("Aura Reed Rust AI")
                    .add_html_section(html, Vec::new());
                let opts = MessageGenerationOptions {
                    user_jid: ctx.bot_jid.clone(),
                    quoted: Some(Box::new(ctx.message.clone())),
                    ..Default::default()
                };
                if ctx.messages.send_ai_rich(&ctx.from, &builder, opts).await.is_ok() {
                    sent_rich = true;
                }
            }
            if !sent_rich {
                let body = format!(
                    "{}\n\n{}\n\n{}",
                    box_section("🤖", &title.to_uppercase(), &[format!("> Tiempo › {:.2}s", secs)]),
                    answer,
                    footer_tag(DEFAULT_BOT_NAME)
                );
                ctx.reply_text(body).await;
            }
            ctx.state
                .debug
                .record_timing("ai", format!("{} {:.0} ms", ctx.cmd_name, secs * 1000.0))
                .await;
        }
        Err(err) => {
            ctx.state
                .debug
                .record_error("ai", format!("{}: {}", ctx.cmd_name, err))
                .await;
            ctx.reply_text(format!("Error consultando *{}*: {}", title, err))
                .await;
        }
    }
    true
}
