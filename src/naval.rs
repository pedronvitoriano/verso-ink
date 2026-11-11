//! Batalha naval 10×10 (navios de 5, 4, 3, 3 e 2). Contra a CPU (caça e alvo) ou duas pessoas,
//! passando o Kindle com uma tela de troca que esconde os mares.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 10;
const SHIPS: [usize; 5] = [5, 4, 3, 3, 2];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Menu,
    Cpu(u8),
    Local,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Posicionando (pode sortear de novo).
    Setup(u8),
    /// Tela de troca entre jogadores (local).
    Handoff(u8),
    Play,
    Over(u8),
}

#[derive(Clone)]
struct Sea {
    /// 0 água, 1..=5 navio (índice + 1).
    ship: [u8; N * N],
    shot: [bool; N * N],
}

impl Sea {
    fn random(rng: &mut Rng) -> Sea {
        let mut ship = [0u8; N * N];
        for (k, &len) in SHIPS.iter().enumerate() {
            loop {
                let horiz = rng.next() % 2 == 0;
                let r = (rng.next() % N as u64) as usize;
                let c = (rng.next() % N as u64) as usize;
                let cells: Vec<usize> = (0..len).map(|i| if horiz { (r, c + i) } else { (r + i, c) }).filter(|&(a, b)| a < N && b < N).map(|(a, b)| a * N + b).collect();
                // Sem encostar em outro navio (nem na diagonal).
                let free = cells.len() == len
                    && cells.iter().all(|&i| {
                        let (a, b) = ((i / N) as i32, (i % N) as i32);
                        (-1..=1).all(|dr| (-1..=1).all(|dc| {
                            let (x, y) = (a + dr, b + dc);
                            !(0..N as i32).contains(&x) || !(0..N as i32).contains(&y) || ship[(x as usize) * N + y as usize] == 0
                        }))
                    });
                if free {
                    for i in cells {
                        ship[i] = k as u8 + 1;
                    }
                    break;
                }
            }
        }
        Sea { ship, shot: [false; N * N] }
    }

    fn sunk(&self, k: u8) -> bool {
        (0..N * N).filter(|&i| self.ship[i] == k).all(|i| self.shot[i])
    }

    fn all_sunk(&self) -> bool {
        (1..=SHIPS.len() as u8).all(|k| self.sunk(k))
    }

    fn afloat(&self) -> usize {
        (1..=SHIPS.len() as u8).filter(|&k| !self.sunk(k)).count()
    }
}

pub struct Naval {
    mode: Mode,
    phase: Phase,
    /// Mar de cada jogador (0 = jogador 1 / você, 1 = jogador 2 / CPU).
    seas: [Sea; 2],
    turn: u8,
    view_own: bool,
    msg: String,
    rng: Rng,
}

const MENU: [&str; 4] = ["Contra a CPU · Fácil", "Contra a CPU · Médio", "Contra a CPU · Difícil", "Duas pessoas"];

impl Naval {
    pub fn new() -> Self {
        let mut rng = Rng::new();
        let seas = [Sea::random(&mut rng), Sea::random(&mut rng)];
        Naval { mode: Mode::Menu, phase: Phase::Setup(0), seas, turn: 0, view_own: false, msg: String::new(), rng }
    }

    fn geom() -> (i32, i32, i32) {
        minigame::grid_geom(N as i32, N as i32, 300, 880)
    }

    fn cell(i: usize) -> Rect {
        let (x0, y0, s) = Self::geom();
        Rect::new(x0 + (i % N) as i32 * s, y0 + (i / N) as i32 * s, s, s)
    }

