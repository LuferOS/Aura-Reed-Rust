use crate::context::CommandContext;
use baileys_rs::utils::unix_timestamp_seconds;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

const IMG_BASE: &str = "https://ik.imagekit.io/Aurareedimages/Gacha%20Images/";
const CLAIM_TTL_SECS: u64 = 120;

struct GachaChar {
    id: &'static str,
    rarity: &'static str,
    char_type: &'static str,
    source: &'static str,
    value: i64,
    first_img_id: i64,
    folder: &'static str,
    files: &'static [&'static str],
}

const CATALOG: &[GachaChar] = &[
    GachaChar { id: "Mumen Rider", rarity: "Especial", char_type: "Defensor", source: "One Punch Man", value: 1000, first_img_id: 31, folder: "Mumen%20Rider/Mumen%20Rider", files: &["1.jpeg", "2.jpeg", "3.jpeg", "4.jpeg", "5.jpeg", "6.jpeg", "7.jpeg", "8.jpeg", "9.jpeg", "10.jpeg"] },
    GachaChar { id: "Inosuke Hashibira", rarity: "Especial", char_type: "Defensor", source: "Demon Slayer", value: 1000, first_img_id: 81, folder: "Inosuke%20Hashibira/Inosuke%20Hashibira", files: &["1.jpeg", "2.jpg", "3.jpg", "4.jpg", "5.jpg", "6.jpg", "7.jpeg", "8.jpg", "9.jpeg", "10.jpeg"] },
    GachaChar { id: "Usopp", rarity: "Especial", char_type: "Soporte", source: "One Piece", value: 1000, first_img_id: 91, folder: "Usopp/Usopp", files: &["1.webp", "2.jpeg", "3.avif", "4.jpg", "5.jpeg", "6.png", "7.jpeg", "8.jpg", "9.jpeg", "10.webp"] },
    GachaChar { id: "Krilin", rarity: "Especial", char_type: "Soporte", source: "Dragon Ball", value: 1000, first_img_id: 101, folder: "Krilin/Krilin", files: &["1.avif", "2.jpeg", "3.jpg", "4.avif", "5.jpeg", "6.jpeg", "7.png", "8.webp", "9.webp", "10.png"] },
    GachaChar { id: "Eijiro Kirishima", rarity: "Super especial", char_type: "Defensor", source: "My Hero Academia", value: 2500, first_img_id: 41, folder: "Eijiro%20Kirishima/Eijiro%20Kirishima", files: &["1.jpeg", "2.jpg", "3.jpeg", "4.jpg", "5.jpeg", "6.jpeg", "7.jpg", "8.jpeg", "9.jpeg", "10.webp"] },
    GachaChar { id: "Piccolo", rarity: "Super especial", char_type: "Defensor", source: "Dragon Ball", value: 2500, first_img_id: 111, folder: "Picolo/Piccolo", files: &["1.jpeg", "2.jpeg", "3.jpeg", "4.jpeg", "5.jpg", "6.jpeg", "7.jpeg", "8.jpeg", "9.webp", "10.jpeg"] },
    GachaChar { id: "Rock Lee", rarity: "Super especial", char_type: "Atacante", source: "Naruto", value: 2500, first_img_id: 121, folder: "Rock%20Lee", files: &["1.webp", "2.jpeg", "3.png", "4.jpeg", "5.jpeg", "6.jpeg", "7.jpeg", "8.jpeg", "9.jpeg", "10.jpeg"] },
    GachaChar { id: "Ichika Nakano", rarity: "Epico", char_type: "Asesino", source: "Gotoubun no Hanayome", value: 4000, first_img_id: 1, folder: "Ichika%20Nakano/Ichika%20Nakano", files: &["1.jpeg", "2.jpeg", "3.jpeg", "4.jpg", "5.jpg", "6.jpeg", "7.jpg", "8.jpeg", "9.jpeg", "10.jpg"] },
    GachaChar { id: "Yotsuba Nakano", rarity: "Epico", char_type: "Soporte", source: "Gotoubun no Hanayome", value: 4000, first_img_id: 51, folder: "Yotsuba%20Nakano/Yotsuba%20Nakano", files: &["1.jpeg", "2.jpeg", "3.jpg", "4.jpeg", "5.avif", "6.webp", "7.webp", "8.jpg", "9.jpeg", "10.jpg"] },
    GachaChar { id: "Itsuki Nakano", rarity: "Epico", char_type: "Defensor", source: "Gotoubun no Hanayome", value: 4000, first_img_id: 61, folder: "Itsuki%20Nakano/Itsuki%20Nakano", files: &["1.jpeg", "2.jpeg", "3.webp", "4.webp", "5.jpg", "6.jpg", "7.jpeg", "8.jpeg", "9.jpeg", "10.jpeg"] },
    GachaChar { id: "Ochaco Uraraka", rarity: "Epico", char_type: "Asesina", source: "My Hero Academia", value: 4000, first_img_id: 71, folder: "Ochaco%20Uraraka/Ochaco%20Uraraka", files: &["1.jpg", "2.jpeg", "3.jpeg", "4.jpg", "5.jpeg", "6.jpeg", "7.jpeg", "8.jpeg", "9.jpeg", "10.jpeg"] },
    GachaChar { id: "Miku Nakano", rarity: "Mitico", char_type: "Support", source: "Gotoubun no Hanayome", value: 4500, first_img_id: 21, folder: "Miku%20Nacano", files: &["images.jpeg", "images%20(3).jpeg", "images%20(2).jpeg", "images%20(7).jpeg", "images%20(5).jpeg", "images%20(1).jpeg", "images%20(4).jpeg", "images%20(8).jpeg", "images%20(9).jpeg", "images%20(6).jpeg"] },
    GachaChar { id: "Katsuki Bakugo", rarity: "Mitico", char_type: "Atacante", source: "My Hero Academia", value: 4500, first_img_id: 151, folder: "Katsuki%20Bakugo/Katsuki%20Bakugo", files: &["1.webp", "2.jpeg", "3.jpeg", "4.jpeg", "5.jpeg", "6.jpeg", "7.jpeg", "8.jpeg", "9.jpeg", "10.jpeg"] },
    GachaChar { id: "Shoto Todoroki", rarity: "Mitico", char_type: "Atacante", source: "My Hero Academia", value: 4500, first_img_id: 161, folder: "Shoto%20Todoroki/Shoto%20Todoroki", files: &["1.jpg", "2.jpeg", "3.jpeg", "4.jpeg", "5.jpeg", "6.png", "7.jpeg", "8.jpeg", "9.avif", "10.jpg"] },
    GachaChar { id: "Nino Nakano", rarity: "Legendario", char_type: "Balance", source: "Gotoubun no Hanayome", value: 8000, first_img_id: 11, folder: "Nino%20Nakano/Nino%20Nakano", files: &["1.jpg", "2.jpg", "3.webp", "4.png", "5.jpg", "6.jpeg", "7.jpeg", "8.jpg", "9.jpg", "10.webp"] },
    GachaChar { id: "Izuku Midoriya", rarity: "Legendario", char_type: "Balance", source: "My Hero Academia", value: 8000, first_img_id: 131, folder: "Izuku%20Mirodiya/Izuku%20Mirodiya", files: &["1.jpg", "2.jpeg", "3.jpeg", "4.jpeg", "5.jpg", "6.jpg", "6.jpg", "8.jpg", "9.jpg", "10.webp"] },
    GachaChar { id: "All Might", rarity: "Legendario", char_type: "Defensor", source: "My Hero Academia", value: 8000, first_img_id: 141, folder: "All%20Might/All%20Might", files: &["1.webp", "2.jpeg", "3.jpeg", "4.jpeg", "5.jpeg", "6.webp", "7.jpg", "8.jpeg", "9.jpeg", "10.jpeg"] },
];

