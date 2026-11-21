use baileys_rs::types::ProfilePictureType;
use crate::context::{CommandContext, PendingProposal};
use std::time::{Duration, Instant};

const SUPPORTED_LANGS: &[&str] = &[
    "en", "es", "br", "cn", "cz", "de", "fr", "gr", "hi", "hu", "id", "it", "jp", "kr", "nl",
    "ph", "pl", "ro", "ru", "sv", "th", "uk", "vn",
];

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

fn valid_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 || parts[0].len() != 2 || parts[1].len() != 2 || parts[2].len() != 4 {
        return false;
    }
    let (Ok(d), Ok(m), Ok(y)) = (
        parts[0].parse::<u32>(),
        parts[1].parse::<u32>(),
        parts[2].parse::<u32>(),
    ) else {
        return false;
    };
    if !(1..=12).contains(&m) || !(1900..=2026).contains(&y) {
        return false;
    }
    let max_d = match m {
        2 => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    (1..=max_d).contains(&d)
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "profile" | "perfil" | "me" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            let display_name = u
                .name
                .as_deref()
                .or(u.username.as_deref())
                .unwrap_or("Sin nombre");
            let partner = u
                .marriage_to
                .as_deref()
                .map(|m| format!("@{}", jid_tag(m)))
                .unwrap_or_else(|| "Soltero/a".to_string());
            let mut mentions = vec![target.clone()];
            if let Some(ref spouse) = u.marriage_to {
                mentions.push(spouse.clone());
            }
            ctx.reply_with_mentions(
                format!(
                    "*Perfil de @{}*\n\
                    Nombre: {}\n\
                    Rol: {}\n\
                    Genero: {}\n\
                    Cumpleanos: {}\n\
                    Pareja: {}\n\
                    Bio: {}\n\
                    Idioma: {}\n\
                    Aura: {} XP (Nivel {})\n\
                    Bolsillo: {} | Banco: {}\n\
                    Comandos usados: {}",
                    jid_tag(&target),
                    display_name,
                    u.role,
                    u.genre,
                    u.birth_date.as_deref().unwrap_or("No definido"),
                    partner,
                    u.description,
                    u.user_lang,
                    u.aura_points,
                    u.aura_level,
                    u.coins,
                    u.bank,
                    u.cmds_used_count
                ),
                mentions,
            )
            .await;
            true
        }
        "lvl" | "level" | "nivel" | "rango" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            ctx.reply_with_mentions(
                format!(
                    "*Nivel Aura de @{}*\nAura XP: *{}*\nNivel: *{}*",
                    jid_tag(&target),
                    u.aura_points,
                    u.aura_level
                ),
                vec![target],
            )
            .await;
            true
        }
        "setmyname" | "minombre" => {
            let val = ctx.text.trim();
            if val.is_empty() {
                ctx.reply_text(format!("Uso: `{}setmyname <tu nombre>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let clipped: String = val.chars().take(40).collect();
            ctx.state
                .db
                .update_user_field(&ctx.sender, "name", Some(&clipped));
            ctx.reply_text(format!("Nombre actualizado a: *{}*", clipped))
                .await;
            true
        }
        "setdesc" | "descripcion" | "bio" => {
            let val = ctx.text.trim();
            if val.is_empty() {
                ctx.reply_text(format!("Uso: `{}setdesc <tu bio>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let clipped: String = val.chars().take(120).collect();
            ctx.state
                .db
                .update_user_field(&ctx.sender, "description", Some(&clipped));
            ctx.reply_text(format!("Biografia actualizada:\n{}", clipped))
                .await;
            true
        }
        "setgenre" | "genero" => {
            let val = ctx.text.trim().to_ascii_lowercase();
            let normalized = match val.as_str() {
                "hombre" | "masculino" => "Hombre",
                "mujer" | "femenino" => "Mujer",
                _ => {
                    ctx.reply_text(format!(
                        "Uso: `{}setgenre <hombre|mujer>`",
                        ctx.used_prefix
                    ))
                    .await;
                    return true;
                }
            };
            ctx.state
                .db
                .update_user_field(&ctx.sender, "genre", Some(normalized));
            ctx.reply_text(format!("Genero actualizado a: *{}*", normalized))
                .await;
            true
        }
        "setbirt" | "setbirth" | "cumple" => {
            let val = ctx.text.trim();
            if !valid_date(val) {
                ctx.reply_text(format!(
                    "Formato invalido. Usa `{}cumple DD/MM/AAAA`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            ctx.state
                .db
                .update_user_field(&ctx.sender, "birth_date", Some(val));
            ctx.reply_text(format!("Cumpleanos guardado: *{}*", val))
                .await;
            true
        }
        "userlang" | "setuserlang" | "mylang" | "idioma" => {
            let lang = ctx
                .args
                .first()
                .map(|s| s.trim().to_ascii_lowercase())
                .unwrap_or_default();
            if !SUPPORTED_LANGS.contains(&lang.as_str()) {
                ctx.reply_text(format!(
                    "Idiomas validos: {}",
                    SUPPORTED_LANGS.join(", ")
                ))
                .await;
                return true;
            }
            ctx.state
                .db
                .update_user_field(&ctx.sender, "UserLang", Some(&lang));
            ctx.reply_text(format!("Idioma actualizado a *{}*.", lang))
                .await;
            true
        }
        "marry" | "casarse" | "matrimonio" => {
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde a otra persona para proponer matrimonio.")
                    .await;
                return true;
            }
            let me = ctx.state.db.ensure_user(&ctx.sender, None);
            let other = ctx.state.db.ensure_user(&target, None);
            if me.marriage_to.is_some() || other.marriage_to.is_some() {
                ctx.reply_text("Uno de los dos ya tiene pareja registrada.")
                    .await;
                return true;
            }
            if let Ok(mut map) = ctx.state.proposals.lock() {
                map.insert(
                    target.clone(),
                    PendingProposal {
                        kind: "marry".to_string(),
                        from: ctx.sender.clone(),
                        to: target.clone(),
                        created_at: Instant::now(),
                    },
                );
            }
            ctx.reply_with_mentions(
                format!(
                    "@{} le propone matrimonio a @{}.\nResponde con `{}accept` o `{}reject`.",
                    jid_tag(&ctx.sender),
                    jid_tag(&target),
                    ctx.used_prefix,
                    ctx.used_prefix
                ),
                vec![ctx.sender.clone(), target],
            )
            .await;
            true
        }
        "divorce" | "divorcio" => {
            let me = ctx.state.db.ensure_user(&ctx.sender, None);
            let Some(spouse) = me.marriage_to else {
                ctx.reply_text("No tienes pareja registrada.").await;
                return true;
            };
            if let Ok(mut map) = ctx.state.proposals.lock() {
                map.insert(
                    spouse.clone(),
                    PendingProposal {
                        kind: "divorce".to_string(),
                        from: ctx.sender.clone(),
                        to: spouse.clone(),
                        created_at: Instant::now(),
                    },
                );
            }
            ctx.reply_with_mentions(
                format!(
                    "@{} solicita el divorcio a @{}.\nResponde con `{}accept` o `{}reject`.",
                    jid_tag(&ctx.sender),
                    jid_tag(&spouse),
                    ctx.used_prefix,
                    ctx.used_prefix
                ),
                vec![ctx.sender.clone(), spouse],
            )
            .await;
            true
        }
        "accept" | "aceptar" | "acepto" => {
            let prop = ctx.state.proposals.lock().ok().and_then(|mut m| {
                let p = m.remove(&ctx.sender)?;
                if p.created_at.elapsed() > Duration::from_secs(300) {
                    None
                } else {
                    Some(p)
                }
            });
            let Some(prop) = prop else {
                ctx.reply_text("No tienes ninguna propuesta pendiente.")
                    .await;
                return true;
            };
            if prop.kind == "marry" {
                let u1 = ctx.state.db.ensure_user(&prop.from, None);
                let u2 = ctx.state.db.ensure_user(&prop.to, None);
                if u1.marriage_to.is_some() || u2.marriage_to.is_some() {
                    ctx.reply_text("La propuesta ya no es valida.").await;
                    return true;
                }
                ctx.state
                    .db
                    .update_user_field(&prop.from, "marriage_to", Some(&prop.to));
                ctx.state
                    .db
                    .update_user_field(&prop.to, "marriage_to", Some(&prop.from));
                ctx.reply_with_mentions(
                    format!(
                        "@{} y @{} ahora estan casados!",
                        jid_tag(&prop.from),
                        jid_tag(&prop.to)
                    ),
                    vec![prop.from, prop.to],
                )
                .await;
            } else {
                ctx.state
                    .db
                    .update_user_field(&prop.from, "marriage_to", None);
                ctx.state
                    .db
                    .update_user_field(&prop.to, "marriage_to", None);
                ctx.reply_with_mentions(
                    format!(
                        "@{} y @{} han completado el divorcio.",
                        jid_tag(&prop.from),
                        jid_tag(&prop.to)
                    ),
                    vec![prop.from, prop.to],
                )
                .await;
            }
            true
        }
        "reject" | "rechazar" | "decline" => {
            let prop = ctx
                .state
                .proposals
                .lock()
                .ok()
                .and_then(|mut m| m.remove(&ctx.sender));
            let Some(prop) = prop else {
                ctx.reply_text("No tienes ninguna propuesta pendiente.")
                    .await;
                return true;
            };
            if prop.kind == "marry" {
                ctx.reply_text("Propuesta de matrimonio rechazada.").await;
            } else {
                ctx.reply_text("Solicitud de divorcio rechazada.").await;
            }
            true
        }
        "eboard" | "auratop" => {
            let page = ctx
                .args
                .first()
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|&p| p >= 1)
                .unwrap_or(1);
            let offset = (page - 1) * 10;
            let rows = ctx.state.db.top_users_by_aura(10, offset);
            if rows.is_empty() {
                ctx.reply_text("No hay registros en esa pagina.").await;
                return true;
            }
            let mut mentions = Vec::with_capacity(rows.len());
            let mut lines = Vec::with_capacity(rows.len());
            for (idx, u) in rows.iter().enumerate() {
                mentions.push(u.jid.clone());
                lines.push(format!(
                    "{}. @{} — *{} XP* (Nv.{})",
                    offset + idx + 1,
                    jid_tag(&u.jid),
                    u.aura_points,
                    u.aura_level
                ));
            }
            ctx.reply_with_mentions(
                format!("*Aura Top (Pagina {})*\n{}", page, lines.join("\n")),
                mentions,
            )
            .await;
            true
        }
        "pfp" | "foto" => {
            let target = ctx.target_jid();
            match ctx
                .messages
                .socket
                .profile_picture_url(&target, ProfilePictureType::Image, None, Some(5_000))
                .await
            {
                Ok(Some(url)) => {
                    match ctx.state.http.get(&url).send().await {
                        Ok(resp) if resp.status().is_success() => {
                            let mime = resp
                                .headers()
                                .get("content-type")
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or("image/jpeg")
                                .to_string();
                            match resp.bytes().await {
                                Ok(bytes) => {
                                    let caption = Some(format!("Foto de perfil de @{}", jid_tag(&target)));
                                    if let Err(err) = ctx
                                        .send_image_bytes(bytes.to_vec(), caption, vec![target.clone()], &mime)
                                        .await
                                    {
                                        ctx.reply_text(format!("Error enviando foto: {}", err)).await;
                                    }
                                }
                                Err(err) => {
                                    ctx.reply_text(format!("Error leyendo imagen: {}", err)).await;
                                }
                            }
                        }
                        _ => {
                            ctx.reply_with_mentions(
                                format!("Foto de @{}:\n{}", jid_tag(&target), url),
                                vec![target],
                            )
                            .await;
                        }
                    }
                }
                Ok(None) => {
                    ctx.reply_with_mentions(
                        format!("@{} no tiene foto de perfil publica.", jid_tag(&target)),
                        vec![target],
                    )
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("No pude obtener la foto: {}", err))
                        .await;
                }
            }
            true
        }
        _ => false,
    }
}
