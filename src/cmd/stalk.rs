use crate::context::CommandContext;

fn extract_invite_code(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if let Some(pos) = trimmed.find("chat.whatsapp.com/") {
        let rest = &trimmed[pos + "chat.whatsapp.com/".len()..];
        let code: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if code.len() >= 18 {
            return Some(code);
        }
    }
    None
}

pub async fn run(ctx: &CommandContext) -> bool {
    if !matches!(ctx.cmd_name.as_str(), "inspect" | "inspeccionar") {
        return false;
    }
    let Some(code) = extract_invite_code(&ctx.text) else {
        ctx.reply_text(format!(
            "Envia un enlace valido de grupo. Uso: `{}inspect https://chat.whatsapp.com/...`",
            ctx.used_prefix
        ))
        .await;
        return true;
    };
    match ctx.messages.socket.group_get_invite_info(&code).await {
        Ok(meta) => {
            let desc = meta
                .desc
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("Sin descripcion");
            ctx.reply_text(format!(
                "*Inspeccion de Grupo*\n\
                Nombre: *{}*\n\
                ID: `{}`\n\
                Miembros: *{}*\n\
                Owner: {}\n\
                Solo admins escriben: {}\n\
                Solo admins editan: {}\n\
                Comunidad: {}\n\n\
                *Descripcion*\n{}",
                meta.subject,
                meta.id,
                meta.size.max(meta.participants.len()),
                meta.owner.as_deref().unwrap_or("Desconocido"),
                if meta.announce { "Si" } else { "No" },
                if meta.restrict { "Si" } else { "No" },
                if meta.is_community { "Si" } else { "No" },
                desc.chars().take(400).collect::<String>()
            ))
            .await;
        }
        Err(err) => {
            ctx.reply_text(format!("No pude inspeccionar el enlace: {}", err))
                .await;
        }
    }
    true
}
