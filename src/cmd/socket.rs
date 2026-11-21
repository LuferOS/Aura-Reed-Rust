use crate::config::DEFAULT_BOT_NAME;
use crate::context::CommandContext;
use crate::ui::{box_section, current_date_dmy, footer_tag, fyt_bold, jid_tag, menu_category_box};
use baileys_rs::defaults::MediaType;
use baileys_rs::proto::message::{ExtendedTextMessage, ImageMessage};
use baileys_rs::socket::{SubBotConfig, SubBotEvent};
use baileys_rs::utils::{
    download_content_from_message, normalize_message_content, AnyMessageContent,
    DownloadableMessage, MediaDownloadOptions, MessageGenerationOptions,
};
use std::borrow::Borrow;
use std::sync::Arc;
use std::time::Duration;

fn resolve_category_alias(raw: &str) -> &'static str {
    match raw {
        "ai" | "ia" | "inteligenciaartificial" => "ai",
        "download" | "downloads" | "descarga" | "descargas" => "download",
        "search" | "busqueda" | "buscador" => "search",
        "sticker" | "stickers" | "pegatinas" => "sticker",
        "group" | "grupo" | "grupos" => "group",
        "economy" | "economia" => "economy",
        "profile" | "perfil" => "profile",
        "gacha" | "waifu" | "waifus" | "harem" => "gacha",
        "games" | "juegos" => "games",
        "funy" | "fun" | "diversion" => "funy",
        "interaccion" | "interaction" | "interacciones" => "interaccion",
        "nsfw" | "18" | "+18" => "nsfw",
        "stalk" | "inspeccion" => "stalk",
        "utils" | "utilidades" | "utiles" => "utils",
        "socket" | "sockets" | "subbot" | "subbots" => "socket",
        "system" | "sistema" | "owner" => "system",
        _ => "",
    }
}

