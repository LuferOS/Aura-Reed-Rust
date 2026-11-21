use crate::config::is_default_owner;
use crate::context::{same_bare_jid, CommandContext};
use crate::db::GroupWarnEntry;
use baileys_rs::proto::message::ExtendedTextMessage;
use baileys_rs::types::{GroupSetting, ParticipantAction, WaMessageKey};
use baileys_rs::utils::{
    normalize_message_content, unix_timestamp_seconds, AnyMessageContent, MessageGenerationOptions,
};
use serde_json::Value;
use std::borrow::Borrow;

const SUPPORTED_LANGS: &[&str] = &[
    "en", "es", "br", "cn", "cz", "de", "fr", "gr", "hi", "hu", "id", "it", "jp", "kr", "nl",
    "ph", "pl", "ro", "ru", "sv", "th", "uk", "vn",
];

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

fn parse_toggle(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "activar" | "enable" => Some(true),
        "off" | "0" | "false" | "desactivar" | "disable" => Some(false),
        _ => None,
    }
}

async fn require_admin(ctx: &CommandContext, need_bot_admin: bool) -> bool {
    if !ctx.is_group {
        ctx.reply_text("Este comando solo funciona en grupos.")
            .await;
        return false;
    }
    let (is_admin, is_bot_admin) = ctx.admin_status().await;
    if !is_admin {
        ctx.reply_text("Solo administradores del grupo pueden usar este comando.")
            .await;
        return false;
    }
    if need_bot_admin && !is_bot_admin {
        ctx.reply_text("Necesito ser administrador del grupo para ejecutar esta accion.")
            .await;
        return false;
    }
    true
}