    fn name(&self, p: u8) -> &'static str {
        match (self.mode, p) {
            (Mode::Cpu(_), 0) => "Você",
            (Mode::Cpu(_), _) => "CPU",
            (_, 0) => "Jogador 1",
            _ => "Jogador 2",
        }
    }

    /// Atira no mar do adversário de `p`. Devolve a frase do resultado.
    fn fire(&mut self, p: u8, i: usize) -> Option<String> {
        let sea = &mut self.seas[1 - p as usize];
        if sea.shot[i] {
            return None;
        }
        sea.shot[i] = true;
        let k = sea.ship[i];
        let pos = format!("{}{}", (b'A' + (i / N) as u8) as char, i % N + 1);
        let text = if k == 0 {
            format!("{pos}: água.")
        } else if sea.sunk(k) {
            format!("{pos}: afundou um navio de {}!", SHIPS[k as usize - 1])
        } else {
            format!("{pos}: acertou!")
        };
        if sea.all_sunk() {
            self.phase = Phase::Over(p);
        } else if k == 0 {
            self.turn = 1 - p;
            if self.mode == Mode::Local {
                self.phase = Phase::Handoff(self.turn);
            }
        }
        Some(text)
    }

    fn draw_sea(&self, fb: &mut Framebuffer, f: &Fonts, owner: usize, reveal: bool) {
        let sea = &self.seas[owner];
        let (x0, y0, s) = Self::geom();
        // Moldura antes das casas (ela pinta o miolo).
        fb.round_outline(Rect::new(x0 - 6, y0 - 6, s * N as i32 + 12, s * N as i32 + 12), 10, 4, INK, PAPER);
        for i in 0..N * N {
            let r = Self::cell(i);
            fb.fill(Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), 0xF2);
            let k = sea.ship[i];
            let show_ship = k != 0 && (reveal || (sea.shot[i] && sea.sunk(k)));
            if show_ship {
                fb.round_rect(Rect::new(r.x + 5, r.y + 5, r.w - 10, r.h - 10), 10, if sea.shot[i] { INK } else { 0x99 });
            }
            if sea.shot[i] {
                let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                if k == 0 {
                    fb.round_rect(Rect::new(cx - 6, cy - 6, 12, 12), 6, MUTED);
                } else {
                    let d = s as f32 * 0.22;
                    let col = if show_ship { PAPER } else { INK };
                    ui::stroke(fb, (cx as f32 - d, cy as f32 - d), (cx as f32 + d, cy as f32 + d), 6.0, col);
                    ui::stroke(fb, (cx as f32 + d, cy as f32 - d), (cx as f32 - d, cy as f32 + d), 6.0, col);
                }
            }
        }
        for k in 0..=N as i32 {
            fb.fill(Rect::new(x0 + k * s - 1, y0, 2, s * N as i32), 0xBB);
            fb.fill(Rect::new(x0, y0 + k * s - 1, s * N as i32, 2), 0xBB);
        }
        let _ = f;
    }
}

