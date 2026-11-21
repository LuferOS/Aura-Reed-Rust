use crate::config::{DEFAULT_BOT_NAME, DEFAULT_BOT_VERSION};
use crate::context::CommandContext;
use baileys_rs::socket::SubBotConfig;

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "menu" | "help" => {
            let p = &ctx.used_prefix;
            let body = format!(
                "*{bot} Rust* v{ver}\n\
                Prefijos: {prefs}\n\n\
                *[system]*\n\
                - `{p}ping` / `{p}info` / `{p}ram` / `{p}snipe` / `{p}lang` / `{p}debug`\n\n\
                *[socket]*\n\
                - `{p}menu` / `{p}bots` / `{p}code <numero>` / `{p}stopsub <id>`\n\n\
                *[sticker]*\n\
                - `{p}s` / `{p}setmeta` / `{p}brat` / `{p}bratv` / `{p}emojimix` / `{p}toimg`\n\n\
                *[ai]*\n\
                - `{p}gpt` / `{p}gemini` / `{p}deepseek` / `{p}grok` / `{p}copilot`\n\n\
                *[economy]*\n\
                - `{p}bal` / `{p}baltop` / `{p}einfo` / `{p}dep` / `{p}with` / `{p}pay`\n\
                - `{p}daily` / `{p}weekly` / `{p}fortnightly` / `{p}monthly`\n\
                - `{p}work` / `{p}mine` / `{p}hunt` / `{p}crime` / `{p}slut` / `{p}adventure`\n\
                - `{p}cf` / `{p}ruleta` / `{p}ppt` / `{p}steal`\n\n\
                *[profile]*\n\
                - `{p}perfil` / `{p}pfp` / `{p}lvl` / `{p}eboard` / `{p}setmyname` / `{p}setdesc`\n\
                - `{p}setgenre` / `{p}cumple` / `{p}idioma` / `{p}marry` / `{p}divorce` / `{p}accept` / `{p}reject`\n\n\
                *[group]*\n\
                - `{p}antilink` / `{p}antitoxic` / `{p}antispam` / `{p}anticalls` / `{p}onlyadmin` / `{p}nsfw` / `{p}bot`\n\
                - `{p}kick` / `{p}add` / `{p}promote` / `{p}demote` / `{p}open` / `{p}close` / `{p}link` / `{p}revoke` / `{p}leave`\n\
                - `{p}tagall` / `{p}tag` / `{p}infogroup` / `{p}welcome` / `{p}bye` / `{p}setwelcome` / `{p}setsubject` / `{p}setdescgp`\n\
                - `{p}setprefix` / `{p}warn` / `{p}delwarn` / `{p}warns` / `{p}setwarnlimit` / `{p}mute` / `{p}unmute` / `{p}del`\n\n\
                *[download]*\n\
                - `{p}play` / `{p}play2` / `{p}tiktok` / `{p}ttaudio` / `{p}ig` / `{p}fb` / `{p}spotify` / `{p}x` / `{p}apk` / `{p}mf` / `{p}ssweb`\n\n\
                *[gacha]*\n\
                - `{p}rw` / `{p}claim` / `{p}harem` / `{p}winfo` / `{p}wimage` / `{p}animelist` / `{p}animeinfo` / `{p}ginfo`\n\
                - `{p}vote` / `{p}waifusboard` / `{p}htop` / `{p}sell` / `{p}wshop` / `{p}buyc` / `{p}givechar` / `{p}delchar` / `{p}setfav` / `{p}robwaifu`\n\n\
                *[search]*\n\
                - `{p}yts` / `{p}tts` / `{p}sps` / `{p}pin` / `{p}lrc`\n\n\
                *[games & stalk]*\n\
                - `{p}ahorcado` / `{p}ruletarusa` / `{p}aviator` / `{p}slots` / `{p}dado` / `{p}juegos` / `{p}inspect`\n\n\
                *[interaccion & nsfw]*\n\
                - `{p}hug` / `{p}kiss` / `{p}slap` / `{p}pat` / `{p}poke` / `{p}bite` / `{p}cry` / `{p}dance` / `{p}kill` / `{p}bonk` (+30)\n\
                - `{p}r34` / `{p}xnxx` / `{p}xvideos` / `{p}spank` / `{p}yuri` (requiere `{p}nsfw on`)\n\n\
                *[funy & utils]*\n\
                - `{p}8bal` / `{p}ship` / `{p}demandar` / `{p}top` / `{p}cuernos` / `{p}lid` / `{p}get` / `{p}setrole` / `{p}banuser`",
                bot = DEFAULT_BOT_NAME,
                ver = DEFAULT_BOT_VERSION,
                prefs = ctx.state.config.prefixes.join(" "),
                p = p
            );
            ctx.reply_text(body).await;
            true
        }
        "bots" | "subbots" => {
            let statuses = ctx.state.subbots.list_statuses().await;
            if statuses.is_empty() {
                ctx.reply_text("No hay SubBots activos en este momento.")
                    .await;
                return true;
            }
            let lines = statuses
                .iter()
                .map(|s| {
                    format!(
                        "- `{}` | {} | {}",
                        s.session_id,
                        if s.connected { "online" } else { "offline" },
                        s.jid.as_deref().unwrap_or("vinculando")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_text(format!("*SubBots ({})*\n{}", statuses.len(), lines))
                .await;
            true
        }
        "code" | "serbot" => {
            let phone: String = ctx
                .text
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if phone.len() < 8 {
                ctx.reply_text(format!(
                    "Uso: `{}code 573001234567`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            let session_dir = ctx.state.config.sub_sessions_dir.join(&phone);
            let mut sub_cfg = SubBotConfig::new(
                phone.clone(),
                session_dir,
                ctx.state.wa_version,
            );
            sub_cfg.pair_phone = Some(phone.clone());
            match ctx.state.subbots.spawn(sub_cfg).await {
                Ok(()) => {
                    ctx.reply_text(format!(
                        "SubBot `{}` iniciado. Espera el codigo de vinculacion en consola/chat.",
                        phone
                    ))
                    .await;
                }
                Err(err) => {
                    ctx.reply_text(format!("Error iniciando SubBot: {}", err))
                        .await;
                }
            }
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