async fn handle_toggle(
    ctx: &CommandContext,
    col: &str,
    label: &str,
    current: bool,
    need_bot_admin: bool,
) -> bool {
    if !require_admin(ctx, need_bot_admin).await {
        return true;
    }
    let raw = ctx.args.first().map(String::as_str).unwrap_or("");
    let Some(enabled) = parse_toggle(raw) else {
        ctx.reply_text(format!(
            "*{}*\nEstado actual: *{}*\nUso: `{}{} on` o `{}{} off`",
            label,
            if current { "Activado" } else { "Desactivado" },
            ctx.used_prefix,
            ctx.cmd_name,
            ctx.used_prefix,
            ctx.cmd_name
        ))
        .await;
        return true;
    };
    ctx.state.db.set_group_flag(&ctx.from, col, enabled);
    ctx.reply_text(format!(
        "*{}* ahora esta *{}*.",
        label,
        if enabled { "activado" } else { "desactivado" }
    ))
    .await;
    true
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "antilink" | "antienlace" | "antigp" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "antilink", "Sistema AntiLink", g.antilink, true).await
        }
        "antitoxic" | "antitoxicos" | "antitx" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "antiToxic", "Sistema AntiToxic", g.anti_toxic, true).await
        }
        "antispam" | "antispammsg" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "antiSpam", "Sistema AntiSpam", g.anti_spam, true).await
        }
        "anticalls" | "antillamadas" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "antiCalls", "Sistema AntiCalls", g.anti_calls, false).await
        }
        "antistatus" | "antiestados" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "antiStatus", "Sistema AntiStatus", g.anti_status, true).await
        }
        "onlyadmin" | "adminonly" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "onlyAdmin", "Modo Solo Admins", g.only_admin, false).await
        }
        "nsfw" | "modonsfw" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let raw = ctx.args.first().map(String::as_str).unwrap_or("");
            let cur_enabled = !ctx.state.db.is_category_blocked(&ctx.from, "nsfw");
            let Some(enabled) = parse_toggle(raw) else {
                ctx.reply_text(format!(
                    "*Modo NSFW*\nEstado actual: *{}*\nUso: `{}nsfw on|off`",
                    if cur_enabled { "Activado" } else { "Desactivado" },
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            ctx.state
                .db
                .set_category_blocked(&ctx.from, "nsfw", !enabled);
            ctx.reply_text(format!(
                "Modo NSFW *{}* en este grupo.",
                if enabled { "activado" } else { "desactivado" }
            ))
            .await;
            true
        }
        "banchat" | "banearchat" | "mutechat" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            ctx.state
                .db
                .set_group_data_field(&ctx.from, "chatBanned", Value::Bool(true));
            ctx.reply_text("Chat silenciado para el bot. Usa `unbanchat` para reactivar.")
                .await;
            true
        }
        "unbanchat" | "desbanearchat" | "unmutechat" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            ctx.state
                .db
                .set_group_data_field(&ctx.from, "chatBanned", Value::Bool(false));
            ctx.reply_text("Bot reactivado en este grupo.").await;
            true
        }
        "tempmsg" | "tempmessage" | "ephemeral" | "ephemeralmsg" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let raw = ctx.args.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let secs: Option<u32> = match raw.as_str() {
                "off" | "0" | "disable" | "desactivar" => Some(0),
                "1d" | "24h" | "24" | "1" => Some(86_400),
                "7d" | "7" => Some(604_800),
                "90d" | "90" => Some(7_776_000),
                _ => None,
            };
            let Some(secs) = secs else {
                ctx.reply_text(format!(
                    "Uso: `{}tempmsg <24h|7d|90d|off>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            match ctx.messages.socket.group_toggle_ephemeral(&ctx.from, secs).await {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    let label = match secs {
                        0 => "desactivados",
                        86_400 => "24 horas",
                        604_800 => "7 dias",
                        _ => "90 dias",
                    };
                    ctx.reply_text(format!("Mensajes temporales configurados a: *{}*.", label))
                        .await;
                }
                Err(err) => ctx.reply_text(format!("Error configurando temporales: {}", err)).await,
            }
            true
        }
        "topactivos" | "activos" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let top = ctx.state.top_group_activity(&ctx.from, 10);
            if top.is_empty() {
                ctx.reply_text("Aun no hay actividad registrada desde el ultimo arranque.")
                    .await;
                return true;
            }
            let mentions: Vec<String> = top.iter().map(|(j, _)| j.clone()).collect();
            let lines: Vec<String> = top
                .iter()
                .enumerate()
                .map(|(i, (j, cnt))| format!("{}. @{} — *{}* msgs", i + 1, jid_tag(j), cnt))
                .collect();
            ctx.reply_with_mentions(
                format!(
                    "{}\n\n{}",
                    crate::ui::box_section("🔥", "TOP ACTIVOS", &lines),
                    crate::ui::footer_tag(crate::config::DEFAULT_BOT_NAME)
                ),
                mentions,
            )
            .await;
            true
        }
        "disable" | "enable" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let valid = ["ai", "download", "economy", "funy", "gacha", "games", "interaccion", "nsfw", "profile", "search", "stalk", "sticker", "utils"];
            let cat = ctx.args.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            if !valid.contains(&cat.as_str()) {
                let blocked = ctx.state.db.list_blocked_categories(&ctx.from);
                ctx.reply_text(format!(
                    "Uso: `{}{} <categoria>`\nCategorias: {}\nBloqueadas: {}",
                    ctx.used_prefix,
                    ctx.cmd_name,
                    valid.join(", "),
                    if blocked.is_empty() { "ninguna".to_string() } else { blocked.join(", ") }
                ))
                .await;
                return true;
            }
            let block = ctx.cmd_name == "disable";
            ctx.state.db.set_category_blocked(&ctx.from, &cat, block);
            ctx.reply_text(format!(
                "Categoria *{}* ahora esta *{}* en este grupo.",
                cat,
                if block { "desactivada" } else { "activada" }
            ))
            .await;
            true
        }
        "adminsystem" | "adminsys" | "configgrupo" | "groupconfig" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let g = ctx.state.db.ensure_group(&ctx.from);
            let map = ctx.state.db.get_group_data_map(&ctx.from);
            let blocked = ctx.state.db.list_blocked_categories(&ctx.from);
            let warn_lim = ctx.state.db.get_group_warn_limit(&ctx.from);
            let st = |b: bool| if b { "✅ ON" } else { "❌ OFF" };
            let body = format!(
                "{}\n\n{}",
                crate::ui::box_section(
                    "🛡️",
                    "CONFIGURACION DEL GRUPO",
                    &[
                        format!("> Bot activo › {}", st(g.bot_online)),
                        format!("> AntiLink › {} | AntiToxic › {}", st(g.antilink), st(g.anti_toxic)),
                        format!("> AntiSpam › {} | AntiCalls › {}", st(g.anti_spam), st(g.anti_calls)),
                        format!("> OnlyAdmin › {} | Self › {}", st(g.only_admin), st(map.get("self").and_then(Value::as_bool).unwrap_or(false))),
                        format!("> Welcome › {} | Bye › {}", st(map.get("welcome").and_then(Value::as_bool).unwrap_or(false)), st(map.get("bye").and_then(Value::as_bool).unwrap_or(false))),
                        format!("> Warn limit › {} | Prefijo › {}", warn_lim, g.prefix.as_deref().unwrap_or("global")),
                        format!("> Bloqueadas › {}", if blocked.is_empty() { "ninguna".to_string() } else { blocked.join(", ") }),
                    ],
                ),
                crate::ui::footer_tag(crate::config::DEFAULT_BOT_NAME)
            );
            ctx.reply_text(body).await;
            true
        }
        "testwelcome" | "testbienvenida" => {
            if !ctx.is_group {
                return true;
            }
            let map = ctx.state.db.get_group_data_map(&ctx.from);
            let tpl = map
                .get("welcomeMessage")
                .and_then(Value::as_str)
                .unwrap_or("Bienvenido/a @user al grupo!");
            let rendered = tpl.replace("@user", &format!("@{}", jid_tag(&ctx.sender)));
            ctx.reply_with_mentions(rendered, vec![ctx.sender.clone()])
                .await;
            true
        }
        "alerts" | "alertas" | "adminalerts" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let map = ctx.state.db.get_group_data_map(&ctx.from);
            let cur = map.get("alerts").and_then(Value::as_bool).unwrap_or(false);
            let raw = ctx.args.first().map(String::as_str).unwrap_or("");
            let Some(enabled) = parse_toggle(raw) else {
                ctx.reply_text(format!(
                    "*Alertas del Grupo*\nEstado: *{}*\nUso: `{}alerts on|off`",
                    if cur { "Activadas" } else { "Desactivadas" },
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            ctx.state
                .db
                .set_group_data_field(&ctx.from, "alerts", Value::Bool(enabled));
            ctx.reply_text(format!(
                "Alertas *{}*.",
                if enabled { "activadas" } else { "desactivadas" }
            ))
            .await;
            true
        }
        "restrictcmd" | "cmdrestrict" | "blockcmd" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let mut map = ctx.state.db.get_group_data_map(&ctx.from);
            let mut list: Vec<String> = map
                .get("restrictedCmds")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default();
            let target_cmd = ctx.args.first().map(|s| s.trim_start_matches('.').to_ascii_lowercase()).unwrap_or_default();
            if target_cmd.is_empty() {
                ctx.reply_text(format!(
                    "*Comandos restringidos:* {}\nUso: `{}restrictcmd <comando>` (alterna bloqueo)",
                    if list.is_empty() { "ninguno".to_string() } else { list.join(", ") },
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            let blocked = if let Some(pos) = list.iter().position(|x| x == &target_cmd) {
                list.remove(pos);
                false
            } else {
                list.push(target_cmd.clone());
                true
            };
            map.insert(
                "restrictedCmds".to_string(),
                Value::Array(list.into_iter().map(Value::String).collect()),
            );
            ctx.state.db.set_group_data_field(
                &ctx.from,
                "restrictedCmds",
                map.remove("restrictedCmds").unwrap_or(Value::Array(Vec::new())),
            );
            ctx.reply_text(format!(
                "Comando `{}` ahora esta *{}* en este grupo.",
                target_cmd,
                if blocked { "restringido" } else { "permitido" }
            ))
            .await;
            true
        }
        "bot" => {
            let g = ctx.state.db.ensure_group(&ctx.from);
            handle_toggle(ctx, "botOnline", "Bot en Grupo", g.bot_online, false).await
        }
        "welcome" | "bienvenida" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let map = ctx.state.db.get_group_data_map(&ctx.from);
            let cur = map.get("welcome").and_then(Value::as_bool).unwrap_or(false);
            let raw = ctx.args.first().map(String::as_str).unwrap_or("");
            let Some(enabled) = parse_toggle(raw) else {
                ctx.reply_text(format!(
                    "*Bienvenida*\nEstado actual: *{}*\nUso: `{}welcome on|off`",
                    if cur { "Activada" } else { "Desactivada" },
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            ctx.state
                .db
                .set_group_data_field(&ctx.from, "welcome", Value::Bool(enabled));
            ctx.reply_text(format!(
                "Bienvenida *{}*.",
                if enabled { "activada" } else { "desactivada" }
            ))
            .await;
            true
        }
        "bye" | "saybye" | "despedida" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let map = ctx.state.db.get_group_data_map(&ctx.from);
            let cur = map.get("bye").and_then(Value::as_bool).unwrap_or(false);
            let raw = ctx.args.first().map(String::as_str).unwrap_or("");
            let Some(enabled) = parse_toggle(raw) else {
                ctx.reply_text(format!(
                    "*Despedida*\nEstado actual: *{}*\nUso: `{}bye on|off`",
                    if cur { "Activada" } else { "Desactivada" },
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            ctx.state
                .db
                .set_group_data_field(&ctx.from, "bye", Value::Bool(enabled));
            ctx.reply_text(format!(
                "Despedida *{}*.",
                if enabled { "activada" } else { "desactivada" }
            ))
            .await;
            true
        }
        "setwelcome" | "setbienvenida" | "setbye" | "setdespedida" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let is_welcome = matches!(ctx.cmd_name.as_str(), "setwelcome" | "setbienvenida");
            let field = if is_welcome {
                "welcomeMessage"
            } else {
                "byeMessage"
            };
            let val = ctx.text.trim();
            if val.is_empty() {
                let cur = ctx
                    .state
                    .db
                    .get_group_data_map(&ctx.from)
                    .get(field)
                    .and_then(Value::as_str)
                    .unwrap_or("Mensaje por defecto")
                    .to_string();
                ctx.reply_text(format!(
                    "Mensaje actual: {}\nUso: `{}{} <texto|reset>`",
                    cur, ctx.used_prefix, ctx.cmd_name
                ))
                .await;
                return true;
            }
            if matches!(val.to_ascii_lowercase().as_str(), "reset" | "default") {
                ctx.state
                    .db
                    .set_group_data_field(&ctx.from, field, Value::Null);
                ctx.reply_text("Mensaje restablecido al valor por defecto.")
                    .await;
            } else {
                ctx.state
                    .db
                    .set_group_data_field(&ctx.from, field, Value::String(val.to_string()));
                ctx.reply_text("Mensaje personalizado guardado.").await;
            }
            true
        }
        "kick" | "ban" | "sacar" | "expulsar" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text(format!(
                    "Menciona o responde al usuario. Uso: `{}kick @usuario`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            if is_default_owner(&target)
                || ctx
                    .bot_jid
                    .as_deref()
                    .is_some_and(|b| same_bare_jid(b, &target))
            {
                ctx.reply_text("No puedo expulsar a ese usuario.").await;
                return true;
            }
            match ctx
                .messages
                .socket
                .group_participants_update(&ctx.from, &[target.clone()], ParticipantAction::Remove)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_with_mentions(
                        format!("@{} ha sido expulsado del grupo.", jid_tag(&target)),
                        vec![target],
                    )
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error al expulsar: {}", err)).await;
                }
            }
            true
        }
        "add" | "agregar" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let digits: String = ctx.text.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() < 8 {
                ctx.reply_text(format!("Uso: `{}add 573001234567`", ctx.used_prefix))
                    .await;
                return true;
            }
            let target = format!("{}@s.whatsapp.net", digits);
            match ctx
                .messages
                .socket
                .group_participants_update(&ctx.from, &[target.clone()], ParticipantAction::Add)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_with_mentions(
                        format!("Solicitud enviada para agregar a @{}.", digits),
                        vec![target],
                    )
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error al agregar: {}", err)).await;
                }
            }
            true
        }
        "promote" | "ascender" | "haceradmin" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde al usuario a ascender.")
                    .await;
                return true;
            }
            match ctx
                .messages
                .socket
                .group_participants_update(&ctx.from, &[target.clone()], ParticipantAction::Promote)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_with_mentions(
                        format!("@{} ahora es administrador.", jid_tag(&target)),
                        vec![target],
                    )
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error al promover: {}", err)).await;
                }
            }
            true
        }
        "demote" | "descender" | "quitaradmin" | "quitardadmin" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde al usuario a descender.")
                    .await;
                return true;
            }
            match ctx
                .messages
                .socket
                .group_participants_update(&ctx.from, &[target.clone()], ParticipantAction::Demote)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_with_mentions(
                        format!("@{} ya no es administrador.", jid_tag(&target)),
                        vec![target],
                    )
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error al descender: {}", err))
                        .await;
                }
            }
            true
        }
        "open" | "abrir" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            match ctx
                .messages
                .socket
                .group_setting_update(&ctx.from, GroupSetting::NotAnnouncement)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_text("*Grupo abierto*: todos los miembros pueden enviar mensajes.")
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error abriendo grupo: {}", err))
                        .await;
                }
            }
            true
        }
        "close" | "cerrar" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            match ctx
                .messages
                .socket
                .group_setting_update(&ctx.from, GroupSetting::Announcement)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_text("*Grupo cerrado*: solo administradores pueden enviar mensajes.")
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error cerrando grupo: {}", err))
                        .await;
                }
            }
            true
        }
        "link" | "linkgroup" | "grupo" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            match ctx.messages.socket.group_invite_code(&ctx.from).await {
                Ok(Some(code)) => {
                    ctx.reply_text(format!(
                        "*Enlace del grupo*\nhttps://chat.whatsapp.com/{}",
                        code
                    ))
                    .await;
                }
                Ok(None) => {
                    ctx.reply_text("No se pudo obtener el codigo de invitacion.")
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error obteniendo enlace: {}", err))
                        .await;
                }
            }
            true
        }
        "revoke" | "resetlink" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            match ctx.messages.socket.group_revoke_invite(&ctx.from).await {
                Ok(Some(code)) => {
                    ctx.reply_text(format!(
                        "*Enlace restablecido*\nhttps://chat.whatsapp.com/{}",
                        code
                    ))
                    .await;
                }
                Ok(None) => {
                    ctx.reply_text("Enlace anterior revocado.").await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error revocando enlace: {}", err))
                        .await;
                }
            }
            true
        }
        "leave" | "salir" | "groupleave" | "leavegroup" | "salirgrupo" | "abandonar" => {
            if !ctx.is_group {
                return true;
            }
            if !ctx.is_owner && !matches!(ctx.user.role.as_str(), "owner" | "coowner" | "mod") {
                ctx.reply_text("Comando reservado para owners y moderadores.")
                    .await;
                return true;
            }
            ctx.reply_text("Saliendo del grupo...").await;
            let _ = ctx.messages.socket.group_leave(&ctx.from).await;
            ctx.invalidate_group_cache();
            true
        }
        "setsubject" | "setnamegp" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let subject = ctx.text.trim();
            if subject.is_empty() {
                ctx.reply_text(format!("Uso: `{}setsubject <nuevo nombre>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match ctx
                .messages
                .socket
                .group_update_subject(&ctx.from, subject)
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_text(format!("Nombre del grupo cambiado a *{}*.", subject))
                        .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error cambiando nombre: {}", err))
                        .await;
                }
            }
            true
        }
        "setgroupdesc" | "setdescgp" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let desc = ctx.text.trim();
            if desc.is_empty() {
                ctx.reply_text(format!("Uso: `{}setdescgp <descripcion>`", ctx.used_prefix))
                    .await;
                return true;
            }
            match ctx
                .messages
                .socket
                .group_update_description(&ctx.from, Some(desc))
                .await
            {
                Ok(_) => {
                    ctx.invalidate_group_cache();
                    ctx.reply_text("Descripcion del grupo actualizada.").await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error actualizando descripcion: {}", err))
                        .await;
                }
            }
            true
        }
        "tagall" | "all" | "todos" | "invocar" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let Some(meta) = ctx.group_metadata(true).await else {
                ctx.reply_text("No pude obtener la lista de participantes.")
                    .await;
                return true;
            };
            let members: Vec<String> = meta.participants.iter().map(|p| p.id.clone()).collect();
            if members.is_empty() {
                ctx.reply_text("No se encontraron miembros.").await;
                return true;
            }
            let note = if !ctx.text.trim().is_empty() {
                ctx.text.trim().to_string()
            } else {
                ctx.quoted_text().unwrap_or_else(|| "Activense".to_string())
            };
            let tags = members
                .iter()
                .map(|j| format!("- @{}", jid_tag(j)))
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_with_mentions(
                format!(
                    "*Invocando a todos ({})*\nMensaje: {}\n\n{}",
                    members.len(),
                    note,
                    tags
                ),
                members,
            )
            .await;
            true
        }
        "tag" | "tg" | "hidetag" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let Some(meta) = ctx.group_metadata(false).await else {
                ctx.reply_text("No pude obtener los participantes del grupo.")
                    .await;
                return true;
            };
            let members: Vec<String> = meta.participants.iter().map(|p| p.id.clone()).collect();
            let body = if !ctx.text.trim().is_empty() {
                ctx.text.trim().to_string()
            } else {
                ctx.quoted_text().unwrap_or_else(|| "Atencion grupo".to_string())
            };
            ctx.reply_with_mentions(body, members).await;
            true
        }
        "infogroup" | "infogp" | "ingp" | "gp" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let Some(meta) = ctx.group_metadata(true).await else {
                ctx.reply_text("No se pudo obtener la informacion del grupo.")
                    .await;
                return true;
            };
            let g = ctx.state.db.ensure_group(&ctx.from);
            let map = ctx.state.db.get_group_data_map(&ctx.from);
            let welcome = map.get("welcome").and_then(Value::as_bool).unwrap_or(false);
            let bye = map.get("bye").and_then(Value::as_bool).unwrap_or(false);
            let admins_count = meta
                .participants
                .iter()
                .filter(|p| p.admin.is_some())
                .count();
            let desc = meta
                .desc
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("Sin descripcion");
            let flag = |b: bool| if b { "ON" } else { "OFF" };
            ctx.reply_text(format!(
                "*Informacion del Grupo*\n\
                Nombre: *{}*\n\
                ID: `{}`\n\
                Miembros: {} | Admins: {}\n\
                Cerrado: {} | Prefijo: {}\n\n\
                *Proteccion*\n\
                AntiLink: {} | AntiToxic: {} | AntiSpam: {}\n\
                AntiCalls: {} | OnlyAdmin: {} | Bot: {}\n\
                Welcome: {} | Bye: {}\n\n\
                *Descripcion*\n{}",
                meta.subject,
                meta.id,
                meta.participants.len(),
                admins_count,
                if meta.announce { "Si" } else { "No" },
                g.prefix.as_deref().unwrap_or("global"),
                flag(g.antilink),
                flag(g.anti_toxic),
                flag(g.anti_spam),
                flag(g.anti_calls),
                flag(g.only_admin),
                flag(g.bot_online),
                flag(welcome),
                flag(bye),
                desc.chars().take(400).collect::<String>()
            ))
            .await;
            true
        }
        "setprefix" | "prefix" | "delprefix" | "deleteprefix" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            if matches!(ctx.cmd_name.as_str(), "delprefix" | "deleteprefix") {
                ctx.state.db.set_group_prefix(&ctx.from, None);
                ctx.reply_text("Prefijo personalizado eliminado. Se usaran los prefijos globales.")
                    .await;
                return true;
            }
            let val = ctx.args.first().map(|s| s.trim()).unwrap_or("");
            if val.is_empty() {
                ctx.state.db.set_group_prefix(&ctx.from, None);
                ctx.reply_text("El grupo volvera a usar los prefijos globales.")
                    .await;
                return true;
            }
            if val.chars().count() > 2 || val.chars().any(char::is_whitespace) {
                ctx.reply_text("El prefijo debe ser de 1 o 2 caracteres sin espacios.")
                    .await;
                return true;
            }
            ctx.state.db.set_group_prefix(&ctx.from, Some(val));
            ctx.reply_text(format!("Prefijo del grupo actualizado a `{}`.", val))
                .await;
            true
        }
        "setgrouplang" | "grouplang" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let lang = ctx
                .args
                .first()
                .map(|s| s.trim().to_ascii_lowercase())
                .unwrap_or_default();
            if !SUPPORTED_LANGS.contains(&lang.as_str()) {
                ctx.reply_text(format!("Idiomas validos: {}", SUPPORTED_LANGS.join(", ")))
                    .await;
                return true;
            }
            ctx.state.db.set_group_lang(&ctx.from, &lang);
            ctx.reply_text(format!("Idioma del grupo actualizado a *{}*.", lang))
                .await;
            true
        }
        "setwarnlimit" | "warnlimit" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let limit = ctx
                .args
                .first()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            if limit < 1 || limit > 20 {
                ctx.reply_text(format!("Uso: `{}setwarnlimit <1-20>`", ctx.used_prefix))
                    .await;
                return true;
            }
            ctx.state.db.set_group_warn_limit(&ctx.from, limit);
            ctx.reply_text(format!("Limite de advertencias fijado en *{}*.", limit))
                .await;
            true
        }
        "warn" | "advertir" | "aviso" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text(format!(
                    "Menciona o responde a un usuario. Uso: `{}warn @user <motivo>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            let reason = ctx
                .args
                .iter()
                .filter(|a| !a.starts_with('@'))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let reason = if reason.trim().is_empty() {
                "Sin motivo especificado".to_string()
            } else {
                reason.trim().to_string()
            };
            let count = ctx.state.db.add_group_warn(
                &ctx.from,
                &target,
                GroupWarnEntry {
                    reason: reason.clone(),
                    date: unix_timestamp_seconds().to_string(),
                },
            );
            let limit = ctx.state.db.get_group_warn_limit(&ctx.from);
            if count >= limit && !is_default_owner(&target) {
                ctx.state.db.clear_group_warns(&ctx.from, &target);
                let _ = ctx
                    .messages
                    .socket
                    .group_participants_update(
                        &ctx.from,
                        &[target.clone()],
                        ParticipantAction::Remove,
                    )
                    .await;
                ctx.invalidate_group_cache();
                ctx.reply_with_mentions(
                    format!(
                        "*Limite alcanzado ({}/{})*\n@{} fue expulsado.\nUltimo motivo: {}",
                        count,
                        limit,
                        jid_tag(&target),
                        reason
                    ),
                    vec![target],
                )
                .await;
            } else {
                ctx.reply_with_mentions(
                    format!(
                        "*Advertencia ({}/{})*\nUsuario: @{}\nMotivo: {}",
                        count,
                        limit,
                        jid_tag(&target),
                        reason
                    ),
                    vec![target],
                )
                .await;
            }
            true
        }
        "delwarn" | "unwarn" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde al usuario.").await;
                return true;
            }
            match ctx.state.db.remove_last_warn(&ctx.from, &target) {
                Some(rem) => {
                    ctx.reply_with_mentions(
                        format!(
                            "Se retiro 1 advertencia a @{}. Restantes: *{}*.",
                            jid_tag(&target),
                            rem
                        ),
                        vec![target],
                    )
                    .await;
                }
                None => {
                    ctx.reply_with_mentions(
                        format!("@{} no tiene advertencias registradas.", jid_tag(&target)),
                        vec![target],
                    )
                    .await;
                }
            }
            true
        }
        "listwarn" | "warns" | "verwarn" | "viewwarn" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let target = ctx.target_jid();
            let list = ctx.state.db.get_group_warns(&ctx.from, &target);
            let limit = ctx.state.db.get_group_warn_limit(&ctx.from);
            if list.is_empty() {
                ctx.reply_with_mentions(
                    format!("@{} tiene 0/{} advertencias.", jid_tag(&target), limit),
                    vec![target],
                )
                .await;
                return true;
            }
            let lines = list
                .iter()
                .enumerate()
                .map(|(i, w)| format!("{}. {}", i + 1, w.reason))
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_with_mentions(
                format!(
                    "*Advertencias de @{} ({}/{})*\n{}",
                    jid_tag(&target),
                    list.len(),
                    limit,
                    lines
                ),
                vec![target],
            )
            .await;
            true
        }
        "mute" | "silenciar" => {
            if !require_admin(ctx, true).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde al usuario a silenciar.")
                    .await;
                return true;
            }
            ctx.state.db.set_user_muted(&ctx.from, &target, true);
            ctx.reply_with_mentions(
                format!(
                    "@{} fue *silenciado*. Sus mensajes en este grupo seran eliminados.",
                    jid_tag(&target)
                ),
                vec![target],
            )
            .await;
            true
        }
        "unmute" | "dessilenciar" => {
            if !require_admin(ctx, false).await {
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde al usuario.").await;
                return true;
            }
            ctx.state.db.set_user_muted(&ctx.from, &target, false);
            ctx.reply_with_mentions(
                format!("@{} ya puede volver a escribir.", jid_tag(&target)),
                vec![target],
            )
            .await;
            true
        }
        "del" | "delete" | "borrar" | "dmsg" | "forcedel" | "ownerdel" => {
            if ctx.is_group && !require_admin(ctx, true).await {
                return true;
            }
            let quoted_key = ctx.message.message.as_ref().and_then(|proto| {
                let norm = normalize_message_content(proto);
                let ext = norm.extended_text_message.as_ref()?;
                let inner: &ExtendedTextMessage = Borrow::borrow(ext);
                let ci = inner.context_info.as_ref()?;
                let stanza_id = ci.stanza_id.clone()?;
                Some(WaMessageKey {
                    remote_jid: Some(ctx.from.clone()),
                    from_me: Some(
                        ci.participant
                            .as_deref()
                            .zip(ctx.bot_jid.as_deref())
                            .is_some_and(|(p, b)| same_bare_jid(p, b)),
                    ),
                    id: Some(stanza_id),
                    participant: ci.participant.clone(),
                    ..Default::default()
                })
            });
            let Some(key) = quoted_key else {
                ctx.reply_text("Responde al mensaje que deseas eliminar.")
                    .await;
                return true;
            };
            let opts = MessageGenerationOptions {
                user_jid: ctx.bot_jid.clone(),
                ..Default::default()
            };
            if let Err(err) = ctx
                .messages
                .send_message(&ctx.from, AnyMessageContent::Delete(key), opts)
                .await
            {
                ctx.reply_text(format!("No pude eliminar el mensaje: {}", err))
                    .await;
            }
            true
        }
        _ => false,
    }
}
