//! Domínio: conquista determinística de territórios em fases ou contra adversários.

use crate::fases;
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 16;
const PATH: [usize; N] = [0, 1, 2, 3, 7, 6, 5, 4, 8, 9, 10, 11, 15, 14, 13, 12];
const XS: [i32; 5] = [72, 292, 527, 747, 1000];
const YS: [i32; 5] = [380, 540, 720, 885, 1060];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Territory { owner: u8, troops: u16 }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage { Reinforce, Attack, Move }

#[derive(Clone, Debug, PartialEq, Eq)]
struct Board {
    cells: [Territory; N],
    stage: Stage,
    remaining: u16,
    turn: u32,
}

fn adjacent(a: usize, b: usize) -> bool {
    a < N && b < N && ((a / 4 == b / 4 && a.abs_diff(b) == 1) || a.abs_diff(b) == 4)
}

fn reinforcement(cells: &[Territory; N], owner: u8) -> u16 {
    ((cells.iter().filter(|c| c.owner == owner).count() / 3) as u16).max(3)
}

/// O ataque usa todos os soldados menos um; os dois lados perdem a mesma quantidade.
fn attack(cells: &mut [Territory; N], a: usize, b: usize, owner: u8) -> bool {
    if !adjacent(a, b) || cells[a].owner != owner || cells[b].owner == owner || cells[a].troops < 2 { return false; }
    let attacking = cells[a].troops - 1;
    let defending = cells[b].troops;
    cells[a].troops = 1;
    if attacking > defending {
        cells[b] = Territory { owner, troops: attacking - defending };
    } else {
        cells[b].troops = defending.saturating_sub(attacking).max(1);
    }
    true
}

fn move_troops(cells: &mut [Territory; N], a: usize, b: usize, owner: u8) -> bool {
    if !adjacent(a, b) || cells[a].owner != owner || cells[b].owner != owner || cells[a].troops < 2 { return false; }
    let amount = cells[a].troops - 1;
    cells[a].troops = 1;
    cells[b].troops = cells[b].troops.saturating_add(amount);
    true
}

/// Uma rotação ou reflexão do percurso atravessa os 16 territórios vizinhos.
fn puzzle_path(level: u32) -> [usize; N] {
    let variant = level.saturating_sub(1) % 8;
    PATH.map(|i| {
        let (mut x, mut y) = (i % 4, i / 4);
        if variant & 1 != 0 { x = 3 - x; }
        if variant & 2 != 0 { y = 3 - y; }
        if variant & 4 != 0 { std::mem::swap(&mut x, &mut y); }
        y * 4 + x
    })
}

/// Confere que o percurso pode ser vencido sem sorte, guardando reforços na frente.
fn path_solvable(mut cells: [Territory; N], path: &[usize; N], initial: u16) -> bool {
    cells[path[0]].troops = initial;
    let mut frontier = 0;
    for _ in 0..3 {
        let reinforcements = reinforcement(&cells, 0);
        cells[path[frontier]].troops += reinforcements;
        while frontier < N - 1 && cells[path[frontier]].troops > cells[path[frontier + 1]].troops + 1 {
            attack(&mut cells, path[frontier], path[frontier + 1], 0);
            frontier += 1;
        }
        if frontier == N - 1 { return true; }
    }
    false
}

fn puzzle(level: u32) -> Board {
    let path = puzzle_path(level);
    let base = 1 + (level.saturating_sub(1) / 4).min(80) as u16;
    let spread = 1 + (level / 4).min(6) as u64;
    let mut rng = minigame::Rng::seeded((level as u64).wrapping_add(0x6d617061));
    let mut cells = [Territory { owner: 1, troops: base }; N];
    for &i in &path[1..] { cells[i].troops += (rng.next() % spread) as u16; }
    cells[path[0]] = Territory { owner: 0, troops: 1 };
    let cost: u16 = path[1..].iter().map(|&i| cells[i].troops + 1).sum();
    // Reduz a folga nas fases seguintes, mas só até o limite comprovadamente solucionável.
    let desired = 6 + (level / 3).min(10) as u16;
    let slack = (1..=desired).rev().find(|&n| cost > n && path_solvable(cells, &path, cost - n)).unwrap_or(1);
    cells[path[0]].troops = cost - slack;
    Board { cells, stage: Stage::Reinforce, remaining: 3, turn: 1 }
}