impl MiniGame for Naval {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.mode == Mode::Menu {
            minigame::draw_menu(fb, f, "Naval", "Afunde os cinco navios do adversário.", &MENU);
            return;
        }
        match self.phase {
            Phase::Handoff(p) => {
                minigame::header(fb, f, "Batalha naval", "");
                f.draw(fb, Face::Display, 90.0, MARGIN, 600, INK, &format!("Vez do {}", self.name(p)));
                f.draw(fb, Face::Body, 38.0, MARGIN, 680, INK, "Passe o Kindle e toque em Pronto.");
                f.draw(fb, Face::Body, 32.0, MARGIN, 740, MUTED, &self.msg);
                minigame::pill(fb, f, minigame::bottom_button(0, 1), "Pronto", true);
                return;
            }
            Phase::Setup(p) => {
                let who = if self.mode == Mode::Local { format!("{}: seus navios", self.name(p)) } else { "Seus navios".into() };
                minigame::header(fb, f, "Batalha naval", &who);
            }
            _ => {
                let status = match self.phase {
                    Phase::Over(p) => if self.name(p) == "Você" { "Você venceu! Frota afundada.".into() } else { format!("{} venceu.", self.name(p)) },
                    _ => {
                        let left = self.seas[1 - self.turn as usize].afloat();
                        format!("{} atira · {} navios inimigos", self.name(self.turn), left)
                    }
                };
                minigame::header(fb, f, "Batalha naval", &status);
            }
        }
        let me = self.turn as usize;
        let (owner, reveal) = match self.phase {
            Phase::Setup(p) => (p as usize, true),
            Phase::Over(_) => (1 - me, true),
            _ if self.view_own => (me, true),
            _ => (1 - me, false),
        };
        self.draw_sea(fb, f, owner, reveal);
        f.draw(fb, Face::Body, 28.0, MARGIN, 1250, MUTED, &self.msg);
        let buttons: Vec<&str> = match self.phase {
            Phase::Setup(_) => vec!["Sortear de novo", "Começar"],
            Phase::Over(_) => vec!["Novo jogo"],
            _ => vec![if self.view_own { "Ver mar inimigo" } else { "Ver meu mar" }, "Novo jogo"],
        };
        for (k, l) in buttons.iter().enumerate() {
            minigame::pill(fb, f, minigame::bottom_button(k, buttons.len()), l, matches!(self.phase, Phase::Setup(_)) && k == 1);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.mode == Mode::Menu {
            return (0..MENU.len()).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        match self.phase {
            Phase::Handoff(_) => vec![(810, minigame::bottom_button(0, 1))],
            Phase::Setup(_) => vec![(811, minigame::bottom_button(0, 2)), (812, minigame::bottom_button(1, 2))],
            Phase::Over(_) => vec![(813, minigame::bottom_button(0, 1))],
            Phase::Play => {
                let mut v = vec![(814, minigame::bottom_button(0, 2)), (813, minigame::bottom_button(1, 2))];
                let my_turn = self.mode == Mode::Local || self.turn == 0;
                if my_turn && !self.view_own {
                    let sea = &self.seas[1 - self.turn as usize];
                    v.extend((0..N * N).filter(|&i| !sea.shot[i]).map(|i| (i as u32, Self::cell(i))));
                }
                v
            }
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900..=903 => {
                let mode = if id == 903 { Mode::Local } else { Mode::Cpu((id - 900) as u8) };
                *self = Naval { mode, ..Naval::new() };
                Redraw::Full
            }
            810 => {
                self.phase = Phase::Play;
                self.view_own = false;
                Redraw::Full
            }
            811 => {
                if let Phase::Setup(p) = self.phase {
                    self.seas[p as usize] = Sea::random(&mut self.rng);
                }
                Redraw::Full
            }
            812 => {
                // Local: o jogador 2 posiciona depois do 1 (com troca no meio).
                self.phase = match (self.mode, self.phase) {
                    (Mode::Local, Phase::Setup(0)) => Phase::Setup(1),
                    (Mode::Local, _) => Phase::Handoff(0),
                    _ => Phase::Play,
                };
                self.msg = String::new();
                Redraw::Full
            }
            813 => {
                self.mode = Mode::Menu;
                Redraw::Full
            }
            814 => {
                self.view_own = !self.view_own;
                Redraw::Full
            }
            i if (i as usize) < N * N => {
                if let Some(t) = self.fire(self.turn, i as usize) {
                    self.msg = format!("{}: {t}", self.name(if matches!(self.phase, Phase::Handoff(_)) { 1 - self.turn } else { self.turn }));
                }
                if matches!(self.phase, Phase::Handoff(_) | Phase::Over(_)) { Redraw::Full } else { Redraw::Part(Rect::new(0, 170, W, 1448 - 170)) }
            }
            _ => Redraw::None,
        }
    }

    fn cpu_turn(&self) -> bool {
        matches!(self.mode, Mode::Cpu(_)) && self.turn == 1 && self.phase == Phase::Play
    }

    /// Caça e alvo: depois de acertar, tenta os vizinhos; no difícil, caça em xadrez (paridade).
    fn cpu_move(&mut self) {
        let Mode::Cpu(level) = self.mode else { return };
        let sea = &self.seas[0];
        let hit_open: Vec<usize> = (0..N * N).filter(|&i| sea.shot[i] && sea.ship[i] != 0 && !sea.sunk(sea.ship[i])).collect();
        let mut cands: Vec<usize> = Vec::new();
        if level >= 1 {
            for &h in &hit_open {
                let (r, c) = (h / N, h % N);
                for (dr, dc) in [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)] {
                    let (a, b) = (r as i32 + dr, c as i32 + dc);
                    if (0..N as i32).contains(&a) && (0..N as i32).contains(&b) {
                        let j = a as usize * N + b as usize;
                        if !sea.shot[j] {
                            cands.push(j);
                        }
                    }
                }
            }
        }
        if cands.is_empty() {
            cands = (0..N * N).filter(|&i| !sea.shot[i] && (level < 2 || (i / N + i % N) % 2 == 0)).collect();
            if cands.is_empty() {
                cands = (0..N * N).filter(|&i| !sea.shot[i]).collect();
            }
        }
        let i = cands[(self.rng.next() % cands.len() as u64) as usize];
        if let Some(t) = self.fire(1, i) {
            self.msg = format!("CPU: {t}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frota_tem_17_casas_e_afunda() {
        let mut rng = Rng::new();
        let mut s = Sea::random(&mut rng);
        assert_eq!(s.ship.iter().filter(|&&k| k != 0).count(), 17);
        for i in 0..N * N {
            s.shot[i] = true;
        }
        assert!(s.all_sunk());
    }
}
