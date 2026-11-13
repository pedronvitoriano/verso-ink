//! Jogo da velha e super jogo da velha (o "jogo da velha +"): contra a CPU ou a dois.
//!
//! No super, o tabuleiro tem 9 velhas menores. A casa onde você joga manda o outro para a velha
//! de mesma posição; se ela já acabou (ganha ou cheia), ele joga onde quiser. Ganha quem fizer
//! três velhas em linha. A CPU do clássico joga perfeito, com um tropeço de vez em quando; a do
//! super pensa por simulação (Monte Carlo), dentro de um tempo curto.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const LINES: [[usize; 3]; 8] = [[0, 1, 2], [3, 4, 5], [6, 7, 8], [0, 3, 6], [1, 4, 7], [2, 5, 8], [0, 4, 8], [2, 4, 6]];

/// Dono de uma linha completa (1 = X, 2 = O), e qual linha.
fn winner3(b: &[u8; 9]) -> Option<(u8, [usize; 3])> {
    LINES.iter().find(|l| b[l[0]] != 0 && b[l[0]] == b[l[1]] && b[l[1]] == b[l[2]]).map(|l| (b[l[0]], *l))
}

// ---------- Clássico ----------

/// Minimax completo (o tabuleiro é pequeno): valor para `me`.
fn minimax(b: &mut [u8; 9], turn: u8, me: u8, depth: i32) -> i32 {
    if let Some((w, _)) = winner3(b) {
        return if w == me { 10 - depth } else { depth - 10 };
    }
    if b.iter().all(|&c| c != 0) {
        return 0;
    }
    let mut best = if turn == me { i32::MIN } else { i32::MAX };
    for i in 0..9 {
        if b[i] == 0 {
            b[i] = turn;
            let v = minimax(b, 3 - turn, me, depth + 1);
            b[i] = 0;
            best = if turn == me { best.max(v) } else { best.min(v) };
        }
    }
    best
}

fn best_classic(b: &[u8; 9], me: u8, rng: &mut Rng, slip: u64) -> usize {
    let free: Vec<usize> = (0..9).filter(|&i| b[i] == 0).collect();
    // Tropeço de vez em quando (senão ninguém ganha da CPU): uma casa qualquer.
    if rng.next() % 100 < slip {
        return free[(rng.next() % free.len() as u64) as usize];
    }
    let mut best = (i32::MIN, Vec::new());
    for &i in &free {
        let mut t = *b;
        t[i] = me;
        let v = minimax(&mut t, 3 - me, me, 1);
        if v > best.0 {
            best = (v, vec![i]);
        } else if v == best.0 {
            best.1.push(i);
        }
    }
    best.1[(rng.next() % best.1.len() as u64) as usize]
}

// ---------- Super ----------

#[derive(Clone, Copy)]
struct Big {
    cells: [u8; 81],
    /// Situação de cada velha: 0 em jogo, 1/2 ganha por X/O, 3 cheia (empate).
    small: [u8; 9],
    /// Velha onde o próximo tem de jogar (None = qualquer uma em jogo).
    next: Option<usize>,
    turn: u8,
}

impl Big {
    fn new(first: u8) -> Self {
        Big { cells: [0; 81], small: [0; 9], next: None, turn: first }
    }
    fn sub(&self, s: usize) -> [u8; 9] {
        let mut b = [0; 9];
        b.copy_from_slice(&self.cells[s * 9..s * 9 + 9]);
        b
    }
    fn result(&self) -> Option<u8> {
        let won = self.small.map(|v| if v == 3 { 0 } else { v });
        if let Some((w, _)) = winner3(&won) {
            return Some(w);
        }
        (self.small.iter().all(|&v| v != 0)).then_some(0)
    }
    fn moves(&self) -> Vec<usize> {
        let open = |s: usize| self.small[s] == 0;
        let subs: Vec<usize> = match self.next {
            Some(s) if open(s) => vec![s],
            _ => (0..9).filter(|&s| open(s)).collect(),
        };
        subs.into_iter().flat_map(|s| (0..9).filter(move |&c| self.cells[s * 9 + c] == 0).map(move |c| s * 9 + c)).collect()
    }
    fn play(&mut self, m: usize) {
        let (s, c) = (m / 9, m % 9);
        self.cells[m] = self.turn;
        let b = self.sub(s);
        if let Some((w, _)) = winner3(&b) {
            self.small[s] = w;
        } else if b.iter().all(|&v| v != 0) {
            self.small[s] = 3;
        }
        self.next = Some(c);
        self.turn = 3 - self.turn;
    }
}