fn render_category_block(p: &str, cat: &str) -> Option<String> {
    match cat {
        "ai" => Some(menu_category_box(
            "AI",
            &[
                (&["chatgpt", "ia", "gpt"], "Habla con ChatGPT."),
                (&["deepseek", "dia", "spt"], "Habla con DeepSeek."),
                (&["gemini", "gia", "gai"], "Habla con Gemini."),
                (&["grok", "kia", "gkai"], "Habla con Grok."),
                (&["copilot", "copi", "cpt"], "Habla con Copilot."),
            ],
            p,
        )),
        "download" => Some(menu_category_box(
            "DOWNLOAD",
            &[
                (&["play", "ytmp3", "yta"], "Descarga audio de YouTube."),
                (&["docplay", "ytdmp3", "dmp3"], "Descarga audio YouTube como documento."),
                (&["play2", "ytmp4", "ytv"], "Descarga video de YouTube."),
                (&["docplay2", "dytmp4", "dmp4"], "Descarga video YouTube como documento."),
                (&["tiktok", "tt", "ttdl"], "Descarga videos de TikTok."),
                (&["dtk", "dtt", "tiktok2"], "Descarga TikTok como documento."),
                (&["tta", "ttaudio", "dtta"], "Descarga audio de TikTok (MP3/doc)."),
                (&["spotify", "sp", "dsp"], "Descarga canciones de Spotify."),
                (&["ig", "instagram"], "Descarga reels o fotos de Instagram."),
                (&["fb", "facebook", "fbreels"], "Descarga videos de Facebook."),
                (&["x", "twitter", "tw"], "Descarga videos de X/Twitter."),
                (&["apk", "apkdl", "app"], "Descarga archivos APK de Android."),
                (&["mf", "mediafire", "ssweb"], "MediaFire y capturas web."),
            ],
            p,
        )),
        "search" => Some(menu_category_box(
            "SEARCH",
            &[
                (&["yts", "ytsearch", "plays"], "Busca videos en YouTube."),
                (&["tts", "ttsearch"], "Busca videos en TikTok."),
                (&["sps", "spsearch"], "Busca canciones en Spotify."),
                (&["pin", "pinterest"], "Busca imagenes en Pinterest."),
                (&["lrc", "lyrics", "letra"], "Busca letras de canciones."),
            ],
            p,
        )),
        "sticker" => Some(menu_category_box(
            "STICKER",
            &[
                (&["s", "sticker", "stiker"], "Crea stickers de imagenes o videos."),
                (&["brat", "bratv"], "Stickers de texto estilo Brat y animado."),
                (&["emojimix", "ekitchen"], "Combina dos emojis en un sticker."),
                (&["setmeta", "toimg"], "Metadata de sticker y sticker a imagen."),
            ],
            p,
        )),
        "group" => Some(menu_category_box(
            "GROUP",
            &[
                (&["kick", "add", "promote", "demote"], "Gestion de miembros y admins."),
                (&["open", "close", "link", "revoke"], "Control y enlace del grupo."),
                (&["tagall", "tag", "infogroup", "adminsys"], "Menciones e info del grupo."),
                (&["antilink", "antitoxic", "antispam", "onlyadmin"], "Sistemas de moderacion."),
                (&["warn", "delwarn", "warns", "setwarnlimit"], "Sistema de advertencias."),
                (&["mute", "unmute", "del", "tempmsg"], "Silenciar, borrar y temporales."),
                (&["welcome", "bye", "setwelcome", "testwelcome"], "Bienvenida y despedida."),
                (&["banchat", "unbanchat", "disable", "enable"], "Baneo de chat y categorias."),
                (&["topactivos", "setprefix", "nsfw", "leave"], "Actividad, prefijo y salida."),
            ],
            p,
        )),
        "economy" => Some(menu_category_box(
            "ECONOMY",
            &[
                (&["bal", "baltop", "einfo"], "Saldo, top global y cooldowns."),
                (&["dep", "with", "pay"], "Banco y transferencias."),
                (&["daily", "weekly", "fortnightly", "monthly"], "Recompensas con racha."),
                (&["work", "mine", "hunt", "crime", "slut", "adventure"], "Trabajos y misiones."),
                (&["cf", "ruleta", "ppt", "steal"], "Apuestas y robo de monedas."),
            ],
            p,
        )),
        "profile" => Some(menu_category_box(
            "PROFILE",
            &[
                (&["perfil", "pfp", "lvl", "eboard"], "Perfil, foto, nivel y Aura Top."),
                (&["setmyname", "setdesc", "setgenre", "cumple"], "Edita tu perfil."),
                (&["idioma", "marry", "divorce", "accept", "reject"], "Idioma y matrimonio."),
            ],
            p,
        )),
        "gacha" => Some(menu_category_box(
            "GACHA",
            &[
                (&["rw", "claim", "harem"], "Invoca, reclama y mira tu coleccion."),
                (&["winfo", "wimage", "animelist", "animeinfo"], "Catalogo de personajes y series."),
                (&["ginfo", "gcheck", "vote", "waifusboard", "htop"], "Estadisticas y rankings."),
                (&["sell", "wshop", "buyc", "removerventa"], "Tienda de cartas del grupo."),
                (&["givechar", "giveallharem", "delchar", "setfav", "delfav", "robwaifu"], "Gestion de harem."),
            ],
            p,
        )),
        "games" => Some(menu_category_box(
            "GAMES",
            &[
                (&["ahorcado", "ah"], "Ahorcado interactivo en el chat."),
                (&["ruletarusa", "rr"], "Ruleta rusa 1 de 6 en grupos."),
                (&["aviator", "slots", "dado", "juegos"], "Minijuegos de apuestas."),
            ],
            p,
        )),
        "funy" => Some(menu_category_box(
            "FUNY",
            &[
                (&["8bal", "ship", "demandar"], "Bola 8, medidor de amor y juicio."),
                (&["top", "cuernos"], "Top 10 aleatorio y detector de amantes."),
            ],
            p,
        )),
        "interaccion" => Some(menu_category_box(
            "INTERACCION",
            &[
                (&["hug", "kiss", "slap", "pat", "poke"], "Reacciones anime SFW (+35 comandos)."),
                (&["bite", "cry", "dance", "kill", "bonk", "yeet"], "Mas interacciones SFW."),
            ],
            p,
        )),
        "nsfw" => Some(menu_category_box(
            "NSFW",
            &[
                (&["r34", "xnxx", "xvideos"], "Buscador y contenido +18 (requiere .nsfw on)."),
                (&["spank", "yuri", "blowjob", "fuck", "undress"], "Interacciones NSFW."),
            ],
            p,
        )),
        "stalk" => Some(menu_category_box(
            "STALK",
            &[(&["inspect", "inspeccionar"], "Inspecciona un enlace de grupo WhatsApp.")],
            p,
        )),
        "utils" => Some(menu_category_box(
            "UTILS",
            &[
                (&["read", "compress"], "Revela ViewOnce y comprime videos."),
                (&["lid", "get", "report"], "Identificadores, HTTP GET y reportes."),
                (&["setrole", "banuser", "unbanuser"], "Roles y baneos (owner)."),
            ],
            p,
        )),
        "socket" => Some(menu_category_box(
            "SOCKET",
            &[
                (&["menu", "help", "bots"], "Menu interactivo y lista de SubBots."),
                (&["code", "stopsub"], "Vincular SubBot por codigo y detener."),
                (&["setbotname", "setcurrency", "setbanner", "self"], "Personalizacion del bot."),
            ],
            p,
        )),
        "system" => Some(menu_category_box(
            "SYSTEM",
            &[
                (&["ping", "info", "infobot", "ram"], "Latencia, estado y memoria RAM."),
                (&["snipe", "lang", "subbotstats"], "Mensajes borrados, idiomas y cluster."),
                (&["autoadmin", "bc", "r", "restart", "debug"], "Herramientas de owner."),
            ],
            p,
        )),
        _ => None,
    }
}

