use crate::context::CommandContext;

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "lid" | "jid" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            ctx.reply_with_mentions(
                format!(
                    "*Identificadores*\nChat: `{}`\nUsuario: `{}`\nLID DB: `{}`\nRol: `{}`",
                    ctx.from,
                    target,
                    u.lid.as_deref().unwrap_or("N/A"),
                    u.role
                ),
                vec![target],
            )
            .await;
            true
        }
        "get" => {
            let raw = ctx.args.first().map(|s| s.trim()).unwrap_or("");
            if raw.is_empty() {
                ctx.reply_text(format!("Uso: `{}get <url>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let url = if raw.starts_with("http://") || raw.starts_with("https://") {
                raw.to_string()
            } else {
                format!("https://{}", raw)
            };
            match ctx.state.http.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    let ct = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("unknown")
                        .to_string();
                    let body = resp.text().await.unwrap_or_default();
                    let preview: String = body.chars().take(1800).collect();
                    ctx.reply_text(format!(
                        "*HTTP {}* ({})\n\n```{}```",
                        status.as_u16(),
                        ct,
                        preview
                    ))
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error HTTP: {}", err)).await;
                }
            }
            true
        }
        "setrole" | "setuserrole" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let target = ctx.target_jid();
            let role = ctx
                .args
                .iter()
                .find(|a| !a.starts_with('@'))
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if target == ctx.sender
                || !matches!(role.as_str(), "user" | "premium" | "mod" | "coowner" | "owner")
            {
                ctx.reply_text(format!(
                    "Uso: `{}setrole @usuario <user|premium|mod|coowner|owner>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            ctx.state
                .db
                .update_user_field(&target, "role", Some(&role));
            ctx.reply_with_mentions(
                format!("Rol de @{} actualizado a *{}*.", jid_tag(&target), role),
                vec![target],
            )
            .await;
            true
        }
        "banuser" | "unbanuser" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde a otro usuario.").await;
                return true;
            }
            let banned = ctx.cmd_name == "banuser";
            ctx.state.db.set_user_banned(&target, banned);
            ctx.reply_with_mentions(
                format!(
                    "@{} ha sido *{}*.",
                    jid_tag(&target),
                    if banned { "baneado" } else { "desbaneado" }
                ),
                vec![target],
            )
            .await;
            true
        }
        _ => false,
    }
}
