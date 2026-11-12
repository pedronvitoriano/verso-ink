//! Aprender, no jeito dos apps de idioma: XP por atividade (lição de partitura, treinos, revisão
//! dos destaques, treino de violão), meta diária, ofensiva (dias seguidos com pelo menos uma
//! lição), protetor que salva um dia perdido (ganha um a cada 7 dias de ofensiva, guarda até 2)
//! e a comemoração quando a ofensiva cresce ou a meta é batida.
//!
//! Arquivos (em data/): aprender.txt (dia|xp|fonte), aprender_protetor.txt (quantos guardados).

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const W: i32 = 1072;
const MARGIN: i32 = 72;
/// XP mínimo para o dia contar na ofensiva (uma lição).
pub const MIN_DAY: u32 = 10;
const MAX_FREEZE: u32 = 2;

fn data(name: &str) -> std::path::PathBuf {
    let d = crate::home_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d.join(name)
}

pub fn today() -> i64 {
    crate::data::today(crate::offset_min())
}

/// Meta do dia em XP (settings/meta_xp; padrão 30).
pub fn goal() -> u32 {
    std::fs::read_to_string(crate::home_dir().join("settings/meta_xp")).ok().and_then(|s| s.trim().parse().ok()).filter(|n| (10..=200).contains(n)).unwrap_or(30)
}

pub fn set_goal(n: u32) {
    let _ = std::fs::create_dir_all(crate::home_dir().join("settings"));
    let _ = std::fs::write(crate::home_dir().join("settings/meta_xp"), n.to_string());
}

/// (dia, xp, fonte) de cada registro.
fn log() -> Vec<(i64, u32, String)> {
    std::fs::read_to_string(data("aprender.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, '|');
            Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next().unwrap_or("").to_string()))
        })
        .collect()
}

fn append(day: i64, xp: u32, src: &str) {
    let mut s = std::fs::read_to_string(data("aprender.txt")).unwrap_or_default();
    s.push_str(&format!("{day}|{xp}|{src}\n"));
    let _ = std::fs::write(data("aprender.txt"), s);
}