/// Monte Carlo com UCT, até o tempo acabar: a jogada mais visitada.
fn best_super(g: &Big, rng: &mut Rng, budget: std::time::Duration) -> usize {
    struct Node {
        mv: usize,
        parent: usize,
        kids: Vec<usize>,
        untried: Vec<usize>,
        wins: f32,
        visits: f32,
        /// Quem jogou o lance que levou a este nó.
        by: u8,
    }
    let mut nodes = vec![Node { mv: usize::MAX, parent: usize::MAX, kids: vec![], untried: g.moves(), wins: 0.0, visits: 0.0, by: 3 - g.turn }];
    let t0 = std::time::Instant::now();
    let mut iters = 0;
    while iters < 200 || (t0.elapsed() < budget && iters < 60_000) {
        iters += 1;
        let mut s = *g;
        let mut n = 0;
        // Seleção.
        while nodes[n].untried.is_empty() && !nodes[n].kids.is_empty() {
            let lnp = nodes[n].visits.ln();
            n = *nodes[n]
                .kids
                .iter()
                .max_by(|&&a, &&b| {
                    let ua = nodes[a].wins / nodes[a].visits + 1.2 * (lnp / nodes[a].visits).sqrt();
                    let ub = nodes[b].wins / nodes[b].visits + 1.2 * (lnp / nodes[b].visits).sqrt();
                    ua.partial_cmp(&ub).unwrap()
                })
                .unwrap();
            s.play(nodes[n].mv);
        }
        // Expansão.
        if !nodes[n].untried.is_empty() && s.result().is_none() {
            let k = (rng.next() % nodes[n].untried.len() as u64) as usize;
            let mv = nodes[n].untried.swap_remove(k);
            let by = s.turn;
            s.play(mv);
            nodes.push(Node { mv, parent: n, kids: vec![], untried: s.moves(), wins: 0.0, visits: 0.0, by });
            let id = nodes.len() - 1;
            nodes[n].kids.push(id);
            n = id;
        }
        // Simulação.
        let mut r = s.result();
        while r.is_none() {
            let ms = s.moves();
            if ms.is_empty() {
                r = Some(0);
                break;
            }
            s.play(ms[(rng.next() % ms.len() as u64) as usize]);
            r = s.result();
        }
        let w = r.unwrap_or(0);
        // Retropropagação.
        loop {
            nodes[n].visits += 1.0;
            nodes[n].wins += if w == 0 { 0.5 } else if w == nodes[n].by { 1.0 } else { 0.0 };
            if nodes[n].parent == usize::MAX {
                break;
            }
            n = nodes[n].parent;
        }
    }
    nodes[0].kids.iter().max_by(|&&a, &&b| nodes[a].visits.partial_cmp(&nodes[b].visits).unwrap()).map(|&k| nodes[k].mv).unwrap_or_else(|| g.moves()[0])
}

// ---------- O jogo ----------

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Classic,
    Super,
}

pub struct Velha {
    menu: bool,
    kind: Kind,
    cpu: bool,
    small: [u8; 9],
    turn: u8,
    big: Big,
    history: Vec<usize>,
    /// Quem começou esta partida (alterna a cada uma).
    first: u8,
    score: (u32, u32, u32),
    counted: bool,
    rng: Rng,
}

const OPTIONS: [&str; 4] = ["Jogo da velha · contra a CPU", "Super velha · contra a CPU", "Jogo da velha · duas pessoas", "Super velha · duas pessoas"];

impl Velha {
    pub fn new() -> Self {
        Velha { menu: true, kind: Kind::Classic, cpu: true, small: [0; 9], turn: 1, big: Big::new(1), history: Vec::new(), first: 1, score: (0, 0, 0), counted: false, rng: Rng::new() }
    }

