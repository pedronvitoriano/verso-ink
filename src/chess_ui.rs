//! Telas do xadrez: central, escada de treinadores, fim de partida, partidas anteriores e revisão.

use crate::apps::fit;
use crate::chess::{self, Game, Sprites};
use crate::coach::{self, Class, Saved};
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MARGIN: i32 = 72;
const W: i32 = 1072;

fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    ui::button(fb, f, r, label, filled);
}

fn wrap(f: &Fonts, face: Face, size: f32, text: &str, max_w: i32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for w in text.split_whitespace() {
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if f.measure(face, size, &cand) > max_w && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur = w.to_string();
        } else {
            cur = cand;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

// ---------- Central ----------

pub fn hub_row(k: usize) -> Rect {
    Rect::new(MARGIN, 600 + k as i32 * 136, W - 2 * MARGIN, 118)
}

/// Linhas do menu do xadrez e o que cada uma abre (0 continuar, 1 treinadores, 4 Claude,
/// 2 duas pessoas, 3 partidas anteriores).
pub fn hub_ids(has_resume: bool) -> Vec<usize> {
    let mut v = if has_resume { vec![0] } else { vec![] };
    v.extend([1, 4, 2, 3]);
    v
}

pub fn draw_hub(fb: &mut Framebuffer, f: &Fonts, resume: Option<String>) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, "Xadrez");
    let (r, n) = coach::rating();
    f.draw(fb, Face::Strong, 30.0, MARGIN, 420, MUTED, "SEU RATING ESTIMADO");
    let end = f.draw(fb, Face::Display, 120.0, MARGIN - 4, 550, INK, &r.to_string());
    let sub = if n == 0 { "ainda sem partidas contra treinadores".to_string() } else { format!("{n} {} contra treinadores", if n == 1 { "partida" } else { "partidas" }) };
    f.draw(fb, Face::Body, 30.0, end + 24, 550, MUTED, &sub);
    let rows: Vec<(String, String, bool)> = [
        resume.map(|s| ("Continuar partida".to_string(), s, true)),
        Some(("Treinadores".to_string(), format!("Escada de 100 a 3000 · liberado até {}", coach::coach(coach::unlocked()).rating), false)),
        Some(("Contra o Claude".to_string(), "Ele joga, comenta cada lance e analisa a partida no fim".to_string(), false)),
        Some(("Duas pessoas".to_string(), "Partida local, no mesmo Kindle".to_string(), false)),
        Some(("Partidas anteriores".to_string(), "Revisão com os melhores lances".to_string(), false)),
    ]
    .into_iter()
    .flatten()
    .collect();
    for (k, (t, s, filled)) in rows.iter().enumerate() {
        let r = hub_row(k);
        let (fg, sub) = if *filled { (PAPER, 0xCC) } else { (INK, MUTED) };
        ui::card(fb, r, 40, if *filled { INK } else { PAPER }, false);
        f.draw(fb, Face::Display, 46.0, r.x + 36, r.y + 58, fg, t);
        f.draw(fb, Face::Body, 26.0, r.x + 38, r.y + 96, sub, &fit(f, Face::Body, 26.0, s, r.w - 80));
    }
    // Relógio das próximas partidas.
    let r = clock_row_rect();
    fb.fill(Rect::new(MARGIN, r.y - 18, W - 2 * MARGIN, 3), 0xCC);
    f.draw(fb, Face::Strong, 34.0, MARGIN, r.y + 60, INK, "Relógio");
    let v = crate::chess::control_name(crate::chess::load_control());
    let vw = f.measure(Face::Display, 38.0, &v);
    f.draw(fb, Face::Display, 38.0, r.x + r.w - 50 - vw, r.y + 62, INK, &v);
    let (cx, cy) = (r.x + r.w - 14, r.y + r.h / 2);
    ui::stroke(fb, ((cx - 12) as f32, (cy - 18) as f32), (cx as f32, cy as f32), 5.0, INK);
    ui::stroke(fb, (cx as f32, cy as f32), ((cx - 12) as f32, (cy + 18) as f32), 5.0, INK);
}

pub fn clock_row_rect() -> Rect {
    Rect::new(MARGIN, 1310, W - 2 * MARGIN, 96)
}