pub fn freezes() -> u32 {
    std::fs::read_to_string(data("aprender_protetor.txt")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn set_freezes(n: u32) {
    let _ = std::fs::write(data("aprender_protetor.txt"), n.min(MAX_FREEZE).to_string());
}

/// XP de um dia.
fn xp_on(log: &[(i64, u32, String)], day: i64) -> u32 {
    log.iter().filter(|(d, _, _)| *d == day).map(|(_, x, _)| *x).sum()
}

/// O dia conta para a ofensiva (lição feita ou protetor usado)?
fn kept(log: &[(i64, u32, String)], day: i64) -> bool {
    xp_on(log, day) >= MIN_DAY || log.iter().any(|(d, _, s)| *d == day && s == "protetor")
}

/// Ofensiva calculada sobre o registro: dias seguidos até hoje (ou até ontem, se hoje ainda não).
fn streak_in(log: &[(i64, u32, String)], today: i64) -> u32 {
    let mut d = if kept(log, today) { today } else { today - 1 };
    let mut n = 0;
    while kept(log, d) {
        n += 1;
        d -= 1;
    }
    n
}

/// Usa protetores para dias perdidos logo antes de hoje, se a ofensiva estava viva.
fn settle(today: i64) {
    let mut lg = log();
    let mut fz = freezes();
    // Até dois dias perdidos seguidos (ontem e anteontem) podem ser salvos.
    for back in [2i64, 1] {
        let day = today - back;
        if fz > 0 && !kept(&lg, day) && kept(&lg, day - 1) && !lg.iter().any(|(d, _, _)| *d > day) {
            append(day, 0, "protetor");
            lg.push((day, 0, "protetor".into()));
            fz -= 1;
            set_freezes(fz);
        }
    }
}

/// Estado do aprendizado agora.
pub struct Status {
    pub streak: u32,
    pub today_done: bool,
    pub xp_today: u32,
    pub goal: u32,
    pub best: u32,
    pub freezes: u32,
    /// XP dos últimos 7 dias (o último é hoje) e se cada dia contou.
    pub week: [(u32, bool); 7],
}

pub fn status() -> Status {
    let t = today();
    settle(t);
    let lg = log();
    let mut week = [(0, false); 7];
    for (k, w) in week.iter_mut().enumerate() {
        let d = t - 6 + k as i64;
        *w = (xp_on(&lg, d), kept(&lg, d));
    }
    // Melhor ofensiva: maior corrida de dias que contaram.
    let mut days: Vec<i64> = lg.iter().map(|(d, _, _)| *d).collect();
    days.sort();
    days.dedup();
    let (mut best, mut run, mut prev) = (0u32, 0u32, i64::MIN);
    for d in days.into_iter().filter(|d| kept(&lg, *d)) {
        run = if d == prev + 1 { run + 1 } else { 1 };
        best = best.max(run);
        prev = d;
    }
    Status { streak: streak_in(&lg, t), today_done: kept(&lg, t), xp_today: xp_on(&lg, t), goal: goal(), best, freezes: freezes(), week }
}

/// Motivo da comemoração.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cel {
    /// A ofensiva cresceu hoje (dias; ganhou protetor?).
    Streak(u32, bool),
    /// Meta do dia batida.
    Goal(u32),
}

/// Soma XP de uma atividade. Devolve a comemoração, se houver.
pub fn add(xp: u32, src: &str) -> Option<Cel> {
    if xp == 0 {
        return None;
    }
    let t = today();
    settle(t);
    let lg = log();
    let before = xp_on(&lg, t);
    append(t, xp, src);
    let after = before + xp;
    if before < MIN_DAY && after >= MIN_DAY && !lg.iter().any(|(d, _, s)| *d == t && s == "protetor") {
        let lg = log();
        let s = streak_in(&lg, t);
        let earned = s % 7 == 0 && freezes() < MAX_FREEZE;
        if earned {
            set_freezes(freezes() + 1);
        }
        return Some(Cel::Streak(s, earned));
    }
    let g = goal();
    (before < g && after >= g).then_some(Cel::Goal(g))
}

// ---------- Desenho ----------

/// Chama (desenho do Codex; sem ele, um disco). `lit` = ofensiva viva hoje.
pub fn flame(fb: &mut Framebuffer, r: Rect, lit: bool) {
    if lit {
        if !crate::sprites::draw(fb, "chama", r) {
            fb.round_rect(r, r.w / 2, INK);
        }
    } else {
        // Apagada: a mesma chama, desbotada (mistura com o papel).
        if crate::sprites::draw(fb, "chama", r) {
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    fb.blend(x, y, PAPER, 165);
                }
            }
        } else {
            fb.round_outline(Rect::new(r.x + r.w / 6, r.y + r.h / 6, r.w * 2 / 3, r.h * 2 / 3), r.w / 3, 5, 0x88, PAPER);
        }
    }
}

/// Selo compacto: chama + dias e a barra de XP de hoje. Largura ~300, altura 84.
pub fn chip(fb: &mut Framebuffer, f: &Fonts, x: i32, y: i32, st: &Status) {
    let r = Rect::new(x, y, 300, 84);
    ui::card(fb, r, 42, PAPER, false);
    flame(fb, Rect::new(r.x + 12, r.y + 8, 66, 66), st.today_done);
    let n = st.streak.to_string();
    f.draw(fb, Face::Display, 44.0, r.x + 84, r.y + 58, if st.today_done { INK } else { MUTED }, &n);
    let nx = r.x + 84 + f.measure(Face::Display, 44.0, &n) + 16;
    let t = format!("{}/{} XP", st.xp_today.min(999), st.goal);
    f.draw(fb, Face::Strong, 22.0, nx, r.y + 36, MUTED, &t);
    let bar = Rect::new(nx, r.y + 48, r.x + r.w - 22 - nx, 12);
    fb.round_rect(bar, 6, 0xC8);
    let fill = (bar.w as u32 * st.xp_today.min(st.goal) / st.goal.max(1)) as i32;
    if fill > 0 {
        fb.round_rect(Rect::new(bar.x, bar.y, fill.max(12), bar.h), 6, INK);
    }
}

