//! UNO, portado das regras do UNO do Pedro (C, AED1 com Danilo Carrasquel): 108 cartas, +2 e +4
//! acumulam (+2 sobre +2, +2 sobre +4 se for da cor escolhida, +4 sobre qualquer +2/+4), Bloqueio,
//! Inverte, Troca a cor, carta comprada pode ser jogada na hora e quem esquece o "UNO!" compra 2.
//! No e-ink, cor vira padrão: vermelho cheio, azul listrado, verde pontilhado, amarelo com moldura.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Red,
    Green,
    Blue,
    Yellow,
    Black,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Num(u8),
    Skip,
    Reverse,
    Draw2,
    Draw4,
    Wild,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Card {
    pub kind: Kind,
    pub color: Color,
}

const COLORS: [Color; 4] = [Color::Red, Color::Green, Color::Blue, Color::Yellow];

fn color_name(c: Color) -> &'static str {
    match c {
        Color::Red => "vermelho",
        Color::Green => "verde",
        Color::Blue => "azul",
        Color::Yellow => "amarelo",
        Color::Black => "preta",
    }
}

fn label(k: Kind) -> String {
    match k {
        Kind::Num(n) => n.to_string(),
        Kind::Skip => "Bloqueio".into(),
        Kind::Reverse => "Inverte".into(),
        Kind::Draw2 => "+2".into(),
        Kind::Draw4 => "+4".into(),
        Kind::Wild => "Troca a cor".into(),
    }
}