// ---------- Escolha do relógio ----------

/// Grupos da tela de relógio: (nome, explicação, índices em CONTROLS).
const CLOCK_GROUPS: [(&str, &str, &[usize]); 4] = [
    ("Bala", "menos de 3 minutos", &[0, 1]),
    ("Relâmpago", "3 a 8 minutos", &[2, 3, 4, 5]),
    ("Rápida", "8 a 25 minutos", &[6, 7, 8]),
    ("Clássica", "mais de 25 minutos", &[9, 10]),
];

/// `None` = sem tempo; `Some(k)` = CONTROLS[k].
pub fn clock_option_rect(k: Option<usize>) -> Rect {
    let w = (W - 2 * MARGIN - 3 * 16) / 4;
    let Some(k) = k else { return Rect::new(MARGIN, 420, W - 2 * MARGIN, 100) };
    for (g, (_, _, ks)) in CLOCK_GROUPS.iter().enumerate() {
        if let Some(j) = ks.iter().position(|&x| x == k) {
            return Rect::new(MARGIN + j as i32 * (w + 16), 620 + g as i32 * 190, w, 100);
        }
    }
    Rect::new(0, 0, 0, 0)
}

pub fn draw_clock_picker(fb: &mut Framebuffer, f: &Fonts) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 110.0, MARGIN - 4, 290, INK, "Relógio");
    f.draw(fb, Face::Strong, 28.0, MARGIN, 350, MUTED, "Vale para as próximas partidas, contra treinador ou a dois.");
    let cur = crate::chess::load_control();
    pill(fb, f, clock_option_rect(None), "Sem tempo", cur.is_none());
    for (g, (name, hint, ks)) in CLOCK_GROUPS.iter().enumerate() {
        let y = 600 + g as i32 * 190;
        let end = f.draw(fb, Face::Strong, 30.0, MARGIN, y, INK, name);
        f.draw(fb, Face::Body, 26.0, end + 14, y, MUTED, hint);
        for &k in ks.iter() {
            let r = clock_option_rect(Some(k));
            pill(fb, f, r, &crate::chess::control_label(k), cur == Some(k));
        }
    }
    let note = match cur {
        None => "Sem relógio: pense o quanto quiser.".to_string(),
        Some(k) => {
            let (b, i) = crate::chess::CONTROLS[k];
            let inc = if i > 0 { format!(", e cada lance devolve {i} s") } else { String::new() };
            format!("{}: {} minutos para cada um{inc}. Acabou o tempo, perdeu.", crate::chess::control_label(k), b / 60)
        }
    };
    for (n, l) in wrap(f, Face::Body, 30.0, &note, W - 2 * MARGIN).iter().take(2).enumerate() {
        f.draw(fb, Face::Body, 30.0, MARGIN, 1390 - 44 + n as i32 * 40, INK, l);
    }
}

// ---------- Escada ----------

pub const PER_PAGE: usize = 6;

/// Cartões em grade de 2 colunas por 3 linhas.
pub fn coach_row(i: usize) -> Rect {
    let gap = 24;
    let w = (W - 2 * MARGIN - gap) / 2;
    let h = 270;
    Rect::new(MARGIN + (i % 2) as i32 * (w + gap), 410 + (i / 2) as i32 * (h + 22), w, h)
}
pub fn prev_rect() -> Rect {
    Rect::new(MARGIN, 1300, 260, 88)
}
pub fn next_rect() -> Rect {
    Rect::new(W - MARGIN - 260, 1300, 260, 88)
}

/// Arquivo do rosto: "Mestra Vera" → treinadores/mestra-vera.png.
fn portrait_path(name: &str) -> std::path::PathBuf {
    let slug = crate::apps::fold(name).split_whitespace().collect::<Vec<_>>().join("-");
    crate::home_dir().join("treinadores").join(format!("{slug}.png"))
}

thread_local! {
    static PORTRAITS: std::cell::RefCell<std::collections::HashMap<(String, i32), Option<Vec<u8>>>> = Default::default();
}

