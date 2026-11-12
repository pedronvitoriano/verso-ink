//! Paciência (Klondike): toque numa carta e depois no destino; tocar de novo na mesma carta
//! manda para a fundação quando der. Comprar 1 (fácil) ou 3 (difícil). Naipes desenhados em vetor:
//! copas e ouros em cinza, espadas e paus em preto.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Card {
    rank: u8, // 1..=13
    suit: u8, // 0 copas, 1 ouros, 2 espadas, 3 paus
    up: bool,
}

impl Card {
    fn red(self) -> bool {
        self.suit < 2
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pile {
    Waste,
    Found(usize),
    Tab(usize),
}

#[derive(Clone)]
struct State {
    stock: Vec<Card>,
    waste: Vec<Card>,
    found: [Vec<Card>; 4],
    tab: [Vec<Card>; 7],
}

pub struct Paciencia {
    menu: bool,
    draw3: bool,
    s: State,
    undo: Vec<State>,
    /// Seleção: pilha e índice da carta (na coluna, a partir dela para baixo).
    sel: Option<(Pile, usize)>,
    moves: usize,
}

const CW: i32 = 128;
const CH: i32 = 178;
const GAP: i32 = 8;
const X0: i32 = (W - 7 * CW - 6 * GAP) / 2;
const TOP: i32 = 270;
const TAB_Y: i32 = 480;
const TAB_BOTTOM: i32 = 1300;

fn col_x(c: usize) -> i32 {
    X0 + c as i32 * (CW + GAP)
}

impl Paciencia {
    pub fn new() -> Self {
        let empty = State { stock: vec![], waste: vec![], found: Default::default(), tab: Default::default() };
        Paciencia { menu: true, draw3: false, s: empty, undo: vec![], sel: None, moves: 0 }
    }

    fn deal(&mut self, draw3: bool) {
        let mut deck: Vec<Card> = (0..52).map(|i| Card { rank: (i % 13) as u8 + 1, suit: (i / 13) as u8, up: false }).collect();
        Rng::new().shuffle(&mut deck);
        let mut tab: [Vec<Card>; 7] = Default::default();
        for (c, col) in tab.iter_mut().enumerate() {
            for k in 0..=c {
                let mut card = deck.pop().unwrap();
                card.up = k == c;
                col.push(card);
            }
        }
        self.menu = false;
        self.draw3 = draw3;
        self.s = State { stock: deck, waste: vec![], found: Default::default(), tab };
        self.undo.clear();
        self.sel = None;
        self.moves = 0;
    }

    fn won(&self) -> bool {
        self.s.found.iter().all(|f| f.len() == 13)
    }

    /// Distância vertical entre cartas da coluna (aperta se a coluna for grande).
    fn offsets(&self, c: usize) -> (i32, i32) {
        let col = &self.s.tab[c];
        let down = col.iter().filter(|x| !x.up).count() as i32;
        let up = col.len() as i32 - down;
        let (mut d, mut u) = (24, 50);
        let avail = TAB_BOTTOM - TAB_Y - CH;
        while down * d + (up - 1).max(0) * u > avail && u > 22 {
            u -= 2;
            d = d.min(u / 2).max(10);
        }
        (d, u)
    }

    fn tab_card_rect(&self, c: usize, k: usize) -> Rect {
        let (d, u) = self.offsets(c);
        let mut y = TAB_Y;
        for card in &self.s.tab[c][..k] {
            y += if card.up { u } else { d };
        }
        let h = if k + 1 == self.s.tab[c].len() { CH } else if self.s.tab[c][k].up { u } else { d };
        Rect::new(col_x(c), y, CW, h)
    }

    fn pile_rect(p: Pile) -> Rect {
        match p {
            Pile::Waste => Rect::new(col_x(1), TOP, CW, CH),
            Pile::Found(i) => Rect::new(col_x(3 + i), TOP, CW, CH),
            Pile::Tab(c) => Rect::new(col_x(c), TAB_Y, CW, TAB_BOTTOM - TAB_Y),
        }
    }

    fn stock_rect() -> Rect {
        Rect::new(col_x(0), TOP, CW, CH)
    }

    fn moving(&self) -> Vec<Card> {
        match self.sel {
            Some((Pile::Waste, _)) => self.s.waste.last().copied().into_iter().collect(),
            Some((Pile::Tab(c), k)) => self.s.tab[c][k..].to_vec(),
            Some((Pile::Found(i), _)) => self.s.found[i].last().copied().into_iter().collect(),
            None => vec![],
        }
    }

    fn can_drop(&self, cards: &[Card], to: Pile) -> bool {
        let Some(first) = cards.first() else { return false };
        match to {
            Pile::Found(i) => {
                cards.len() == 1 && match self.s.found[i].last() {
                    None => first.rank == 1,
                    Some(t) => t.suit == first.suit && first.rank == t.rank + 1,
                }
            }
            Pile::Tab(c) => match self.s.tab[c].last() {
                None => first.rank == 13,
                Some(t) => t.up && t.red() != first.red() && first.rank + 1 == t.rank,
            },
            Pile::Waste => false,
        }
    }

    fn do_move(&mut self, to: Pile) -> bool {
        let cards = self.moving();
        if !self.can_drop(&cards, to) {
            return false;
        }
        self.undo.push(self.s.clone());
        match self.sel.unwrap() {
            (Pile::Waste, _) => {
                self.s.waste.pop();
            }
            (Pile::Tab(c), k) => {
                self.s.tab[c].truncate(k);
                if let Some(last) = self.s.tab[c].last_mut() {
                    last.up = true;
                }
            }
            (Pile::Found(i), _) => {
                self.s.found[i].pop();
            }
        }
        match to {
            Pile::Found(i) => self.s.found[i].extend(cards),
            Pile::Tab(c) => self.s.tab[c].extend(cards),
            Pile::Waste => {}
        }
        self.sel = None;
        self.moves += 1;
        true
    }

    /// Tenta mandar a carta selecionada para alguma fundação.
    fn to_foundation(&mut self) -> bool {
        (0..4).any(|i| self.do_move(Pile::Found(i)))
    }

    fn draw_stock(&mut self) {
        self.undo.push(self.s.clone());
        if self.s.stock.is_empty() {
            self.s.stock = self.s.waste.drain(..).rev().map(|mut c| {
                c.up = false;
                c
            }).collect();
        } else {
            let n = if self.draw3 { 3 } else { 1 };
            for _ in 0..n {
                if let Some(mut c) = self.s.stock.pop() {
                    c.up = true;
                    self.s.waste.push(c);
                }
            }
        }
        self.sel = None;
    }

    fn suit_icon(fb: &mut Framebuffer, suit: u8, cx: f32, cy: f32, s: f32, v: u8) {
        match suit {
            0 => {
                // Copas: dois círculos e uma ponta.
                for dx in [-0.25, 0.25] {
                    let r = Rect::new((cx + dx * s - 0.27 * s) as i32, (cy - 0.35 * s) as i32, (0.54 * s) as i32, (0.54 * s) as i32);
                    fb.round_rect(r, (0.27 * s) as i32, v);
                }
                for k in 0..((0.6 * s) as i32) {
                    let y = cy - 0.05 * s + k as f32;
                    let half = 0.5 * s * (1.0 - k as f32 / (0.6 * s));
                    fb.fill(Rect::new((cx - half) as i32, y as i32, (2.0 * half) as i32, 1), v);
                }
            }
            1 => {
                // Ouros: losango.
                for k in 0..(s as i32) {
                    let t = k as f32 / s;
                    let half = 0.42 * s * (1.0 - (2.0 * t - 1.0).abs());
                    fb.fill(Rect::new((cx - half) as i32, (cy - 0.5 * s + k as f32) as i32, (2.0 * half) as i32 + 1, 1), v);
                }
            }
            2 => {
                // Espadas: copas de ponta-cabeça e um pé.
                for dx in [-0.25, 0.25] {
                    let r = Rect::new((cx + dx * s - 0.27 * s) as i32, (cy - 0.05 * s) as i32, (0.54 * s) as i32, (0.5 * s) as i32);
                    fb.round_rect(r, (0.25 * s) as i32, v);
                }
                for k in 0..((0.55 * s) as i32) {
                    let y = cy - 0.5 * s + k as f32;
                    let half = 0.5 * s * (k as f32 / (0.55 * s));
                    fb.fill(Rect::new((cx - half) as i32, y as i32, (2.0 * half) as i32 + 1, 1), v);
                }
                fb.fill(Rect::new((cx - 0.06 * s) as i32, (cy + 0.3 * s) as i32, (0.12 * s) as i32 + 1, (0.25 * s) as i32), v);
            }
            _ => {
                // Paus: três bolas e um pé.
                for (dx, dy) in [(0.0, -0.25), (-0.27, 0.08), (0.27, 0.08)] {
                    let d = 0.46 * s;
                    fb.round_rect(Rect::new((cx + dx * s - d / 2.0) as i32, (cy + dy * s - d / 2.0) as i32, d as i32, d as i32), (d / 2.0) as i32, v);
                }
                fb.fill(Rect::new((cx - 0.06 * s) as i32, (cy + 0.1 * s) as i32, (0.12 * s) as i32 + 1, (0.4 * s) as i32), v);
            }
        }
    }

    fn rank_text(r: u8) -> String {
        match r {
            1 => "A".into(),
            11 => "J".into(),
            12 => "Q".into(),
            13 => "K".into(),
            n => n.to_string(),
        }
    }

    fn draw_card(fb: &mut Framebuffer, f: &Fonts, r: Rect, c: Card, selected: bool) {
        let full = Rect::new(r.x, r.y, CW, CH);
        if !c.up {
            fb.round_rect(full, 14, INK);
            fb.round_outline(Rect::new(full.x + 8, full.y + 8, CW - 16, CH - 16), 10, 3, 0x88, INK);
            return;
        }
        fb.round_outline(full, 14, if selected { 8 } else { 3 }, INK, PAPER);
        let v = if c.red() { 0x77 } else { INK };
        let t = Self::rank_text(c.rank);
        f.draw(fb, Face::Display, 40.0, full.x + 12, full.y + 44, v, &t);
        Self::suit_icon(fb, c.suit, (full.x + CW - 30) as f32, (full.y + 28) as f32, 30.0, v);
        Self::suit_icon(fb, c.suit, (full.x + CW / 2) as f32, (full.y + CH / 2 + 22) as f32, 70.0, v);
    }

    fn slot(fb: &mut Framebuffer, r: Rect) {
        fb.round_outline(Rect::new(r.x, r.y, CW, CH), 14, 2, 0xBB, PAPER);
    }
}

impl MiniGame for Paciencia {
    fn risky(&self, id: u32) -> bool {
        id == 802 && !self.won()
    }

    fn at_menu(&self) -> bool {
        self.menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Paciência", "Monte as quatro fundações de A até K por naipe.", &["Comprar 1 (fácil)", "Comprar 3 (clássico)"]);
            return;
        }
        let status = if self.won() { format!("Venceu em {} jogadas!", self.moves) } else { format!("{} jogadas · toque e depois o destino", self.moves) };
        minigame::header(fb, f, "Paciência", &status);
        // Monte e descarte.
        let st = Self::stock_rect();
        if self.s.stock.is_empty() {
            Self::slot(fb, st);
            let (cx, cy) = (st.x as f32 + CW as f32 / 2.0, st.y as f32 + CH as f32 / 2.0);
            for k in 0..3 {
                let a = k as f32 * 2.0;
                ui::stroke(fb, (cx + 30.0 * a.cos(), cy + 30.0 * a.sin()), (cx + 30.0 * (a + 1.6).cos(), cy + 30.0 * (a + 1.6).sin()), 5.0, MUTED);
            }
        } else {
            Self::draw_card(fb, f, st, Card { rank: 1, suit: 0, up: false }, false);
        }
        match self.s.waste.last() {
            Some(c) => Self::draw_card(fb, f, Self::pile_rect(Pile::Waste), *c, matches!(self.sel, Some((Pile::Waste, _)))),
            None => Self::slot(fb, Self::pile_rect(Pile::Waste)),
        }
        for i in 0..4 {
            let r = Self::pile_rect(Pile::Found(i));
            match self.s.found[i].last() {
                Some(c) => Self::draw_card(fb, f, r, *c, matches!(self.sel, Some((Pile::Found(j), _)) if j == i)),
                None => {
                    Self::slot(fb, r);
                    Self::suit_icon(fb, i as u8, (r.x + CW / 2) as f32, (r.y + CH / 2) as f32, 60.0, 0xDD);
                }
            }
        }
        for c in 0..7 {
            if self.s.tab[c].is_empty() {
                Self::slot(fb, Rect::new(col_x(c), TAB_Y, CW, CH));
            }
            for k in 0..self.s.tab[c].len() {
                let r = self.tab_card_rect(c, k);
                let sel = matches!(self.sel, Some((Pile::Tab(cc), kk)) if cc == c && k >= kk);
                Self::draw_card(fb, f, r, self.s.tab[c][k], sel);
            }
        }
        let labels = if self.won() { vec!["Jogar de novo"] } else { vec!["Desfazer", "Novo jogo"] };
        for (k, l) in labels.iter().enumerate() {
            minigame::pill(fb, f, minigame::bottom_button(k, labels.len()), l, self.won());
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu {
            return (0..2).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.won() {
            return vec![(803, minigame::bottom_button(0, 1))];
        }
        let mut v = vec![(801, minigame::bottom_button(0, 2)), (802, minigame::bottom_button(1, 2)), (700, Self::stock_rect()), (701, Self::pile_rect(Pile::Waste))];
        v.extend((0..4).map(|i| (710 + i as u32, Self::pile_rect(Pile::Found(i)))));
        // Cartas viradas de cada coluna (de baixo para cima, para o toque achar a de cima primeiro).
        for c in 0..7 {
            for k in (0..self.s.tab[c].len()).rev() {
                if self.s.tab[c][k].up {
                    v.push(((c * 100 + k) as u32, self.tab_card_rect(c, k)));
                }
            }
            v.push((720 + c as u32, Self::pile_rect(Pile::Tab(c))));
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let area = Redraw::Part(Rect::new(0, 170, W, 1448 - 170));
        match id {
            900 | 901 => {
                self.deal(id == 901);
                return Redraw::Full;
            }
            803 | 802 => {
                self.menu = true;
                return Redraw::Full;
            }
            801 => {
                if let Some(s) = self.undo.pop() {
                    self.s = s;
                    self.sel = None;
                }
            }
            700 => self.draw_stock(),
            701 => {
                if self.sel == Some((Pile::Waste, 0)) {
                    self.to_foundation();
                    self.sel = None;
                } else if !self.s.waste.is_empty() {
                    self.sel = Some((Pile::Waste, 0));
                }
            }
            710..=713 => {
                let i = (id - 710) as usize;
                if self.sel.is_some() {
                    if !self.do_move(Pile::Found(i)) {
                        self.sel = None;
                    }
                } else if !self.s.found[i].is_empty() {
                    self.sel = Some((Pile::Found(i), 0));
                }
            }
            720..=726 => {
                let c = (id - 720) as usize;
                if self.sel.is_some() && !self.do_move(Pile::Tab(c)) {
                    self.sel = None;
                }
            }
            id => {
                let (c, k) = ((id / 100) as usize, (id % 100) as usize);
                if c >= 7 || k >= self.s.tab[c].len() {
                    return Redraw::None;
                }
                match self.sel {
                    // Tocar de novo na mesma carta: tenta a fundação.
                    Some((Pile::Tab(cc), kk)) if cc == c && kk == k => {
                        if !self.to_foundation() {
                            self.sel = None;
                        }
                    }
                    Some(_) => {
                        if !self.do_move(Pile::Tab(c)) {
                            self.sel = Some((Pile::Tab(c), k));
                        }
                    }
                    None => self.sel = Some((Pile::Tab(c), k)),
                }
            }
        }
        if self.won() { Redraw::Full } else { area }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distribuicao_e_regras() {
        let mut g = Paciencia::new();
        g.deal(false);
        assert_eq!(g.s.stock.len(), 24);
        assert!(g.s.tab.iter().enumerate().all(|(c, col)| col.len() == c + 1 && col.last().unwrap().up));
        g.s.tab[0] = vec![Card { rank: 13, suit: 2, up: true }];
        g.sel = Some((Pile::Tab(0), 0));
        g.s.tab[1] = vec![];
        assert!(g.do_move(Pile::Tab(1))); // rei vai para coluna vazia
        g.s.waste = vec![Card { rank: 12, suit: 0, up: true }];
        g.sel = Some((Pile::Waste, 0));
        assert!(g.do_move(Pile::Tab(1))); // dama vermelha sobre rei preto
    }
}