    fn start(&mut self, kind: Kind, cpu: bool, keep_score: bool) {
        if !keep_score || kind != self.kind || cpu != self.cpu {
            self.score = (0, 0, 0);
            self.first = 1;
        }
        self.kind = kind;
        self.cpu = cpu;
        self.small = [0; 9];
        self.turn = self.first;
        self.big = Big::new(self.first);
        self.history.clear();
        self.counted = false;
        self.menu = false;
    }

    /// Resultado: Some(0) empate, Some(1/2) vencedor; e a linha vencedora (clássico).
    fn result(&self) -> Option<u8> {
        match self.kind {
            Kind::Classic => winner3(&self.small).map(|w| w.0).or_else(|| self.small.iter().all(|&c| c != 0).then_some(0)),
            Kind::Super => self.big.result(),
        }
    }

    fn turn(&self) -> u8 {
        match self.kind {
            Kind::Classic => self.turn,
            Kind::Super => self.big.turn,
        }
    }

    fn play(&mut self, m: usize) {
        match self.kind {
            Kind::Classic => {
                self.small[m] = self.turn;
                self.turn = 3 - self.turn;
            }
            Kind::Super => self.big.play(m),
        }
        self.history.push(m);
        if let (Some(r), false) = (self.result(), self.counted) {
            self.counted = true;
            match r {
                1 => self.score.0 += 1,
                2 => self.score.1 += 1,
                _ => self.score.2 += 1,
            }
            if self.cpu && r == 1 {
                record_win(self.kind);
            }
            // A próxima partida começa com o outro.
            self.first = 3 - self.first;
        }
    }

    /// Refaz a partida sem os últimos `n` lances.
    fn undo(&mut self, n: usize) {
        let keep: Vec<usize> = self.history[..self.history.len().saturating_sub(n)].to_vec();
        let (score, first, kind, cpu) = (self.score, self.first, self.kind, self.cpu);
        self.start(kind, cpu, true);
        self.score = score;
        self.first = first;
        self.turn = first;
        self.big = Big::new(first);
        for m in keep {
            match self.kind {
                Kind::Classic => {
                    self.small[m] = self.turn;
                    self.turn = 3 - self.turn;
                }
                Kind::Super => self.big.play(m),
            }
            self.history.push(m);
        }
    }

    fn legal(&self, m: usize) -> bool {
        match self.kind {
            Kind::Classic => m < 9 && self.small[m] == 0,
            Kind::Super => self.big.moves().contains(&m),
        }
    }