/// Desenha o rosto do treinador (quadrado de lado `side`), se o arquivo existir. Guarda em memória.
pub fn draw_portrait(fb: &mut Framebuffer, name: &str, x: i32, y: i32, side: i32, dim: bool) -> bool {
    let px = PORTRAITS.with(|c| {
        c.borrow_mut()
            .entry((name.to_string(), side))
            .or_insert_with(|| {
                let img = image::open(portrait_path(name)).ok()?.to_luma8();
                Some(image::imageops::resize(&img, side as u32, side as u32, image::imageops::FilterType::Triangle).into_raw())
            })
            .clone()
    });
    let Some(mut px) = px else { return false };
    if dim {
        // Bloqueado: rosto clareado.
        px.iter_mut().for_each(|v| *v = 0xAA + (*v as u16 * 0x55 / 255) as u8);
    }
    fb.blit(x, y, side, side, &px);
    fb.round_outline(Rect::new(x - 3, y - 3, side + 6, side + 6), 16, 3, if dim { 0xAA } else { INK }, PAPER);
    fb.blit(x, y, side, side, &px);
    true
}

pub fn draw_coaches(fb: &mut Framebuffer, f: &Fonts, page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 110.0, MARGIN - 4, 290, INK, "Treinadores");
    let open = coach::unlocked();
    let sub = format!("Vença um para liberar o próximo · {} de {} liberados", (open + 1).min(coach::LADDER), coach::LADDER);
    f.draw(fb, Face::Strong, 28.0, MARGIN, 350, MUTED, &sub);
    for k in 0..PER_PAGE {
        let i = page * PER_PAGE + k;
        if i >= coach::LADDER {
            break;
        }
        let c = coach::coach(i);
        let r = coach_row(k);
        let (locked, current, beaten) = (i > open, i == open, i < open);
        // Próximo desafio: cartão preto. Vencidos: contorno. Bloqueados: apagados.
        let (fg, muted) = if current { (PAPER, 0xCC) } else if locked { (0x55, 0x70) } else { (INK, MUTED) };
        if current {
            fb.round_rect(r, 28, INK);
        } else {
            fb.round_outline(r, 28, if locked { 3 } else { 4 }, if locked { 0x77 } else { INK }, PAPER);
        }
        let side = 150;
        if current {
            fb.round_rect(Rect::new(r.x + 16, r.y + 16, side + 12, side + 12), 18, PAPER);
        }
        let has = draw_portrait(fb, c.name, r.x + 22, r.y + 22, side, locked);
        let tx = if has { r.x + 22 + side + 22 } else { r.x + 26 };
        let room = r.x + r.w - 20 - tx;
        let tag = if current { "PRÓXIMO" } else if beaten { "VENCIDO" } else { "BLOQUEADO" };
        f.draw(fb, Face::Strong, 20.0, tx, r.y + 46, muted, tag);
        f.draw(fb, Face::Display, 40.0, tx, r.y + 96, fg, &fit(f, Face::Display, 40.0, c.name, room));
        f.draw(fb, Face::Display, 48.0, tx, r.y + 154, fg, &c.rating.to_string());
        let style = if locked { format!("Vença {} para liberar", coach::coach(i - 1).name) } else { c.style.to_string() };
        for (n, l) in wrap(f, Face::Body, 24.0, &style, r.w - 44).iter().take(2).enumerate() {
            f.draw(fb, Face::Body, 24.0, r.x + 24, r.y + 214 + n as i32 * 30, muted, l);
        }
    }
    let pages = coach::LADDER.div_ceil(PER_PAGE);
    if page > 0 {
        pill(fb, f, prev_rect(), "‹ Anterior", false);
    }
    if page + 1 < pages {
        pill(fb, f, next_rect(), "Próxima ›", false);
    }
    let pg = format!("{} de {pages}", page + 1);
    let pw = f.measure(Face::Strong, 30.0, &pg);
    f.draw(fb, Face::Strong, 30.0, (W - pw) / 2, 1356, MUTED, &pg);
}

// ---------- Partida contra treinador ----------

pub fn coach_line_rect() -> Rect {
    Rect::new(0, 166, W, chess::BY - 8 - 166)
}

/// Balão de fala do treinador (à direita do busto).
pub fn bubble_rect() -> Rect {
    let x = MARGIN + 250;
    Rect::new(x, 184, W - MARGIN - x, 166)
}

