use crate::config::ALYA_API;
use crate::context::CommandContext;
use serde_json::Value;
use std::time::Instant;

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
    let resp = req
        .send()
        .await
        .map_err(|e| format!("red: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let json: Value = resp
        .json()
        .await
        .map_err(|e| format!("json: {}", e))?;
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
            ctx.reply_text(format!("*{}* ({:.2}s)\n\n{}", title, secs, answer))
                .await;
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