fn find_image_for_banner(ctx: &CommandContext) -> Option<ImageMessage> {
    let proto = ctx.message.message.as_ref()?;
    let norm = normalize_message_content(proto);
    if let Some(ref img) = norm.image_message {
        let i: &ImageMessage = Borrow::borrow(img);
        return Some(i.clone());
    }
    if let Some(ref ext) = norm.extended_text_message {
        let inner: &ExtendedTextMessage = Borrow::borrow(ext);
        let q = inner.context_info.as_ref()?.quoted_message.as_ref()?;
        let q_norm = normalize_message_content(q);
        if let Some(ref q_img) = q_norm.image_message {
            let qi: &ImageMessage = Borrow::borrow(q_img);
            return Some(qi.clone());
        }
    }
    None
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "menu" | "help" | "h" | "menú" => {
            let p = &ctx.used_prefix;
            let bot_key = ctx.bot_jid.as_deref().unwrap_or("main");
            let (bot_name, _, _) = ctx.state.db.get_bot_setting(bot_key);
            let display_user = ctx
                .user
                .name
                .as_deref()
                .or(ctx.user.username.as_deref())
                .or(ctx.push_name.as_deref())
                .unwrap_or("Usuario");

            let req_raw = ctx.args.first().map(|s| s.to_ascii_lowercase());
            if let Some(ref raw) = req_raw {
                let cat = resolve_category_alias(raw);
                if let Some(block) = render_category_block(p, cat) {
                    let body = format!("{}\n\n{}", block, footer_tag(&format!("{} BOT", bot_name.to_uppercase())));
                    ctx.reply_text(body).await;
                } else {
                    let cats = "ai, download, search, sticker, group, economy, profile, gacha, games, funy, interaccion, nsfw, stalk, utils, socket, system";
                    ctx.reply_text(format!(
                        "{}\n\n{}",
                        box_section("⚠️", "CATEGORIA NO ENCONTRADA", &[format!("> Disponibles › {}", cats)]),
                        footer_tag(&bot_name)
                    ))
                    .await;
                }
                return true;
            }

            let header = format!(
                "╭━━〔 {} 〕━━⬣\n\
                ┃ > {} {}\n\
                ┃ > {} Principal (Rust)\n\
                ┃ > {} 2.5\n\
                ┃ > {} LuferOS\n\
                ┃ > {} [ {} ]\n\
                ┃ > {} {}\n\
                ┃ > {} https://api.aetheryx.xyz/\n\
                ╰━━━━━━━━━━━━━⬣",
                fyt_bold(&bot_name.to_uppercase()),
                fyt_bold("Usuario:"),
                display_user,
                fyt_bold("Instancia:"),
                fyt_bold("Version:"),
                fyt_bold("Dueno:"),
                fyt_bold("Prefijo:"),
                p,
                fyt_bold("Fecha:"),
                current_date_dmy(),
                fyt_bold("Url:")
            );

            let cats = [
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
                "nsfw",
                "stalk",
                "utils",
                "socket",
                "system",
            ];
            let sections: Vec<String> = cats
                .iter()
                .filter_map(|c| render_category_block(p, c))
                .collect();
            let full_menu = format!(
                "{}\n\n{}\n\n{}",
                header,
                sections.join("\n\n"),
                footer_tag(&format!("{} BOT", bot_name.to_uppercase()))
            );

            let banner_paths = [
                ctx.state.config.data_dir.join("BotBanner.jpg"),
                std::path::PathBuf::from("assets/img/BotBanner.jpg"),
            ];
            let mut sent_banner = false;
            for bp in banner_paths {
                if let Ok(bytes) = tokio::fs::read(&bp).await {
                    if !bytes.is_empty()
                        && ctx
                            .send_image_bytes(bytes, Some(full_menu.clone()), Vec::new(), "image/jpeg")
                            .await
                            .is_ok()
                    {
                        sent_banner = true;
                        break;
                    }
                }
            }
            if !sent_banner {
                ctx.reply_text(full_menu).await;
            }
            true
        }
        "setbotname" | "setname" | "botname" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let name = ctx.text.trim();
            if name.is_empty() || name.chars().count() > 60 {
                ctx.reply_text(format!("Uso: `{}setbotname <nombre>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let bot_key = ctx.bot_jid.as_deref().unwrap_or("main");
            ctx.state.db.set_bot_name(bot_key, name);
            ctx.reply_text(format!("Nombre del bot actualizado a *{}*.", name))
                .await;
            true
        }
        "setcurrency" | "setmoneda" | "currency" | "setbotcurrency" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let bot_key = ctx.bot_jid.as_deref().unwrap_or("main");
            let raw = ctx.text.trim();
            if raw.is_empty() {
                let (_, cur_name, cur_sym) = ctx.state.db.get_bot_setting(bot_key);
                ctx.reply_text(format!(
                    "Moneda actual: *{}* ({})\nUso: `{}setcurrency <nombre> [simbolo]`",
                    cur_name, cur_sym, ctx.used_prefix
                ))
                .await;
                return true;
            }
            if matches!(raw.to_ascii_lowercase().as_str(), "reset" | "default") {
                ctx.state.db.set_bot_currency(bot_key, "AuraCoins", "₡");
                ctx.reply_text("Moneda restablecida a *AuraCoins* (₡).").await;
                return true;
            }
            let parts: Vec<&str> = raw.split_whitespace().collect();
            let (name, sym) = if parts.len() >= 2 && parts.last().unwrap().chars().count() <= 4 {
                (parts[..parts.len() - 1].join(" "), (*parts.last().unwrap()).to_string())
            } else {
                (raw.to_string(), "₡".to_string())
            };
            ctx.state.db.set_bot_currency(bot_key, &name, &sym);
            ctx.reply_text(format!("Moneda actualizada a *{}* ({}).", name, sym))
                .await;
            true
        }
        "setbanner" | "setmenuimage" | "setmenubanner" => {
            if !ctx.is_owner {
                ctx.reply_text("Comando reservado para owners.").await;
                return true;
            }
            let Some(img) = find_image_for_banner(ctx) else {
                ctx.reply_text(format!("Responde a una imagen con `{}setbanner`.", ctx.used_prefix))
                    .await;
                return true;
            };
            let dl = DownloadableMessage {
                media_key: img.media_key,
                direct_path: img.direct_path,
                url: img.url,
            };
            let dl_opts = MediaDownloadOptions {
                timeout_ms: Some(12_000),
                ..Default::default()
            };
            match download_content_from_message(&dl, MediaType::Image, &dl_opts).await {
                Ok(bytes) => {
                    let dst = ctx.state.config.data_dir.join("BotBanner.jpg");
                    if tokio::fs::write(&dst, bytes).await.is_ok() {
                        ctx.reply_text("Banner de `.menu` guardado correctamente.").await;
                    } else {
                        ctx.reply_text("No pude guardar el banner en disco.").await;
                    }
                }
                Err(err) => ctx.reply_text(format!("Error descargando banner: {}", err)).await,
            }
            true
        }
        "self" | "setself" => {
            if !ctx.is_owner && !matches!(ctx.user.role.as_str(), "owner" | "coowner" | "mod") {
                ctx.reply_text("Comando reservado para owners y moderadores.")
                    .await;
                return true;
            }
            let val = ctx.args.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();
            let enabled = match val.as_str() {
                "on" | "1" | "true" => true,
                "off" | "0" | "false" => false,
                _ => {
                    ctx.reply_text(format!("Uso: `{}self on|off`", ctx.used_prefix))
                        .await;
                    return true;
                }
            };
            ctx.state
                .db
                .set_group_data_field(&ctx.from, "self", serde_json::Value::Bool(enabled));
            ctx.reply_text(format!(
                "Modo Self *{}* en este chat.",
                if enabled { "activado" } else { "desactivado" }
            ))
            .await;
            true
        }
        "bots" | "subbots" | "btos" | "lista-bots" => {
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