/// Treinador em cima do tabuleiro, como num app de jogo: o busto recortado no papel (sem
/// moldura), apoiado na borda do tabuleiro, e o que ele diz num balão ao lado.
pub fn draw_coach_line(fb: &mut Framebuffer, f: &Fonts, who: &str, text: &str) {
    let r = coach_line_rect();
    fb.fill(r, PAPER);
    let name = who.split(" (").next().unwrap_or(who);
    let slug = crate::apps::fold(name).split_whitespace().collect::<Vec<_>>().join("-");
    let side = r.y + r.h - 170 + 2;
    if !crate::sprites::draw(fb, &format!("busto-{slug}"), Rect::new(MARGIN - 44, 170, side, side))
        && !draw_portrait(fb, name, MARGIN, 190, 200, false)
    {
        // Sem retrato (o Claude): a faísca dele, grande, num disco.
        ui::icon_disc(fb, MARGIN, 196, 200, "claude");
    }
    let bb = bubble_rect();
    fb.round_rect(bb, 30, PAPER);
    fb.round_outline(bb, 30, 4, INK, PAPER);
    // Rabicho do balão apontando para o treinador.
    let (tx, ty) = (bb.x, bb.y + 96);
    for k in 0..26 {
        let h = 26 - k;
        fb.fill(Rect::new(tx - k, ty - h / 2, 1, h), INK);
    }
    for k in 4..22 {
        let h = (26 - k) - 8;
        if h > 0 {
            fb.fill(Rect::new(tx - k + 4, ty - h / 2, 1, h), PAPER);
        }
    }
    fb.fill(Rect::new(tx, ty - 9, 4, 18), PAPER);
    for (k, l) in wrap(f, Face::Body, 31.0, text, bb.w - 64).iter().take(3).enumerate() {
        f.draw(fb, Face::Body, 31.0, bb.x + 32, bb.y + 52 + k as i32 * 42, INK, l);
    }
    // Nome e rating embaixo do balão (o relógio dele fica à direita, na mesma linha).
    let end = f.draw(fb, Face::Strong, 32.0, bb.x + 6, 412, INK, name);
    let rating = who.split(" (").nth(1).map(|s| s.trim_end_matches(')')).unwrap_or("");
    f.draw(fb, Face::Body, 28.0, end + 14, 412, MUTED, rating);
}

// ---------- Fim de partida ----------

pub fn end_row(k: usize) -> Rect {
    Rect::new(MARGIN, 900 + k as i32 * 150, W - 2 * MARGIN, 124)
}

pub fn draw_end(fb: &mut Framebuffer, f: &Fonts, title: &str, detail: &str, ratings: Option<(u32, u32)>, unlocked: Option<String>, vs_claude: bool) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 360, INK, title);
    for (k, l) in wrap(f, Face::Body, 38.0, detail, W - 2 * MARGIN).iter().take(2).enumerate() {
        f.draw(fb, Face::Body, 38.0, MARGIN, 450 + k as i32 * 54, INK, l);
    }
    if let Some((a, b)) = ratings {
        f.draw(fb, Face::Strong, 30.0, MARGIN, 620, MUTED, "RATING ESTIMADO");
        let diff = b as i32 - a as i32;
        let s = format!("{a} → {b} ({}{diff})", if diff >= 0 { "+" } else { "" });
        f.draw(fb, Face::Display, 72.0, MARGIN - 4, 710, INK, &s);
    }
    if let Some(u) = unlocked {
        f.draw(fb, Face::Strong, 32.0, MARGIN, 800, INK, &u);
    }
    pill(fb, f, end_row(0), "Revisar a partida", true);
    pill(fb, f, end_row(1), "Jogar de novo", false);
    pill(fb, f, end_row(2), if vs_claude { "Análise do Claude" } else { "Escolher treinador" }, vs_claude);
}

// ---------- Partidas anteriores ----------

pub fn game_row(i: usize) -> Rect {
    Rect::new(MARGIN, 400 + i as i32 * 140, W - 2 * MARGIN, 136)
}

