//! Batalha naval 10×10 com a frota do Batalha-Naval do Pedro (Java, 2º período): porta-aviões (5),
//! 2 navios-tanque (4), 2 contratorpedeiros (3) e 3 submarinos (2); quem acerta joga de novo.
//! Contra a CPU (caça e alvo) ou duas pessoas, passando o Kindle com uma tela de troca.

use crate::fb::{Framebuffer, Rect};
use crate::fases;
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 10;
const SHIPS: [usize; 8] = [5, 4, 4, 3, 3, 2, 2, 2];
const NAMES: [&str; 8] = ["o porta-aviões", "um navio-tanque", "um navio-tanque", "um contratorpedeiro", "um contratorpedeiro", "um submarino", "um submarino", "um submarino"];

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
    /// Local: troca antes do jogador 2 posicionar (esconde o mar do jogador 1).
    SetupHandoff,
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

const PIECE: [&str; 8] = ["porta-aviões", "navio-tanque", "navio-tanque", "contratorpedeiro", "contratorpedeiro", "submarino", "submarino", "submarino"];

impl Sea {
    fn empty() -> Sea {
        Sea { ship: [0; N * N], shot: [false; N * N] }
    }

    /// Casas do navio `k` começando em `start` (proa) na orientação dada, se couber sem encostar.
    fn fit(&self, k: usize, start: usize, horiz: bool) -> Option<Vec<usize>> {
        let (r, c) = (start / N, start % N);
        let len = SHIPS[k];
        let cells: Vec<usize> = (0..len).map(|i| if horiz { (r, c + i) } else { (r + i, c) }).filter(|&(a, b)| a < N && b < N).map(|(a, b)| a * N + b).collect();
        let ok = cells.len() == len
            && cells.iter().all(|&i| {
                let (a, b) = ((i / N) as i32, (i % N) as i32);
                (-1..=1).all(|dr| (-1..=1).all(|dc| {
                    let (x, y) = (a + dr, b + dc);
                    !(0..N as i32).contains(&x) || !(0..N as i32).contains(&y) || self.ship[(x as usize) * N + y as usize] == 0
                }))
            });
        ok.then_some(cells)
    }

    fn place(&mut self, k: usize, start: usize, horiz: bool) -> bool {
        match self.fit(k, start, horiz) {
            Some(cells) => {
                for i in cells {
                    self.ship[i] = k as u8 + 1;
                }
                true
            }
            None => false,
        }
    }

    fn remove(&mut self, k: usize) {
        for v in self.ship.iter_mut() {
            if *v == k as u8 + 1 {
                *v = 0;
            }
        }
    }