pub fn deck() -> Vec<Card> {
    let mut d = Vec::with_capacity(108);
    for &c in &COLORS {
        d.push(Card { kind: Kind::Num(0), color: c });
        for _ in 0..2 {
            for n in 1..=9 {
                d.push(Card { kind: Kind::Num(n), color: c });
            }
            for k in [Kind::Skip, Kind::Reverse, Kind::Draw2] {
                d.push(Card { kind: k, color: c });
            }
        }
    }
    for _ in 0..4 {
        d.push(Card { kind: Kind::Draw4, color: Color::Black });
        d.push(Card { kind: Kind::Wild, color: Color::Black });
    }
    d
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Menu,
    Cpu(usize),
    Local(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Turn,
    /// Escolhendo a cor depois de um preto (índice da carta já jogada).
    PickColor,
    /// Local: tela de troca antes da vez do jogador.
    Handoff,
    Over(usize),
}

pub struct Uno {
    mode: Mode,
    phase: Phase,
    draw: Vec<Card>,
    discard: Vec<Card>,
    hands: Vec<Vec<Card>>,
    turn: usize,
    dir: i32,
    color: Color,
    /// Cartas acumuladas por +2/+4 contra o próximo.
    pending: usize,
    /// Comprou nesta vez: pode jogar a comprada ou passar.
    drew: Option<usize>,
    /// Quem ficou com uma carta sem dizer UNO (e pode ser pego).
    uno_open: Option<usize>,
    said_uno: bool,
    page: usize,
    msg: String,
    rng: Rng,
}

const MENU: [&str; 4] = ["Contra 1 CPU", "Contra 3 CPUs", "Duas pessoas", "Quatro pessoas"];

impl Uno {
    pub fn new() -> Self {
        Uno {
            mode: Mode::Menu,
            phase: Phase::Turn,
            draw: Vec::new(),
            discard: Vec::new(),
            hands: Vec::new(),
            turn: 0,
            dir: 1,
            color: Color::Red,
            pending: 0,
            drew: None,
            uno_open: None,
            said_uno: false,
            page: 0,
            msg: String::new(),
            rng: Rng::new(),
        }
    }

    fn players(&self) -> usize {
        match self.mode {
            Mode::Cpu(n) => n + 1,
            Mode::Local(n) => n,
            Mode::Menu => 0,
        }
    }

    fn is_cpu(&self, p: usize) -> bool {
        matches!(self.mode, Mode::Cpu(_)) && p != 0
    }

    fn name(&self, p: usize) -> String {
        match self.mode {
            Mode::Cpu(_) if p == 0 => "Você".into(),
            Mode::Cpu(1) => "CPU".into(),
            Mode::Cpu(_) => format!("CPU {p}"),
            _ => format!("Jogador {}", p + 1),
        }
    }

    fn start(&mut self, mode: Mode) {
        let mut rng = Rng::new();
        let mut d = deck();
        rng.shuffle(&mut d);
        *self = Uno { mode, rng, ..Uno::new() };
        self.draw = d;
        self.hands = vec![Vec::new(); self.players()];
        for _ in 0..7 {
            for p in 0..self.players() {
                let c = self.take();
                self.hands[p].push(c);
            }
        }
        // A primeira do descarte é sempre um número (como na mesa do jogo original).
        loop {
            let c = self.take();
            if matches!(c.kind, Kind::Num(_)) {
                self.color = c.color;
                self.discard.push(c);
                break;
            }
            self.draw.insert(0, c);
        }
        self.phase = if matches!(mode, Mode::Local(_)) { Phase::Handoff } else { Phase::Turn };
        self.msg = "Combine cor ou símbolo com a carta da mesa.".into();
    }

    fn take(&mut self) -> Card {
        if self.draw.is_empty() {
            // Reembaralha o descarte, deixando a carta de cima.
            let top = self.discard.pop();
            self.draw = std::mem::take(&mut self.discard);
            self.rng.shuffle(&mut self.draw);
            if let Some(t) = top {
                self.discard.push(t);
            }
        }
        self.draw.pop().unwrap_or(Card { kind: Kind::Num(0), color: Color::Red })
    }

    fn top(&self) -> Card {
        *self.discard.last().unwrap()
    }

    /// Regras de quando uma carta pode ser jogada (inclui o acúmulo de +2/+4).
    pub fn playable(&self, c: Card) -> bool {
        let top = self.top();
        if self.pending > 0 {
            return match (c.kind, top.kind) {
                (Kind::Draw2, Kind::Draw2) => true,
                (Kind::Draw2, Kind::Draw4) => c.color == self.color,
                (Kind::Draw4, Kind::Draw2 | Kind::Draw4) => true,
                _ => false,
            };
        }
        c.color == Color::Black || c.color == self.color || c.kind == top.kind
    }

    fn next(&self, from: usize, steps: i32) -> usize {
        let n = self.players() as i32;
        ((from as i32 + self.dir * steps).rem_euclid(n)) as usize
    }

    fn end_turn(&mut self, skip: bool) {
        self.drew = None;
        self.page = 0;
        self.turn = self.next(self.turn, if skip { 2 } else { 1 });
        if matches!(self.mode, Mode::Local(_)) && !matches!(self.phase, Phase::Over(_)) {
            self.phase = Phase::Handoff;
        }
    }

    /// Joga a carta `i` da mão de quem está na vez.
    fn play(&mut self, i: usize) {
        let p = self.turn;
        let c = self.hands[p].remove(i);
        self.discard.push(c);
        if c.color != Color::Black {
            self.color = c.color;
        }
        self.msg = format!("{} jogou {} {}.", self.name(p), label(c.kind), if c.color == Color::Black { String::new() } else { color_name(c.color).to_string() });
        if self.hands[p].is_empty() {
            self.phase = Phase::Over(p);
            return;
        }
        // Ficou com uma: precisa dizer UNO (a CPU diz sozinha, quase sempre).
        if self.hands[p].len() == 1 {
            let says = if self.is_cpu(p) { self.rng.next() % 10 < 8 } else { self.said_uno };
            self.uno_open = if says { None } else { Some(p) };
            if says {
                self.msg.push_str(&format!(" {} disse UNO!", self.name(p)));
            }
        }
        self.said_uno = false;
        match c.kind {
            Kind::Draw2 => self.pending += 2,
            Kind::Draw4 => self.pending += 4,
            _ => {}
        }
        if c.color == Color::Black && !self.is_cpu(p) {
            self.phase = Phase::PickColor;
            return;
        }
        if c.color == Color::Black {
            self.color = self.cpu_color(p);
            self.msg.push_str(&format!(" Cor: {}.", color_name(self.color)));
        }
        self.after_play(c);
    }

    fn after_play(&mut self, c: Card) {
        let two = self.players() == 2;
        match c.kind {
            Kind::Skip => self.end_turn(true),
            Kind::Reverse => {
                self.dir = -self.dir;
                self.end_turn(two); // com dois jogadores, inverter é como bloquear
            }
            _ => self.end_turn(false),
        }
    }

    /// Quem está na vez compra (ou paga o acúmulo, perdendo a vez).
    fn draw_turn(&mut self) {
        let p = self.turn;
        if self.pending > 0 {
            let n = self.pending;
            for _ in 0..n {
                let c = self.take();
                self.hands[p].push(c);
            }
            self.pending = 0;
            self.msg = format!("{} comprou {n} e passou a vez.", self.name(p));
            self.end_turn(false);
            return;
        }
        let c = self.take();
        self.hands[p].push(c);
        self.drew = Some(self.hands[p].len() - 1);
        self.msg = if self.playable(c) { format!("{} comprou uma carta que serve.", self.name(p)) } else { format!("{} comprou uma carta.", self.name(p)) };
        if !self.playable(c) {
            self.end_turn(false);
        }
    }

    fn cpu_color(&self, p: usize) -> Color {
        *COLORS.iter().max_by_key(|&&c| self.hands[p].iter().filter(|x| x.color == c).count()).unwrap()
    }

    /// Pegar quem esqueceu o UNO: compra 2.
    fn catch(&mut self, who: usize) {
        for _ in 0..2 {
            let c = self.take();
            self.hands[who].push(c);
        }
        self.uno_open = None;
        self.msg = format!("{} esqueceu o UNO e comprou 2.", self.name(who));
    }

    // ---------- Desenho ----------

    /// Cartas da mão em leque: (índice na mão, retângulo, serve agora).
    fn hand_layout(&self) -> Vec<(usize, Rect, bool)> {
        let me = self.me();
        let Some(hand) = self.hands.get(me) else { return vec![] };
        let my_turn = self.turn == me && self.phase == Phase::Turn && !self.is_cpu(self.turn);
        let shown: Vec<(usize, Card)> = hand.iter().copied().enumerate().skip(self.page * 14).take(14).collect();
        let n = shown.len() as i32;
        let (w, h) = (156, 230);
        let span = W - 2 * MARGIN + 40;
        let step = if n <= 1 { 0 } else { ((span - w) / (n - 1)).min(w + 12) };
        let total = if n == 0 { 0 } else { w + step * (n - 1) };
        let x0 = (W - total) / 2;
        shown
            .into_iter()
            .enumerate()
            .map(|(k, (i, c))| {
                let ok = my_turn && self.playable(c) && self.drew.is_none_or(|d| d == i);
                let y = if ok { 990 } else { 1030 };
                (i, Rect::new(x0 + k as i32 * step, y, w, h), ok)
            })
            .collect()
    }

    /// Área de toque de cada carta: a faixa visível (a última inteira).
    fn hand_hits(&self) -> Vec<(usize, Rect, bool)> {
        let lay = self.hand_layout();
        let mut v = Vec::new();
        for (k, (i, r, ok)) in lay.iter().enumerate() {
            let w = lay.get(k + 1).map(|n| n.1.x - r.x).unwrap_or(r.w);
            v.push((*i, Rect::new(r.x, r.y, w, r.h), *ok));
        }
        v
    }

    fn deck_rect() -> Rect {
        Rect::new(W / 2 - 40 - 210, 520, 210, 308)
    }
    fn top_rect() -> Rect {
        Rect::new(W / 2 + 40, 500, 230, 338)
    }
    fn pick_rect(k: usize) -> Rect {
        let (w, h, gap) = (200, 294, 24);
        let x0 = (W - (4 * w + 3 * gap)) / 2;
        Rect::new(x0 + k as i32 * (w + gap), 1000, w, h)
    }

    /// Tom do campo e cor do texto/desenho sobre ele.
    fn field(c: Color) -> (u8, u8) {
        match c {
            Color::Red => (0x14, PAPER),
            Color::Blue => (0x5C, PAPER),
            Color::Green => (0x9C, INK),
            Color::Yellow => (0xD6, INK),
            Color::Black => (0x14, PAPER),
        }
    }

    /// Símbolo da cor (para não depender só do tom): ● vermelho, ◆ azul, ▲ verde, ■ amarelo.
    fn color_mark(fb: &mut Framebuffer, cx: i32, cy: i32, d: i32, c: Color, v: u8) {
        match c {
            Color::Red => fb.round_rect(Rect::new(cx - d, cy - d, 2 * d, 2 * d), d, v),
            Color::Blue => {
                for k in 0..=d {
                    fb.fill(Rect::new(cx - k, cy - d + k, 2 * k + 1, 1), v);
                    fb.fill(Rect::new(cx - k, cy + d - k, 2 * k + 1, 1), v);
                }
            }
            Color::Green => {
                for k in 0..=(2 * d) {
                    let half = k / 2;
                    fb.fill(Rect::new(cx - half, cy - d + k, 2 * half + 1, 1), v);
                }
            }
            Color::Yellow => fb.fill(Rect::new(cx - d * 8 / 10, cy - d * 8 / 10, d * 16 / 10, d * 16 / 10), v),
            Color::Black => {}
        }
    }

    /// Padrão do campo de cada cor (além do tom): listras no azul, pontos no verde.
    fn field_pattern(fb: &mut Framebuffer, r: Rect, c: Color) {
        match c {
            Color::Blue => {
                for y in r.y..r.y + r.h {
                    let mut x = r.x + (14 - (r.x + y).rem_euclid(14)) % 14;
                    while x < r.x + r.w {
                        fb.fill(Rect::new(x, y, 3.min(r.x + r.w - x), 1), 0x7C);
                        x += 14;
                    }
                }
            }
            Color::Green => {
                let mut y = r.y + 8;
                while y < r.y + r.h - 6 {
                    let mut x = r.x + 8 + if (y / 14) % 2 == 0 { 0 } else { 7 };
                    while x < r.x + r.w - 6 {
                        fb.round_rect(Rect::new(x, y, 5, 5), 2, 0x80);
                        x += 14;
                    }
                    y += 14;
                }
            }
            _ => {}
        }
    }

    /// Oval branco inclinado, como no baralho de verdade.
    fn oval(fb: &mut Framebuffer, r: Rect, v: u8) {
        let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
        let (a, b) = (r.w as f32 * 0.40, r.h as f32 * 0.36);
        let (sn, cs) = (0.5f32, 0.866f32);
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                let (dx, dy) = (x as f32 - cx, y as f32 - cy);
                let (u, w) = (dx * cs + dy * sn, -dx * sn + dy * cs);
                if (u / b).powi(2) + (w / a).powi(2) <= 1.0 {
                    fb.fill(Rect::new(x, y, 1, 1), v);
                }
            }
        }
    }

    /// Símbolo da carta no meio do oval (tamanho `s`).
    fn symbol(fb: &mut Framebuffer, f: &Fonts, cx: i32, cy: i32, s: i32, c: Card, ink: u8) {
        let (x, y, sf) = (cx as f32, cy as f32, s as f32);
        match c.kind {
            Kind::Num(n) => {
                let t = n.to_string();
                let size = sf * 1.25;
                let w = f.measure(Face::Display, size, &t);
                f.draw(fb, Face::Display, size, cx - w / 2, cy + (size * 0.36) as i32, ink, &t);
                if n == 6 || n == 9 {
                    fb.fill(Rect::new(cx - w / 2, cy + (size * 0.46) as i32, w, (sf * 0.07).max(3.0) as i32), ink);
                }
            }
            Kind::Draw2 | Kind::Draw4 => {
                let t = if c.kind == Kind::Draw2 { "+2" } else { "+4" };
                let size = sf * 1.05;
                let w = f.measure(Face::Display, size, t);
                f.draw(fb, Face::Display, size, cx - w / 2, cy + (size * 0.36) as i32, ink, t);
            }
            Kind::Skip => {
                let rr = (sf * 0.42) as i32;
                fb.round_outline(Rect::new(cx - rr, cy - rr, 2 * rr, 2 * rr), rr, (sf * 0.12) as i32, ink, PAPER);
                ui::stroke(fb, (x - sf * 0.28, y + sf * 0.28), (x + sf * 0.28, y - sf * 0.28), sf * 0.12, ink);
            }
            Kind::Reverse => {
                let d = sf * 0.34;
                ui::stroke(fb, (x - d, y - d * 0.3), (x + d * 0.6, y - d * 0.3), sf * 0.11, ink);
                ui::stroke(fb, (x + d * 0.6, y - d * 0.3), (x + d * 0.1, y - d * 0.8), sf * 0.11, ink);
                ui::stroke(fb, (x + d, y + d * 0.3), (x - d * 0.6, y + d * 0.3), sf * 0.11, ink);
                ui::stroke(fb, (x - d * 0.6, y + d * 0.3), (x - d * 0.1, y + d * 0.8), sf * 0.11, ink);
            }
            Kind::Wild => {
                // Quatro quadrantes, um de cada cor.
                let rr = (sf * 0.42) as i32;
                fb.round_rect(Rect::new(cx - rr - 3, cy - rr - 3, 2 * rr + 6, 2 * rr + 6), 8, ink);
                for (k, col) in COLORS.iter().enumerate() {
                    let q = Rect::new(cx - rr + (k as i32 % 2) * rr, cy - rr + (k as i32 / 2) * rr, rr, rr);
                    fb.fill(q, Self::field(*col).0);
                    Self::field_pattern(fb, q, *col);
                }
                for (k, col) in COLORS.iter().enumerate() {
                    let q = (cx - rr / 2 + (k as i32 % 2) * rr, cy - rr / 2 + (k as i32 / 2) * rr);
                    Self::color_mark(fb, q.0, q.1, rr / 5, *col, Self::field(*col).1);
                }
            }
        }
    }

    /// Canto: o mesmo símbolo pequeno e o símbolo da cor.
    fn corner(fb: &mut Framebuffer, f: &Fonts, r: Rect, c: Card, ink: u8) {
        let size = (r.w as f32 * 0.24).max(20.0);
        let (x, y) = (r.x + r.w / 9, r.y + r.w / 9);
        match c.kind {
            Kind::Num(_) | Kind::Draw2 | Kind::Draw4 => {
                let t = match c.kind {
                    Kind::Num(n) => n.to_string(),
                    Kind::Draw2 => "+2".into(),
                    _ => "+4".into(),
                };
                f.draw(fb, Face::Display, size, x, y + size as i32, ink, &t);
            }
            _ => Self::symbol(fb, f, x + size as i32 / 2, y + size as i32 / 2 + 4, size as i32, c, ink),
        }
        if c.color != Color::Black {
            let d = (r.w / 16).max(5);
            Self::color_mark(fb, x + d, y + size as i32 + 12 + d, d, c.color, ink);
            Self::color_mark(fb, r.x + r.w - r.w / 9 - d, r.y + r.h - r.w / 9 - d, d, c.color, ink);
        }
    }

    fn draw_card(fb: &mut Framebuffer, f: &Fonts, r: Rect, c: Card, dim: bool) {
        let rad = r.w / 9;
        fb.round_rect(r, rad, INK);
        let face = Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6);
        fb.round_rect(face, rad - 2, PAPER);
        let b = r.w / 14;
        let field = Rect::new(r.x + b, r.y + b, r.w - 2 * b, r.h - 2 * b);
        let (tone, ink) = Self::field(c.color);
        fb.round_rect(field, rad - b / 2, tone);
        Self::field_pattern(fb, field, c.color);
        Self::oval(fb, field, PAPER);
        Self::symbol(fb, f, r.x + r.w / 2, r.y + r.h / 2, r.w * 2 / 5, c, if c.color == Color::Black { INK } else { Self::field(c.color).0.min(0x40) });
        Self::corner(fb, f, r, c, ink);
        if dim {
            // Não serve agora: véu claro, ainda legível.
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    fb.blend(x, y, PAPER, 120);
                }
            }
        }
    }

    /// Carta de escolher cor: só o campo da cor e o símbolo dela grande no oval.
    fn draw_color_card(fb: &mut Framebuffer, r: Rect, c: Color) {
        let rad = r.w / 9;
        fb.round_rect(r, rad, INK);
        fb.round_rect(Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6), rad - 2, PAPER);
        let b = r.w / 14;
        let field = Rect::new(r.x + b, r.y + b, r.w - 2 * b, r.h - 2 * b);
        fb.round_rect(field, rad - b / 2, Self::field(c).0);
        Self::field_pattern(fb, field, c);
        Self::oval(fb, field, PAPER);
        Self::color_mark(fb, r.x + r.w / 2, r.y + r.h / 2, r.w / 5, c, Self::field(c).0.min(0x40));
    }

    /// Verso da carta: preto com o oval branco e "UNO".
    fn draw_back(fb: &mut Framebuffer, f: &Fonts, r: Rect) {
        let rad = r.w / 9;
        fb.round_rect(r, rad, INK);
        let face = Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6);
        fb.round_rect(face, rad - 2, PAPER);
        let b = (r.w / 14).max(3);
        let field = Rect::new(r.x + b, r.y + b, r.w - 2 * b, r.h - 2 * b);
        fb.round_rect(field, rad - b / 2, INK);
        if r.w >= 90 {
            Self::oval(fb, field, PAPER);
            let size = r.w as f32 * 0.26;
            let w = f.measure(Face::Display, size, "UNO");
            f.draw(fb, Face::Display, size, r.x + (r.w - w) / 2, r.y + r.h / 2 + (size * 0.36) as i32, INK, "UNO");
        } else {
            let o = Rect::new(field.x + field.w / 5, field.y + field.h / 4, field.w * 3 / 5, field.h / 2);
            fb.round_rect(o, o.w / 2, 0x88);
        }
    }

    /// Adversários no alto: leque de cartas viradas, nome e quantas cartas.
    fn draw_opponents(&self, fb: &mut Framebuffer, f: &Fonts) {
        let me = self.me();
        let others: Vec<usize> = (0..self.players()).filter(|&p| p != me).collect();
        let n = others.len().max(1) as i32;
        let slot = (W - 2 * MARGIN) / n;
        for (k, &p) in others.iter().enumerate() {
            let cx = MARGIN + slot * k as i32 + slot / 2;
            let cards = self.hands[p].len() as i32;
            let shown = cards.min(9);
            let (w, h) = (66, 98);
            let step = if shown <= 1 { 0 } else { (slot - 60 - w).min(24 * (shown - 1)) / (shown - 1) };
            let total = w + step * (shown - 1).max(0);
            for i in 0..shown {
                Self::draw_back(fb, f, Rect::new(cx - total / 2 + i * step, 272, w, h));
            }
            let label = format!("{} · {}", self.name(p), cards);
            let turn = p == self.turn && matches!(self.phase, Phase::Turn);
            let lw = f.measure(Face::Strong, 30.0, &label);
            let y = 420;
            if turn {
                let pill = Rect::new(cx - lw / 2 - 20, y - 34, lw + 40, 48);
                fb.round_rect(pill, 24, INK);
                f.draw(fb, Face::Strong, 30.0, cx - lw / 2, y, PAPER, &label);
            } else {
                f.draw(fb, Face::Strong, 30.0, cx - lw / 2, y, INK, &label);
            }
            if cards == 1 {
                f.draw(fb, Face::Display, 30.0, cx + lw / 2 + 14, y, INK, "UNO!");
            }
        }
    }

    fn btn(k: usize, n: usize) -> Rect {
        minigame::bottom_button(k, n)
    }

    fn me(&self) -> usize {
        if matches!(self.mode, Mode::Cpu(_)) { 0 } else { self.turn }
    }

    fn buttons(&self) -> Vec<(u32, &'static str)> {
        if self.phase != Phase::Turn || (self.is_cpu(self.turn)) {
            return vec![(803, "Novo jogo")];
        }
        let mut v = Vec::new();
        if self.drew.is_some() {
            v.push((801, "Passar"));
        } else {
            v.push((800, if self.pending > 0 { "Comprar acúmulo" } else { "Comprar" }));
        }
        // Alguém ficou com uma carta sem dizer UNO: dá para pegar.
        if self.uno_open.is_some_and(|w| w != self.me()) {
            v.push((806, "Pegar sem UNO"));
        }
        if self.hands[self.me()].len() == 2 {
            v.push((802, if self.said_uno { "UNO dito" } else { "UNO!" }));
        }
        if self.hands[self.me()].len() > 14 {
            v.push((804, "Mais cartas"));
        }
        v
    }
}