fn battle(opponents: u8) -> Board {
    let mut cells = [Territory { owner: 0, troops: 3 }; N];
    for (i, cell) in cells.iter_mut().enumerate() {
        let owner = if opponents == 1 { ((i / 4 + i % 4) % 2) as u8 } else { ((i / 4 + i % 4) % 3) as u8 };
        *cell = Territory { owner, troops: 3 };
    }
    let remaining = reinforcement(&cells, 0);
    Board { cells, stage: Stage::Reinforce, remaining, turn: 1 }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode { Phase, Daily, Cpu(u8) }

struct Daily { last: i64, streak: u32 }
impl Daily {
    fn load() -> Self {
        let s = std::fs::read_to_string(crate::home_dir().join("data/dominio-dia.txt")).unwrap_or_default();
        let mut p = s.split_whitespace();
        Self { last: p.next().and_then(|x| x.parse().ok()).unwrap_or(i64::MIN), streak: p.next().and_then(|x| x.parse().ok()).unwrap_or(0) }
    }
    fn current(&self, day: i64) -> u32 { if self.last >= day - 1 { self.streak } else { 0 } }
    fn record(&mut self, day: i64) -> bool {
        if self.last >= day { return false; }
        self.streak = if self.last == day - 1 { self.streak + 1 } else { 1 };
        self.last = day;
        true
    }
    fn win(&mut self, day: i64) {
        if !self.record(day) { return; }
        let dir = crate::home_dir().join("data");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("dominio-dia.txt"), format!("{} {}\n", self.last, self.streak));
    }
}

pub struct Dominio {
    menu: bool,
    mode: Mode,
    level: u32,
    daily_day: i64,
    board: Board,
    selected: Option<usize>,
    history: Vec<Board>,
    result: Option<bool>,
    progress: fases::Progress,
    daily: Daily,
    cpu_pending: bool,
}