pub fn draw_games(fb: &mut Framebuffer, f: &Fonts, games: &[Saved], page: usize, offset_min: i64) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 100.0, MARGIN - 4, 290, INK, "Partidas");
    f.draw(fb, Face::Strong, 28.0, MARGIN, 350, MUTED, if games.is_empty() { "Jogue contra um treinador e a partida aparece aqui" } else { "Toque para revisar" });
    for (k, g) in games.iter().skip(page * PER_PAGE).take(PER_PAGE).enumerate() {
        let r = game_row(k);
        fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
        let c = coach::coach(g.level);
        let res = match g.result.as_str() {
            "vitória" => "Vitória",
            "derrota" => "Derrota",
            _ => "Empate",
        };
        f.draw(fb, Face::Display, 46.0, MARGIN, r.y + 64, INK, &format!("{res} · {} ({})", c.name, c.rating));
        let days = (g.when + offset_min * 60).div_euclid(86_400);
        let today = crate::data::today(offset_min);
        let when = match today - days {
            0 => "hoje".to_string(),
            1 => "ontem".to_string(),
            n => format!("há {n} dias"),
        };
        let n = g.moves.len().div_ceil(2);
        f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 108, MUTED, &format!("{when} · precisão {}% · {n} lances · {}", coach::accuracy(&g.records), if g.user_white { "brancas" } else { "pretas" }));
    }
    let pages = games.len().div_ceil(PER_PAGE).max(1);
    if page > 0 {
        pill(fb, f, prev_rect(), "‹ Anterior", false);
    }
    if page + 1 < pages {
        pill(fb, f, next_rect(), "Próxima ›", false);
    }
}

// ---------- Revisão ----------

pub fn review_btn(k: usize) -> Rect {
    let w = (W - 2 * MARGIN - 2 * 16) / 3;
    Rect::new(MARGIN + k as i32 * (w + 16), 1316, w, 100)
}

/// Mostra a posição antes do seu lance `k`: o lance jogado em cinza e o melhor com contorno.
pub fn draw_review(fb: &mut Framebuffer, f: &Fonts, sp: &Sprites, g: &Saved, k: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let Some(rec) = g.records.get(k) else {
        f.draw(fb, Face::Display, 48.0, MARGIN, 400, INK, "Sem lances para revisar.");
        return;
    };
    let mut game = Game::new_coach(g.level, g.user_white);
    for m in g.moves.iter().take(rec.ply) {
        game.play_uci(m);
    }
    let played = g.moves.get(rec.ply).and_then(|m| chess::parse_uci(m)).map(|x| x.0);
    game.last = played;
    let best = chess::parse_uci(&rec.best_uci).map(|x| x.0);
    let head = format!("Lance {} · {}{} · {}", rec.ply / 2 + 1, rec.san, rec.class.mark(), rec.class.name());
    f.draw(fb, Face::Display, 40.0, MARGIN, 214, INK, &fit(f, Face::Display, 40.0, &head, W - 2 * MARGIN - 180));
    let pos_label = format!("{} de {}", k + 1, g.records.len());
    let pw = f.measure(Face::Body, 28.0, &pos_label);
    f.draw(fb, Face::Body, 28.0, W - MARGIN - pw, 214, MUTED, &pos_label);
    // Ordem: borda do tabuleiro, casas e, por cima, a moldura do melhor lance.
    let b = chess::board_rect();
    fb.round_outline(Rect::new(b.x - 6, b.y - 6, b.w + 12, b.h + 12), 8, 4, INK, PAPER);
    for i in 0..64 {
        chess::draw_square(fb, &game, sp, i);
    }
    if let Some(bm) = best.filter(|b| Some(*b) != played) {
        for sq in [bm.from, bm.to] {
            let r = chess::square_rect(sq, game.flipped());
            fb.round_outline(Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4), 14, 12, INK, 0xF4);
            if let Some(p) = game.pos.board[sq] {
                sp.draw(fb, p, r.x + 4, r.y + 4);
            }
        }
    }
    let text = if Some(rec.best_uci.as_str()) == g.moves.get(rec.ply).map(|s| s.as_str()) || rec.class == Class::Best {
        "Você achou o melhor lance.".to_string()
    } else if rec.class == Class::Good {
        format!("Bom lance. O motor preferia {}, por pouco.", rec.best_san)
    } else {
        let why = if rec.why.is_empty() || rec.why.starts_with("Melhor era") { String::new() } else { format!("{} ", rec.why) };
        format!("{why}Melhor lance: {} (casas com moldura). O seu está em cinza.", rec.best_san)
    };
    for (n, l) in wrap(f, Face::Body, 30.0, &text, W - 2 * MARGIN).iter().take(3).enumerate() {
        f.draw(fb, Face::Body, 30.0, MARGIN, 290 + n as i32 * 42, INK, l);
    }
    pill(fb, f, review_btn(0), "‹ Anterior", false);
    pill(fb, f, review_btn(1), "Insights", true);
    pill(fb, f, review_btn(2), "Próximo ›", false);
}