    fn random(rng: &mut Rng) -> Sea {
        // Com 8 navios sem encostar, um sorteio pode ficar sem saída: recomeça a frota.
        'fleet: loop {
        let mut ship = [0u8; N * N];
        for (k, &len) in SHIPS.iter().enumerate() {
            let mut tries = 0;
            loop {
                tries += 1;
                if tries > 400 {
                    continue 'fleet;
                }
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
        return Sea { ship, shot: [false; N * N] };
        }
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
    /// Posicionamento manual: próximo navio e orientação.
    placing: usize,
    horiz: bool,
    /// Fase contra a CPU (0 = treino ou duas pessoas).
    fase: u32,
    practice: bool,
    prog: fases::Progress,
}

/// Força da CPU na fase: caça em volta dos acertos (fase 4) e atira em xadrez (fase 10).
pub fn cpu_level(k: u32) -> u8 {
    crate::caixas::cpu_level(k)
}

const MENU: [&str; 4] = ["Contra a CPU · Fácil", "Contra a CPU · Médio", "Contra a CPU · Difícil", "Duas pessoas"];

impl Naval {
    pub fn new() -> Self {
        let mut rng = Rng::new();
        let seas = [Sea::random(&mut rng), Sea::random(&mut rng)];
        Naval { mode: Mode::Menu, phase: Phase::Setup(0), seas, turn: 0, view_own: false, msg: String::new(), rng, placing: 0, horiz: true, fase: 0, practice: false, prog: fases::Progress::load("naval") }
    }

    /// Começa uma partida nova mantendo o progresso das fases.
    fn reset(&mut self, mode: Mode, fase: u32) {
        let prog = std::mem::replace(&mut self.prog, fases::Progress::load_from(std::path::PathBuf::new()));
        *self = Naval { mode, prog, ..Naval::new() };
        self.fase = fase;
        // Você posiciona a sua frota; a da CPU é sorteada.
        self.seas[0] = Sea::empty();
        if mode == Mode::Local {
            self.seas[1] = Sea::empty();
        }
    }

    fn title(&self) -> String {
        if self.fase > 0 { fases::title(self.fase) } else { "Batalha naval".to_string() }
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
            format!("{pos}: afundou {}!", NAMES[k as usize - 1])
        } else {
            format!("{pos}: acertou!")
        };
        if sea.all_sunk() {
            self.phase = Phase::Over(p);
            if p == 0 && self.fase > 0 {
                self.prog.record_win(self.fase, fases::today());
            }
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
                let m = s / 8;
                let art = Rect::new(r.x + m, r.y + m, r.w - 2 * m, r.h - 2 * m);
                if k == 0 && crate::sprites::draw(fb, "peca-agua", art) {
                } else if k != 0 && crate::sprites::draw(fb, "peca-acerto", art) {
                } else if k == 0 {
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
    fn risky(&self, id: u32) -> bool {
        id == 813 && matches!(self.phase, Phase::Play)
    }

    fn at_menu(&self) -> bool {
        self.mode == Mode::Menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.mode == Mode::Menu && self.practice {
            minigame::draw_menu(fb, f, "Treino", "Escolha a força da CPU. Não conta para as fases.", &MENU[..3]);
            return;
        }
        if self.mode == Mode::Menu {
            let main = format!("Fase {} · contra a CPU", self.prog.level);
            fases::draw_menu(fb, f, "Naval", "Afunde os 8 navios da frota adversária.", &self.prog, &main, &["Treino contra a CPU", "Duas pessoas"]);
            return;
        }
        match self.phase {
            Phase::Handoff(p) => {
                minigame::header(fb, f, &self.title(), "");
                f.draw(fb, Face::Display, 90.0, MARGIN, 600, INK, &format!("Vez do {}", self.name(p)));
                f.draw(fb, Face::Body, 38.0, MARGIN, 680, INK, "Passe o Kindle e toque em Pronto.");
                f.draw(fb, Face::Body, 32.0, MARGIN, 740, MUTED, &self.msg);
                minigame::pill(fb, f, minigame::bottom_button(0, 1), "Pronto", true);
                return;
            }
            Phase::SetupHandoff => {
                minigame::header(fb, f, &self.title(), "");
                f.draw(fb, Face::Display, 80.0, MARGIN, 600, INK, "Vez do Jogador 2");
                f.draw(fb, Face::Body, 38.0, MARGIN, 680, INK, "Passe o Kindle para posicionar a frota.");
                minigame::pill(fb, f, minigame::bottom_button(0, 1), "Pronto", true);
                return;
            }
            Phase::Setup(p) => {
                let who = if self.mode == Mode::Local { format!("{}: ", self.name(p)) } else { String::new() };
                let text = if self.placing < SHIPS.len() {
                    format!("{who}{} o {} ({}) · {} de 8", if who.is_empty() { "Posicione" } else { "posicione" }, PIECE[self.placing], SHIPS[self.placing], self.placing + 1)
                } else {
                    format!("{who}{}", if who.is_empty() { "Frota pronta. Toque em Começar." } else { "frota pronta. Toque em Começar." })
                };
                minigame::header(fb, f, &self.title(), &text);
            }
            _ => {
                let status = match self.phase {
                    Phase::Over(p) => if self.name(p) == "Você" { "Você venceu! Frota afundada.".into() } else { format!("{} venceu.", self.name(p)) },
                    _ => {
                        let left = self.seas[1 - self.turn as usize].afloat();
                        format!("{} atira · {} navios inimigos", self.name(self.turn), left)
                    }
                };
                minigame::header(fb, f, &self.title(), &status);
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
        let hint = if matches!(self.phase, Phase::Setup(_)) && self.placing < SHIPS.len() && self.msg.is_empty() { "Toque na casa da proa (topo ou ponta esquerda do navio)." } else { self.msg.as_str() };
        f.draw(fb, Face::Body, 28.0, MARGIN, 1250, MUTED, hint);
        if let (Phase::Over(p), true) = (self.phase, self.fase > 0) {
            if p == 0 {
                return fases::draw_win(fb, f, &self.prog);
            }
            return fases::draw_retry(fb, f, self.fase);
        }
        let buttons: Vec<&str> = match self.phase {
            Phase::Setup(_) => vec![if self.horiz { "Girar: deitado" } else { "Girar: em pé" }, "Sortear", "Desfazer", "Começar"],
            Phase::Over(_) => vec!["Novo jogo"],
            _ => vec![if self.view_own { "Ver mar inimigo" } else { "Ver meu mar" }, "Novo jogo"],
        };
        for (k, l) in buttons.iter().enumerate() {
            let ready = self.placing >= SHIPS.len();
            minigame::pill(fb, f, minigame::bottom_button(k, buttons.len()), l, matches!(self.phase, Phase::Setup(_)) && k == 3 && ready);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.mode == Mode::Menu && self.practice {
            return (0..3).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.mode == Mode::Menu {
            return (0..3).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        if matches!(self.phase, Phase::Over(_)) && self.fase > 0 {
            return fases::win_targets();
        }
        match self.phase {
            Phase::Handoff(_) => vec![(810, minigame::bottom_button(0, 1))],
            Phase::SetupHandoff => vec![(817, minigame::bottom_button(0, 1))],
            Phase::Setup(_) => {
                let mut v = vec![(815, minigame::bottom_button(0, 4)), (811, minigame::bottom_button(1, 4)), (816, minigame::bottom_button(2, 4))];
                if self.placing >= SHIPS.len() {
                    v.push((812, minigame::bottom_button(3, 4)));
                } else {
                    v.extend((0..N * N).map(|i| (i as u32, Self::cell(i))));
                }
                v
            }
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

    fn back(&mut self) -> bool {
        // Só o submenu de treino volta ao menu do jogo; da partida, Início sai e o app guarda
        // a partida para continuar depois.
        if self.mode == Mode::Menu && self.practice {
            self.practice = false;
            return true;
        }
        false
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 | fases::WIN_NEXT => {
                let k = self.prog.level;
                self.reset(Mode::Cpu(cpu_level(k)), k);
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            902 => {
                self.reset(Mode::Local, 0);
                Redraw::Full
            }
            910..=912 => {
                self.reset(Mode::Cpu((id - 910) as u8), 0);
                Redraw::Full
            }
            fases::WIN_MENU => {
                self.mode = Mode::Menu;
                Redraw::Full
            }
            815 => {
                self.horiz = !self.horiz;
                Redraw::Full
            }
            816 => {
                if let Phase::Setup(p) = self.phase {
                    if self.placing > 0 {
                        self.placing -= 1;
                        self.seas[p as usize].remove(self.placing);
                    }
                }
                self.msg.clear();
                Redraw::Full
            }
            817 => {
                self.phase = Phase::Setup(1);
                self.placing = 0;
                self.msg.clear();
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
                    self.placing = SHIPS.len();
                }
                self.msg.clear();
                Redraw::Full
            }
            812 => {
                // Local: o jogador 2 posiciona depois do 1 (com troca no meio).
                self.phase = match (self.mode, self.phase) {
                    (Mode::Local, Phase::Setup(0)) => Phase::SetupHandoff,
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
            i if (i as usize) < N * N && matches!(self.phase, Phase::Setup(_)) => {
                let Phase::Setup(p) = self.phase else { return Redraw::None };
                if self.seas[p as usize].place(self.placing, i as usize, self.horiz) {
                    self.placing += 1;
                    self.msg.clear();
                } else {
                    self.msg = "Não cabe aí (ou encosta em outro navio).".into();
                }
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
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
    fn frota_tem_25_casas_e_afunda() {
        let mut rng = Rng::new();
        let mut s = Sea::random(&mut rng);
        assert_eq!(s.ship.iter().filter(|&&k| k != 0).count(), 25);
        for i in 0..N * N {
            s.shot[i] = true;
        }
        assert!(s.all_sunk());
    }
}