impl Dominio {
    pub fn new() -> Self {
        Self { menu: true, mode: Mode::Phase, level: 1, daily_day: fases::today(), board: puzzle(1), selected: None, history: vec![], result: None, progress: fases::Progress::load("dominio"), daily: Daily::load(), cpu_pending: false }
    }
    #[cfg(test)]
    pub fn final_para_print() -> Self {
        let mut game = Self::new();
        game.menu = false;
        game.board.cells = [Territory { owner: 0, troops: 1 }; N];
        game.result = Some(true);
        game
    }
    fn start(&mut self, mode: Mode) {
        self.mode = mode;
        self.daily_day = fases::today();
        self.level = match mode { Mode::Phase => self.progress.level, Mode::Daily => self.daily_day.rem_euclid(20) as u32 + 1, Mode::Cpu(_) => 0 };
        self.board = match mode { Mode::Cpu(n) => battle(n), _ => puzzle(self.level) };
        self.menu = false;
        self.selected = None;
        self.history.clear();
        self.result = None;
        self.cpu_pending = false;
    }
    fn footer(i: usize) -> Rect {
        let r = minigame::bottom_button(i, 2);
        Rect::new(r.x, 1274, r.w, r.h)
    }
    fn hint(fb: &mut Framebuffer, f: &Fonts, text: &str) {
        let w = f.measure(Face::Body, 26.0, text);
        f.draw(fb, Face::Body, 26.0, (W - w) / 2, 1236, MUTED, text);
    }
    fn rect(i: usize) -> Rect {
        let (x, y) = (i % 4, i / 4);
        Rect::new(XS[x], YS[y], XS[x + 1] - XS[x], YS[y + 1] - YS[y])
    }
    fn status(&self) -> String {
        if let Some(won) = self.result { return if won { "Você conquistou o mapa!" } else { "Não foi desta vez" }.into(); }
        if self.cpu_pending { return "Adversários jogando…".into(); }
        match self.board.stage {
            Stage::Reinforce => format!("Distribua {} reforço{}", self.board.remaining, if self.board.remaining == 1 { "" } else { "s" }),
            Stage::Attack => "Escolha de onde atacar".into(),
            Stage::Move => "Mova soldados ou encerre".into(),
        }
    }
    fn save_step(&mut self) { self.history.push(self.board.clone()); }
    fn check_win(&mut self) {
        if self.board.cells.iter().all(|c| c.owner == 0) {
            self.result = Some(true);
            match self.mode {
                Mode::Phase => self.progress.record_win(self.level, fases::today()),
                Mode::Daily => self.daily.win(self.daily_day),
                Mode::Cpu(_) => {}
            }
        }
    }
    fn next_turn(&mut self) {
        self.selected = None;
        self.history.clear();
        if matches!(self.mode, Mode::Cpu(_)) { self.cpu_pending = true; return; }
        if self.board.turn >= 3 { self.result = Some(false); return; }
        self.board.turn += 1;
        self.board.stage = Stage::Reinforce;
        self.board.remaining = reinforcement(&self.board.cells, 0);
    }
    fn take_cell(&mut self, i: usize) {
        if i >= N { return; }
        match self.board.stage {
            Stage::Reinforce => {
                if self.board.cells[i].owner == 0 && self.board.remaining > 0 {
                    self.save_step();
                    self.board.cells[i].troops = self.board.cells[i].troops.saturating_add(1);
                    self.board.remaining -= 1;
                }
            }
            Stage::Attack | Stage::Move => {
                if let Some(a) = self.selected {
                    let legal = adjacent(a, i) && match self.board.stage {
                        Stage::Attack => self.board.cells[i].owner != 0,
                        Stage::Move => self.board.cells[i].owner == 0,
                        _ => false,
                    };
                    if legal {
                        self.save_step();
                        if self.board.stage == Stage::Attack { attack(&mut self.board.cells, a, i, 0); }
                        else { move_troops(&mut self.board.cells, a, i, 0); }
                        self.selected = None;
                        self.check_win();
                        return;
                    }
                }
                self.selected = if self.board.cells[i].owner == 0 && self.board.cells[i].troops >= 2 { Some(i) } else { None };
            }
        }
    }
    fn draw_board(&self, fb: &mut Framebuffer, f: &Fonts) {
        for i in 0..N {
            let r = Self::rect(i);
            fb.fill(r, INK);
            let inner = Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8);
            fb.fill(inner, PAPER);
            match self.board.cells[i].owner {
                1 => {
                    // Hachura vertical, visível mesmo com a tela sem tons de cinza.
                    let mut x = inner.x + 12;
                    while x < inner.x + inner.w - 4 { fb.fill(Rect::new(x, inner.y + 4, 3, inner.h - 8), INK); x += 18; }
                }
                2 => {
                    let mut y = inner.y + 12;
                    while y < inner.y + inner.h - 4 {
                        let mut x = inner.x + 12;
                        while x < inner.x + inner.w - 4 { fb.fill(Rect::new(x, y, 5, 5), INK); x += 20; }
                        y += 20;
                    }
                }
                _ => {}
            }
            if self.selected == Some(i) { fb.round_outline(Rect::new(r.x + 8, r.y + 8, r.w - 16, r.h - 16), 12, 6, INK, PAPER); }
            let label = Rect::new(r.x + 34, r.y + (r.h - 88) / 2, r.w - 68, 88);
            fb.round_rect(label, 12, PAPER);
            fb.round_outline(label, 12, 3, INK, PAPER);
            let count = self.board.cells[i].troops.to_string();
            let tw = f.measure(Face::Display, 48.0, &count);
            f.draw(fb, Face::Display, 48.0, r.x + (r.w - tw) / 2, label.y + 61, INK, &count);
        }
    }
}

