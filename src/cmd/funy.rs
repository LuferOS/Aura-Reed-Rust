use crate::config::DEFAULT_BOT_NAME;
use crate::context::{same_bare_jid, CommandContext};
use crate::ui::{box_section, footer_tag, jid_tag};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn quick_hash(seed: &str) -> u64 {
    let mut h = DefaultHasher::new();
    seed.hash(&mut h);
    h.finish()
}

fn now_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "8bal" | "8ball" | "8bola" => {
            if ctx.text.trim().is_empty() {
                ctx.reply_text(format!(
                    "{}\n\n{}",
                    box_section(
                        "🎱",
                        "FALTA PREGUNTA",
                        &[format!("> Uso › `{}8bal <pregunta>`", ctx.used_prefix)],
                    ),
                    footer_tag(DEFAULT_BOT_NAME)
                ))
                .await;
                return true;
            }
            let answers = [
                "Definitivamente si.",
                "Sin ninguna duda.",
                "Puedes confiar en ello.",
                "Es muy probable.",
                "Pregunta de nuevo mas tarde.",
                "Mejor no decirte ahora.",
                "No cuentes con ello.",
                "Mi respuesta es no.",
                "Las fuentes dicen que no.",
                "Muy dudoso.",
            ];
            let idx = ((quick_hash(ctx.text.trim()) ^ now_nanos()) as usize) % answers.len();
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section(
                    "🎱",
                    "BOLA 8",
                    &[
                        format!("> Pregunta › {}", ctx.text.trim()),
                        format!("> Respuesta › *{}*", answers[idx]),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            ))
            .await;
            true
        }
        "ship" => {
            let mentioned = ctx.mentioned_jids();
            let (first, second) = if mentioned.len() >= 2 {
                (mentioned[0].clone(), mentioned[1].clone())
            } else if mentioned.len() == 1 && !same_bare_jid(&mentioned[0], &ctx.sender) {
                (ctx.sender.clone(), mentioned[0].clone())
            } else {
                ctx.reply_text(format!(
                    "『💘』 No pude identificar a los usuarios. Menciona a alguien (o a dos personas) para hacer el ship.\n\n*Uso:* `{}ship @usuario` o `{}ship @u1 @u2`",
                    ctx.used_prefix, ctx.used_prefix
                ))
                .await;
                return true;
            };

            let mut pair = [first.as_str(), second.as_str()];
            pair.sort();
            let pct = (quick_hash(&format!("{}:{}", pair[0], pair[1])) % 101) as usize;
            let filled = (pct + 5) / 10;
            let bar = format!(
                "{}{}",
                "💖".repeat(filled.min(10)),
                "🖤".repeat(10usize.saturating_sub(filled.min(10)))
            );
            let verdict = match pct {
                0..=25 => "Mejor solo amigos 💀",
                26..=55 => "Hay chispa, pero falta trabajo 👀",
                56..=80 => "Muy buena compatibilidad ✨",
                _ => "Pareja perfecta! 💍",
            };
            let body = format!(
                "{}\n\n{}",
                box_section(
                    "💘",
                    "LOVE METER",
                    &[
                        format!("@{} 💞 @{}", jid_tag(&first), jid_tag(&second)),
                        format!("{} *{}%*", bar, pct),
                        format!("> {}", verdict),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_with_mentions(body, vec![first, second]).await;
            true
        }
        "demandar" => {
            let target = ctx.target_jid();
            if same_bare_jid(&target, &ctx.sender) {
                ctx.reply_text(format!(
                    "Uso: `{}demandar @usuario <motivo>`",
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
                "exceso de facha no declarada".to_string()
            } else {
                reason.trim().to_string()
            };
            let body = format!(
                "{}\n\n{}",
                box_section(
                    "⚖️",
                    "ORDEN JUDICIAL",
                    &[
                        format!("> Demandante › @{}", jid_tag(&ctx.sender)),
                        format!("> Acusado › @{}", jid_tag(&target)),
                        format!("> Cargo › {}", reason),
                        "> Estado › *En juicio*".to_string(),
                    ],
                ),
                footer_tag("TRIBUNAL AURA")
            );
            ctx.reply_with_mentions(body, vec![ctx.sender.clone(), target])
                .await;
            true
        }
        "top" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let Some(meta) = ctx.group_metadata(false).await else {
                ctx.reply_text("No pude obtener los participantes del grupo.")
                    .await;
                return true;
            };
            let mut ids: Vec<String> = meta
                .participants
                .iter()
                .map(|p| crate::ui::normalize_jid(&p.id))
                .collect();
            if ids.is_empty() {
                ctx.reply_text("No hay suficientes participantes.").await;
                return true;
            }
            let mut seed = now_nanos();
            for i in (1..ids.len()).rev() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let j = (seed as usize) % (i + 1);
                ids.swap(i, j);
            }
            ids.truncate(10);
            let topic = if ctx.text.trim().is_empty() {
                "LOS MAS LOCOS".to_string()
            } else {
                ctx.text.trim().to_uppercase()
            };
            let lines: Vec<String> = ids
                .iter()
                .enumerate()
                .map(|(i, id)| format!("{}. @{}", i + 1, jid_tag(id)))
                .collect();
            ctx.reply_with_mentions(
                format!(
                    "{}\n\n{}",
                    box_section("🏆", &format!("TOP 10 {}", topic), &lines),
                    footer_tag(DEFAULT_BOT_NAME)
                ),
                ids,
            )
            .await;
            true
        }
        "cuernos" | "amante" | "noviosecreto" | "casialgo" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let Some(meta) = ctx.group_metadata(false).await else {
                ctx.reply_text("No pude obtener los participantes del grupo.")
                    .await;
                return true;
            };
            let pool: Vec<String> = meta
                .participants
                .iter()
                .map(|p| crate::ui::normalize_jid(&p.id))
                .filter(|id| {
                    !same_bare_jid(id, &ctx.sender)
                        && !ctx
                            .bot_jid
                            .as_deref()
                            .is_some_and(|b| same_bare_jid(id, b))
                })
                .collect();
            if pool.is_empty() {
                ctx.reply_text("No hay suficientes participantes en el grupo.")
                    .await;
                return true;
            }
            let lugares = [
                "Motel de 200 pesos 🛌",
                "En los banos del colegio/trabajo 🚽",
                "Detras del Oxxo a las 3 AM 🏪",
                "En el asiento de atras del Uber 🚗",
                "Jugando duo dinamico en Free Fire 🎮",
                "Viendo Netflix sin ver la tele 📺",
            ];
            let seed = now_nanos() ^ quick_hash(&ctx.sender);
            let amante = pool[(seed as usize) % pool.len()].clone();
            let lugar = lugares[((seed >> 8) as usize) % lugares.len()];
            let body = format!(
                "{}\n\n{}",
                box_section(
                    "🕵️",
                    "DETECTOR DE AMANTES",
                    &[
                        format!("@{} fue visto con @{}", jid_tag(&ctx.sender), jid_tag(&amante)),
                        format!("> Lugar › *{}*", lugar),
                    ],
                ),
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_with_mentions(body, vec![ctx.sender.clone(), amante])
                .await;
            true
        }
        _ => false,
    }
}
