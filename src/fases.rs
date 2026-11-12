//! Esquema de fases comum aos jogos com dificuldade: a dificuldade sobe sozinha a cada vitória,
//! de 5 em 5 (a partir da 10) vem uma fase difícil e de 10 em 10 um desafio. Guarda a fase atual,
//! a sequência de dias com vitória e o total resolvido, por jogo, em `data/fases/<jogo>.txt`.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{INK, MUTED};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Normal,
    Hard,
    Challenge,
}

pub fn kind(k: u32) -> Kind {
    if k >= 10 && k % 10 == 0 {
        Kind::Challenge
    } else if k >= 10 && k % 5 == 0 {
        Kind::Hard
    } else {
        Kind::Normal
    }
}

/// "Fase 15 · Difícil".
pub fn title(k: u32) -> String {
    match kind(k) {
        Kind::Normal => format!("Fase {k}"),
        Kind::Hard => format!("Fase {k} · Difícil"),
        Kind::Challenge => format!("Fase {k} · Desafio"),
    }
}

/// Mistura de 0 (fase 1) a 1 (bem avançado), útil para escalar parâmetros.
pub fn ramp(k: u32, full_at: u32) -> f32 {
    ((k.saturating_sub(1)) as f32 / full_at.max(1) as f32).min(1.0)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    file: PathBuf,
    pub level: u32,
    pub streak: u32,
    pub best: u32,
    pub last_day: i64,
    pub solved: u32,
}

impl Progress {
    pub fn load(game: &str) -> Self {
        let d = crate::home_dir().join("data/fases");
        let _ = std::fs::create_dir_all(&d);
        Self::load_from(d.join(format!("{game}.txt")))
    }

    pub fn load_from(file: PathBuf) -> Self {
        let mut s = Progress { file, level: 1, streak: 0, best: 0, last_day: i64::MIN, solved: 0 };
        for l in std::fs::read_to_string(&s.file).unwrap_or_default().lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            match (k, v.trim().parse::<i64>()) {
                ("fase", Ok(v)) => s.level = v.max(1) as u32,
                ("sequencia", Ok(v)) => s.streak = v as u32,
                ("melhor", Ok(v)) => s.best = v as u32,
                ("ultimo_dia", Ok(v)) => s.last_day = v,
                ("resolvidas", Ok(v)) => s.solved = v as u32,
                _ => {}
            }
        }
        s
    }

    pub fn save(&self) {
        let tmp = self.file.with_extension("tmp");
        let text = format!("fase={}\nsequencia={}\nmelhor={}\nultimo_dia={}\nresolvidas={}\n", self.level, self.streak, self.best, self.last_day, self.solved);
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(tmp, &self.file);
        }
    }

    /// Sequência que vale hoje (zera se passou um dia sem vencer).
    pub fn current_streak(&self, today: i64) -> u32 {
        if self.last_day >= today - 1 { self.streak } else { 0 }
    }

    /// Venceu a fase `k`: avança (se era a fase atual) e conta o dia na sequência.
    pub fn record_win(&mut self, k: u32, today: i64) {
        if k != self.level {
            return;
        }
        if self.last_day != today {
            self.streak = if self.last_day == today - 1 { self.streak + 1 } else { 1 };
        }
        self.last_day = today;
        self.best = self.best.max(self.streak);
        self.solved += 1;
        self.level += 1;
        self.save();
    }
}

pub fn today() -> i64 {
    crate::data::today(crate::offset_min())
}

/// Menu de fases: a fase atual em destaque, depois as opções do jogo (treino, duas pessoas…)
/// e, embaixo, a sequência. Os alvos são `menu_row(0..)`.
pub fn draw_menu(fb: &mut Framebuffer, f: &Fonts, title: &str, subtitle: &str, p: &Progress, main: &str, extra: &[&str]) {
    let mut opts = vec![main];
    opts.extend_from_slice(extra);
    minigame::draw_menu(fb, f, title, subtitle, &opts);
    let y = minigame::menu_row(opts.len()).y + 40;
    let streak = p.current_streak(today());
    f.draw(fb, Face::Display, 56.0, MARGIN, y + 40, INK, &format!("Sequência: {streak} dia{}", if streak == 1 { "" } else { "s" }));
    let lines = [
        format!("Melhor sequência: {} · Fases vencidas: {}", p.best, p.solved),
        "A dificuldade sobe a cada fase. De 5 em 5 vem uma difícil;".to_string(),
        "de 10 em 10, um desafio.".to_string(),
    ];
    for (k, l) in lines.iter().enumerate() {
        f.draw(fb, Face::Body, 30.0, MARGIN, y + 110 + k as i32 * 50, MUTED, l);
    }
    minigame::menu_art(fb, title, y + 280);
}

/// Rodapé da vitória numa fase: sequência, aviso da próxima especial e os botões Menu e próxima.
pub const WIN_MENU: u32 = 880;
pub const WIN_NEXT: u32 = 881;

pub fn win_targets() -> Vec<(u32, Rect)> {
    vec![(WIN_MENU, minigame::bottom_button(0, 2)), (WIN_NEXT, minigame::bottom_button(1, 2))]
}

pub fn draw_win(fb: &mut Framebuffer, f: &Fonts, p: &Progress) {
    fb.fill(Rect::new(0, 1250, W, 1448 - 1250), crate::ui::PAPER);
    let streak = p.current_streak(today());
    let teaser = if kind(p.level) == Kind::Normal { "" } else { " · a próxima é especial…" };
    minigame::hint(fb, f, &format!("Sequência: {streak} dia{}{teaser}", if streak == 1 { "" } else { "s" }));
    minigame::pill(fb, f, minigame::bottom_button(0, 2), "Menu", false);
    minigame::pill(fb, f, minigame::bottom_button(1, 2), &format!("Fase {}", p.level), true);
}

/// Rodapé da derrota numa fase: tentar a mesma fase de novo.
pub fn draw_retry(fb: &mut Framebuffer, f: &Fonts, k: u32) {
    fb.fill(Rect::new(0, 1250, W, 1448 - 1250), crate::ui::PAPER);
    minigame::hint(fb, f, "A fase continua a mesma até você vencer.");
    minigame::pill(fb, f, minigame::bottom_button(0, 2), "Menu", false);
    minigame::pill(fb, f, minigame::bottom_button(1, 2), &format!("Tentar a fase {k}"), true);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn especiais_e_progresso() {
        assert_eq!(kind(5), Kind::Normal);
        assert_eq!(kind(10), Kind::Challenge);
        assert_eq!(kind(15), Kind::Hard);
        let file = std::env::temp_dir().join(format!("verso-ink-fases-{}.txt", std::process::id()));
        let mut p = Progress::load_from(file.clone());
        p.record_win(1, 100);
        p.record_win(1, 100); // fase velha não conta de novo
        assert_eq!((p.level, p.solved, p.streak), (2, 1, 1));
        p.record_win(2, 101);
        let q = Progress::load_from(file.clone());
        assert_eq!((q.level, q.streak, q.best), (3, 2, 2));
        let _ = std::fs::remove_file(file);
    }
}