pub fn chip_rect(x: i32, y: i32) -> Rect {
    Rect::new(x, y, 300, 84)
}

const DAYS: [&str; 7] = ["D", "S", "T", "Q", "Q", "S", "S"];

/// Os 7 dias como bolinhas (cheia = contou), com a letra do dia embaixo.
pub fn week_row(fb: &mut Framebuffer, f: &Fonts, x: i32, y: i32, w: i32, st: &Status) {
    let step = w / 7;
    let t = today();
    for (k, (_, ok)) in st.week.iter().enumerate() {
        let cx = x + k as i32 * step + step / 2;
        let d = 64;
        let r = Rect::new(cx - d / 2, y, d, d);
        if *ok {
            fb.round_rect(r, d / 2, INK);
            ui::stroke(fb, ((cx - 14) as f32, (y + 33) as f32), ((cx - 3) as f32, (y + 44) as f32), 6.0, PAPER);
            ui::stroke(fb, ((cx - 3) as f32, (y + 44) as f32), ((cx + 16) as f32, (y + 22) as f32), 6.0, PAPER);
        } else {
            fb.round_outline(r, d / 2, 4, if k == 6 { INK } else { 0xB0 }, PAPER);
        }
        // Dia da semana (1970-01-01 foi quinta: dia 0 → índice 4).
        let wd = (t - 6 + k as i64 + 4).rem_euclid(7) as usize;
        let l = DAYS[wd];
        let lw = f.measure(Face::Strong, 24.0, l);
        f.draw(fb, Face::Strong, 24.0, cx - lw / 2, y + d + 34, if k == 6 { INK } else { MUTED }, l);
    }
}

pub fn cel_button() -> Rect {
    Rect::new(MARGIN, 1290, W - 2 * MARGIN, 104)
}

/// Tela de comemoração (por cima do módulo).
pub fn draw_celebration(fb: &mut Framebuffer, f: &Fonts, c: Cel) {
    fb.fill(fb.screen(), PAPER);
    let st = status();
    let center = |fb: &mut Framebuffer, face: Face, size: f32, y: i32, color: u8, t: &str| {
        let w = f.measure(face, size, t);
        f.draw(fb, face, size, (W - w) / 2, y, color, t);
    };
    match c {
        Cel::Streak(n, earned) => {
            flame(fb, Rect::new((W - 360) / 2, 150, 360, 360), true);
            center(fb, Face::Display, 200.0, 720, INK, &n.to_string());
            center(fb, Face::Display, 60.0, 810, INK, if n == 1 { "dia de ofensiva!" } else { "dias de ofensiva!" });
            let msg = match (n, earned) {
                (_, true) => "Uma semana inteira! Você ganhou um protetor: ele salva a ofensiva se um dia escapar.".to_string(),
                (1, _) => "Começou. Volte amanhã para a chama crescer.".to_string(),
                (n, _) if n == st.best && n > 2 => format!("Seu recorde! {n} dias seguidos aprendendo."),
                _ => "Mais um dia. Pouco todo dia é o que funciona.".to_string(),
            };
            for (k, l) in crate::chat::wrap_words(f, Face::Body, 32.0, &msg, W - 2 * MARGIN).iter().take(3).enumerate() {
                center(fb, Face::Body, 32.0, 880 + k as i32 * 44, MUTED, l);
            }
            week_row(fb, f, MARGIN, 1030, W - 2 * MARGIN, &st);
        }
        Cel::Goal(g) => {
            if !crate::sprites::draw(fb, "alvo", Rect::new((W - 340) / 2, 170, 340, 340)) {
                fb.round_outline(Rect::new((W - 300) / 2, 190, 300, 300), 150, 14, INK, PAPER);
            }
            center(fb, Face::Display, 84.0, 680, INK, "Meta do dia batida!");
            center(fb, Face::Strong, 40.0, 760, MUTED, &format!("{} XP hoje · meta {g}", st.xp_today));
            for (k, l) in crate::chat::wrap_words(f, Face::Body, 32.0, "O que vier agora é bônus. Amanhã a chama espera por você.", W - 2 * MARGIN).iter().take(2).enumerate() {
                center(fb, Face::Body, 32.0, 850 + k as i32 * 44, MUTED, l);
            }
            week_row(fb, f, MARGIN, 1000, W - 2 * MARGIN, &st);
        }
    }
    ui::button(fb, f, cel_button(), "Continuar", true);
}

