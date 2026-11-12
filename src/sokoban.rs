//! Sokoban: empurre as caixas até os alvos. Fases geradas ao contrário (das caixas já nos alvos,
//! puxando para trás), então toda fase tem solução. Controles: setas na tela.

use crate::fb::{Framebuffer, Rect};
use crate::fases;
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 9;
const LEVELS: [(usize, usize, &str); 3] = [(2, 18, "Fácil · 2 caixas"), (3, 30, "Médio · 3 caixas"), (4, 45, "Difícil · 4 caixas")];

#[derive(Clone)]
struct State {
    player: usize,
    boxes: Vec<usize>,
}

pub struct Sokoban {
    menu: bool,
    wall: Vec<bool>,
    goal: Vec<bool>,
    start: State,
    s: State,
    history: Vec<State>,
    moves: usize,
    level: usize,
    /// Fase em jogo (0 = treino livre).
    fase: u32,
    practice: bool,
    prog: fases::Progress,
}

/// Caixas e quantos puxões para trás (mais puxões = caixas mais longe dos alvos).
fn fase_params(k: u32) -> (usize, usize) {
    let mut boxes = (2 + (k.saturating_sub(1) / 6) as usize).min(4);
    let mut pulls = 16 + (44.0 * fases::ramp(k, 30)) as usize;
    match fases::kind(k) {
        fases::Kind::Hard => boxes = (boxes + 1).min(5),
        fases::Kind::Challenge => {
            boxes = (boxes + 1).min(5);
            pulls = pulls * 3 / 2;
        }
        fases::Kind::Normal => {}
    }
    (boxes, pulls)
}

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

fn step(i: usize, d: (i32, i32)) -> Option<usize> {
    let (r, c) = ((i / N) as i32 + d.0, (i % N) as i32 + d.1);
    ((0..N as i32).contains(&r) && (0..N as i32).contains(&c)).then(|| r as usize * N + c as usize)
}

/// Sala com paredes nas bordas e alguns blocos internos, sem isolar espaços.
fn room(rng: &mut Rng) -> Vec<bool> {
    loop {
        let mut wall: Vec<bool> = (0..N * N).map(|i| i / N == 0 || i / N == N - 1 || i % N == 0 || i % N == N - 1).collect();
        for _ in 0..8 {
            let i = (rng.next() % (N * N) as u64) as usize;
            wall[i] = true;
        }
        // Todas as casas livres precisam estar ligadas.
        let free: Vec<usize> = (0..N * N).filter(|&i| !wall[i]).collect();
        let Some(&first) = free.first() else { continue };
        let mut seen = vec![false; N * N];
        let mut stack = vec![first];
        while let Some(i) = stack.pop() {
            if seen[i] || wall[i] {
                continue;
            }
            seen[i] = true;
            stack.extend(DIRS.iter().filter_map(|&d| step(i, d)));
        }
        if free.iter().all(|&i| seen[i]) && free.len() > 30 {
            return wall;
        }
    }
}

/// Gera uma fase: caixas nos alvos e jogador ao lado; depois puxa caixas para trás `pulls` vezes.
fn generate(boxes_n: usize, pulls: usize) -> (Vec<bool>, Vec<bool>, State) {
    let mut rng = Rng::new();
    loop {
        let wall = room(&mut rng);
        let free: Vec<usize> = (0..N * N).filter(|&i| !wall[i]).collect();
        let mut goals: Vec<usize> = Vec::new();
        while goals.len() < boxes_n {
            let g = free[(rng.next() % free.len() as u64) as usize];
            if !goals.contains(&g) {
                goals.push(g);
            }
        }
        let mut s = State { player: 0, boxes: goals.clone() };
        let Some(&p) = free.iter().find(|i| !goals.contains(i)) else { continue };
        s.player = p;
        let mut moved = 0;
        for _ in 0..pulls * 40 {
            if moved >= pulls {
                break;
            }
            let d = DIRS[(rng.next() % 4) as usize];
            // Puxar: o jogador anda para trás; se havia caixa na frente dele (do outro lado), ela vem junto.
            let Some(back) = step(s.player, (-d.0, -d.1)) else { continue };
            if wall[back] || s.boxes.contains(&back) {
                continue;
            }
            let front = step(s.player, d);
            let pull = rng.next() % 2 == 0;
            if let Some(f) = front.filter(|f| pull && s.boxes.contains(f)) {
                let k = s.boxes.iter().position(|&b| b == f).unwrap();
                s.boxes[k] = s.player;
                moved += 1;
            }
            s.player = back;
        }
        let goal: Vec<bool> = (0..N * N).map(|i| goals.contains(&i)).collect();
        let solved_already = s.boxes.iter().all(|&b| goal[b]);
        if moved >= pulls / 2 && !solved_already {
            return (wall, goal, s);
        }
    }
}