    fn name(&self, p: u8) -> &'static str {
        match (self.cpu, p) {
            (true, 1) => "Você",
            (true, _) => "A CPU",
            (false, 1) => "X",
            _ => "O",
        }
    }

    fn status(&self) -> String {
        let placar = if self.cpu {
            format!("Você {} × {} CPU", self.score.0, self.score.1)
        } else {
            format!("X {} × {} O", self.score.0, self.score.1)
        };
        let empates = if self.score.2 > 0 { format!(" · {} empate{}", self.score.2, if self.score.2 == 1 { "" } else { "s" }) } else { String::new() };
        match self.result() {
            Some(0) => format!("Deu velha: empate. {placar}{empates}"),
            Some(p) => format!("{} {}! {placar}{empates}", self.name(p), if self.cpu && p == 1 { "venceu" } else { "vence" }),
            None => {
                let vez = if self.cpu && self.turn() == 1 { "Sua vez".to_string() } else if self.cpu { "Vez da CPU".to_string() } else { format!("Vez do {}", self.name(self.turn())) };
                format!("{vez} · Placar: {placar}{empates}")
            }
        }
    }

    // ----- geometria -----

    fn board() -> Rect {
        let s = W - 2 * MARGIN;
        Rect::new(MARGIN, 300, s, s)
    }

    /// Casa do clássico.
    fn cell3(i: usize) -> Rect {
        let b = Self::board();
        let c = (b.w - 48) / 3;
        Rect::new(b.x + 24 + (i % 3) as i32 * c, b.y + 24 + (i / 3) as i32 * c, c, c)
    }

    /// Velha menor do super (s = 0..8) e a casa dentro dela.
    fn sub_rect(s: usize) -> Rect {
        let b = Self::board();
        let gap = 22;
        let c = (b.w - 2 * 20 - 2 * gap) / 3;
        Rect::new(b.x + 20 + (s % 3) as i32 * (c + gap), b.y + 20 + (s / 3) as i32 * (c + gap), c, c)
    }
    fn cell9(m: usize) -> Rect {
        let r = Self::sub_rect(m / 9);
        let c = r.w / 3;
        Rect::new(r.x + (m % 9 % 3) as i32 * c, r.y + (m % 9 / 3) as i32 * c, c, c)
    }

    fn draw_mark(fb: &mut Framebuffer, r: Rect, p: u8, t: f32) {
        let m = r.w / 5;
        let (x0, y0, x1, y1) = ((r.x + m) as f32, (r.y + m) as f32, (r.x + r.w - m) as f32, (r.y + r.h - m) as f32);
        match p {
            1 => {
                ui::stroke(fb, (x0, y0), (x1, y1), t, INK);
                ui::stroke(fb, (x1, y0), (x0, y1), t, INK);
            }
            2 => {
                let ri = Rect::new(r.x + m, r.y + m, r.w - 2 * m, r.h - 2 * m);
                fb.round_outline(ri, ri.w / 2, t as i32, INK, PAPER);
            }
            _ => {}
        }
    }

    fn draw_classic(&self, fb: &mut Framebuffer) {
        let b = Self::board();
        ui::card(fb, b, 36, PAPER, false);
        // Grade: duas linhas grossas em cada sentido.
        let c = Self::cell3(0);
        for k in 1..3 {
            fb.round_rect(Rect::new(c.x + k * c.w - 5, c.y + 12, 10, 3 * c.w - 24), 5, INK);
            fb.round_rect(Rect::new(c.x + 12, c.y + k * c.w - 5, 3 * c.w - 24, 10), 5, INK);
        }
        let last = self.history.last().copied();
        for i in 0..9 {
            let r = Self::cell3(i);
            if Some(i) == last && self.result().is_none() {
                fb.round_rect(Rect::new(r.x + 20, r.y + 20, r.w - 40, r.h - 40), 24, 0xE6);
            }
            Self::draw_mark(fb, r, self.small[i], 16.0);
        }
        if let Some((_, l)) = winner3(&self.small) {
            let (a, z) = (Self::cell3(l[0]), Self::cell3(l[2]));
            ui::stroke(fb, ((a.x + a.w / 2) as f32, (a.y + a.h / 2) as f32), ((z.x + z.w / 2) as f32, (z.y + z.h / 2) as f32), 14.0, INK);
        }
    }

    fn draw_super(&self, fb: &mut Framebuffer) {
        let b = Self::board();
        ui::card(fb, b, 36, 0xF2, false);
        let moves = if self.result().is_none() { self.big.moves() } else { vec![] };
        let active: Vec<usize> = (0..9).filter(|&s| moves.iter().any(|m| m / 9 == s)).collect();
        let last = self.history.last().copied();
        for s in 0..9 {
            let r = Self::sub_rect(s);
            // Onde dá para jogar agora: velha branca com borda grossa; as outras, apagadas.
            let on = active.contains(&s);
            if on && active.len() < 9 {
                ui::card(fb, r, 18, PAPER, false);
            } else {
                fb.round_rect(r, 18, if on { PAPER } else { 0xE2 });
                fb.round_outline(r, 18, 2, 0x99, if on { PAPER } else { 0xE2 });
            }
            let c = r.w / 3;
            for k in 1..3 {
                fb.fill(Rect::new(r.x + k * c - 1, r.y + 10, 3, r.h - 20), if on { INK } else { 0x88 });
                fb.fill(Rect::new(r.x + 10, r.y + k * c - 1, r.w - 20, 3), if on { INK } else { 0x88 });
            }
            for k in 0..9 {
                let m = s * 9 + k;
                let cr = Self::cell9(m);
                if Some(m) == last {
                    fb.round_rect(Rect::new(cr.x + 6, cr.y + 6, cr.w - 12, cr.h - 12), 10, 0xD0);
                }
                Self::draw_mark(fb, cr, self.big.cells[m], 6.0);
            }
            // Velha ganha: o símbolo grande por cima, num véu claro.
            if matches!(self.big.small[s], 1 | 2) {
                for y in r.y..r.y + r.h {
                    for x in r.x..r.x + r.w {
                        fb.blend(x, y, PAPER, 150);
                    }
                }
                Self::draw_mark(fb, Rect::new(r.x - 10, r.y - 10, r.w + 20, r.h + 20), self.big.small[s], 20.0);
            }
        }
        let won = self.big.small.map(|v| if v == 3 { 0 } else { v });
        if let Some((_, l)) = winner3(&won) {
            let (a, z) = (Self::sub_rect(l[0]), Self::sub_rect(l[2]));
            ui::stroke(fb, ((a.x + a.w / 2) as f32, (a.y + a.h / 2) as f32), ((z.x + z.w / 2) as f32, (z.y + z.h / 2) as f32), 16.0, INK);
        }
    }
}

