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
        Kind::Skip => "BLOQ".into(),
        Kind::Reverse => "INV".into(),
        Kind::Draw2 => "+2".into(),
        Kind::Draw4 => "+4".into(),
        Kind::Wild => "COR".into(),
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

    fn card_rect(k: usize) -> Rect {
        let (w, h, gap) = (122, 178, 10);
        let col = (k % 7) as i32;
        let row = (k / 7 % 2) as i32;
        Rect::new(MARGIN - 20 + col * (w + gap), 860 + row * (h + 14), w, h)
    }

    fn draw_card(fb: &mut Framebuffer, f: &Fonts, r: Rect, c: Card, big: bool, dim: bool) {
        let rad = 16;
        fb.round_rect(r, rad, INK);
        let inner = Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8);
        fb.round_rect(inner, rad - 4, PAPER);
        // Padrão da cor.
        let fg = match c.color {
            Color::Red => {
                fb.round_rect(inner, rad - 4, INK);
                PAPER
            }
            Color::Black => {
                fb.round_rect(inner, rad - 4, 0x55);
                PAPER
            }
            Color::Blue => {
                for y in (inner.y..inner.y + inner.h).step_by(14) {
                    fb.fill(Rect::new(inner.x + 6, y, inner.w - 12, 5), 0x88);
                }
                INK
            }
            Color::Green => {
                for y in (inner.y + 8..inner.y + inner.h - 4).step_by(16) {
                    for x in (inner.x + 8..inner.x + inner.w - 4).step_by(16) {
                        fb.round_rect(Rect::new(x, y, 6, 6), 3, 0x88);
                    }
                }
                INK
            }
            Color::Yellow => {
                fb.round_outline(Rect::new(inner.x + 8, inner.y + 8, inner.w - 16, inner.h - 16), 10, 6, INK, PAPER);
                INK
            }
        };
        // Selo branco no meio para o símbolo ficar legível em qualquer padrão.
        let seal = Rect::new(r.x + r.w / 2 - r.w * 36 / 100, r.y + r.h / 2 - r.w * 30 / 100, r.w * 72 / 100, r.w * 60 / 100);
        if c.color != Color::Red && c.color != Color::Black {
            fb.round_rect(seal, 12, PAPER);
        }
        let t = label(c.kind);
        let size = if big { 96.0 } else if t.len() > 2 { 28.0 } else { 56.0 };
        let tw = f.measure(Face::Display, size, &t);
        f.draw(fb, Face::Display, size, r.x + (r.w - tw) / 2, r.y + r.h / 2 + (size * 0.36) as i32, fg, &t);
        let name = color_name(c.color);
        let ns = if big { 30.0 } else { 18.0 };
        let nw = f.measure(Face::Strong, ns, name);
        f.draw(fb, Face::Strong, ns, r.x + (r.w - nw) / 2, r.y + r.h - 16, fg, name);
        if dim {
            // Cartas que não servem agora ficam esmaecidas.
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    fb.blend(x, y, PAPER, 170);
                }
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
            Phase::Handoff => format!("Vez do {}", self.name(self.turn)),
            Phase::Turn => {
                let who = self.name(self.turn);
                if self.pending > 0 { format!("Vez de: {who} · acúmulo de +{}", self.pending) } else { format!("Vez de: {who}") }
            }
        };
        minigame::header(fb, f, "UNO", &status);
        if self.phase == Phase::Handoff {
            f.draw(fb, Face::Body, 38.0, MARGIN, 640, INK, "Passe o Kindle e toque em Pronto.");
            f.draw(fb, Face::Body, 32.0, MARGIN, 700, MUTED, &self.msg);
            minigame::pill(fb, f, Self::btn(0, 1), "Pronto", true);
            return;
        }
        // Mesa: descarte grande, cor atual e quantas cartas cada um tem.
        let top = Rect::new(MARGIN, 290, 260, 380);
        Self::draw_card(fb, f, top, Card { kind: self.top().kind, color: if self.top().color == Color::Black { self.color } else { self.top().color } }, true, false);
        let x = MARGIN + 300;
        f.draw(fb, Face::Strong, 28.0, x, 330, MUTED, "COR DA VEZ");
        f.draw(fb, Face::Display, 56.0, x, 395, INK, color_name(self.color));
        for p in 0..self.players() {
            let line = format!("{}: {} {}", self.name(p), self.hands[p].len(), if self.hands[p].len() == 1 { "carta" } else { "cartas" });
            let mark = if p == self.turn { "• " } else { "" };
            f.draw(fb, if p == self.turn { Face::Strong } else { Face::Body }, 32.0, x, 470 + p as i32 * 48, INK, &format!("{mark}{line}"));
        }
        for (k, l) in wrap2(f, &self.msg, W - 2 * MARGIN).iter().enumerate() {
            f.draw(fb, Face::Body, 30.0, MARGIN, 740 + k as i32 * 42, INK, l);
        }
        if self.phase == Phase::PickColor {
            for (k, c) in COLORS.iter().enumerate() {
                Self::draw_card(fb, f, Self::card_rect(k), Card { kind: Kind::Wild, color: *c }, false, false);
            }
            return;
        }
        // Mão de quem joga (contra a CPU, sempre a sua).
        let me = self.me();
        let show = !self.is_cpu(self.turn) || matches!(self.mode, Mode::Cpu(_));
        if show {
            for (k, c) in self.hands[me].iter().enumerate().skip(self.page * 14).take(14) {
                let my_turn = self.turn == me && self.phase == Phase::Turn;
                let ok = my_turn && self.playable(*c) && self.drew.is_none_or(|d| d == k);
                Self::draw_card(fb, f, Self::card_rect(k - self.page * 14), *c, false, my_turn && !ok);
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
            Phase::PickColor => return (0..4).map(|k| (700 + k as u32, Self::card_rect(k))).collect(),
            Phase::Over(_) => return vec![(803, Self::btn(0, 1))],
            Phase::Turn => {}
        }
        let b = self.buttons();
        let mut v: Vec<(u32, Rect)> = b.iter().enumerate().map(|(k, (id, _))| (*id, Self::btn(k, b.len()))).collect();
        if !self.is_cpu(self.turn) {
            let me = self.me();
            for (k, c) in self.hands[me].iter().enumerate().skip(self.page * 14).take(14) {
                if self.playable(*c) && self.drew.is_none_or(|d| d == k) {
                    v.push((k as u32, Self::card_rect(k - self.page * 14)));
                }
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

fn wrap2(f: &Fonts, text: &str, max_w: i32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for w in text.split_whitespace() {
        let cur = lines.last().unwrap().clone();
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if f.measure(Face::Body, 30.0, &cand) > max_w && !cur.is_empty() {
            lines.push(w.to_string());
        } else {
            *lines.last_mut().unwrap() = cand;
        }
    }
    lines.truncate(2);
    lines
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