impl Sokoban {
    pub fn new() -> Self {
        Sokoban {
            menu: true,
            wall: vec![],
            goal: vec![],
            start: State { player: 0, boxes: vec![] },
            s: State { player: 0, boxes: vec![] },
            history: vec![],
            moves: 0,
            level: 0,
            fase: 0,
            practice: false,
            prog: fases::Progress::load("sokoban"),
        }
    }

    fn begin(&mut self, level: usize) {
        let (n, pulls, _) = LEVELS[level];
        self.load(n, pulls);
        self.level = level;
        self.fase = 0;
    }

    fn start_fase(&mut self) {
        let (n, pulls) = fase_params(self.prog.level);
        self.load(n, pulls);
        self.fase = self.prog.level;
    }

    fn load(&mut self, n: usize, pulls: usize) {
        let (wall, goal, s) = generate(n, pulls);
        self.menu = false;
        self.practice = false;
        self.wall = wall;
        self.goal = goal;
        self.start = s.clone();
        self.s = s;
        self.history.clear();
        self.moves = 0;
    }

    fn won(&self) -> bool {
        self.s.boxes.iter().all(|&b| self.goal[b])
    }

    fn walk(&mut self, d: (i32, i32)) -> bool {
        let Some(next) = step(self.s.player, d) else { return false };
        if self.wall[next] {
            return false;
        }
        if let Some(k) = self.s.boxes.iter().position(|&b| b == next) {
            let Some(beyond) = step(next, d) else { return false };
            if self.wall[beyond] || self.s.boxes.contains(&beyond) {
                return false;
            }
            self.history.push(self.s.clone());
            self.s.boxes[k] = beyond;
        } else {
            self.history.push(self.s.clone());
        }
        self.s.player = next;
        self.moves += 1;
        true
    }

    fn cell(i: usize) -> Rect {
        let s = 92;
        let x0 = (W - s * N as i32) / 2;
        Rect::new(x0 + (i % N) as i32 * s, 270 + (i / N) as i32 * s, s, s)
    }

    fn arrow_rect(k: usize) -> Rect {
        // Cruz de setas: cima, baixo, esquerda, direita.
        let (cx, cy, s) = (W / 2, 1222, 112);
        match k {
            0 => Rect::new(cx - s / 2, cy - s - 6, s, s),
            1 => Rect::new(cx - s / 2, cy + 6 + 0, s, s),
            2 => Rect::new(cx - s / 2 - s - 12, cy - s / 2, s, s),
            _ => Rect::new(cx + s / 2 + 12, cy - s / 2, s, s),
        }
    }
}