impl MiniGame for Dominio {
    fn at_menu(&self) -> bool { self.menu }
    fn summary(&self) -> String { if self.menu { String::new() } else { self.status() } }
    fn context(&self) -> String { format!("Domínio. {} Turno {}. Cada território, linha a linha: {:?}", self.status(), self.board.turn, self.board.cells) }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            let main = format!("Fase {}", self.progress.level);
            minigame::draw_menu(fb, f, "Domínio", "Conquiste o mapa, um território de cada vez.", &[&main, "Desafio de hoje", "Contra 1 adversário", "Contra 2 adversários"]);
            let line = format!("Sequência nas fases: {} dias", self.progress.current_streak(fases::today()));
            f.draw(fb, Face::Strong, 30.0, MARGIN, 1125, INK, &line);
            f.draw(fb, Face::Body, 28.0, MARGIN, 1175, MUTED, &format!("Desafios diários: {} dias seguidos", self.daily.current(fases::today())));
            minigame::menu_art(fb, "Domínio", 1200);
            return;
        }
        let title = match self.mode { Mode::Phase => fases::title(self.level), Mode::Daily => "Desafio de hoje".into(), Mode::Cpu(_) => "Domínio".into() };
        minigame::header(fb, f, &title, &self.status());
        let line = if matches!(self.mode, Mode::Cpu(_)) { format!("Turno {}  ·  Liso: você  ·  Listras e pontos: adversários", self.board.turn) } else { format!("Turno {} de 3  ·  Liso: você  ·  Listras: adversário", self.board.turn) };
        f.draw(fb, Face::Strong, 28.0, MARGIN, 300, INK, &line);
        f.draw(fb, Face::Body, 26.0, MARGIN, 346, MUTED, "O número mostra quantos soldados estão no território.");
        self.draw_board(fb, f);
        if let Some(won) = self.result {
            let hint = if won { "Todos os territórios são seus." } else if matches!(self.mode, Mode::Cpu(_)) { "Seus territórios foram conquistados." } else { "Conquiste todos os territórios em até 3 turnos." };
            Self::hint(fb, f, hint);
            minigame::pill(fb, f, Self::footer(0), "Menu", false);
            minigame::pill(fb, f, Self::footer(1), if won && self.mode == Mode::Phase { "Próxima fase" } else { "Jogar novamente" }, true);
        } else {
            let hint = match self.board.stage {
                Stage::Reinforce => "Toque nos seus territórios para distribuir os reforços.",
                Stage::Attack => "Toque no seu território e depois no vizinho inimigo.",
                Stage::Move => "Toque na origem e no destino vizinho, ou encerre o turno.",
            };
            Self::hint(fb, f, hint);
            minigame::pill(fb, f, Self::footer(0), "Desfazer", false);
            let next = match self.board.stage { Stage::Reinforce => "Começar ataque", Stage::Attack => "Mover soldados", Stage::Move => "Encerrar turno" };
            minigame::pill(fb, f, Self::footer(1), next, true);
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu { return (0..4).map(|i| (900 + i as u32, minigame::menu_row(i))).collect(); }
        let mut targets = vec![(800, Self::footer(0)), (801, Self::footer(1))];
        if self.result.is_none() && !self.cpu_pending { targets.extend((0..N).map(|i| (i as u32, Self::rect(i)))); }
        targets
    }
    fn tap(&mut self, id: u32, _: i32, _: i32) -> Redraw {
        if self.menu {
            match id { 900 => self.start(Mode::Phase), 901 => self.start(Mode::Daily), 902 => self.start(Mode::Cpu(1)), 903 => self.start(Mode::Cpu(2)), _ => return Redraw::None }
            return Redraw::Full;
        }
        if self.result.is_some() {
            match id { 800 => self.menu = true, 801 => self.start(self.mode), _ => return Redraw::None }
            return Redraw::Full;
        }
        if self.cpu_pending { return Redraw::None; }
        match id {
            0..=15 => self.take_cell(id as usize),
            800 => { if let Some(b) = self.history.pop() { self.board = b; self.selected = None; } }
            801 => match self.board.stage {
                Stage::Reinforce if self.board.remaining == 0 => { self.save_step(); self.board.stage = Stage::Attack; self.selected = None; }
                Stage::Attack => { self.save_step(); self.board.stage = Stage::Move; self.selected = None; }
                Stage::Move => self.next_turn(),
                _ => return Redraw::None,
            },
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn cpu_turn(&self) -> bool { self.cpu_pending && self.result.is_none() }
    fn cpu_move(&mut self) {
        let n = match self.mode { Mode::Cpu(n) => n, _ => return };
        for owner in 1..=n {
            if !self.board.cells.iter().any(|c| c.owner == owner) { continue; }
            let reinforcements = reinforcement(&self.board.cells, owner);
            let frontier = (0..N).filter(|&i| self.board.cells[i].owner == owner && (0..N).any(|j| adjacent(i, j) && self.board.cells[j].owner != owner)).max_by_key(|&i| self.board.cells[i].troops);
            if let Some(i) = frontier { self.board.cells[i].troops += reinforcements; }
            for _ in 0..N {
                let candidate = (0..N).flat_map(|a| (0..N).map(move |b| (a, b))).filter(|&(a, b)| adjacent(a, b) && self.board.cells[a].owner == owner && self.board.cells[b].owner != owner && self.board.cells[a].troops > self.board.cells[b].troops + 1).max_by_key(|&(a, b)| self.board.cells[a].troops - self.board.cells[b].troops);
                if let Some((a, b)) = candidate { attack(&mut self.board.cells, a, b, owner); } else { break; }
            }
        }
        self.cpu_pending = false;
        if !self.board.cells.iter().any(|c| c.owner == 0) { self.result = Some(false); return; }
        self.board.turn += 1;
        self.board.stage = Stage::Reinforce;
        self.board.remaining = reinforcement(&self.board.cells, 0);
    }
    fn back(&mut self) -> bool { if self.menu { false } else { self.menu = true; true } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fronteiras_e_combate() {
        assert!(adjacent(3, 7));
        assert!(!adjacent(3, 4));
        let mut b = puzzle(1);
        b.cells[0].troops = 6;
        b.cells[1].troops = 2;
        assert!(attack(&mut b.cells, 0, 1, 0));
        assert_eq!(b.cells[0].troops, 1);
        assert_eq!(b.cells[1], Territory { owner: 0, troops: 3 });
        assert!(!attack(&mut b.cells, 0, 1, 0));
        assert!(move_troops(&mut b.cells, 1, 0, 0));
        assert_eq!(b.cells[0].troops, 3);
    }
    #[test]
    fn ataque_sem_conquista_preserva_um_defensor() {
        let mut b = puzzle(1);
        b.cells[0].troops = 3;
        b.cells[1].troops = 5;
        assert!(attack(&mut b.cells, 0, 1, 0));
        assert_eq!(b.cells[1], Territory { owner: 1, troops: 3 });
        b.cells[0].troops = 4;
        assert!(attack(&mut b.cells, 0, 1, 0));
        assert_eq!(b.cells[1], Territory { owner: 1, troops: 1 });
    }
    #[test]
    fn reforcos_e_cpu() {
        let mut cells = battle(2).cells;
        assert_eq!(reinforcement(&cells, 0), 3);
        for c in &mut cells { c.owner = 0; }
        assert_eq!(reinforcement(&cells, 0), 5);
        let mut game = Dominio::new();
        game.start(Mode::Cpu(2));
        game.cpu_pending = true;
        game.cpu_move();
        assert!(!game.cpu_pending);
        assert!(game.board.cells.iter().all(|c| c.troops > 0));
    }
    #[test]
    fn fases_tem_solucao_sem_sorte() {
        for level in 1..=1000 {
            let b = puzzle(level);
            let path = puzzle_path(level);
            assert!(path.windows(2).all(|pair| adjacent(pair[0], pair[1])));
            assert!(path_solvable(b.cells, &path, b.cells[path[0]].troops), "fase {level}");
            if level > 1 { assert_ne!(b.cells, puzzle(level - 1).cells, "fases iguais: {} e {level}", level - 1); }
        }
    }
    #[test]
    fn sequencia_respeita_o_dia_do_desafio_e_nao_volta_no_tempo() {
        let mut daily = Daily { last: 99, streak: 4 };
        assert!(daily.record(100));
        assert_eq!((daily.last, daily.streak), (100, 5));
        assert!(!daily.record(100));
        assert!(daily.record(101));
        assert!(!daily.record(100));
        assert_eq!((daily.last, daily.streak), (101, 6));
    }
    #[test]
    fn desfaz_reforco_e_passo() {
        let mut game = Dominio::new();
        game.start(Mode::Phase);
        let before = game.board.clone();
        game.take_cell(0);
        game.tap(800, 0, 0);
        assert_eq!(game.board, before);
    }
}