static GACHA_RNG: AtomicU64 = AtomicU64::new(0xa076_1d64_78bd_642f);

fn next_rand() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut x = GACHA_RNG.fetch_add(now ^ 0xe703_7ed1_a0b4_28db, Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn jid_tag(jid: &str) -> &str {
    jid.split('@').next().unwrap_or(jid)
}

fn rarity_weight(r: &str) -> u64 {
    match r {
        "Especial" => 50,
        "Super especial" => 25,
        "Epico" => 15,
        "Mitico" => 8,
        "Legendario" => 2,
        _ => 10,
    }
}

fn find_char(query: &str) -> Option<&'static GachaChar> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return None;
    }
    CATALOG
        .iter()
        .find(|c| c.id.to_ascii_lowercase() == q)
        .or_else(|| CATALOG.iter().find(|c| c.id.to_ascii_lowercase().contains(&q)))
}

fn char_image_url(ch: &GachaChar, image_id: i64) -> String {
    let idx = (image_id - ch.first_img_id).clamp(0, (ch.files.len() - 1) as i64) as usize;
    format!("{}{}/{}", IMG_BASE, ch.folder, ch.files[idx])
}

fn parse_char_and_variant(raw: &str) -> (String, Option<i64>) {
    if let Some((left, right)) = raw.rsplit_once('#') {
        if let Ok(id) = right.trim().parse::<i64>() {
            return (left.trim().to_string(), Some(id));
        }
    }
    (raw.trim().to_string(), None)
}

