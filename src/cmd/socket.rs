use crate::config::{DEFAULT_BOT_NAME, DEFAULT_BOT_VERSION};
use crate::context::CommandContext;
use crate::ui::{box_section, footer_tag, fyt_bold, jid_tag};
use baileys_rs::socket::{SubBotConfig, SubBotEvent};
use baileys_rs::utils::{AnyMessageContent, MessageGenerationOptions};
use std::sync::Arc;
use std::time::Duration;

fn render_category_block(p: &str, cat: &str) -> Option<String> {
    let (icon, title, items): (&str, &str, Vec<String>) = match cat {
        "system" => (
            "⚙️",
            "SYSTEM",
            vec![
                format!("`{p}ping` › `{p}info` › `{p}system` › `{p}ram`"),
                format!("`{p}snipe` › `{p}lang` › `{p}debug`"),
            ],
        ),
        "socket" => (
            "🔌",
            "SOCKET",
            vec![
                format!("`{p}menu` › `{p}bots` › `{p}code` › `{p}stopsub`"),
            ],
        ),
        "ai" => (
            "🤖",
            "AI",
            vec![
                format!("`{p}gpt` › `{p}gemini` › `{p}deepseek`"),
                format!("`{p}grok` › `{p}copilot`"),
            ],
        ),
        "download" => (
            "📥",
            "DOWNLOAD",
            vec![
                format!("`{p}play` › `{p}play2` › `{p}spotify`"),
                format!("`{p}tiktok` › `{p}ttaudio` › `{p}ig` › `{p}fb` › `{p}x`"),
                format!("`{p}apk` › `{p}mf` › `{p}ssweb`"),
            ],
        ),
        "search" => (
            "🔍",
            "SEARCH",
            vec![
                format!("`{p}yts` › `{p}tts` › `{p}sps` › `{p}pin` › `{p}lrc`"),
            ],
        ),
        "sticker" => (
            "🎨",
            "STICKER",
            vec![
                format!("`{p}s` › `{p}setmeta` › `{p}toimg`"),
                format!("`{p}brat` › `{p}bratv` › `{p}emojimix`"),
            ],
        ),
        "group" => (
            "🛡️",
            "GROUP",
            vec![
                format!("`{p}kick` › `{p}add` › `{p}promote` › `{p}demote`"),
                format!("`{p}open` › `{p}close` › `{p}link` › `{p}revoke` › `{p}leave`"),
                format!("`{p}tagall` › `{p}tag` › `{p}infogroup` › `{p}del`"),
                format!("`{p}antilink` › `{p}antitoxic` › `{p}antispam` › `{p}onlyadmin` › `{p}nsfw`"),
                format!("`{p}warn` › `{p}delwarn` › `{p}warns` › `{p}setwarnlimit` › `{p}mute` › `{p}unmute`"),
                format!("`{p}welcome` › `{p}bye` › `{p}setwelcome` › `{p}setprefix` › `{p}bot`"),
            ],
        ),
        "economy" => (
            "💰",
            "ECONOMY",
            vec![
                format!("`{p}bal` › `{p}baltop` › `{p}einfo` › `{p}dep` › `{p}with` › `{p}pay`"),
                format!("`{p}daily` › `{p}weekly` › `{p}fortnightly` › `{p}monthly`"),
                format!("`{p}work` › `{p}mine` › `{p}hunt` › `{p}crime` › `{p}slut` › `{p}adventure`"),
                format!("`{p}cf` › `{p}ruleta` › `{p}ppt` › `{p}steal`"),
            ],
        ),
        "profile" => (
            "👤",
            "PROFILE",
            vec![
                format!("`{p}perfil` › `{p}pfp` › `{p}lvl` › `{p}eboard`"),
                format!("`{p}setmyname` › `{p}setdesc` › `{p}setgenre` › `{p}cumple` › `{p}idioma`"),
                format!("`{p}marry` › `{p}divorce` › `{p}accept` › `{p}reject`"),
            ],
        ),
        "gacha" => (
            "🎴",
            "GACHA",
            vec![
                format!("`{p}rw` › `{p}claim` › `{p}harem` › `{p}winfo` › `{p}wimage`"),
                format!("`{p}animelist` › `{p}animeinfo` › `{p}ginfo` › `{p}vote`"),
                format!("`{p}waifusboard` › `{p}htop` › `{p}sell` › `{p}wshop` › `{p}buyc`"),
                format!("`{p}givechar` › `{p}delchar` › `{p}setfav` › `{p}robwaifu`"),
            ],
        ),
        "games" => (
            "🎮",
            "GAMES & STALK",
            vec![
                format!("`{p}ahorcado` › `{p}ruletarusa` › `{p}aviator` › `{p}slots` › `{p}dado`"),
                format!("`{p}juegos` › `{p}inspect`"),
            ],
        ),
        "funy" => (
            "🎲",
            "FUNY & UTILS",
            vec![
                format!("`{p}8bal` › `{p}ship` › `{p}demandar` › `{p}top` › `{p}cuernos`"),
                format!("`{p}lid` › `{p}get` › `{p}setrole` › `{p}banuser`"),
            ],
        ),
        "interaccion" | "nsfw" => (
            "✨",
            "INTERACCION & NSFW",
            vec![
                format!("`{p}hug` › `{p}kiss` › `{p}slap` › `{p}pat` › `{p}poke` › `{p}bite` › `{p}bonk` (+30)"),
                format!("`{p}r34` › `{p}xnxx` › `{p}xvideos` › `{p}spank` › `{p}yuri`"),
            ],
        ),
        _ => return None,
    };
    Some(box_section(icon, title, &items))
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "menu" | "help" | "h" | "menú" => {
            let p = &ctx.used_prefix;
            let req_cat = ctx.args.first().map(|s| s.to_ascii_lowercase());
            if let Some(cat) = req_cat {
                if let Some(block) = render_category_block(p, &cat) {
                    let body = format!("{}\n\n{}", block, footer_tag(DEFAULT_BOT_NAME));
                    ctx.reply_text(body).await;
                } else {
                    let cats = "system, socket, ai, download, search, sticker, group, economy, profile, gacha, games, funy, interaccion";
                    ctx.reply_text(format!(
                        "{}\n\n{}",
                        box_section(
                            "⚠️",
                            "CATEGORIA NO ENCONTRADA",
                            &[format!("> Disponibles › {}", cats)],
                        ),
                        footer_tag(DEFAULT_BOT_NAME)
                    ))
                    .await;
                }
                return true;
            }

            let all_cats = [
                "system",
                "socket",
                "ai",
                "download",
                "search",
                "sticker",
                "group",
                "economy",
                "profile",
                "gacha",
                "games",
                "funy",
                "interaccion",
            ];
            let header = box_section(
                "⚡",
                DEFAULT_BOT_NAME,
                &[
                    format!("> {} › v{} (Rust)", fyt_bold("Version"), DEFAULT_BOT_VERSION),
                    format!("> {} › {}", fyt_bold("Prefijos"), ctx.state.config.prefixes.join(" ")),
                    format!("> {} › `{}menu <categoria>`", fyt_bold("Filtrar"), p),
                ],
            );
            let sections: Vec<String> = all_cats
                .iter()
                .filter_map(|c| render_category_block(p, c))
                .collect();
            let body = format!(
                "{}\n\n{}\n\n{}",
                header,
                sections.join("\n\n"),
                footer_tag(DEFAULT_BOT_NAME)
            );
            ctx.reply_text(body).await;
            true
        }
        "bots" | "subbots" => {
            let statuses = ctx.state.subbots.list_statuses().await;
            if statuses.is_empty() {
                ctx.reply_text(format!(
                    "{}\n\n{}",
                    box_section(
                        "🤖",
                        "SUB-BOTS",
                        &["> No hay SubBots activos en este momento.".to_string()],
                    ),
                    footer_tag(DEFAULT_BOT_NAME)
                ))
                .await;
                return true;
            }
            let lines: Vec<String> = statuses
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    format!(
                        "{}. `{}` › {} ({})",
                        i + 1,
                        s.session_id,
                        if s.connected { "🟢 online" } else { "🟡 conectando" },
                        s.jid.as_deref().unwrap_or("vinculando")
                    )
                })
                .collect();
            ctx.reply_text(format!(
                "{}\n\n{}",
                box_section("🤖", &format!("SUB-BOTS ({})", statuses.len()), &lines),
                footer_tag(DEFAULT_BOT_NAME)
            ))
            .await;
            true
        }
        "code" | "serbot" | "pairingcode" | "vincularcode" => {
            let manual_phone: String = ctx
                .args
                .first()
                .map(|s| s.chars().filter(|c| c.is_ascii_digit()).collect())
                .unwrap_or_default();
            let is_lid = ctx.sender.ends_with("@lid");
            let auto_phone: String = if is_lid {
                String::new()
            } else {
                jid_tag(&ctx.sender)
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect()
            };
            let phone = if !manual_phone.is_empty() {
                manual_phone
            } else {
                auto_phone
            };

            if phone.len() < 8 || phone.len() > 15 {
                ctx.reply_text(format!(
                    "{}\n\n{}",
                    box_section(
                        "⚠️",
                        "NUMERO REQUERIDO",
                        &[
                            "> Indica tu numero con codigo de pais.".to_string(),
                            format!("> Uso › `{}code 573001234567`", ctx.used_prefix),
                        ],
                    ),
                    footer_tag(DEFAULT_BOT_NAME)
                ))
                .await;
                return true;
            }

            let tag = jid_tag(&ctx.sender);
            let prep = box_section(
                "⚡",
                "PREPARACION",
                &[
                    format!("Hola @{}", tag),
                    "ya casi recibes el".to_string(),
                    "codigo de vinculacion".to_string(),
                ],
            );
            ctx.reply_with_mentions(prep, vec![ctx.sender.clone()]).await;

            let mut rx = ctx.state.subbots.subscribe();
            let session_dir = ctx.state.config.sub_sessions_dir.join(&phone);
            let _ = tokio::fs::remove_dir_all(&session_dir).await;
            let mut sub_cfg = SubBotConfig::new(phone.clone(), session_dir, ctx.state.wa_version);
            sub_cfg.pair_phone = Some(phone.clone());

            if let Err(err) = ctx.state.subbots.spawn(sub_cfg).await {
                ctx.reply_text(format!(
                    "{}\n\n{}",
                    box_section("❌", "ERROR SUB-BOT", &[format!("> {}", err)]),
                    footer_tag(DEFAULT_BOT_NAME)
                ))
                .await;
                return true;
            }

            let messages = Arc::clone(&ctx.messages);
            let from = ctx.from.clone();
            let bot_jid = ctx.bot_jid.clone();
            let quoted_msg = ctx.message.clone();
            let target_session = phone.clone();
            let prefix = ctx.used_prefix.clone();

            tokio::spawn(async move {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
                let mut sent_code = false;
                loop {
                    let rem = deadline.saturating_duration_since(tokio::time::Instant::now());
                    if rem.is_zero() {
                        if !sent_code {
                            let opts = MessageGenerationOptions {
                                user_jid: bot_jid.clone(),
                                quoted: Some(Box::new(quoted_msg.clone())),
                                ..Default::default()
                            };
                            let _ = messages
                                .send_message(
                                    &from,
                                    AnyMessageContent::Text(
                                        "Tiempo agotado esperando el codigo de vinculacion. Intenta de nuevo."
                                            .to_string(),
                                    ),
                                    opts,
                                )
                                .await;
                        }
                        break;
                    }
                    match tokio::time::timeout(rem, rx.recv()).await {
                        Ok(Ok(SubBotEvent::PairingCode { session_id, code }))
                            if session_id == target_session =>
                        {
                            sent_code = true;
                            let formatted = if code.len() == 8 {
                                format!("{}-{}", &code[..4], &code[4..])
                            } else {
                                code.clone()
                            };
                            let inst = box_section(
                                "📲",
                                "INSTRUCCIONES",
                                &[
                                    "1. Sal a tu menu de chats".to_string(),
                                    "2. Toca el boton `⋮`".to_string(),
                                    "3. Ve a `dispositivos`".to_string(),
                                    "4. Toca `vincular dispositivo`".to_string(),
                                    "5. Toca `vincular con codigo`".to_string(),
                                    "6. Espera y confirma conexion".to_string(),
                                ],
                            );
                            let code_box = box_section("🔑", "TU CODIGO", &[formatted.clone()]);
                            let opts = MessageGenerationOptions {
                                user_jid: bot_jid.clone(),
                                quoted: Some(Box::new(quoted_msg.clone())),
                                ..Default::default()
                            };
                            let _ = messages
                                .send_message(
                                    &from,
                                    AnyMessageContent::Text(format!("{}\n\n{}", inst, code_box)),
                                    opts,
                                )
                                .await;
                            let opts2 = MessageGenerationOptions {
                                user_jid: bot_jid.clone(),
                                ..Default::default()
                            };
                            let _ = messages
                                .send_message(&from, AnyMessageContent::Text(formatted), opts2)
                                .await;
                        }
                        Ok(Ok(SubBotEvent::Connected { session_id, .. }))
                            if session_id == target_session =>
                        {
                            let online = format!(
                                "{}\n┃ > El sub-bot ya esta en linea y conectado a WhatsApp.\n┃ > Para desconectar usa `{}stopsub {}`",
                                box_section("⚡", DEFAULT_BOT_NAME, &["✅ SUB-BOT EN LINEA".to_string()]),
                                prefix,
                                target_session
                            );
                            let opts = MessageGenerationOptions {
                                user_jid: bot_jid.clone(),
                                quoted: Some(Box::new(quoted_msg.clone())),
                                ..Default::default()
                            };
                            let _ = messages
                                .send_message(&from, AnyMessageContent::Text(online), opts)
                                .await;
                            break;
                        }
                        Ok(Ok(_)) => {}
                        Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                        Ok(Err(_)) | Err(_) => break,
                    }
                }
            });
            true
        }
        "stopsub" => {
            if !ctx.is_owner {
                ctx.reply_text("Solo owners pueden detener SubBots.").await;
                return true;
            }
            let id = ctx.text.trim();
            if id.is_empty() {
                ctx.reply_text(format!("Uso: `{}stopsub <id>`", ctx.used_prefix))
                    .await;
                return true;
            }
            if ctx.state.subbots.stop(id).await {
                ctx.reply_text(format!("SubBot `{}` detenido.", id)).await;
            } else {
                ctx.reply_text(format!("SubBot `{}` no encontrado.", id))
                    .await;
            }
            true
        }
        _ => false,
    }
}
