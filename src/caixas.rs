//! Pontos e caixas: ligue pontos vizinhos; quem fecha o quarto lado marca a caixa e joga de novo.
//! Contra a CPU (gulosa e cuidadosa com o terceiro lado) ou duas pessoas.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, PAPER};

const N: usize = 5; // caixas por lado
const H: usize = (N + 1) * N; // linhas horizontais
const E: usize = 2 * H; // total de linhas (horizontais + verticais)

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Menu,
    Cpu(u8),
    Local,
}

pub struct Caixas {
    mode: Mode,
    edges: [bool; E],
    owner: [u8; N * N],
    turn: u8,
    last: Option<usize>,
    rng: Rng,
}

const MENU: [&str; 4] = ["Contra a CPU · Fácil", "Contra a CPU · Médio", "Contra a CPU · Difícil", "Duas pessoas"];

/// Linhas que formam a caixa (r, c): cima, baixo, esquerda, direita.
fn box_edges(r: usize, c: usize) -> [usize; 4] {
    [r * N + c, (r + 1) * N + c, H + r * (N + 1) + c, H + r * (N + 1) + c + 1]
}

/// Caixas que usam a linha `e`.
fn boxes_of(e: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    if e < H {
        let (r, c) = (e / N, e % N);
        if r > 0 { v.push((r - 1, c)); }
        if r < N { v.push((r, c)); }
    } else {
        let k = e - H;
        let (r, c) = (k / (N + 1), k % (N + 1));
        if c > 0 { v.push((r, c - 1)); }
        if c < N { v.push((r, c)); }
    }
    v
}

fn sides(edges: &[bool; E], r: usize, c: usize) -> usize {
    box_edges(r, c).iter().filter(|&&e| edges[e]).count()
}

impl Caixas {
    pub fn new() -> Self {
        Caixas { mode: Mode::Menu, edges: [false; E], owner: [0; N * N], turn: 1, last: None, rng: Rng::new() }
    }

    fn geom() -> (i32, i32, i32) {
        let s = ((W - 2 * MARGIN - 40) / N as i32).min(160);
        ((W - s * N as i32) / 2, 360, s)
    }

    fn dot(r: usize, c: usize) -> (i32, i32) {
        let (x0, y0, s) = Self::geom();
        (x0 + c as i32 * s, y0 + r as i32 * s)
    }

    /// Área de toque de cada linha: faixa grossa entre os dois pontos.
    fn edge_rect(e: usize) -> Rect {
        let (_, _, s) = Self::geom();
        let t = 60;
        if e < H {
            let (r, c) = (e / N, e % N);
            let (x, y) = Self::dot(r, c);
            Rect::new(x + 14, y - t / 2, s - 28, t)
        } else {
            let k = e - H;
            let (r, c) = (k / (N + 1), k % (N + 1));
            let (x, y) = Self::dot(r, c);
            Rect::new(x - t / 2, y + 14, t, s - 28)
        }
    }

    /// Marca a linha; devolve quantas caixas fechou (e passa a vez se nenhuma).
    fn play(&mut self, e: usize) -> usize {
        if self.edges[e] {
            return 0;
        }
        self.edges[e] = true;
        self.last = Some(e);
        let mut closed = 0;
        for (r, c) in boxes_of(e) {
            if sides(&self.edges, r, c) == 4 && self.owner[r * N + c] == 0 {
                self.owner[r * N + c] = self.turn;
                closed += 1;
            }
        }
        if closed == 0 {
            self.turn = 3 - self.turn;
        }
        closed
    }

    fn over(&self) -> bool {
        self.edges.iter().all(|&e| e)
    }

    fn score(&self, p: u8) -> usize {
        self.owner.iter().filter(|&&o| o == p).count()
    }

    fn names(&self) -> (&'static str, &'static str) {
        if matches!(self.mode, Mode::Cpu(_)) { ("Você", "CPU") } else { ("Jogador 1", "Jogador 2") }
    }

    /// Quantas caixas o adversário leva em sequência se eu jogar `e` (simulação gulosa).
    fn giveaway(&self, e: usize) -> usize {
        let mut edges = self.edges;
        edges[e] = true;
        let mut taken = 0;
        loop {
            let next = (0..E).find(|&x| !edges[x] && boxes_of(x).iter().any(|&(r, c)| sides(&edges, r, c) == 3));
            let Some(x) = next else { break };
            edges[x] = true;
            taken += boxes_of(x).iter().filter(|&&(r, c)| sides(&edges, r, c) == 4).count();
        }
        taken
    }
}