pub async fn run(ctx: &CommandContext) -> bool {
    let is_gacha_cmd = matches!(
        ctx.cmd_name.as_str(),
        "rw" | "roll"
            | "randomdraw"
            | "random"
            | "gacha"
            | "rollwaifu"
            | "claim"
            | "c"
            | "harem"
            | "miswaifus"
            | "claims"
            | "coleccion"
            | "winfo"
            | "charinfo"
            | "cinfo"
            | "wimage"
            | "charimage"
            | "cimage"
            | "animelist"
            | "slist"
            | "serielist"
            | "animeinfo"
            | "ainfo"
            | "serieinfo"
            | "ginfo"
            | "infogacha"
            | "gachainfo"
            | "vote"
            | "votar"
            | "waifusboard"
            | "topwaifus"
            | "waifustop"
            | "htop"
            | "haremtop"
            | "topharem"
            | "gtop"
            | "gachatop"
            | "sell"
            | "vender"
            | "tiendawaifus"
            | "wshop"
            | "haremshop"
            | "buyc"
            | "buycharacter"
            | "buychar"
            | "removerventa"
            | "removesale"
            | "givechar"
            | "regalar"
            | "givewaifu"
            | "deletechar"
            | "delwaifu"
            | "delchar"
            | "setfav"
            | "wfav"
            | "delfav"
            | "gachacheck"
            | "gcheck"
            | "giveallharem"
            | "trade"
            | "cambiar"
            | "accepttrade"
            | "aceptarintercambio"
            | "setclaim"
            | "setclaimmsg"
            | "delclaimmsg"
            | "robwaifu"
            | "robarwaifu"
    );
    if !is_gacha_cmd {
        return false;
    }

    if !ctx.is_group
        && !matches!(
            ctx.cmd_name.as_str(),
            "winfo" | "charinfo" | "cinfo" | "wimage" | "charimage" | "cimage" | "animelist" | "slist" | "serielist" | "animeinfo" | "ainfo" | "serieinfo"
        )
    {
        ctx.reply_text("El sistema Gacha solo funciona en grupos.")
            .await;
        return true;
    }

    match ctx.cmd_name.as_str() {
        "rw" | "roll" | "randomdraw" | "random" | "gacha" | "rollwaifu" => {
            let claimed: HashSet<(String, i64)> = ctx
                .state
                .db
                .get_group_claimed_pairs(&ctx.from)
                .into_iter()
                .collect();

            let mut available: Vec<(&'static GachaChar, Vec<i64>)> = Vec::new();
            for ch in CATALOG {
                let free_imgs: Vec<i64> = (0..ch.files.len() as i64)
                    .map(|off| ch.first_img_id + off)
                    .filter(|img_id| !claimed.contains(&(ch.id.to_string(), *img_id)))
                    .collect();
                if !free_imgs.is_empty() {
                    available.push((ch, free_imgs));
                }
            }
            if available.is_empty() {
                ctx.reply_text("Todas las cartas del catalogo ya fueron reclamadas en este grupo.")
                    .await;
                return true;
            }

            let total_w: u64 = available.iter().map(|(c, _)| rarity_weight(c.rarity)).sum();
            let mut pick = next_rand() % total_w;
            let mut chosen = &available[0];
            for item in &available {
                let w = rarity_weight(item.0.rarity);
                if pick < w {
                    chosen = item;
                    break;
                }
                pick -= w;
            }
            let ch = chosen.0;
            let img_id = chosen.1[(next_rand() as usize) % chosen.1.len()];
            let votes = ctx
                .state
                .db
                .get_group_vote_counts(&ctx.from)
                .get(ch.id)
                .copied()
                .unwrap_or(0);
            let value = ch.value + votes * 100;
            let url = char_image_url(ch, img_id);

            let caption = format!(
                "*{}* (#{})\n\
                Serie: {}\n\
                Rareza: *{}* | Tipo: {}\n\
                Valor: *{} AuraCoins* (Votos: {})\n\
                Estado: Libre\n\n\
                Responde a esta carta con `{}c` o escribe `{}c {} #{}` ({}s)",
                ch.id,
                img_id,
                ch.source,
                ch.rarity,
                ch.char_type,
                value,
                votes,
                ctx.used_prefix,
                ctx.used_prefix,
                ch.id,
                img_id,
                CLAIM_TTL_SECS
            );

            let mut sent_id: Option<String> = None;
            if let Ok(resp) = ctx.state.http.get(&url).send().await {
                if resp.status().is_success() {
                    let mime = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("image/jpeg")
                        .to_string();
                    if let Ok(bytes) = resp.bytes().await {
                        if let Ok(id_opt) = ctx
                            .send_image_bytes_returning_id(
                                bytes.to_vec(),
                                Some(caption.clone()),
                                Vec::new(),
                                &mime,
                            )
                            .await
                        {
                            sent_id = id_opt;
                        }
                    }
                }
            }
            if sent_id.is_none() {
                ctx.reply_text(caption).await;
            }
            let now = unix_timestamp_seconds();
            let draw_key = sent_id.unwrap_or_else(|| format!("draw_{}_{}", now, img_id));
            ctx.state.db.save_gacha_draw(
                &ctx.from,
                &draw_key,
                &ctx.sender,
                ch.id,
                img_id,
                now,
                CLAIM_TTL_SECS,
            );
            true
        }
        "claim" | "c" => {
            let now = unix_timestamp_seconds();
            let draw = if let Some(q_id) = ctx.quoted_stanza_id() {
                ctx.state.db.find_gacha_draw_by_msg(&ctx.from, &q_id)
            } else if !ctx.text.trim().is_empty() {
                let (name_q, variant_id) = parse_char_and_variant(&ctx.text);
                find_char(&name_q).and_then(|ch| {
                    ctx.state
                        .db
                        .find_pending_gacha_draw_by_char(&ctx.from, ch.id, variant_id, now)
                })
            } else {
                None
            };

            let Some(draw) = draw else {
                ctx.reply_text(format!(
                    "Responde a una carta activa o usa `{}c <nombre>`.",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            if draw.claimed_by.is_some() {
                ctx.reply_text("Esa carta ya fue reclamada.").await;
                return true;
            }
            if now > draw.expires_at {
                ctx.reply_text("El tiempo para reclamar esa carta expiro.")
                    .await;
                return true;
            }
            if draw.drawer_jid != ctx.sender && now.saturating_sub(draw.expires_at.saturating_sub(CLAIM_TTL_SECS)) < 15 {
                ctx.reply_text("Espera 15s de proteccion para el invocador antes de reclamar su carta.")
                    .await;
                return true;
            }
            if ctx.state.db.claim_gacha_card(
                &ctx.from,
                &draw.message_id,
                &draw.character_id,
                draw.image_id,
                &ctx.sender,
                now,
            ) {
                ctx.reply_with_mentions(
                    format!(
                        "@{} reclamo a *{}* (#{})!",
                        jid_tag(&ctx.sender),
                        draw.character_id,
                        draw.image_id
                    ),
                    vec![ctx.sender.clone()],
                )
                .await;
            } else {
                ctx.reply_text("Esta carta ya pertenece a otro usuario.")
                    .await;
            }
            true
        }
        "harem" | "miswaifus" | "claims" | "coleccion" => {
            let page = ctx
                .args
                .iter()
                .find_map(|a| a.parse::<usize>().ok())
                .filter(|&p| p >= 1)
                .unwrap_or(1);
            let target = ctx.target_jid();
            let entries = ctx.state.db.get_user_harem(&ctx.from, &target);
            if entries.is_empty() {
                ctx.reply_with_mentions(
                    format!("@{} aun no tiene cartas en este grupo.", jid_tag(&target)),
                    vec![target],
                )
                .await;
                return true;
            }
            let votes = ctx.state.db.get_group_vote_counts(&ctx.from);
            let fav = ctx.state.db.get_gacha_favorite(&ctx.from, &target);
            let total_pages = entries.len().div_ceil(10);
            let page = page.min(total_pages);
            let start = (page - 1) * 10;
            let total_val: i64 = entries
                .iter()
                .map(|e| {
                    let base = find_char(&e.character_id).map(|c| c.value).unwrap_or(1000);
                    base + votes.get(&e.character_id).copied().unwrap_or(0) * 100
                })
                .sum();
            let lines = entries
                .iter()
                .skip(start)
                .take(10)
                .enumerate()
                .map(|(i, e)| {
                    let ch = find_char(&e.character_id);
                    let val = ch.map(|c| c.value).unwrap_or(1000)
                        + votes.get(&e.character_id).copied().unwrap_or(0) * 100;
                    let star = if fav.as_ref().is_some_and(|(cid, iid)| cid == &e.character_id && *iid == e.image_id) {
                        " [FAV]"
                    } else {
                        ""
                    };
                    format!(
                        "{}. *{}* (#{}){} — {} ({} AC)",
                        start + i + 1,
                        e.character_id,
                        e.image_id,
                        star,
                        ch.map(|c| c.rarity).unwrap_or("-"),
                        val
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_with_mentions(
                format!(
                    "*Harem de @{}* (Pag {}/{})\nCartas: {} | Valor total: *{} AC*\n\n{}",
                    jid_tag(&target),
                    page,
                    total_pages,
                    entries.len(),
                    total_val,
                    lines
                ),
                vec![target],
            )
            .await;
            true
        }
        "winfo" | "charinfo" | "cinfo" => {
            let Some(ch) = find_char(&ctx.text) else {
                ctx.reply_text(format!("Personaje no encontrado. Uso: `{}winfo <nombre>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let votes = if ctx.is_group {
                ctx.state
                    .db
                    .get_group_vote_counts(&ctx.from)
                    .get(ch.id)
                    .copied()
                    .unwrap_or(0)
            } else {
                0
            };
            let claimed_count = if ctx.is_group {
                ctx.state
                    .db
                    .get_group_claimed_pairs(&ctx.from)
                    .iter()
                    .filter(|(cid, _)| cid == ch.id)
                    .count()
            } else {
                0
            };
            ctx.reply_text(format!(
                "*{}*\nSerie: {}\nRareza: *{}* | Tipo: {}\nValor actual: *{} AC* (Votos: {})\nVariantes: {}/{} reclamadas (IDs #{}..#{})",
                ch.id,
                ch.source,
                ch.rarity,
                ch.char_type,
                ch.value + votes * 100,
                votes,
                claimed_count,
                ch.files.len(),
                ch.first_img_id,
                ch.first_img_id + ch.files.len() as i64 - 1
            ))
            .await;
            true
        }
        "wimage" | "charimage" | "cimage" => {
            let (name_q, variant) = parse_char_and_variant(&ctx.text);
            let Some(ch) = find_char(&name_q) else {
                ctx.reply_text(format!("Uso: `{}wimage <personaje>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let img_id = variant.unwrap_or_else(|| {
                ch.first_img_id + ((next_rand() as usize) % ch.files.len()) as i64
            });
            let url = char_image_url(ch, img_id);
            match ctx.state.http.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let mime = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("image/jpeg")
                        .to_string();
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = ctx
                            .send_image_bytes(
                                bytes.to_vec(),
                                Some(format!("*{}* (#{}) — {}", ch.id, img_id, ch.source)),
                                Vec::new(),
                                &mime,
                            )
                            .await;
                    }
                }
                _ => ctx.reply_text(format!("Imagen de *{}*: {}", ch.id, url)).await,
            }
            true
        }
        "animelist" | "slist" | "serielist" => {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for ch in CATALOG {
                *counts.entry(ch.source).or_insert(0) += 1;
            }
            let mut items: Vec<(&str, usize)> = counts.into_iter().collect();
            items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            let lines = items
                .iter()
                .enumerate()
                .map(|(i, (s, n))| format!("{}. *{}* ({} personajes)", i + 1, s, n))
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_text(format!("*Series del Catalogo Gacha*\n\n{}", lines))
                .await;
            true
        }
        "animeinfo" | "ainfo" | "serieinfo" => {
            let q = ctx.text.trim().to_ascii_lowercase();
            if q.is_empty() {
                ctx.reply_text(format!("Uso: `{}animeinfo <serie>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let chars: Vec<&GachaChar> = CATALOG
                .iter()
                .filter(|c| c.source.to_ascii_lowercase().contains(&q))
                .collect();
            if chars.is_empty() {
                ctx.reply_text("No se encontro esa serie en el catalogo.")
                    .await;
                return true;
            }
            let title = chars[0].source;
            let lines = chars
                .iter()
                .map(|c| format!("- *{}* ({}, {} AC)", c.id, c.rarity, c.value))
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_text(format!("*{}* ({} personajes)\n\n{}", title, chars.len(), lines))
                .await;
            true
        }
        "ginfo" | "infogacha" | "gachainfo" => {
            let total_cards: usize = CATALOG.iter().map(|c| c.files.len()).sum();
            let claimed = ctx.state.db.get_group_claimed_pairs(&ctx.from).len();
            let market = ctx.state.db.list_gacha_market(&ctx.from).len();
            ctx.reply_text(format!(
                "*Estado Gacha del Grupo*\nPersonajes: {}\nCartas reclamadas: *{}/{}*\nEn tienda: *{}*",
                CATALOG.len(),
                claimed,
                total_cards,
                market
            ))
            .await;
            true
        }
        "vote" | "votar" => {
            let Some(ch) = find_char(&ctx.text) else {
                ctx.reply_text(format!("Uso: `{}vote <personaje>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let now = unix_timestamp_seconds();
            match ctx.state.db.add_gacha_vote(&ctx.from, ch.id, &ctx.sender, now) {
                Ok(total) => {
                    ctx.reply_text(format!(
                        "Voto registrado por *{}*.\nVotos: *{}* | Nuevo valor: *{} AC*",
                        ch.id,
                        total,
                        ch.value + total * 100
                    ))
                    .await;
                }
                Err(wait) => {
                    ctx.reply_text(format!(
                        "Ya votaste por *{}*. Vuelve en {}h {}m.",
                        ch.id,
                        wait / 3600,
                        (wait % 3600) / 60
                    ))
                    .await;
                }
            }
            true
        }
        "waifusboard" | "topwaifus" | "waifustop" => {
            let votes = ctx.state.db.get_group_vote_counts(&ctx.from);
            let mut rows: Vec<(&GachaChar, i64, i64)> = CATALOG
                .iter()
                .map(|c| {
                    let v = votes.get(c.id).copied().unwrap_or(0);
                    (c, v, c.value + v * 100)
                })
                .collect();
            rows.sort_by(|a, b| b.2.cmp(&a.2));
            let lines = rows
                .iter()
                .take(10)
                .enumerate()
                .map(|(i, (c, v, val))| {
                    format!("{}. *{}* ({}) — *{} AC* ({} votos)", i + 1, c.id, c.rarity, val, v)
                })
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_text(format!("*Top 10 Personajes*\n\n{}", lines))
                .await;
            true
        }
        "htop" | "haremtop" | "topharem" | "gtop" | "gachatop" => {
            let all = ctx.state.db.get_group_harem_all(&ctx.from);
            if all.is_empty() {
                ctx.reply_text("Aun nadie tiene cartas en este grupo.").await;
                return true;
            }
            let votes = ctx.state.db.get_group_vote_counts(&ctx.from);
            let mut by_user: HashMap<String, (usize, i64)> = HashMap::new();
            for e in all {
                let base = find_char(&e.character_id).map(|c| c.value).unwrap_or(1000);
                let val = base + votes.get(&e.character_id).copied().unwrap_or(0) * 100;
                let entry = by_user.entry(e.owner_jid).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += val;
            }
            let mut rank: Vec<(String, usize, i64)> = by_user
                .into_iter()
                .map(|(u, (cnt, val))| (u, cnt, val))
                .collect();
            rank.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));
            let top = rank.into_iter().take(10).collect::<Vec<_>>();
            let mentions: Vec<String> = top.iter().map(|(u, _, _)| u.clone()).collect();
            let lines = top
                .iter()
                .enumerate()
                .map(|(i, (u, cnt, val))| {
                    format!("{}. @{} — *{} cartas* ({} AC)", i + 1, jid_tag(u), cnt, val)
                })
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_with_mentions(format!("*Top Coleccionistas*\n\n{}", lines), mentions)
                .await;
            true
        }
        "sell" | "vender" => {
            let Some(price_str) = ctx.args.last() else {
                ctx.reply_text(format!(
                    "Uso: `{}sell <personaje [#id]> <precio>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            let Ok(price) = price_str.parse::<i64>() else {
                ctx.reply_text(format!(
                    "Indica un precio valido al final. Ejemplo: `{}sell Nino Nakano 15000`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            if price < 100 || ctx.args.len() < 2 {
                ctx.reply_text("El precio minimo es 100 AuraCoins.").await;
                return true;
            }
            let char_part = ctx.args[..ctx.args.len() - 1].join(" ");
            let (name_q, variant) = parse_char_and_variant(&char_part);
            let Some(ch) = find_char(&name_q) else {
                ctx.reply_text("Personaje no encontrado.").await;
                return true;
            };
            let mine = ctx.state.db.get_user_harem(&ctx.from, &ctx.sender);
            let owned = mine.into_iter().find(|e| {
                e.character_id == ch.id && variant.is_none_or(|v| v == e.image_id)
            });
            let Some(card) = owned else {
                ctx.reply_text("No posees esa carta en tu harem.").await;
                return true;
            };
            let now = unix_timestamp_seconds();
            match ctx.state.db.add_gacha_market(
                &ctx.from,
                &ctx.sender,
                &card.character_id,
                card.image_id,
                price,
                now,
            ) {
                Some(id) => {
                    ctx.reply_text(format!(
                        "Carta publicada en tienda (ID *{}*): *{}* (#{}) por *{} AC*.",
                        id, card.character_id, card.image_id, price
                    ))
                    .await;
                }
                None => {
                    ctx.reply_text("Esa carta ya esta publicada en la tienda.")
                        .await;
                }
            }
            true
        }
        "tiendawaifus" | "wshop" | "haremshop" => {
            let items = ctx.state.db.list_gacha_market(&ctx.from);
            if items.is_empty() {
                ctx.reply_text("La tienda de cartas del grupo esta vacia.")
                    .await;
                return true;
            }
            let mentions: Vec<String> = items.iter().map(|i| i.seller_jid.clone()).collect();
            let lines = items
                .iter()
                .map(|i| {
                    format!(
                        "- ID *{}* | *{}* (#{}) — *{} AC* (de @{})",
                        i.id,
                        i.character_id,
                        i.image_id,
                        i.price,
                        jid_tag(&i.seller_jid)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            ctx.reply_with_mentions(
                format!(
                    "*Tienda Gacha*\n{}\n\nUsa `{}buyc <ID>` para comprar.",
                    lines, ctx.used_prefix
                ),
                mentions,
            )
            .await;
            true
        }
        "buyc" | "buycharacter" | "buychar" => {
            let Some(sale_id) = ctx.args.first().and_then(|s| s.parse::<i64>().ok()) else {
                ctx.reply_text(format!("Uso: `{}buyc <ID de venta>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let Some(item) = ctx.state.db.get_gacha_market_item(&ctx.from, sale_id) else {
                ctx.reply_text("Esa venta no existe o ya fue cerrada.").await;
                return true;
            };
            if item.seller_jid == ctx.sender {
                ctx.reply_text("No puedes comprar tu propia carta.").await;
                return true;
            }
            let buyer = ctx.state.db.ensure_user(&ctx.sender, None);
            if buyer.coins < item.price {
                ctx.reply_text(format!(
                    "Fondos insuficientes. Necesitas *{} AC* en bolsillo.",
                    item.price
                ))
                .await;
                return true;
            }
            let now = unix_timestamp_seconds();
            if !ctx.state.db.transfer_gacha_card(
                &ctx.from,
                &item.character_id,
                item.image_id,
                &item.seller_jid,
                &ctx.sender,
                now,
            ) {
                let _ = ctx.state.db.remove_gacha_market(&ctx.from, sale_id, None);
                ctx.reply_text("El vendedor ya no posee esa carta.").await;
                return true;
            }
            let seller = ctx.state.db.ensure_user(&item.seller_jid, None);
            ctx.state
                .db
                .update_user_economy(&ctx.sender, buyer.coins - item.price, buyer.bank, &buyer.data_json);
            ctx.state
                .db
                .update_user_economy(&item.seller_jid, seller.coins + item.price, seller.bank, &seller.data_json);
            ctx.reply_with_mentions(
                format!(
                    "@{} compro *{}* (#{}) a @{} por *{} AC*!",
                    jid_tag(&ctx.sender),
                    item.character_id,
                    item.image_id,
                    jid_tag(&item.seller_jid),
                    item.price
                ),
                vec![ctx.sender.clone(), item.seller_jid],
            )
            .await;
            true
        }
        "removerventa" | "removesale" => {
            let Some(sale_id) = ctx.args.first().and_then(|s| s.parse::<i64>().ok()) else {
                ctx.reply_text(format!("Uso: `{}removerventa <ID>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let seller_filter = if ctx.is_owner { None } else { Some(ctx.sender.as_str()) };
            if ctx.state.db.remove_gacha_market(&ctx.from, sale_id, seller_filter) {
                ctx.reply_text(format!("Venta *{}* retirada de la tienda.", sale_id))
                    .await;
            } else {
                ctx.reply_text("No encontre esa venta a tu nombre.").await;
            }
            true
        }
        "givechar" | "regalar" | "givewaifu" => {
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text(format!("Uso: `{}givechar @usuario <personaje>`", ctx.used_prefix))
                    .await;
                return true;
            }
            let char_part = ctx
                .args
                .iter()
                .filter(|a| !a.starts_with('@'))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let (name_q, variant) = parse_char_and_variant(&char_part);
            let Some(ch) = find_char(&name_q) else {
                ctx.reply_text("Personaje no encontrado.").await;
                return true;
            };
            let mine = ctx.state.db.get_user_harem(&ctx.from, &ctx.sender);
            let Some(card) = mine.into_iter().find(|e| {
                e.character_id == ch.id && variant.is_none_or(|v| v == e.image_id)
            }) else {
                ctx.reply_text("No tienes esa carta en tu harem.").await;
                return true;
            };
            let now = unix_timestamp_seconds();
            if ctx.state.db.transfer_gacha_card(
                &ctx.from,
                &card.character_id,
                card.image_id,
                &ctx.sender,
                &target,
                now,
            ) {
                ctx.reply_with_mentions(
                    format!(
                        "@{} le regalo *{}* (#{}) a @{}!",
                        jid_tag(&ctx.sender),
                        card.character_id,
                        card.image_id,
                        jid_tag(&target)
                    ),
                    vec![ctx.sender.clone(), target],
                )
                .await;
            }
            true
        }
        "deletechar" | "delwaifu" | "delchar" => {
            let (name_q, variant) = parse_char_and_variant(&ctx.text);
            let Some(ch) = find_char(&name_q) else {
                ctx.reply_text(format!("Uso: `{}delchar <personaje [#id]>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let mine = ctx.state.db.get_user_harem(&ctx.from, &ctx.sender);
            let Some(card) = mine.into_iter().find(|e| {
                e.character_id == ch.id && variant.is_none_or(|v| v == e.image_id)
            }) else {
                ctx.reply_text("No tienes esa carta en tu harem.").await;
                return true;
            };
            if ctx
                .state
                .db
                .delete_gacha_card(&ctx.from, &card.character_id, card.image_id, &ctx.sender)
            {
                ctx.reply_text(format!(
                    "Carta *{}* (#{}) liberada al catalogo.",
                    card.character_id, card.image_id
                ))
                .await;
            }
            true
        }
        "setfav" | "wfav" => {
            let (name_q, variant) = parse_char_and_variant(&ctx.text);
            let Some(ch) = find_char(&name_q) else {
                ctx.reply_text(format!("Uso: `{}setfav <personaje>`", ctx.used_prefix))
                    .await;
                return true;
            };
            let mine = ctx.state.db.get_user_harem(&ctx.from, &ctx.sender);
            let Some(card) = mine.into_iter().find(|e| {
                e.character_id == ch.id && variant.is_none_or(|v| v == e.image_id)
            }) else {
                ctx.reply_text("Solo puedes marcar como favorita una carta de tu harem.")
                    .await;
                return true;
            };
            ctx.state
                .db
                .set_gacha_favorite(&ctx.from, &ctx.sender, &card.character_id, card.image_id);
            ctx.reply_text(format!(
                "*{}* (#{}) ahora es tu carta favorita.",
                card.character_id, card.image_id
            ))
            .await;
            true
        }
        "delfav" => {
            if ctx.state.db.clear_gacha_favorite(&ctx.from, &ctx.sender) {
                ctx.reply_text("Carta favorita eliminada.").await;
            } else {
                ctx.reply_text("No tenias ninguna carta favorita.").await;
            }
            true
        }
        "gachacheck" | "gcheck" => {
            let claimed = ctx.state.db.get_group_claimed_pairs(&ctx.from);
            let total: usize = CATALOG.iter().map(|c| c.files.len()).sum();
            ctx.reply_text(format!(
                "*Verificacion Gacha*\nPersonajes: *{}*\nVariantes totales: *{}*\nReclamadas en grupo: *{}*\nDisponibles: *{}*",
                CATALOG.len(),
                total,
                claimed.len(),
                total.saturating_sub(claimed.len())
            ))
            .await;
            true
        }
        "giveallharem" => {
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text(format!("Uso: `{}giveallharem @usuario`", ctx.used_prefix))
                    .await;
                return true;
            }
            let now = unix_timestamp_seconds();
            let moved = ctx
                .state
                .db
                .transfer_all_harem(&ctx.from, &ctx.sender, &target, now);
            if moved == 0 {
                ctx.reply_text("No tienes cartas para transferir en este grupo.")
                    .await;
            } else {
                ctx.reply_with_mentions(
                    format!(
                        "@{} transfirio todo su harem (*{}* cartas) a @{}!",
                        jid_tag(&ctx.sender),
                        moved,
                        jid_tag(&target)
                    ),
                    vec![ctx.sender.clone(), target],
                )
                .await;
            }
            true
        }
        "trade" | "cambiar" => {
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text(format!(
                    "Uso: `{}trade @usuario <tu carta> | <carta que pides>`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            }
            let cleaned = ctx
                .args
                .iter()
                .filter(|a| !a.starts_with('@'))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let Some((left, right)) = cleaned.split_once('|') else {
                ctx.reply_text(format!(
                    "Separa las cartas con `|`. Ejemplo: `{}trade @user Nino | Miku`",
                    ctx.used_prefix
                ))
                .await;
                return true;
            };
            let (l_name, l_var) = parse_char_and_variant(left);
            let (r_name, r_var) = parse_char_and_variant(right);
            let (Some(ch_a), Some(ch_b)) = (find_char(&l_name), find_char(&r_name)) else {
                ctx.reply_text("Uno de los dos personajes no existe en el catalogo.").await;
                return true;
            };
            let my_cards = ctx.state.db.get_user_harem(&ctx.from, &ctx.sender);
            let their_cards = ctx.state.db.get_user_harem(&ctx.from, &target);
            let Some(card_a) = my_cards.into_iter().find(|e| e.character_id == ch_a.id && l_var.is_none_or(|v| v == e.image_id)) else {
                ctx.reply_text("No posees la carta que ofreces.").await;
                return true;
            };
            let Some(card_b) = their_cards.into_iter().find(|e| e.character_id == ch_b.id && r_var.is_none_or(|v| v == e.image_id)) else {
                ctx.reply_text("El otro usuario no posee la carta solicitada.").await;
                return true;
            };
            let trade_key = format!("{}::{}", ctx.from, target);
            if let Ok(mut map) = ctx.state.gacha_trades.lock() {
                map.insert(
                    trade_key,
                    crate::context::GachaTradeOffer {
                        from_jid: ctx.sender.clone(),
                        to_jid: target.clone(),
                        offer_char: card_a.character_id.clone(),
                        offer_img: card_a.image_id,
                        want_char: card_b.character_id.clone(),
                        want_img: card_b.image_id,
                        created_at: std::time::Instant::now(),
                    },
                );
            }
            ctx.reply_with_mentions(
                format!(
                    "*Propuesta de Intercambio*\n@{} ofrece *{}* (#{}) por *{}* (#{}) de @{}.\nEscribe `{}accepttrade` para aceptar (3 min).",
                    jid_tag(&ctx.sender),
                    card_a.character_id,
                    card_a.image_id,
                    card_b.character_id,
                    card_b.image_id,
                    jid_tag(&target),
                    ctx.used_prefix
                ),
                vec![ctx.sender.clone(), target],
            )
            .await;
            true
        }
        "accepttrade" | "aceptarintercambio" => {
            let trade_key = format!("{}::{}", ctx.from, ctx.sender);
            let offer = ctx.state.gacha_trades.lock().ok().and_then(|mut m| {
                let o = m.remove(&trade_key)?;
                if o.created_at.elapsed() > std::time::Duration::from_secs(180) {
                    None
                } else {
                    Some(o)
                }
            });
            let Some(o) = offer else {
                ctx.reply_text("No tienes ningun intercambio pendiente o ya expiro.").await;
                return true;
            };
            let now = unix_timestamp_seconds();
            let ok1 = ctx.state.db.transfer_gacha_card(&ctx.from, &o.offer_char, o.offer_img, &o.from_jid, &o.to_jid, now);
            let ok2 = if ok1 {
                ctx.state.db.transfer_gacha_card(&ctx.from, &o.want_char, o.want_img, &o.to_jid, &o.from_jid, now)
            } else {
                false
            };
            if ok1 && ok2 {
                ctx.reply_with_mentions(
                    format!(
                        "*Intercambio completado!*\n@{} recibio *{}* (#{})\n@{} recibio *{}* (#{})",
                        jid_tag(&o.to_jid),
                        o.offer_char,
                        o.offer_img,
                        jid_tag(&o.from_jid),
                        o.want_char,
                        o.want_img
                    ),
                    vec![o.from_jid, o.to_jid],
                )
                .await;
            } else {
                if ok1 {
                    let _ = ctx.state.db.transfer_gacha_card(&ctx.from, &o.offer_char, o.offer_img, &o.to_jid, &o.from_jid, now);
                }
                ctx.reply_text("El intercambio fallo porque una de las cartas cambio de dueno.").await;
            }
            true
        }
        "setclaim" | "setclaimmsg" | "delclaimmsg" => {
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let mut map = serde_json::from_str::<serde_json::Value>(&u.data_json)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            if ctx.cmd_name == "delclaimmsg" || ctx.text.trim().is_empty() {
                map.remove("claimMsg");
                let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
                ctx.state.db.update_user_economy(&ctx.sender, u.coins, u.bank, &json);
                ctx.reply_text("Mensaje personalizado de claim restablecido.").await;
            } else {
                let clipped: String = ctx.text.trim().chars().take(120).collect();
                map.insert("claimMsg".to_string(), serde_json::Value::String(clipped.clone()));
                let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
                ctx.state.db.update_user_economy(&ctx.sender, u.coins, u.bank, &json);
                ctx.reply_text(format!("Mensaje de claim guardado: *{}*", clipped)).await;
            }
            true
        }
        "robwaifu" | "robarwaifu" => {
            let target = ctx.target_jid();
            if target == ctx.sender {
                ctx.reply_text(format!("Uso: `{}robwaifu @usuario`", ctx.used_prefix))
                    .await;
                return true;
            }
            let victim_cards = ctx.state.db.get_user_harem(&ctx.from, &target);
            if victim_cards.is_empty() {
                ctx.reply_text("Ese usuario no tiene cartas para robar.")
                    .await;
                return true;
            }
            let u = ctx.state.db.ensure_user(&ctx.sender, None);
            let mut map = serde_json::from_str::<serde_json::Value>(&u.data_json)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            let now = unix_timestamp_seconds();
            let last = map
                .get("lastRobWaifu")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if last > 0 && now.saturating_sub(last) < 3600 {
                let left = 3600 - now.saturating_sub(last);
                ctx.reply_text(format!(
                    "Espera {}m {}s para volver a intentar un robo de carta.",
                    left / 60,
                    left % 60
                ))
                .await;
                return true;
            }
            map.insert("lastRobWaifu".to_string(), serde_json::Value::from(now));
            let json = serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string());
            ctx.state
                .db
                .update_user_economy(&ctx.sender, u.coins, u.bank, &json);

            if (next_rand() % 100) < 25 {
                let card = &victim_cards[(next_rand() as usize) % victim_cards.len()];
                if ctx.state.db.transfer_gacha_card(
                    &ctx.from,
                    &card.character_id,
                    card.image_id,
                    &target,
                    &ctx.sender,
                    now,
                ) {
                    ctx.reply_with_mentions(
                        format!(
                            "@{} logro robar a *{}* (#{}) del harem de @{}!",
                            jid_tag(&ctx.sender),
                            card.character_id,
                            card.image_id,
                            jid_tag(&target)
                        ),
                        vec![ctx.sender.clone(), target],
                    )
                    .await;
                    return true;
                }
            }
            ctx.reply_with_mentions(
                format!(
                    "@{} fallo el robo contra @{}.",
                    jid_tag(&ctx.sender),
                    jid_tag(&target)
                ),
                vec![ctx.sender.clone(), target],
            )
            .await;
            true
        }
        _ => false,
    }
}
