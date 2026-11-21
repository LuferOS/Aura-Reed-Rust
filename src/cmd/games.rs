use crate::context::{same_bare_jid, CommandContext, HangmanGame};
use crate::lang::{resolve_lang, tr};
use baileys_rs::types::ParticipantAction;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const HANGMAN_WORDS: &[&str] = &[
    "javascript", "programacion", "teclado", "servidor", "codigo", "computadora", "desarrollo",
    "internet", "aplicacion", "tecnologia", "whatsapp", "inteligencia", "sistema", "variable",
    "algoritmo", "terminal", "framework", "funcion", "compilador", "depurador", "interfaz",
    "navegador", "protocolo", "seguridad", "criptografia", "hardware", "software", "procesador",
    "memoria", "puntero", "sintaxis", "libreria", "repositorio", "firewall", "elefante", "jirafa",
    "delfin", "tortuga", "hipopotamo", "murcielago", "rinoceronte", "camaleon", "aguila",
    "tiburon", "leopardo", "serpiente", "gorila", "flamingo", "ardilla", "tigre", "cebra",
    "pantera", "cocodrilo", "ballena", "pulpo", "medusa", "halcon", "caballo", "conejo",
    "argentina", "colombia", "mexico", "venezuela", "chile", "peru", "ecuador", "uruguay",
    "paraguay", "bolivia", "brasil", "francia", "japon", "italia", "alemania", "canada",
    "australia", "portugal", "rusia", "china", "egipto", "grecia", "suiza", "noruega",
    "bicicleta", "helicoptero", "telescopio", "microscopio", "guitarra", "auriculares",
    "refrigerador", "pantalla", "calculadora", "linterna", "sombrilla", "mochila", "cuaderno",
    "martillo", "candado", "telefono", "camara", "espejo", "ventana", "almohada", "hamburguesa",
    "chocolate", "espagueti", "ensalada", "fresas", "sandia", "aguacate", "maracuya", "mandarina",
    "arandanos", "pizza", "lasagna", "burrito", "tacos", "sushi", "empanada", "manzana",
    "platano", "durazno", "cereza", "papaya", "naranja", "tomate", "cebolla", "zanahoria",
];

const HANGMAN_STAGES: &[&str] = &[
    " +---+\n |   |\n     |\n     |\n     |\n======",
    " +---+\n |   |\n O   |\n     |\n     |\n======",
    " +---+\n |   |\n O   |\n |   |\n     |\n======",
    " +---+\n |   |\n O   |\n/|   |\n     |\n======",
    " +---+\n |   |\n O   |\n/|\\  |\n     |\n======",
    " +---+\n |   |\n O   |\n/|\\  |\n/    |\n======",
    " +---+\n |   |\n O   |\n/|\\  |\n/ \\  |\n======",
];

static GAME_RNG: AtomicU64 = AtomicU64::new(0x5851_f42d_4c95_7f2d);