impl MiniGame for Uno {
    fn at_menu(&self) -> bool {
        self.mode == Mode::Menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.mode == Mode::Menu {
            minigame::draw_menu(fb, f, "UNO", "Regras do UNO do Pedro (AED1): +2 e +4 acumulam.", &MENU);
            return;
        }
        let status = match self.phase {
            Phase::Over(p) => if self.name(p) == "Você" { "Você venceu!".to_string() } else { format!("{} venceu.", self.name(p)) },
            Phase::PickColor => "Escolha a cor".into(),
            Phase::Handoff => "Troca de jogador".to_string(),
            Phase::Turn => {
                let who = self.name(self.turn);
                if self.pending > 0 { format!("Vez de: {who} · acúmulo de +{}", self.pending) } else { format!("Vez de: {who}") }
            }
        };
        minigame::header(fb, f, "UNO", &status);
        self.draw_opponents(fb, f);
        // Mesa: monte (virado) e descarte com sombra de pilha.
        let deck = Self::deck_rect();
        for k in (1..3).rev() {
            Self::draw_back(fb, f, Rect::new(deck.x - k * 5, deck.y + k * 5, deck.w, deck.h));
        }
        Self::draw_back(fb, f, deck);
        let left = format!("Monte · {}", self.draw.len());
        let lw = f.measure(Face::Body, 26.0, &left);
        f.draw(fb, Face::Body, 26.0, deck.x + (deck.w - lw) / 2, deck.y + deck.h + 40, MUTED, &left);
        let top = Self::top_rect();
        for k in (1..3).rev() {
            fb.round_rect(Rect::new(top.x + k * 6, top.y + k * 4, top.w, top.h), top.w / 9, if k == 1 { 0xBB } else { 0xDD });
        }
        let shown_top = Card { kind: self.top().kind, color: if self.top().color == Color::Black { self.color } else { self.top().color } };
        Self::draw_card(fb, f, top, shown_top, false);
        // Cor da vez (importa depois de um coringa) e sentido do jogo.
        let chip_y = top.y + top.h + 22;
        let name = format!("Cor: {}", color_name(self.color));
        let cw = f.measure(Face::Strong, 28.0, &name);
        let chip = Rect::new(top.x + top.w / 2 - (cw + 70) / 2, chip_y, cw + 70, 46);
        fb.round_outline(chip, 23, 3, INK, PAPER);
        Self::color_mark(fb, chip.x + 30, chip.y + 23, 11, self.color, INK);
        f.draw(fb, Face::Strong, 28.0, chip.x + 52, chip.y + 33, INK, &name);
        if self.players() > 2 {
            let t = if self.dir > 0 { "sentido horário" } else { "sentido anti-horário" };
            f.draw(fb, Face::Body, 26.0, deck.x, deck.y - 16, MUTED, t);
        }
        if self.pending > 0 {
            let t = format!("+{} acumulado", self.pending);
            let w = f.measure(Face::Display, 40.0, &t);
            f.draw(fb, Face::Display, 40.0, top.x + top.w / 2 - w / 2, top.y - 18, INK, &t);
        }
        // Recado da última jogada.
        let msg = crate::apps::fit(f, Face::Body, 30.0, &self.msg, W - 2 * MARGIN);
        let mw = f.measure(Face::Body, 30.0, &msg);
        f.draw(fb, Face::Body, 30.0, (W - mw) / 2, 940, INK, &msg);
        if self.phase == Phase::PickColor {
            f.draw(fb, Face::Strong, 30.0, MARGIN, 980, MUTED, "ESCOLHA A COR");
            for (k, c) in COLORS.iter().enumerate() {
                let r = Self::pick_rect(k);
                Self::draw_color_card(fb, r, *c);
                let n = color_name(*c);
                let w = f.measure(Face::Strong, 28.0, n);
                f.draw(fb, Face::Strong, 28.0, r.x + (r.w - w) / 2, r.y + r.h + 36, INK, n);
            }
            return;
        }
        if self.phase == Phase::Handoff {
            // Troca de jogador: a mesa fica à vista, a mão só depois do Pronto.
            let who = format!("Vez do {}", self.name(self.turn));
            let w = f.measure(Face::Display, 64.0, &who);
            f.draw(fb, Face::Display, 64.0, (W - w) / 2, 1080, INK, &who);
            let n = self.hands[self.turn].len();
            let sub = format!("Passe o Kindle. Você tem {n} carta{}; elas aparecem no Pronto.", if n == 1 { "" } else { "s" });
            let sw = f.measure(Face::Body, 30.0, &sub);
            f.draw(fb, Face::Body, 30.0, (W - sw) / 2, 1140, MUTED, &sub);
            for i in 0..n.min(9) as i32 {
                Self::draw_back(fb, f, Rect::new(W / 2 - 150 + i * 30 - (n.min(9) as i32 - 1) * 15 + 150 - 33, 1170, 66, 98));
            }
            minigame::pill(fb, f, Self::btn(0, 1), "Pronto", true);
            return;
        }
        // Sua mão (contra a CPU, sempre a sua; local, a de quem joga).
        let show = !self.is_cpu(self.turn) || matches!(self.mode, Mode::Cpu(_));
        if show {
            let hand = &self.hands[self.me()];
            for (i, r, ok) in self.hand_layout() {
                let my_turn = self.turn == self.me() && self.phase == Phase::Turn;
                Self::draw_card(fb, f, r, hand[i], my_turn && !ok);
            }
        }
        let b = self.buttons();
        for (k, (_, l)) in b.iter().enumerate() {
            minigame::pill(fb, f, Self::btn(k, b.len()), l, *l == "UNO!");
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.mode == Mode::Menu {
            return (0..MENU.len()).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        match self.phase {
            Phase::Handoff => return vec![(805, Self::btn(0, 1))],
            Phase::PickColor => return (0..4).map(|k| (700 + k as u32, Self::pick_rect(k))).collect(),
            Phase::Over(_) => return vec![(803, Self::btn(0, 1))],
            Phase::Turn => {}
        }
        let b = self.buttons();
        let mut v: Vec<(u32, Rect)> = b.iter().enumerate().map(|(k, (id, _))| (*id, Self::btn(k, b.len()))).collect();
        if !self.is_cpu(self.turn) {
            // Cartas da direita por cima: procura primeiro a de cima.
            for (i, r, ok) in self.hand_hits().into_iter().rev() {
                if ok {
                    v.insert(0, (i as u32, r));
                }
            }
            if self.drew.is_none() {
                v.push((800, Self::deck_rect()));
            }
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 => self.start(Mode::Cpu(1)),
            901 => self.start(Mode::Cpu(3)),
            902 => self.start(Mode::Local(2)),
            903 => self.start(Mode::Local(4)),
            800 => self.draw_turn(),
            801 => {
                self.msg = format!("{} passou.", self.name(self.turn));
                self.end_turn(false);
            }
            802 => self.said_uno = true,
            803 => self.mode = Mode::Menu,
            804 => self.page = (self.page + 1) % self.hands[self.me()].len().div_ceil(14),
            805 => self.phase = Phase::Turn,
            806 => {
                if let Some(w) = self.uno_open {
                    self.catch(w);
                }
            }
            700..=703 => {
                self.color = COLORS[(id - 700) as usize];
                self.msg.push_str(&format!(" Cor: {}.", color_name(self.color)));
                self.phase = Phase::Turn;
                let c = self.top();
                self.after_play(c);
            }
            k if (k as usize) < self.hands.get(self.me()).map_or(0, |h| h.len()) => {
                // Antes da próxima jogada, a CPU pode pegar quem esqueceu o UNO.
                self.play(k as usize);
            }
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn cpu_turn(&self) -> bool {
        self.phase == Phase::Turn && self.is_cpu(self.turn)
    }

    /// CPU: pega quem esqueceu o UNO, acumula +2/+4 quando dá, guarda os pretos para o fim.
    fn cpu_move(&mut self) {
        let p = self.turn;
        if let Some(w) = self.uno_open.filter(|&w| w != p) {
            if self.rng.next() % 10 < 7 {
                self.catch(w);
            } else {
                self.uno_open = None;
            }
        }
        let hand = self.hands[p].clone();
        let mut opts: Vec<usize> = (0..hand.len()).filter(|&i| self.playable(hand[i])).collect();
        if let Some(d) = self.drew {
            opts.retain(|&i| i == d);
        }
        if opts.is_empty() {
            if self.drew.is_some() {
                self.end_turn(false);
            } else {
                self.draw_turn();
            }
            return;
        }
        // Prefere cores e números; pretos por último.
        opts.sort_by_key(|&i| (hand[i].color == Color::Black, matches!(hand[i].kind, Kind::Num(_))));
        let i = opts[0];
        self.play(i);
    }
}


#[cfg(test)]
mod render {
    use super::*;
    /// `VERSO_INK_SHOTS=pasta cargo test uno_em_imagem -- --ignored`
    #[test]
    #[ignore]
    fn uno_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        for (name, mode) in [("uno-1cpu", Mode::Cpu(1)), ("uno-3cpu", Mode::Cpu(3))] {
            let mut g = Uno::new();
            g.start(mode);
            g.hands[0] = vec![
                Card { kind: Kind::Num(7), color: Color::Red },
                Card { kind: Kind::Num(3), color: Color::Blue },
                Card { kind: Kind::Skip, color: Color::Green },
                Card { kind: Kind::Reverse, color: Color::Yellow },
                Card { kind: Kind::Draw2, color: Color::Red },
                Card { kind: Kind::Num(9), color: Color::Green },
                Card { kind: Kind::Wild, color: Color::Black },
                Card { kind: Kind::Draw4, color: Color::Black },
            ];
            g.discard.push(Card { kind: Kind::Num(7), color: Color::Blue });
            g.color = Color::Blue;
            g.turn = 0;
            let mut fb = crate::fb::Framebuffer::memory();
            g.draw(&mut fb, &f);
            fb.save_pgm(&out.join(format!("{name}.pgm")));
        }
        let mut g = Uno::new();
        g.start(Mode::Local(2));
        g.msg = "Jogador 1 jogou 7 azul.".into();
        let mut fb = crate::fb::Framebuffer::memory();
        g.draw(&mut fb, &f);
        fb.save_pgm(&out.join("uno-troca.pgm"));
        let mut g = Uno::new();
        g.start(Mode::Cpu(1));
        g.phase = Phase::PickColor;
        let mut fb = crate::fb::Framebuffer::memory();
        g.draw(&mut fb, &f);
        fb.save_pgm(&out.join("uno-cor.pgm"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn baralho_e_acumulo() {
        assert_eq!(deck().len(), 108);
        let mut g = Uno::new();
        g.start(Mode::Cpu(1));
        assert_eq!(g.hands[0].len(), 7);
        g.discard.push(Card { kind: Kind::Draw2, color: Color::Blue });
        g.color = Color::Blue;
        g.pending = 2;
        assert!(g.playable(Card { kind: Kind::Draw2, color: Color::Red }));
        assert!(g.playable(Card { kind: Kind::Draw4, color: Color::Black }));
        assert!(!g.playable(Card { kind: Kind::Num(3), color: Color::Blue }));
        g.discard.push(Card { kind: Kind::Draw4, color: Color::Black });
        g.color = Color::Green;
        assert!(g.playable(Card { kind: Kind::Draw2, color: Color::Green }));
        assert!(!g.playable(Card { kind: Kind::Draw2, color: Color::Red }));
    }
}