impl MiniGame for Sokoban {
    fn at_menu(&self) -> bool {
        self.menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu && self.practice {
            let opts: Vec<&str> = LEVELS.iter().map(|l| l.2).collect();
            minigame::draw_menu(fb, f, "Treino", "Escolha quantas caixas. Não conta para as fases.", &opts);
            return;
        }
        if self.menu {
            let main = format!("Fase {}", self.prog.level);
            fases::draw_menu(fb, f, "Sokoban", "Empurre cada caixa até um alvo. Só dá para empurrar.", &self.prog, &main, &["Treino livre"]);
            return;
        }
        let status = if self.won() { format!("Resolvido em {} passos!", self.moves) } else { format!("{} passos", self.moves) };
        let title = if self.fase > 0 { fases::title(self.fase) } else { "Sokoban".to_string() };
        minigame::header(fb, f, &title, &status);
        for i in 0..N * N {
            let r = Self::cell(i);
            // Arte desenhada (mesmo cenário da Masmorra); sem os arquivos, as formas simples.
            if self.wall[i] {
                if !crate::sprites::draw(fb, "parede", r) {
                    fb.fill(r, 0x55);
                }
                continue;
            }
            if !crate::sprites::draw(fb, "chao", r) {
                fb.fill(r, 0xF2);
            }
            let inset = |k: i32| Rect::new(r.x + k, r.y + k, r.w - 2 * k, r.h - 2 * k);
            let boxed = self.s.boxes.contains(&i);
            if self.goal[i] && !boxed && !crate::sprites::draw(fb, "alvo", inset(r.w / 8)) {
                let d = r.w / 5;
                fb.round_outline(Rect::new(r.x + r.w / 2 - d, r.y + r.h / 2 - d, 2 * d, 2 * d), d, 4, INK, 0xF2);
            }
            if boxed {
                let on = self.goal[i];
                if !crate::sprites::draw(fb, if on { "caixa-ok" } else { "caixa" }, inset(4)) {
                    let b = Rect::new(r.x + 8, r.y + 8, r.w - 16, r.h - 16);
                    fb.round_rect(b, 8, if on { INK } else { 0x99 });
                    ui::stroke(fb, (b.x as f32 + 8.0, b.y as f32 + 8.0), ((b.x + b.w) as f32 - 8.0, (b.y + b.h) as f32 - 8.0), 4.0, PAPER);
                    ui::stroke(fb, ((b.x + b.w) as f32 - 8.0, b.y as f32 + 8.0), (b.x as f32 + 8.0, (b.y + b.h) as f32 - 8.0), 4.0, PAPER);
                }
            }
            if i == self.s.player && !crate::sprites::draw(fb, "carregador", inset(2)) && !crate::sprites::draw(fb, "heroi-1", inset(2)) {
                let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                fb.round_rect(Rect::new(cx - 14, cy - 30, 28, 28), 14, INK);
                fb.round_rect(Rect::new(cx - 22, cy, 44, 32), 14, INK);
            }
        }
        for k in 0..4 {
            let r = Self::arrow_rect(k);
            fb.round_outline(r, 24, 4, INK, PAPER);
            let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
            let (dx, dy) = (DIRS[k].1 as f32, DIRS[k].0 as f32);
            let tip = (cx + dx * 26.0, cy + dy * 26.0);
            ui::stroke(fb, (cx - dx * 26.0, cy - dy * 26.0), tip, 7.0, INK);
            ui::stroke(fb, tip, (tip.0 - dx * 18.0 - dy * 18.0, tip.1 - dy * 18.0 - dx * 18.0), 7.0, INK);
            ui::stroke(fb, tip, (tip.0 - dx * 18.0 + dy * 18.0, tip.1 - dy * 18.0 + dx * 18.0), 7.0, INK);
        }
        if self.fase > 0 && self.won() {
            // As setas ficam; o rodapé vira o da fase.
            return fases::draw_win(fb, f, &self.prog);
        }
        let labels = if self.won() { vec!["Próxima fase"] } else { vec!["Desfazer", "Recomeçar"] };
        for (k, l) in labels.iter().enumerate() {
            let r = Rect::new(if k == 0 { MARGIN - 30 } else { W - MARGIN - 230 }, 1340, 260, 90);
            minigame::pill(fb, f, r, l, self.won());
        }
        let _ = (MUTED, Face::Body);
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu && self.practice {
            return (0..LEVELS.len()).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.menu {
            return vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))];
        }
        if self.fase > 0 && self.won() {
            return fases::win_targets();
        }
        let btn = |k: usize| Rect::new(if k == 0 { MARGIN - 30 } else { W - MARGIN - 230 }, 1340, 260, 90);
        if self.won() {
            return vec![(803, btn(0))];
        }
        let mut v: Vec<(u32, Rect)> = (0..4).map(|k| (810 + k as u32, Self::arrow_rect(k))).collect();
        v.push((801, btn(0)));
        v.push((802, btn(1)));
        v
    }

    fn back(&mut self) -> bool {
        // Só o submenu de treino volta ao menu do jogo; da partida, Início sai e o app guarda
        // a partida para continuar depois.
        if self.menu && self.practice {
            self.practice = false;
            return true;
        }
        false
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 | fases::WIN_NEXT => {
                self.start_fase();
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            fases::WIN_MENU => {
                self.menu = true;
                Redraw::Full
            }
            910..=912 => {
                self.begin((id - 910) as usize);
                Redraw::Full
            }
            810..=813 => {
                if self.walk(DIRS[(id - 810) as usize]) {
                    if self.won() && self.fase > 0 {
                        self.prog.record_win(self.fase, fases::today());
                    }
                    if self.won() { Redraw::Full } else { Redraw::Part(Rect::new(0, 170, W, 270 + 92 * N as i32 - 170)) }
                } else {
                    Redraw::None
                }
            }
            801 => {
                if let Some(s) = self.history.pop() {
                    self.s = s;
                    self.moves = self.moves.saturating_sub(1);
                }
                Redraw::Part(Rect::new(0, 170, W, 270 + 92 * N as i32 - 170))
            }
            802 => {
                self.s = self.start.clone();
                self.history.clear();
                self.moves = 0;
                Redraw::Part(Rect::new(0, 170, W, 270 + 92 * N as i32 - 170))
            }
            803 => {
                let l = self.level;
                self.begin(l);
                Redraw::Full
            }
            _ => Redraw::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fases_geram_rapido() {
        for k in [1, 10, 15, 20, 30, 40] {
            let t = std::time::Instant::now();
            let (n, p) = fase_params(k);
            let (_, goal, _) = generate(n, p);
            assert_eq!(goal.iter().filter(|&&g| g).count(), n);
            eprintln!("fase {k}: {n} caixas, {p} puxões, {:?}", t.elapsed());
        }
    }
    #[test]
    fn fase_gerada_tem_caixas_fora_e_dentro_da_sala() {
        let (wall, goal, s) = generate(3, 30);
        assert_eq!(goal.iter().filter(|&&g| g).count(), 3);
        assert!(s.boxes.iter().all(|&b| !wall[b]));
        assert!(!s.boxes.iter().all(|&b| goal[b]));
    }
}