fn next_rand() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut x = GAME_RNG.fetch_add(now ^ 0x1405_7b7e_f767_814f, Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

fn masked_word(game: &HangmanGame) -> String {
    game.word
        .chars()
        .map(|c| {
            if game.guessed.contains(&c) {
                c.to_ascii_uppercase().to_string()
            } else {
                "_".to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_hangman_text(game: &HangmanGame) -> String {
    let stage = HANGMAN_STAGES[(game.wrong as usize).min(HANGMAN_STAGES.len() - 1)];
    let tried = if game.guessed.is_empty() {
        "-".to_string()
    } else {
        game.guessed
            .iter()
            .map(|c| c.to_ascii_uppercase().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "*Ahorcado*\n```\n{}\n```\nPalabra: *{}*\nIntentos fallidos: {}/6\nLetras: {}",
        stage,
        masked_word(game),
        game.wrong,
        tried
    )
}

pub async fn try_hangman_direct_guess(
    state: &std::sync::Arc<crate::context::BotState>,
    messages: &std::sync::Arc<baileys_rs::socket::MessagesSocket>,
    message: &baileys_rs::types::WaMessage,
    from: &str,
    sender: &str,
    bot_jid: &Option<String>,
    raw_text: &str,
) -> bool {
    let trimmed = raw_text.trim().to_ascii_lowercase();
    if trimmed.chars().count() != 1 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    let ch = trimmed.chars().next().unwrap();
    let outcome = {
        let Ok(mut map) = state.hangman_games.lock() else {
            return false;
        };
        let Some(game) = map.get_mut(from) else {
            return false;
        };
        if game.started_at.elapsed() > Duration::from_secs(300) {
            let word = game.word.to_uppercase();
            map.remove(from);
            Some(format!("El ahorcado expiro. La palabra era *{}*.", word))
        } else if game.guessed.contains(&ch) {
            Some(format!("La letra *{}* ya fue usada.", ch.to_ascii_uppercase()))
        } else {
            game.guessed.push(ch);
            if !game.word.contains(ch) {
                game.wrong += 1;
            }
            let won = game.word.chars().all(|c| game.guessed.contains(&c));
            if won {
                let word = game.word.to_uppercase();
                map.remove(from);
                let u = state.db.ensure_user(sender, None);
                state.db.update_user_economy(sender, u.coins + 4_000, u.bank, &u.data_json);
                state.db.add_user_aura(sender, 40);
                Some(format!(
                    "*Ganaste el Ahorcado!*\nPalabra: *{}*\nPremio: +4,000 AuraCoins y +40 XP",
                    word
                ))
            } else if game.wrong >= 6 {
                let word = game.word.to_uppercase();
                let board = render_hangman_text(game);
                map.remove(from);
                Some(format!("{}\n\n*Perdiste!* La palabra era *{}*.", board, word))
            } else {
                Some(render_hangman_text(game))
            }
        }
    };

    if let Some(reply) = outcome {
        let opts = baileys_rs::utils::MessageGenerationOptions {
            user_jid: bot_jid.clone(),
            quoted: Some(Box::new(message.clone())),
            ..Default::default()
        };
        let _ = messages
            .send_message(from, baileys_rs::utils::AnyMessageContent::Text(reply), opts)
            .await;
        return true;
    }
    false
}

pub async fn run(ctx: &CommandContext) -> bool {
    let lang = resolve_lang(&ctx.user.user_lang, None);
    match ctx.cmd_name.as_str() {
        "ayudajuegos" | "juegos" | "games" => {
            let p = &ctx.used_prefix;
            ctx.reply_text(format!(
                "*Centro de Juegos — Aura Reed*\n\
                - `{p}ahorcado` [letra|palabra|stop] — Adivina la palabra\n\
                - `{p}ruletarusa` / `{p}rr` — Ruleta rusa (1/6 de expulsion en grupo)\n\
                - `{p}aviator <apuesta> <x>` — Retira antes de que explote (ej: 1.5 a 20)\n\
                - `{p}slots <apuesta>` — Tragamonedas\n\
                - `{p}dado [apuesta]` — Lanza el dado (4-6 ganas)"
            ))
            .await;
            true
        }
        "ahorcado" | "hangman" | "ah" => {
            let arg = ctx.text.trim().to_ascii_lowercase();
            let reply = {
                let Ok(mut map) = ctx.state.hangman_games.lock() else {
                    return true;
                };
                if map.len() > 64 {
                    map.retain(|_, g| g.started_at.elapsed() <= Duration::from_secs(300));
                }
                if matches!(arg.as_str(), "stop" | "rendirse" | "salir") {
                    if let Some(old) = map.remove(&ctx.from) {
                        format!(
                            "Partida terminada. La palabra era *{}*.",
                            old.word.to_uppercase()
                        )
                    } else {
                        "No hay ninguna partida de ahorcado activa en este chat.".to_string()
                    }
                } else if !arg.is_empty() && map.contains_key(&ctx.from) {
                    let game = map.get_mut(&ctx.from).unwrap();
                    if arg.chars().count() == 1 {
                        let ch = arg.chars().next().unwrap();
                        if !ch.is_ascii_alphabetic() {
                            "Ingresa una letra valida (a-z).".to_string()
                        } else if game.guessed.contains(&ch) {
                            format!("La letra *{}* ya fue probada.", ch.to_ascii_uppercase())
                        } else {
                            game.guessed.push(ch);
                            if !game.word.contains(ch) {
                                game.wrong += 1;
                            }
                            if game.word.chars().all(|c| game.guessed.contains(&c)) {
                                let w = game.word.to_uppercase();
                                map.remove(&ctx.from);
                                let u = ctx.state.db.ensure_user(&ctx.sender, None);
                                ctx.state.db.update_user_economy(
                                    &ctx.sender,
                                    u.coins + 4_000,
                                    u.bank,
                                    &u.data_json,
                                );
                                ctx.state.db.add_user_aura(&ctx.sender, 40);
                                format!(
                                    "*Ganaste!* Palabra: *{}*\n+4,000 AuraCoins | +40 XP",
                                    w
                                )
                            } else if game.wrong >= 6 {
                                let w = game.word.to_uppercase();
                                let board = render_hangman_text(game);
                                map.remove(&ctx.from);
                                format!("{}\n\n*Perdiste!* La palabra era *{}*.", board, w)
                            } else {
                                render_hangman_text(game)
                            }
                        }
                    } else if arg == game.word {
                        let w = game.word.to_uppercase();
                        map.remove(&ctx.from);
                        let u = ctx.state.db.ensure_user(&ctx.sender, None);
                        ctx.state.db.update_user_economy(
                            &ctx.sender,
                            u.coins + 5_000,
                            u.bank,
                            &u.data_json,
                        );
                        ctx.state.db.add_user_aura(&ctx.sender, 50);
                        format!(
                            "*Acertaste la palabra completa: {}!*\n+5,000 AuraCoins | +50 XP",
                            w
                        )
                    } else {
                        game.wrong += 1;
                        if game.wrong >= 6 {
                            let w = game.word.to_uppercase();
                            let board = render_hangman_text(game);
                            map.remove(&ctx.from);
                            format!("{}\n\n*Perdiste!* La palabra era *{}*.", board, w)
                        } else {
                            format!(
                                "No es *{}*.\n\n{}",
                                arg.to_uppercase(),
                                render_hangman_text(game)
                            )
                        }
                    }
                } else {
                    let word = HANGMAN_WORDS[(next_rand() as usize) % HANGMAN_WORDS.len()].to_string();
                    let game = HangmanGame {
                        word,
                        guessed: Vec::new(),
                        wrong: 0,
                        started_at: Instant::now(),
                    };
                    let txt = format!(
                        "{}\n\nEnvia una letra sola o usa `{}ah <letra|palabra>`.",
                        render_hangman_text(&game),
                        ctx.used_prefix
                    );
                    map.insert(ctx.from.clone(), game);
                    txt
                }
            };
            ctx.reply_text(reply).await;
            true
        }
        "ruletarusa" | "rr" => {
            if !ctx.is_group {
                ctx.reply_text(tr(&ctx.state.config.data_dir, lang, "group_only"))
                    .await;
                return true;
            }
            let (is_sender_admin, is_bot_admin) = ctx.admin_status().await;
            if !is_bot_admin {
                ctx.reply_text(tr(&ctx.state.config.data_dir, lang, "need_bot_admin"))
                    .await;
                return true;
            }
            if is_sender_admin {
                ctx.reply_text("Los administradores y owners son inmunes a la Ruleta Rusa.")
                    .await;
                return true;
            }
            let chamber = (next_rand() % 6) + 1;
            if chamber == 1 {
                ctx.reply_with_mentions(
                    format!(
                        "*BANG!* La bala estaba en la recamara. Adios @{}!",
                        jid_tag(&ctx.sender)
                    ),
                    vec![ctx.sender.clone()],
                )
                .await;
                if !ctx
                    .bot_jid
                    .as_deref()
                    .is_some_and(|b| same_bare_jid(b, &ctx.sender))
                {
                    let _ = ctx
                        .messages
                        .socket
                        .group_participants_update(
                            &ctx.from,
                            &[ctx.sender.clone()],
                            ParticipantAction::Remove,
                        )
                        .await;
                    ctx.invalidate_group_cache();
                }
            } else {
                let u = ctx.state.db.ensure_user(&ctx.sender, None);
                ctx.state.db.update_user_economy(
                    &ctx.sender,
                    u.coins + 2_000,
                    u.bank,
                    &u.data_json,
                );
                ctx.reply_with_mentions(
                    format!(
                        "*Click...* @{} sobrevivio a la Ruleta Rusa! (+2,000 AuraCoins)",
                        jid_tag(&ctx.sender)
                    ),
                    vec![ctx.sender.clone()],
                )
                .await;
            }
            true
        }
        "aviator" | "crash" => {
            let bet = ctx
                .args
                .first()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let target_x = ctx
                .args
                .get(1)
                .and_then(|s| s.replace('x', "").parse::<f64>().ok())
                .unwrap_or(0.0);
            if bet < 100 || !(1.1..=25.0).contains(&target_x) {
                ctx.reply_text(format!(
                    "Uso: `{}aviator <apuesta >= 100> <objetivo 1.1 - 25.0>`\nEjemplo: `{}aviator 2000 2.5`",
                    ctx.used_prefix, ctx.used_prefix
                ))
                .await;
                return true;
            }
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            if u.coins < bet {
                ctx.reply_text(format!(
                    "Fondos insuficientes. Bolsillo: *{} AuraCoins*",
                    u.coins
                ))
                .await;
                return true;
            }
            let r = ((next_rand() % 9900) + 1) as f64 / 10000.0;
            let crash_point = (0.97 / (1.0 - r)).clamp(1.0, 50.0);
            let crash_round = (crash_point * 100.0).floor() / 100.0;
            if crash_round >= target_x {
                let profit = ((bet as f64) * (target_x - 1.0)).round() as i64;
                let new_coins = u.coins + profit;
                ctx.state
                    .db
                    .update_user_economy(&ctx.sender, new_coins, u.bank, &u.data_json);
                ctx.reply_text(format!(
                    "*AVIATOR — Retiro Exitoso*\nObjetivo: *{:.2}x* | Explosion: *{:.2}x*\nGanancia: *+{} AuraCoins*\nSaldo: *{} AuraCoins*",
                    target_x, crash_round, profit, new_coins
                ))
                .await;
            } else {
                let new_coins = (u.coins - bet).max(0);
                ctx.state
                    .db
                    .update_user_economy(&ctx.sender, new_coins, u.bank, &u.data_json);
                ctx.reply_text(format!(
                    "*AVIATOR — Crash!*\nEl avion exploto en *{:.2}x* (buscabas {:.2}x).\nPerdiste: *-{} AuraCoins*\nSaldo: *{} AuraCoins*",
                    crash_round, target_x, bet, new_coins
                ))
                .await;
            }
            true
        }
        "slots" | "tragamonedas" => {
            let bet = ctx
                .args
                .first()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(500);
            if bet < 100 {
                ctx.reply_text(format!("Apuesta minima: 100. Uso: `{}slots <apuesta>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            if u.coins < bet {
                ctx.reply_text(format!("Fondos insuficientes. Bolsillo: *{} AuraCoins*", u.coins))
                    .await;
                return true;
            }
            let syms = ["7", "BAR", "CHERRY", "STAR", "DIAMOND", "BELL"];
            let r1 = syms[(next_rand() as usize) % syms.len()];
            let r2 = syms[(next_rand() as usize) % syms.len()];
            let r3 = syms[(next_rand() as usize) % syms.len()];
            let delta = if r1 == r2 && r2 == r3 {
                if r1 == "7" || r1 == "DIAMOND" {
                    bet * 5
                } else {
                    bet * 3
                }
            } else if r1 == r2 || r2 == r3 || r1 == r3 {
                bet / 2
            } else {
                -bet
            };
            let new_coins = (u.coins + delta).max(0);
            ctx.state
                .db
                .update_user_economy(&ctx.sender, new_coins, u.bank, &u.data_json);
            let res = if delta > 0 {
                format!("Ganaste *+{} AuraCoins*!", delta)
            } else {
                format!("Perdiste *-{} AuraCoins*.", bet)
            };
            ctx.reply_text(format!(
                "*SLOTS*\n[ {} | {} | {} ]\n{}\nSaldo: *{} AuraCoins*",
                r1, r2, r3, res, new_coins
            ))
            .await;
            true
        }
        "dado" | "dice" => {
            let bet = ctx
                .args
                .first()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let roll = (next_rand() % 6) + 1;
            if bet >= 100 {
                let u = ctx.state.db.ensure_user(&ctx.sender, None);
                if u.coins < bet {
                    ctx.reply_text(format!("Fondos insuficientes. Bolsillo: *{} AuraCoins*", u.coins))
                        .await;
                    return true;
                }
                let won = roll >= 4;
                let new_coins = if won { u.coins + bet } else { u.coins - bet };
                ctx.state
                    .db
                    .update_user_economy(&ctx.sender, new_coins, u.bank, &u.data_json);
                ctx.reply_text(format!(
                    "Salio *{}*. {}\nSaldo: *{} AuraCoins*",
                    roll,
                    if won {
                        format!("Ganaste *+{} AuraCoins*!", bet)
                    } else {
                        format!("Perdiste *-{} AuraCoins*.", bet)
                    },
                    new_coins
                ))
                .await;
            } else {
                ctx.reply_text(format!("Lanzaste el dado y salio: *{}*", roll))
                    .await;
            }
            true
        }
        _ => false,
    }
}