/// Momentos-chave tocáveis na tela de insights (índices nos registros da partida).
pub fn key_moments(g: &Saved) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..g.records.len()).filter(|&k| g.records[k].class as u8 >= Class::Inaccuracy as u8).collect();
    idx.sort_by_key(|&k| std::cmp::Reverse(g.records[k].loss));
    idx.truncate(3);
    idx.sort();
    idx
}

pub fn moment_rect(i: usize) -> Rect {
    Rect::new(MARGIN, 1010 + i as i32 * 96, W - 2 * MARGIN, 92)
}

/// Insights em blocos: precisão, lances por tipo, fases, evolução e momentos-chave tocáveis.
pub fn draw_insights(fb: &mut Framebuffer, f: &Fonts, g: &Saved, previous: &[Saved], tip: Option<&str>) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let c = coach::coach(g.level);
    let res = match g.result.as_str() {
        "vitória" => "Vitória",
        "derrota" => "Derrota",
        _ => "Empate",
    };
    let head = format!("{res} contra {} ({})", c.name, c.rating);
    let hw = f.measure(Face::Strong, 28.0, &head);
    f.draw(fb, Face::Strong, 28.0, W - MARGIN - hw, 128, MUTED, &head);

    // Precisão grande e rating.
    let acc = coach::accuracy(&g.records);
    // Sem lances seus analisados (desistência logo no início): não há precisão para mostrar.
    let acc_text = if g.records.is_empty() { "—".to_string() } else { format!("{acc}%") };
    let end = f.draw(fb, Face::Display, 180.0, MARGIN - 8, 360, INK, &acc_text);
    f.draw(fb, Face::Strong, 30.0, end + 20, 300, MUTED, "PRECISÃO");
    f.draw(fb, Face::Strong, 30.0, end + 20, 340, MUTED, "ESTIMADA");
    let d = g.rating_after as i32 - g.rating_before as i32;
    let rt = format!("Rating {} → {} ({}{d})", g.rating_before, g.rating_after, if d >= 0 { "+" } else { "" });
    f.draw(fb, Face::Body, 32.0, MARGIN, 410, INK, &rt);

    // Lances por tipo: barras horizontais.
    let kinds = [Class::Best, Class::Good, Class::Inaccuracy, Class::Mistake, Class::Blunder];
    let counts: Vec<usize> = kinds.iter().map(|k| g.records.iter().filter(|r| r.class == *k).count()).collect();
    let max = counts.iter().copied().max().unwrap_or(1).max(1);
    let (label_w, bar_x0) = (250, MARGIN + 250);
    let bar_max = W - MARGIN - bar_x0 - 70;
    for (n, (k, cnt)) in kinds.iter().zip(&counts).enumerate() {
        let y = 470 + n as i32 * 52;
        f.draw(fb, Face::Body, 28.0, MARGIN, y + 32, INK, k.name());
        let w = (bar_max * *cnt as i32 / max as i32).max(if *cnt > 0 { 8 } else { 0 });
        let tone = match k {
            Class::Best | Class::Good => 0xB0,
            Class::Inaccuracy => 0x77,
            _ => INK,
        };
        if w > 0 {
            fb.round_rect(Rect::new(bar_x0, y + 8, w, 30), 8, tone);
        }
        f.draw(fb, Face::Strong, 28.0, bar_x0 + w + 14, y + 33, INK, &cnt.to_string());
        let _ = label_w;
    }

    // Erros por fase e evolução da precisão, lado a lado.
    let y0 = 760;
    f.draw(fb, Face::Strong, 26.0, MARGIN, y0, MUTED, "ERROS POR FASE");
    let mut by = [0usize; 3];
    for r in g.records.iter().filter(|r| r.class as u8 >= Class::Mistake as u8) {
        by[if r.ply < 20 { 0 } else if r.ply < 60 { 1 } else { 2 }] += 1;
    }
    for (k, name) in ["Abertura", "Meio", "Final"].iter().enumerate() {
        let r = Rect::new(MARGIN + k as i32 * 148, y0 + 20, 136, 120);
        if by[k] > 0 {
            fb.round_rect(r, 18, if by[k] >= 2 { INK } else { 0x77 });
        } else {
            fb.round_outline(r, 18, 3, INK, PAPER);
        }
        let fg = if by[k] > 0 { PAPER } else { INK };
        let n = by[k].to_string();
        let nw = f.measure(Face::Display, 56.0, &n);
        f.draw(fb, Face::Display, 56.0, r.x + (r.w - nw) / 2, r.y + 66, fg, &n);
        let lw = f.measure(Face::Body, 22.0, name);
        f.draw(fb, Face::Body, 22.0, r.x + (r.w - lw) / 2, r.y + 102, fg, name);
    }
    let sx = MARGIN + 3 * 148 + 30;
    f.draw(fb, Face::Strong, 26.0, sx, y0, MUTED, "EVOLUÇÃO");
    let mut hist: Vec<u32> = previous.iter().filter(|p| p.file != g.file && !p.records.is_empty()).take(9).map(|p| coach::accuracy(&p.records)).collect();
    hist.reverse();
    if !g.records.is_empty() {
        hist.push(acc);
    }
    let (gx, gy, gw, gh) = (sx, y0 + 24, W - MARGIN - sx, 116);
    fb.fill(Rect::new(gx, gy + gh, gw, 3), INK);
    let step = if hist.len() > 1 { gw / (hist.len() as i32 - 1) } else { 0 };
    let pt = |k: usize, v: u32| (gx + k as i32 * step, gy + gh - (gh * v as i32 / 100));
    for k in 1..hist.len() {
        let (a, b) = (pt(k - 1, hist[k - 1]), pt(k, hist[k]));
        ui::stroke(fb, (a.0 as f32, a.1 as f32), (b.0 as f32, b.1 as f32), 4.0, INK);
    }
    if let Some(&last) = hist.last().filter(|_| hist.len() > 1) {
        let p = pt(hist.len() - 1, last);
        fb.round_rect(Rect::new(p.0 - 9, p.1 - 9, 18, 18), 9, INK);
    }
    if hist.len() <= 1 {
        f.draw(fb, Face::Body, 24.0, gx, gy + 60, MUTED, "aparece a partir da 2ª partida");
    }

    // Momentos-chave: toque para ver na revisão.
    f.draw(fb, Face::Strong, 26.0, MARGIN, 990, MUTED, "MOMENTOS-CHAVE · TOQUE PARA VER");
    let moments = key_moments(g);
    if moments.is_empty() {
        let msg = if g.records.is_empty() { "Nenhum lance seu foi analisado nesta partida." } else { "Nenhum deslize relevante. Partida limpa!" };
        f.draw(fb, Face::Body, 32.0, MARGIN, 1060, INK, msg);
    }
    for (i, &k) in moments.iter().enumerate() {
        let r = g.records[k].clone();
        let row = moment_rect(i);
        fb.round_outline(row, 24, 3, INK, PAPER);
        let left = format!("{}. {}{}", r.ply / 2 + 1, r.san, r.class.mark());
        f.draw(fb, Face::Display, 38.0, row.x + 28, row.y + 58, INK, &left);
        let right = format!("{} · melhor {}", r.class.name(), r.best_san);
        let rw = f.measure(Face::Body, 28.0, &right);
        f.draw(fb, Face::Body, 28.0, row.x + row.w - 28 - rw, row.y + 56, MUTED, &right);
    }
    if let Some(t) = tip {
        let lines = wrap(f, Face::Body, 30.0, t, W - 2 * MARGIN);
        for (n, l) in lines.iter().take(2).enumerate() {
            f.draw(fb, Face::Body, 30.0, MARGIN, 1340 + n as i32 * 42, INK, l);
        }
    }
}
