use baileys_rs::types::ProfilePictureType;
use crate::config::DEFAULT_BOT_NAME;
use crate::context::{CommandContext, PendingProposal};
use crate::ui::{box_section, footer_tag, format_coins, format_number, fyt_bold, jid_tag};
use std::time::{Duration, Instant};

const SUPPORTED_LANGS: &[&str] = &[
    "en", "es", "br", "cn", "cz", "de", "fr", "gr", "hi", "hu", "id", "it", "jp", "kr", "nl",
    "ph", "pl", "ro", "ru", "sv", "th", "uk", "vn",
];

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

fn calc_age(birth: &str) -> Option<u32> {
    let parts: Vec<&str> = birth.split('/').collect();
    if parts.len() != 3 {
        return None;
    }
    let y: u32 = parts[2].parse().ok()?;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let cur_year = 1970 + (now_secs / 31_556_952) as u32;
    cur_year.checked_sub(y)
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "profile" | "perfil" | "me" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            let tag = jid_tag(&target);
            let display_name = u
                .name
                .as_deref()
                .or(u.username.as_deref())
                .or_else(|| if target == ctx.sender { ctx.push_name.as_deref() } else { None })
                .unwrap_or("Sin nombre");
            let gender = if u.genre == "Unknown" {
                "No definido"
            } else {
                u.genre.as_str()
            };
            let birth = u.birth_date.as_deref().unwrap_or("No definido");
            let age_str = u
                .birth_date
                .as_deref()
                .and_then(calc_age)
                .map(|a| format!("{} anos", a))
                .unwrap_or_else(|| "No definida".to_string());
            let partner = u
                .marriage_to
                .as_deref()
                .map(|m| format!("@{}", jid_tag(m)))
                .unwrap_or_else(|| "Soltero/a".to_string());
            let mut mentions = vec![target.clone()];
            if let Some(ref spouse) = u.marriage_to {
                mentions.push(spouse.clone());
            }

            let sec_head = box_section(
                "👤",
                "PERFIL",
                &[format!("📋 {} @{}", fyt_bold("SOBRE"), tag)],
            );
            let sec_bio = box_section("", "BIOGRAFIA", &[u.description.clone()]);
            let sec_basic = box_section(
                "",
                "INFO BASICA",
                &[
                    format!("👤 {} › {}", fyt_bold("Nombre"), display_name),
                    format!("🆔 {} › WB{}", fyt_bold("ID"), tag),
                    format!("⚧️ {} › {}", fyt_bold("Genero"), gender),
                    format!("🎂 {} › {}", fyt_bold("Cumpleanos"), birth),
                    format!("🎈 {} › {}", fyt_bold("Edad"), age_str),
                    format!("💍 {} › {}", fyt_bold("Pareja"), partner),
                ],
            );
            let sec_rank = box_section(
                "",
                "RANGO",
                &[
                    format!("📊 {} › {}", fyt_bold("Nivel Aura"), u.aura_level),
                    format!("✨ {} › {}", fyt_bold("Puntos Aura"), format_number(u.aura_points)),
                    format!("💵 {} › {}", fyt_bold("Cartera"), format_coins(u.coins)),
                    format!("🏦 {} › {}", fyt_bold("Banco"), format_coins(u.bank)),
                ],
            );

            let caption = format!(
                "{}\n\n{}\n\n{}\n\n{}\n\n{}",
                sec_head,
                sec_bio,
                sec_basic,
                sec_rank,
                footer_tag(DEFAULT_BOT_NAME)
            );

            let sent_with_photo = match ctx
                .messages
                .socket
                .profile_picture_url(&target, ProfilePictureType::Image, None, Some(3_500))
                .await
            {
                Ok(Some(url)) => match ctx.state.http.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let mime = resp
                            .headers()
                            .get("content-type")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("image/jpeg")
                            .to_string();
                        match resp.bytes().await {
                            Ok(bytes) => ctx
                                .send_image_bytes(
                                    bytes.to_vec(),
                                    Some(caption.clone()),
                                    mentions.clone(),
                                    &mime,
                                )
                                .await
                                .is_ok(),
                            Err(_) => false,
                        }
                    }
                    _ => false,
                },
                _ => false,
            };

            if !sent_with_photo {
                ctx.reply_with_mentions(caption, mentions).await;
            }
            true
        }
        "lvl" | "level" | "nivel" | "rango" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            let body = format!(
                "{}\n\n{}",
                box_section(
                    "✨",
                    "NIVEL AURA",
                    &[
                        format!("> Usuario › @{}", jid_tag(&target)),
                        format!("> Nivel › *{}*", u.aura_level),
                        format!("> Puntos Aura › *{}*", format_number(u.aura_points)),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_with_mentions(body, vec![target]).await;
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
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section(
                    "✅",
                    "NOMBRE ACTUALIZADO",
                    &[format!("> Ahora te llamaran › *{}*", clipped)],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            ))
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
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section(
                    "✅",
                    "BIO ACTUALIZADA",
                    &[format!("> {}", clipped)],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            ))
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
                    format_number(u.aura_points),
                    u.aura_level
                ));
            }
            ctx.reply_with_mentions(
                format!(
                    "{}\n\n{}",
                    box_section("🏆", &format!("AURA TOP PAGINA {}", page), &lines),
                    footer_tag(DEFAULT_BOT_NAME)
                ),
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
                Ok(Some(url)) => match ctx.state.http.get(&url).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        let mime = resp
                            .headers()
                            .get("content-type")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("image/jpeg")
                            .to_string();
                        match resp.bytes().await {
                            Ok(bytes) => {
                                let caption =
                                    Some(format!("Foto de perfil de @{}", jid_tag(&target)));
                                if let Err(err) = ctx
                                    .send_image_bytes(
                                        bytes.to_vec(),
                                        caption,
                                        vec![target.clone()],
                                        &mime,
                                    )
                                    .await
                                {
                                    ctx.reply_text(format!("Error enviando foto: {}", err))
                                        .await;
                                }
                            }
                            Err(err) => {
                                ctx.reply_text(format!("Error leyendo imagen: {}", err))
                                    .await;
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
                },
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