fn record_path() -> std::path::PathBuf {
    let d = crate::home_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d.join("velha.txt")
}

/// Vitórias contra a CPU: (clássico, super).
fn wins() -> (u32, u32) {
    let s = std::fs::read_to_string(record_path()).unwrap_or_default();
    let mut it = s.split_whitespace().filter_map(|x| x.parse().ok());
    (it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn record_win(kind: Kind) {
    let (a, b) = wins();
    let (a, b) = if kind == Kind::Classic { (a + 1, b) } else { (a, b + 1) };
    let _ = std::fs::write(record_path(), format!("{a} {b}"));
}

impl MiniGame for Velha {
    fn at_menu(&self) -> bool {
        self.menu
    }

    fn summary(&self) -> String {
        if self.menu { String::new() } else { self.status() }
    }

    fn context(&self) -> String {
        if self.menu {
            return "No menu do jogo da velha.".into();
        }
        let mark = |v: u8| match v {
            1 => 'X',
            2 => 'O',
            _ => '.',
        };
        match self.kind {
            Kind::Classic => format!("Jogo da velha (Pedro é X). {}\n{}", self.status(), self.small.chunks(3).map(|r| r.iter().map(|&v| mark(v)).collect::<String>()).collect::<Vec<_>>().join("\n")),
            Kind::Super => format!(
                "Super jogo da velha (Pedro é X). {} Velhas menores (0-8, linha a linha): {}. Próxima obrigatória: {:?}.",
                self.status(),
                self.big.small.iter().map(|&v| if v == 3 { '=' } else { mark(v) }).collect::<String>(),
                self.big.next
            ),
        }
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Velha", "O clássico e o super, contra a CPU ou a dois.", &OPTIONS);
            let (a, b) = wins();
            let y = minigame::menu_row(OPTIONS.len()).y + 40;
            f.draw(fb, Face::Strong, 30.0, MARGIN, y + 20, INK, &format!("Vitórias contra a CPU: {a} no clássico · {b} no super"));
            f.draw(fb, Face::Body, 28.0, MARGIN, y + 66, MUTED, "No super, a casa que você marca manda o outro para a velha daquele lugar.");
            minigame::menu_art(fb, "Velha", y + 110);
            return;
        }
        let title = if self.kind == Kind::Classic { "Jogo da velha" } else { "Super velha" };
        minigame::header(fb, f, title, &self.status());
        match self.kind {
            Kind::Classic => self.draw_classic(fb),
            Kind::Super => self.draw_super(fb),
        }
        let over = self.result().is_some();
        if over {
            minigame::pill(fb, f, minigame::bottom_button(0, 2), "Menu", false);
            minigame::pill(fb, f, minigame::bottom_button(1, 2), "Jogar de novo", true);
        } else {
            minigame::pill(fb, f, minigame::bottom_button(0, 2), "Desfazer", false);
            minigame::pill(fb, f, minigame::bottom_button(1, 2), "Recomeçar", false);
            if self.kind == Kind::Super {
                let hint = match self.big.next {
                    Some(s) if self.big.small[s] == 0 => "Jogue na velha destacada.",
                    _ => "Livre: jogue em qualquer velha que ainda está em jogo.",
                };
                let w = f.measure(Face::Strong, 32.0, hint);
                f.draw(fb, Face::Strong, 32.0, (W - w) / 2, 1290, INK, hint);
            }
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu {
            return (0..OPTIONS.len()).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        let mut v: Vec<(u32, Rect)> = if self.result().is_some() {
            vec![(801, minigame::bottom_button(0, 2)), (802, minigame::bottom_button(1, 2))]
        } else {
            vec![(800, minigame::bottom_button(0, 2)), (803, minigame::bottom_button(1, 2))]
        };
        let human = !self.cpu || self.turn() == 1;
        if self.result().is_none() && human {
            match self.kind {
                Kind::Classic => v.extend((0..9).filter(|&i| self.small[i] == 0).map(|i| (i as u32, Self::cell3(i)))),
                Kind::Super => v.extend(self.big.moves().into_iter().map(|m| (m as u32, Self::cell9(m)))),
            }
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900..=903 => {
                let k = (id - 900) as usize;
                self.start(if k % 2 == 0 { Kind::Classic } else { Kind::Super }, k < 2, false);
                Redraw::Full
            }
            801 => {
                self.menu = true;
                Redraw::Full
            }
            802 | 803 => {
                // Jogar de novo / recomeçar: mesma modalidade, placar mantido.
                let (k, c) = (self.kind, self.cpu);
                let first = self.first;
                self.start(k, c, true);
                if id == 803 {
                    self.first = first;
                }
                Redraw::Full
            }
            800 => {
                // Contra a CPU, desfaz o seu lance e o dela.
                let n = if self.cpu { if self.turn() == 1 { 2 } else { 1 } } else { 1 };
                self.undo(n);
                Redraw::Full
            }
            m if self.legal(m as usize) => {
                self.play(m as usize);
                Redraw::Full
            }
            _ => Redraw::None,
        }
    }

    fn cpu_turn(&self) -> bool {
        !self.menu && self.cpu && self.turn() == 2 && self.result().is_none()
    }

    fn cpu_move(&mut self) {
        let m = match self.kind {
            Kind::Classic => best_classic(&self.small, 2, &mut self.rng, 18),
            Kind::Super => best_super(&self.big, &mut self.rng, std::time::Duration::from_millis(900)),
        };
        self.play(m);
    }

    fn back(&mut self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classico_perfeito_nao_perde() {
        let mut rng = Rng::new();
        // A CPU sem tropeço contra jogadas aleatórias: nunca perde.
        for _ in 0..200 {
            let mut b = [0u8; 9];
            let mut turn = 1;
            while winner3(&b).is_none() && b.iter().any(|&c| c == 0) {
                let m = if turn == 2 {
                    best_classic(&b, 2, &mut rng, 0)
                } else {
                    let free: Vec<usize> = (0..9).filter(|&i| b[i] == 0).collect();
                    free[(rng.next() % free.len() as u64) as usize]
                };
                b[m] = turn;
                turn = 3 - turn;
            }
            assert_ne!(winner3(&b).map(|w| w.0), Some(1));
        }
    }

    #[test]
    fn super_regras() {
        let mut g = Big::new(1);
        assert_eq!(g.moves().len(), 81);
        g.play(4 * 9 + 2); // X na velha do meio, casa 2: O vai para a velha 2
        assert!(g.moves().iter().all(|m| m / 9 == 2));
        // Velha ganha fecha e libera a escolha quando mandam para ela.
        let mut g = Big::new(1);
        g.cells[0] = 1;
        g.cells[1] = 1;
        g.turn = 1;
        g.play(2);
        assert_eq!(g.small[0], 1);
        g.next = Some(0);
        assert!(g.moves().iter().any(|m| m / 9 != 0));
        // A CPU do super escolhe um lance válido.
        let g = Big::new(1);
        let mut rng = Rng::new();
        let m = best_super(&g, &mut rng, std::time::Duration::from_millis(50));
        assert!(g.moves().contains(&m));
    }

    #[test]
    fn partida_contra_cpu_termina() {
        let mut v = Velha::new();
        v.tap(901, 0, 0);
        let mut guard = 0;
        while v.result().is_none() && guard < 100 {
            guard += 1;
            if v.cpu_turn() {
                v.big = {
                    let mut rng = Rng::new();
                    let m = best_super(&v.big, &mut rng, std::time::Duration::from_millis(5));
                    let mut b = v.big;
                    b.play(m);
                    v.history.push(m);
                    b
                };
            } else {
                let m = v.big.moves()[0];
                v.tap(m as u32, 0, 0);
            }
        }
        assert!(v.result().is_some());
    }
}
