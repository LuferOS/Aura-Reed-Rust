use crate::context::CommandContext;
use crate::db::UserRecord;
use baileys_rs::utils::unix_timestamp_seconds;
use serde_json::{Map, Value};
use std::sync::atomic::{AtomicU64, Ordering};

static RNG_STATE: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);

fn next_rand() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut x = RNG_STATE.fetch_add(now ^ 0x517c_c1b7_2722_0a95, Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn rand_range(min: i64, max: i64) -> i64 {
    if max <= min {
        return min;
    }
    let span = (max - min + 1) as u64;
    min + (next_rand() % span) as i64
}

fn rand_pct() -> u64 {
    next_rand() % 100
}

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

fn format_money(amount: i64) -> String {
    let neg = amount < 0;
    let s = amount.unsigned_abs().to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3 + 4);
    for (idx, ch) in s.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    let formatted: String = out.chars().rev().collect();
    if neg {
        format!("-{} AuraCoins", formatted)
    } else {
        format!("{} AuraCoins", formatted)
    }
}

fn format_cooldown(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if d > 0 {
        format!("{}d {}h {}m", d, h, m)
    } else if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn load_data_map(user: &UserRecord) -> Map<String, Value> {
    serde_json::from_str::<Value>(&user.data_json)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn get_ts(map: &Map<String, Value>, key: &str) -> u64 {
    map.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn set_ts(map: &mut Map<String, Value>, key: &str, val: u64) {
    map.insert(key.to_string(), Value::from(val));
}

fn save_user_eco(ctx: &CommandContext, jid: &str, coins: i64, bank: i64, map: &Map<String, Value>) {
    let json = serde_json::to_string(map).unwrap_or_else(|_| "{}".to_string());
    ctx.state.db.update_user_economy(jid, coins, bank, &json);
}

fn parse_amount(raw: &str, max_all: i64) -> i64 {
    let low = raw.trim().to_ascii_lowercase();
    if low == "all" || low == "todo" {
        return max_all;
    }
    low.parse::<i64>().unwrap_or(0)
}

struct ActivitySpec {
    key: &'static str,
    title: &'static str,
    cooldown_s: u64,
    reward_min: i64,
    reward_max: i64,
    xp_min: i64,
    xp_max: i64,
    success: &'static [&'static str],
    fail: &'static [&'static str],
}

async fn run_activity(ctx: &CommandContext, spec: ActivitySpec) {
    let user = ctx.state.db.ensure_user(&ctx.sender, None);
    let mut map = load_data_map(&user);
    let now = unix_timestamp_seconds();
    let last = get_ts(&map, spec.key);
    if last > 0 && now.saturating_sub(last) < spec.cooldown_s {
        let left = spec.cooldown_s - now.saturating_sub(last);
        ctx.reply_text(format!(
            "Estas cansado. Vuelve en *{}*.",
            format_cooldown(left)
        ))
        .await;
        return;
    }

    let xp = rand_range(spec.xp_min, spec.xp_max);
    let ok = rand_pct() >= 30;
    let reward = if ok {
        rand_range(spec.reward_min, spec.reward_max)
    } else {
        0
    };
    let new_coins = user.coins + reward;
    set_ts(&mut map, spec.key, now);
    save_user_eco(ctx, &ctx.sender, new_coins, user.bank, &map);
    ctx.state.db.add_user_aura(&ctx.sender, xp);

    let msg_line = if ok {
        let line = spec.success[(next_rand() as usize) % spec.success.len()];
        format!("{} *{}*", line, format_money(reward))
    } else {
        spec.fail[(next_rand() as usize) % spec.fail.len()].to_string()
    };

    ctx.reply_with_mentions(
        format!(
            "*{}*\n@{} | +{} XP\n{}\nSaldo: *{}*",
            spec.title,
            jid_tag(&ctx.sender),
            xp,
            msg_line,
            format_money(new_coins)
        ),
        vec![ctx.sender.clone()],
    )
    .await;
}

async fn run_streak_claim(
    ctx: &CommandContext,
    title: &str,
    ts_key: &str,
    streak_key: &str,
    cooldown_s: u64,
    grace_s: u64,
    base: i64,
    bonus_unit: i64,
) {
    let user = ctx.state.db.ensure_user(&ctx.sender, None);
    let mut map = load_data_map(&user);
    let now = unix_timestamp_seconds();
    let last = get_ts(&map, ts_key);
    if last > 0 && now.saturating_sub(last) < cooldown_s {
        let left = cooldown_s - now.saturating_sub(last);
        ctx.reply_text(format!(
            "Ya reclamaste *{}*. Vuelve en *{}*.",
            title,
            format_cooldown(left)
        ))
        .await;
        return;
    }

    let prev_streak = get_ts(&map, streak_key);
    let streak = if last > 0 && now.saturating_sub(last) > grace_s {
        1
    } else {
        prev_streak + 1
    };
    let bonus = if streak > 1 {
        let exp = (streak - 2).min(10) as u32;
        bonus_unit.saturating_mul(1i64 << exp)
    } else {
        0
    };
    let total = base + bonus;
    let new_coins = user.coins + total;
    set_ts(&mut map, ts_key, now);
    set_ts(&mut map, streak_key, streak);
    save_user_eco(ctx, &ctx.sender, new_coins, user.bank, &map);

    ctx.reply_with_mentions(
        format!(
            "*{}* (Racha: {})\n@{}\nBase: {}\nBono racha: {}\nTotal ganado: *{}*\nSaldo actual: *{}*",
            title,
            streak,
            jid_tag(&ctx.sender),
            format_money(base),
            format_money(bonus),
            format_money(total),
            format_money(new_coins)
        ),
        vec![ctx.sender.clone()],
    )
    .await;
}

pub async fn run(ctx: &CommandContext) -> bool {
    match ctx.cmd_name.as_str() {
        "balance" | "bal" | "bank" | "coins" => {
            let target = ctx.target_jid();
            let u = ctx.state.db.ensure_user(&target, None);
            let total = u.coins + u.bank;
            ctx.reply_with_mentions(
                format!(
                    "*Balance de @{}*\nBolsillo: *{}*\nBanco: *{}*\nTotal: *{}*",
                    jid_tag(&target),
                    format_money(u.coins),
                    format_money(u.bank),
                    format_money(total)
                ),
                vec![target],
            )
            .await;
            true
        }
        "baltop" | "topbal" | "lb" => {
            let top = ctx.state.db.top_users_by_balance(10);
            if top.is_empty() {
                ctx.reply_text("Aun no hay usuarios en el ranking.").await;
                return true;
            }
            let mut mentions = Vec::with_capacity(top.len());
            let mut lines = Vec::with_capacity(top.len());
            for (idx, u) in top.iter().enumerate() {
                mentions.push(u.jid.clone());
                lines.push(format!(
                    "{}. @{} — *{}*",
                    idx + 1,
                    jid_tag(&u.jid),
                    format_money(u.coins + u.bank)
                ));
            }
            ctx.reply_with_mentions(
                format!("*Top 10 Economia*\n{}", lines.join("\n")),
                mentions,
            )
            .await;
            true
        }
        "einfo" | "eco" | "economy" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let map = load_data_map(&u);
            let now = unix_timestamp_seconds();
            let checks = [
                ("Work", "lastWork", 180u64),
                ("Mine", "lastMine", 1800),
                ("Hunt", "lastHunt", 1800),
                ("Crime", "lastCrime", 3600),
                ("Slut", "lastSlut", 3600),
                ("Adventure", "lastAdventure", 7200),
                ("Steal", "lastSteal", 3600),
                ("Daily", "lastDaily", 86400),
                ("Weekly", "lastWeekly", 604800),
                ("Fortnightly", "lastFortnightly", 1296000),
                ("Monthly", "lastMonthly", 2592000),
            ];
            let mut lines = Vec::with_capacity(checks.len());
            for (label, key, cd) in checks {
                let last = get_ts(&map, key);
                let st = if last > 0 && now.saturating_sub(last) < cd {
                    format_cooldown(cd - now.saturating_sub(last))
                } else {
                    "Disponible".to_string()
                };
                lines.push(format!("- {}: {}", label, st));
            }
            ctx.reply_text(format!("*Estado de Economia*\n{}", lines.join("\n")))
                .await;
            true
        }
        "deposit" | "d" | "dep" | "depositar" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let raw = ctx.args.first().map(String::as_str).unwrap_or("");
            let amount = parse_amount(raw, u.coins);
            if amount <= 0 {
                ctx.reply_text(format!(
                    "Uso: `{}dep <cantidad|all>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            if u.coins < amount {
                ctx.reply_text(format!(
                    "No tienes suficientes monedas. Bolsillo: *{}*",
                    format_money(u.coins)
                ))
                .await;
                return true;
            }
            let map = load_data_map(&u);
            let new_coins = u.coins - amount;
            let new_bank = u.bank + amount;
            save_user_eco(ctx, &ctx.sender, new_coins, new_bank, &map);
            ctx.reply_text(format!(
                "Depositaste *{}*\nBanco: *{}* | Bolsillo: *{}*",
                format_money(amount),
                format_money(new_bank),
                format_money(new_coins)
            ))
            .await;
            true
        }
        "withdraw" | "with" | "retirar" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let raw = ctx.args.first().map(String::as_str).unwrap_or("");
            let amount = parse_amount(raw, u.bank);
            if amount <= 0 {
                ctx.reply_text(format!(
                    "Uso: `{}with <cantidad|all>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            if u.bank < amount {
                ctx.reply_text(format!(
                    "No tienes suficientes fondos. Banco: *{}*",
                    format_money(u.bank)
                ))
                .await;
                return true;
            }
            let map = load_data_map(&u);
            let new_coins = u.coins + amount;
            let new_bank = u.bank - amount;
            save_user_eco(ctx, &ctx.sender, new_coins, new_bank, &map);
            ctx.reply_text(format!(
                "Retiraste *{}*\nBolsillo: *{}* | Banco: *{}*",
                format_money(amount),
                format_money(new_coins),
                format_money(new_bank)
            ))
            .await;
            true
        }
        "pay" | "transfer" | "pagar" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("Menciona o responde a otro usuario para transferir.")
                    .await;
                return true;
            }
            let raw = ctx
                .args
                .iter()
                .find(|a| !a.starts_with('@'))
                .map(String::as_str)
                .unwrap_or("");
            let sender_u = ctx.state.db.ensure_user(&ctx.sender, None);
            let amount = parse_amount(raw, sender_u.coins);
            if amount <= 0 {
                ctx.reply_text(format!(
                    "Cantidad invalida. Uso: `{}pay <cantidad> @usuario`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            if sender_u.coins < amount {
                ctx.reply_text(format!(
                    "Fondos insuficientes. Bolsillo: *{}*",
                    format_money(sender_u.coins)
                ))
                .await;
                return true;
            }
            let receiver_u = ctx.state.db.ensure_user(&target, None);
            let s_map = load_data_map(&sender_u);
            let r_map = load_data_map(&receiver_u);
            save_user_eco(ctx, &ctx.sender, sender_u.coins - amount, sender_u.bank, &s_map);
            save_user_eco(ctx, &target, receiver_u.coins + amount, receiver_u.bank, &r_map);
            ctx.reply_with_mentions(
                format!(
                    "*Transferencia exitosa*\nDe: @{}\nPara: @{}\nMonto: *{}*",
                    jid_tag(&ctx.sender),
                    jid_tag(&target),
                    format_money(amount)
                ),
                vec![ctx.sender.clone(), target],
            )
            .await;
            true
        }
        "daily" | "diario" => {
            run_streak_claim(
                ctx,
                "Recompensa Diaria",
                "lastDaily",
                "dailyStreak",
                86_400,
                172_800,
                12_000,
                12_500,
            )
            .await;
            true
        }
        "weekly" | "semanal" | "memanal" => {
            run_streak_claim(
                ctx,
                "Recompensa Semanal",
                "lastWeekly",
                "weeklyStreak",
                604_800,
                950_400,
                100_000,
                12_500,
            )
            .await;
            true
        }
        "fortnightly" | "quincenal" | "quinsenal" | "quincena" => {
            run_streak_claim(
                ctx,
                "Recompensa Quincenal",
                "lastFortnightly",
                "fortnightlyStreak",
                1_296_000,
                1_900_800,
                200_000,
                25_000,
            )
            .await;
            true
        }
        "monthly" | "mensual" => {
            run_streak_claim(
                ctx,
                "Recompensa Mensual",
                "lastMonthly",
                "monthlyStreak",
                2_592_000,
                3_888_000,
                400_000,
                50_000,
            )
            .await;
            true
        }
        "work" | "trabajar" | "w" | "job" | "empleo" => {
            run_activity(
                ctx,
                ActivitySpec {
                    key: "lastWork",
                    title: "Jornada Laboral",
                    cooldown_s: 180,
                    reward_min: 5000,
                    reward_max: 7000,
                    xp_min: 10,
                    xp_max: 30,
                    success: &[
                        "Trabajaste como programador y ganaste",
                        "Vendiste limonada y ganaste",
                        "Hiciste trabajos freelance y cobraste",
                    ],
                    fail: &[
                        "Tu jornada fue agotadora, pero no encontraste clientes.",
                        "El turno termino sin propinas.",
                    ],
                },
            )
            .await;
            true
        }
        "mine" | "minar" | "chambear" => {
            run_activity(
                ctx,
                ActivitySpec {
                    key: "lastMine",
                    title: "Sistema de Mineria",
                    cooldown_s: 1800,
                    reward_min: 5000,
                    reward_max: 10000,
                    xp_min: 20,
                    xp_max: 50,
                    success: &[
                        "Encontraste oro en una veta profunda y lo vendiste por",
                        "Rompiste una geoda y encontraste gemas por",
                        "Descubriste un cofre antiguo con",
                    ],
                    fail: &[
                        "Se rompio tu pico y tuviste que volver al campamento.",
                        "Un derrumbe bloqueo la veta.",
                    ],
                },
            )
            .await;
            true
        }
        "hunt" | "cazar" | "caza" => {
            run_activity(
                ctx,
                ActivitySpec {
                    key: "lastHunt",
                    title: "Sistema de Combate",
                    cooldown_s: 1800,
                    reward_min: 5000,
                    reward_max: 10000,
                    xp_min: 20,
                    xp_max: 50,
                    success: &[
                        "Derrotaste a un Warden y el gremio te pago",
                        "Superaste la arena y cobraste",
                        "Cazaste una criatura legendaria y obtuviste",
                    ],
                    fail: &[
                        "Un Creeper aparecio detras de ti y tuviste que huir.",
                        "La criatura esquivo tu ataque y perdiste el rastro.",
                    ],
                },
            )
            .await;
            true
        }
        "crime" | "crimen" => {
            run_activity(
                ctx,
                ActivitySpec {
                    key: "lastCrime",
                    title: "Crimen",
                    cooldown_s: 3600,
                    reward_min: 2000,
                    reward_max: 10000,
                    xp_min: 10,
                    xp_max: 25,
                    success: &[
                        "Hackeaste una cuenta y escapaste con",
                        "Robaste una joyeria y huiste con",
                        "Vendiste informacion y ganaste",
                    ],
                    fail: &[
                        "La policia te atrapo y pagaste una fianza.",
                        "El golpe salio mal y perdiste la oportunidad.",
                    ],
                },
            )
            .await;
            true
        }
        "slut" | "putear" | "prost" => {
            run_activity(
                ctx,
                ActivitySpec {
                    key: "lastSlut",
                    title: "Trabajo Nocturno",
                    cooldown_s: 3600,
                    reward_min: 1000,
                    reward_max: 6000,
                    xp_min: 15,
                    xp_max: 40,
                    success: &[
                        "Bailaste en un club VIP y ganaste",
                        "Trabajaste en un evento privado y cobraste",
                        "Vendiste fotos y conseguiste",
                    ],
                    fail: &[
                        "El cliente huyo sin pagar.",
                        "La policia hizo una redada y tuviste que escapar.",
                    ],
                },
            )
            .await;
            true
        }
        "adventure" | "aventura" | "explorar" => {
            run_activity(
                ctx,
                ActivitySpec {
                    key: "lastAdventure",
                    title: "Mision de Aventura",
                    cooldown_s: 7200,
                    reward_min: 10000,
                    reward_max: 20000,
                    xp_min: 40,
                    xp_max: 80,
                    success: &[
                        "Completaste una mision en las Ruinas Olvidadas y ganaste",
                        "Derrotaste a un grupo de duendes y saqueaste",
                        "Protegiste una caravana y te pagaron",
                    ],
                    fail: &[
                        "Caiste en una trampa de la mazmorra y tuviste que retirarte.",
                        "El jefe de la zona te obligo a retirarte.",
                    ],
                },
            )
            .await;
            true
        }
        "cf" | "caraocruz" | "coinflip" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let amount = ctx
                .args
                .first()
                .map(|s| parse_amount(s, u.coins))
                .unwrap_or(0);
            let choice = ctx
                .args
                .get(1)
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if amount <= 0 || !matches!(choice.as_str(), "cara" | "cruz") {
                ctx.reply_text(format!(
                    "Uso: `{}cf <cantidad> <cara|cruz>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            if u.coins < amount {
                ctx.reply_text(format!(
                    "No tienes suficientes monedas. Bolsillo: *{}*",
                    format_money(u.coins)
                ))
                .await;
                return true;
            }
            let mut map = load_data_map(&u);
            let now = unix_timestamp_seconds();
            let last = get_ts(&map, "lastCf");
            if last > 0 && now.saturating_sub(last) < 60 {
                ctx.reply_text(format!(
                    "Espera *{}* para volver a apostar.",
                    format_cooldown(60 - now.saturating_sub(last))
                ))
                .await;
                return true;
            }
            let result = if rand_pct() < 50 { "cara" } else { "cruz" };
            let won = result == choice;
            let new_coins = if won { u.coins + amount } else { u.coins - amount };
            set_ts(&mut map, "lastCf", now);
            save_user_eco(ctx, &ctx.sender, new_coins, u.bank, &map);
            let outcome = if won {
                format!("Ganaste *{}*!", format_money(amount))
            } else {
                format!("Perdiste *{}*.", format_money(amount))
            };
            ctx.reply_text(format!(
                "Salio *{}*. {}\nSaldo: *{}*",
                result,
                outcome,
                format_money(new_coins)
            ))
            .await;
            true
        }
        "ruleta" | "roulette" | "rt" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let amount = ctx
                .args
                .first()
                .map(|s| parse_amount(s, u.coins))
                .unwrap_or(0);
            let color = ctx
                .args
                .get(1)
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if amount <= 0 || !matches!(color.as_str(), "red" | "black" | "green") {
                ctx.reply_text(format!(
                    "Uso: `{}ruleta <cantidad> <red|black|green>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            if u.coins < amount {
                ctx.reply_text(format!(
                    "Fondos insuficientes. Bolsillo: *{}*",
                    format_money(u.coins)
                ))
                .await;
                return true;
            }
            let mut map = load_data_map(&u);
            let now = unix_timestamp_seconds();
            let last = get_ts(&map, "lastRoulete");
            if last > 0 && now.saturating_sub(last) < 60 {
                ctx.reply_text(format!(
                    "Espera *{}* para girar de nuevo.",
                    format_cooldown(60 - now.saturating_sub(last))
                ))
                .await;
                return true;
            }
            let roll = rand_pct();
            let landed = if roll < 5 {
                "green"
            } else if roll < 52 {
                "red"
            } else {
                "black"
            };
            let delta = if landed == color {
                if landed == "green" {
                    amount * 5
                } else {
                    amount
                }
            } else {
                -amount
            };
            let new_coins = (u.coins + delta).max(0);
            set_ts(&mut map, "lastRoulete", now);
            save_user_eco(ctx, &ctx.sender, new_coins, u.bank, &map);
            let msg = if delta > 0 {
                format!("Salio *{}*! Ganaste *{}*.", landed, format_money(delta))
            } else {
                format!("Salio *{}*. Perdiste *{}*.", landed, format_money(amount))
            };
            ctx.reply_text(format!("{}\nSaldo: *{}*", msg, format_money(new_coins)))
                .await;
            true
        }
        "ppt" | "rps" | "juego" => {
            let choice = ctx
                .args
                .first()
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            if !matches!(choice.as_str(), "piedra" | "papel" | "tijera") {
                ctx.reply_text(format!(
                    "Uso: `{}ppt <piedra|papel|tijera>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let mut map = load_data_map(&u);
            let now = unix_timestamp_seconds();
            let last = get_ts(&map, "lastPpt");
            if last > 0 && now.saturating_sub(last) < 600 {
                ctx.reply_text(format!(
                    "Espera *{}* para jugar de nuevo.",
                    format_cooldown(600 - now.saturating_sub(last))
                ))
                .await;
                return true;
            }
            let opts = ["piedra", "papel", "tijera"];
            let bot = opts[(next_rand() as usize) % 3];
            let reward = if choice == bot {
                500
            } else if (choice == "piedra" && bot == "tijera")
                || (choice == "papel" && bot == "piedra")
                || (choice == "tijera" && bot == "papel")
            {
                2500
            } else {
                0
            };
            let new_coins = u.coins + reward;
            set_ts(&mut map, "lastPpt", now);
            save_user_eco(ctx, &ctx.sender, new_coins, u.bank, &map);
            let res = if choice == bot {
                format!("Empate! Recibes *{}*.", format_money(reward))
            } else if reward > 0 {
                format!("Ganaste *{}*!", format_money(reward))
            } else {
                "Perdiste esta ronda.".to_string()
            };
            ctx.reply_text(format!(
                "Tu: *{}* vs Bot: *{}*\n{}\nSaldo: *{}*",
                choice,
                bot,
                res,
                format_money(new_coins)
            ))
            .await;
            true
        }
        "steal" | "robar" => {
            if !ctx.is_group {
                ctx.reply_text("Este comando solo funciona en grupos.")
                    .await;
                return true;
            }
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text("No puedes robarte a ti mismo.").await;
                return true;
            }
            let thief = ctx.state.db.ensure_user(&ctx.sender, None);
            let victim = ctx.state.db.ensure_user(&target, None);
            let mut t_map = load_data_map(&thief);
            let now = unix_timestamp_seconds();
            let last = get_ts(&t_map, "lastSteal");
            if last > 0 && now.saturating_sub(last) < 3600 {
                ctx.reply_text(format!(
                    "Espera *{}* para volver a intentar un robo.",
                    format_cooldown(3600 - now.saturating_sub(last))
                ))
                .await;
                return true;
            }
            if victim.coins < 500 {
                ctx.reply_text("La cartera de este usuario tiene menos de 500 AuraCoins.")
                    .await;
                return true;
            }
            set_ts(&mut t_map, "lastSteal", now);
            if rand_pct() < 40 {
                let max_steal = (victim.coins / 4).max(200).min(15_000);
                let stolen = rand_range(200, max_steal);
                let v_map = load_data_map(&victim);
                save_user_eco(ctx, &ctx.sender, thief.coins + stolen, thief.bank, &t_map);
                save_user_eco(ctx, &target, victim.coins - stolen, victim.bank, &v_map);
                ctx.reply_with_mentions(
                    format!(
                        "@{} le robo *{}* a @{}!",
                        jid_tag(&ctx.sender),
                        format_money(stolen),
                        jid_tag(&target)
                    ),
                    vec![ctx.sender.clone(), target],
                )
                .await;
            } else {
                let fine = rand_range(300, 1500).min(thief.coins);
                save_user_eco(ctx, &ctx.sender, thief.coins - fine, thief.bank, &t_map);
                ctx.reply_with_mentions(
                    format!(
                        "@{} fallo el robo y pago una multa de *{}*.",
                        jid_tag(&ctx.sender),
                        format_money(fine)
                    ),
                    vec![ctx.sender.clone()],
                )
                .await;
            }
            true
        }
        _ => false,
    }
}