/// Painel da ofensiva (para o topo dos módulos de aprender): chama, dias, protetores e a semana.
pub fn draw_panel(fb: &mut Framebuffer, f: &Fonts, r: Rect, st: &Status) {
    ui::card(fb, r, 30, PAPER, false);
    flame(fb, Rect::new(r.x + 20, r.y + 18, 110, 110), st.today_done);
    let n = st.streak.to_string();
    f.draw(fb, Face::Display, 70.0, r.x + 146, r.y + 84, if st.today_done { INK } else { MUTED }, &n);
    let nx = r.x + 146 + f.measure(Face::Display, 70.0, &n) + 14;
    f.draw(fb, Face::Strong, 26.0, nx, r.y + 58, INK, if st.streak == 1 { "dia de ofensiva" } else { "dias de ofensiva" });
    let sub = if st.today_done {
        format!("Hoje feito · recorde {}", st.best.max(st.streak))
    } else if st.streak > 0 {
        "Faça uma lição hoje para não perder".to_string()
    } else {
        "Uma lição hoje acende a chama".to_string()
    };
    f.draw(fb, Face::Body, 24.0, nx, r.y + 90, MUTED, &crate::apps::fit(f, Face::Body, 24.0, &sub, r.x + r.w - 150 - nx));
    // Protetores guardados, à direita.
    for k in 0..st.freezes.min(MAX_FREEZE) as i32 {
        let sr = Rect::new(r.x + r.w - 76 - k * 62, r.y + 22, 56, 56);
        if !crate::sprites::draw(fb, "escudo", sr) {
            fb.round_outline(sr, 18, 4, INK, PAPER);
        }
    }
    // Meta de hoje: barra larga embaixo.
    let t = format!("{} de {} XP hoje", st.xp_today, st.goal);
    f.draw(fb, Face::Strong, 24.0, r.x + 24, r.y + r.h - 30, MUTED, &t);
    let bx = r.x + 24 + f.measure(Face::Strong, 24.0, &t) + 20;
    let bar = Rect::new(bx, r.y + r.h - 50, r.x + r.w - 24 - bx, 18);
    fb.round_rect(bar, 9, 0xC8);
    let fill = (bar.w as u32 * st.xp_today.min(st.goal) / st.goal.max(1)) as i32;
    if fill > 0 {
        fb.round_rect(Rect::new(bar.x, bar.y, fill.max(18), bar.h), 9, INK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lg(v: &[(i64, u32, &str)]) -> Vec<(i64, u32, String)> {
        v.iter().map(|(d, x, s)| (*d, *x, s.to_string())).collect()
    }

    #[test]
    fn ofensiva() {
        // Ontem e anteontem com lição, hoje ainda não: ofensiva 2 (viva).
        let l = lg(&[(98, 10, "licao"), (99, 12, "treino")]);
        assert_eq!(streak_in(&l, 100), 2);
        // Hoje feito: 3.
        let l2 = lg(&[(98, 10, "licao"), (99, 12, "treino"), (100, 10, "licao")]);
        assert_eq!(streak_in(&l2, 100), 3);
        // Buraco ontem: zera (sem protetor).
        let l3 = lg(&[(97, 10, "licao"), (98, 10, "licao")]);
        assert_eq!(streak_in(&l3, 100), 0);
        // Protetor no buraco mantém.
        let l4 = lg(&[(97, 10, "licao"), (98, 10, "licao"), (99, 0, "protetor")]);
        assert_eq!(streak_in(&l4, 100), 3);
        // Pouco XP não conta.
        let l5 = lg(&[(99, 4, "treino")]);
        assert_eq!(streak_in(&l5, 100), 0);
    }
}