impl MiniGame for Caixas {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.mode == Mode::Menu {
            minigame::draw_menu(fb, f, "Caixas", "Feche o quarto lado e jogue de novo.", &MENU);
            return;
        }
        let (a, b) = self.names();
        let (sa, sb) = (self.score(1), self.score(2));
        let status = if self.over() {
            match sa.cmp(&sb) {
                std::cmp::Ordering::Greater => format!("{a} {}! {sa} a {sb}.", if a == "Você" { "venceu" } else { "vence" }),
                std::cmp::Ordering::Less => format!("{b} vence: {sb} a {sa}."),
                _ => format!("Empate: {sa} a {sb}."),
            }
        } else {
            let who = if self.turn == 1 { a } else { b };
            format!("{a} {sa} × {sb} {b} · vez de: {who}")
        };
        minigame::header(fb, f, "Pontos e caixas", &status);
        let (x0, y0, s) = Self::geom();
        // Caixas fechadas: jogador 1 em preto, jogador 2 em cinza.
        for r in 0..N {
            for c in 0..N {
                let o = self.owner[r * N + c];
                if o != 0 {
                    let rect = Rect::new(x0 + c as i32 * s + 8, y0 + r as i32 * s + 8, s - 16, s - 16);
                    fb.round_rect(rect, 10, if o == 1 { INK } else { 0xAA });
                    let l = if o == 1 { "1" } else { "2" };
                    let lw = f.measure(Face::Display, 48.0, l);
                    f.draw(fb, Face::Display, 48.0, rect.x + (rect.w - lw) / 2, rect.y + rect.h / 2 + 18, PAPER, l);
                }
            }
        }
        for e in 0..E {
            if !self.edges[e] {
                continue;
            }
            let r = Self::edge_rect(e);
            let thick = if Some(e) == self.last { 12 } else { 7 };
            if e < H {
                fb.fill(Rect::new(r.x - 14, r.y + r.h / 2 - thick / 2, r.w + 28, thick), INK);
            } else {
                fb.fill(Rect::new(r.x + r.w / 2 - thick / 2, r.y - 14, thick, r.h + 28), INK);
            }
        }
        for r in 0..=N {
            for c in 0..=N {
                let (x, y) = Self::dot(r, c);
                fb.round_rect(Rect::new(x - 11, y - 11, 22, 22), 11, INK);
            }
        }
        minigame::hint(fb, f, "Toque entre dois pontos para traçar a linha.");
        minigame::pill(fb, f, minigame::bottom_button(0, 1), "Novo jogo", false);
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.mode == Mode::Menu {
            return (0..MENU.len()).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        let mut v = vec![(800u32, minigame::bottom_button(0, 1))];
        let my_turn = !matches!(self.mode, Mode::Cpu(_)) || self.turn == 1;
        if my_turn && !self.over() {
            v.extend((0..E).filter(|&e| !self.edges[e]).map(|e| (e as u32, Self::edge_rect(e))));
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900..=903 => {
                let mode = if id == 903 { Mode::Local } else { Mode::Cpu((id - 900) as u8) };
                *self = Caixas { mode, ..Caixas::new() };
                Redraw::Full
            }
            800 => {
                self.mode = Mode::Menu;
                Redraw::Full
            }
            e if (e as usize) < E => {
                self.play(e as usize);
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            _ => Redraw::None,
        }
    }

    fn cpu_turn(&self) -> bool {
        matches!(self.mode, Mode::Cpu(_)) && self.turn == 2 && !self.over()
    }

    fn cpu_move(&mut self) {
        let Mode::Cpu(level) = self.mode else { return };
        let free: Vec<usize> = (0..E).filter(|&e| !self.edges[e]).collect();
        // 1) Fecha caixa quando pode (exceto no fácil, às vezes).
        let closing: Vec<usize> = free.iter().copied().filter(|&e| boxes_of(e).iter().any(|&(r, c)| sides(&self.edges, r, c) == 3)).collect();
        let pick = |v: &[usize], rng: &mut Rng| v[(rng.next() % v.len() as u64) as usize];
        if !closing.is_empty() && !(level == 0 && self.rng.next() % 3 == 0) {
            let e = pick(&closing, &mut self.rng);
            self.play(e);
            return;
        }
        // 2) Linha segura: não deixa nenhuma caixa com três lados.
        let safe: Vec<usize> = free.iter().copied().filter(|&e| boxes_of(e).iter().all(|&(r, c)| sides(&self.edges, r, c) < 2)).collect();
        if level >= 1 && !safe.is_empty() {
            let e = pick(&safe, &mut self.rng);
            self.play(e);
            return;
        }
        // 3) Sem saída: entrega o menor número de caixas (difícil calcula; os outros sorteiam).
        let e = if level == 2 {
            *free.iter().min_by_key(|&&e| self.giveaway(e)).unwrap()
        } else {
            pick(&free, &mut self.rng)
        };
        self.play(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fechar_caixa_marca_e_joga_de_novo() {
        let mut g = Caixas { mode: Mode::Local, ..Caixas::new() };
        let [a, b, c, d] = box_edges(0, 0);
        g.play(a);
        g.play(b);
        g.play(c);
        let turn = g.turn;
        assert_eq!(g.play(d), 1);
        assert_eq!(g.owner[0], turn);
        assert_eq!(g.turn, turn); // joga de novo
    }
}
