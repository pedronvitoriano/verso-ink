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
    fb.fill(r, PAPER);
    let fg = if filled {
        fb.round_rect(r, r.h / 2, INK);
        PAPER
    } else {
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        INK
    };
    let size = if f.measure(Face::Strong, 38.0, label) > r.w - 40 { 30.0 } else { 38.0 };
    let tw = f.measure(Face::Strong, size, label);
    f.draw(fb, Face::Strong, size, r.x + (r.w - tw) / 2, r.y + r.h / 2 + (size * 0.36) as i32, fg, label);
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
    Rect::new(MARGIN, 640 + k as i32 * 160, W - 2 * MARGIN, 136)
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
        Some(("Duas pessoas".to_string(), "Partida local, no mesmo Kindle".to_string(), false)),
        Some(("Partidas anteriores".to_string(), "Revisão com os melhores lances".to_string(), false)),
    ]
    .into_iter()
    .flatten()
    .collect();
    for (k, (t, s, filled)) in rows.iter().enumerate() {
        let r = hub_row(k);
        let (fg, sub) = if *filled { (PAPER, 0xCC) } else { (INK, MUTED) };
        if *filled {
            fb.round_rect(r, 44, INK);
        } else {
            fb.round_outline(r, 44, 4, INK, PAPER);
        }
        f.draw(fb, Face::Display, 50.0, r.x + 40, r.y + 66, fg, t);
        f.draw(fb, Face::Body, 28.0, r.x + 42, r.y + 108, sub, &fit(f, Face::Body, 28.0, s, r.w - 80));
    }
}

// ---------- Escada ----------

pub const PER_PAGE: usize = 6;

pub fn coach_row(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 400 + i as i32 * 140, W - 2 * MARGIN + 32, 136)
}
pub fn prev_rect() -> Rect {
    Rect::new(MARGIN, 1300, 260, 88)
}
pub fn next_rect() -> Rect {
    Rect::new(W - MARGIN - 260, 1300, 260, 88)
}

pub fn draw_coaches(fb: &mut Framebuffer, f: &Fonts, page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 110.0, MARGIN - 4, 290, INK, "Treinadores");
    f.draw(fb, Face::Strong, 28.0, MARGIN, 350, MUTED, "Vença um para liberar o próximo");
    let open = coach::unlocked();
    for k in 0..PER_PAGE {
        let i = page * PER_PAGE + k;
        if i >= coach::LADDER {
            break;
        }
        let c = coach::coach(i);
        let r = coach_row(k);
        let locked = i > open;
        fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
        let ink = if locked { 0xAA } else { INK };
        f.draw(fb, Face::Display, 50.0, MARGIN, r.y + 66, ink, c.name);
        let rt = c.rating.to_string();
        let rw = f.measure(Face::Display, 56.0, &rt);
        f.draw(fb, Face::Display, 56.0, W - MARGIN - rw, r.y + 70, ink, &rt);
        let sub = if locked { format!("Vença {} para liberar", coach::coach(i - 1).name) } else { c.style.to_string() };
        f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 112, if locked { 0xAA } else { MUTED }, &sub);
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
    Rect::new(0, 166, W, 80)
}

/// Fala do treinador acima do tabuleiro (até duas linhas).
pub fn draw_coach_line(fb: &mut Framebuffer, f: &Fonts, who: &str, text: &str) {
    let r = coach_line_rect();
    fb.fill(r, PAPER);
    let head = format!("{who}: ");
    let hw = f.measure(Face::Strong, 27.0, &head);
    f.draw(fb, Face::Strong, 27.0, MARGIN, r.y + 32, INK, &head);
    let lines = wrap(f, Face::Body, 27.0, text, W - 2 * MARGIN - hw);
    for (k, l) in lines.iter().take(2).enumerate() {
        f.draw(fb, Face::Body, 27.0, MARGIN + hw, r.y + 32 + k as i32 * 36, INK, l);
    }
}

// ---------- Fim de partida ----------

pub fn end_row(k: usize) -> Rect {
    Rect::new(MARGIN, 900 + k as i32 * 150, W - 2 * MARGIN, 124)
}

pub fn draw_end(fb: &mut Framebuffer, f: &Fonts, title: &str, detail: &str, ratings: Option<(u32, u32)>, unlocked: Option<String>) {
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
    pill(fb, f, end_row(2), "Escolher treinador", false);
}

// ---------- Partidas anteriores ----------

pub fn game_row(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 400 + i as i32 * 140, W - 2 * MARGIN + 32, 136)
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
        f.draw(fb, Face::Body, 30.0, MARGIN, 1196 + n as i32 * 40, INK, l);
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
    Rect::new(MARGIN - 16, 1010 + i as i32 * 96, W - 2 * MARGIN + 32, 92)
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
