//! Cripta: roguelike parado, de decisões. Cada andar é uma grade coberta; você descobre casas
//! vizinhas às já abertas. Monstros não se mexem: bloqueiam a passagem até você decidir lutar,
//! e a luta é calculada antes (você vê quanto vai perder). Explorar casas vazias recupera vida.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 6;
const CELL: i32 = 136;
const GX: i32 = (W - N as i32 * CELL) / 2;
const GY: i32 = 330;
const NAMES: [&str; 5] = ["Rato", "Morcego", "Esqueleto", "Ogro", "Dragão"];

fn name(k: u8) -> &'static str {
    NAMES[(k.clamp(1, 5) - 1) as usize]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Thing {
    Empty,
    Monster(u8),
    Potion,
    Sword,
    Gold,
    Stairs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Menu,
    Play,
    Dead,
}

pub struct Cripta {
    state: State,
    floor: u32,
    thing: Vec<Thing>,
    open: Vec<bool>,
    hp: i32,
    max_hp: i32,
    atk: i32,
    level: i32,
    xp: i32,
    gold: i32,
    potions: i32,
    selected: Option<usize>,
    note: String,
    best: u32,
    /// Onde o herói está (última casa livre aonde chegou) e passos (quadro da animação).
    hero: usize,
    steps: u32,
}

fn neighbors(i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = (i / N, i % N);
    [(r > 0).then(|| i - N), (r + 1 < N).then(|| i + N), (c > 0).then(|| i - 1), (c + 1 < N).then(|| i + 1)].into_iter().flatten()
}

fn best_file() -> std::path::PathBuf {
    crate::home_dir().join("data/cripta_recorde.txt")
}

/// Vida e força do monstro de nível `k` no andar `floor`.
fn monster_stats(k: u8, floor: u32) -> (i32, i32) {
    let k = k as i32;
    let f = floor as i32 - 1;
    (2 + 3 * k + f, k + f / 2)
}

impl Cripta {
    pub fn new() -> Self {
        let best = std::fs::read_to_string(best_file()).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        Cripta {
            state: State::Menu,
            floor: 1,
            thing: vec![Thing::Empty; N * N],
            open: vec![false; N * N],
            hp: 10,
            max_hp: 10,
            atk: 2,
            level: 1,
            xp: 0,
            gold: 0,
            potions: 1,
            selected: None,
            note: String::new(),
            best,
            hero: 0,
            steps: 0,
        }
    }

    fn start(&mut self) {
        let best = self.best;
        *self = Cripta::new();
        self.best = best;
        self.state = State::Play;
        self.build_floor();
    }

    /// Monta o andar: entrada aberta, escada longe dela, monstros, poções, espada e ouro.
    fn build_floor(&mut self) {
        let mut rng = Rng::new();
        let f = self.floor;
        loop {
            let mut thing = vec![Thing::Empty; N * N];
            let entry = (rng.next() % (N * N) as u64) as usize;
            let far: Vec<usize> = (0..N * N).filter(|&i| (i / N).abs_diff(entry / N) + (i % N).abs_diff(entry % N) >= 5).collect();
            if far.is_empty() {
                continue;
            }
            let stairs = far[(rng.next() % far.len() as u64) as usize];
            thing[stairs] = Thing::Stairs;
            let mut free: Vec<usize> = (0..N * N).filter(|&i| i != entry && i != stairs).collect();
            rng.shuffle(&mut free);
            let top = (f as u8 + 1).min(5);
            let monsters = 7 + f as usize;
            let mut it = free.into_iter();
            for k in 0..monsters {
                // Mais fracos são mais comuns; o mais forte do andar aparece uma vez.
                let lvl = if k == 0 { top } else { (1 + (rng.next() % top as u64) as u8).min(1 + (rng.next() % top as u64) as u8) };
                thing[it.next().unwrap()] = Thing::Monster(lvl);
            }
            for t in [Thing::Potion, Thing::Potion, Thing::Sword, Thing::Gold, Thing::Gold, Thing::Gold] {
                thing[it.next().unwrap()] = t;
            }
            // A escada precisa ser alcançável passando só por monstros vencíveis: sempre é,
            // porque monstros são casas que se abrem ao vencer. Basta não ficar ilhada (grade sem paredes).
            self.thing = thing;
            self.open = vec![false; N * N];
            self.open[entry] = true;
            self.hero = entry;
            self.selected = None;
            self.note = format!("Andar {f}. Toque numa casa vizinha para descobrir.");
            return;
        }
    }

    /// Casa aberta e livre (dá para passar por ela).
    fn passable(&self, i: usize) -> bool {
        self.open[i] && !matches!(self.thing[i], Thing::Monster(_))
    }

    fn reachable(&self, i: usize) -> bool {
        !self.open[i] && neighbors(i).any(|j| self.passable(j))
    }

    /// Quanto de vida a luta custa (o herói bate primeiro).
    fn fight_cost(&self, k: u8) -> i32 {
        let (mhp, matk) = monster_stats(k, self.floor);
        let rounds = (mhp + self.atk - 1) / self.atk;
        (rounds - 1) * matk
    }

    fn gain_xp(&mut self, k: u8) {
        self.xp += k as i32;
        while self.xp >= self.level * 5 {
            self.xp -= self.level * 5;
            self.level += 1;
            self.max_hp += 3;
            self.hp = self.max_hp;
            self.atk += 1;
            self.note = format!("Subiu para o nível {}! Vida cheia e força {}.", self.level, self.atk);
        }
    }

    fn reveal(&mut self, i: usize) {
        self.open[i] = true;
        self.steps += 1;
        if !matches!(self.thing[i], Thing::Monster(_)) {
            self.hero = i;
        }
        self.note = match self.thing[i] {
            Thing::Empty => {
                // Explorar descansa: casa vazia recupera 1 de vida.
                let before = self.hp;
                self.hp = (self.hp + 1).min(self.max_hp);
                if self.hp > before { "Corredor vazio. Você recupera 1 de vida.".into() } else { "Corredor vazio.".into() }
            }
            Thing::Monster(k) => {
                let (mhp, matk) = monster_stats(k, self.floor);
                format!("{} (vida {mhp}, força {matk}) bloqueia a passagem.", name(k))
            }
            Thing::Potion => {
                self.potions += 1;
                self.thing[i] = Thing::Empty;
                "Poção guardada.".into()
            }
            Thing::Sword => {
                self.atk += 1;
                self.thing[i] = Thing::Empty;
                format!("Espada melhor: força {}.", self.atk)
            }
            Thing::Gold => {
                let g = 5 + 5 * self.floor as i32;
                self.gold += g;
                self.thing[i] = Thing::Empty;
                format!("{g} de ouro.")
            }
            Thing::Stairs => "A escada! Toque nela para descer.".into(),
        };
    }

    fn fight(&mut self, i: usize, k: u8) {
        let cost = self.fight_cost(k);
        self.hp -= cost;
        if self.hp <= 0 {
            self.hp = 0;
            self.state = State::Dead;
            let score = self.score();
            if score > self.best {
                self.best = score;
                let _ = std::fs::write(best_file(), score.to_string());
            }
            self.note = format!("{} venceu. Fim da aventura.", name(k));
            return;
        }
        self.thing[i] = Thing::Empty;
        self.hero = i;
        self.steps += 1;
        self.note = format!("Venceu {} perdendo {cost} de vida.", name(k));
        self.gain_xp(k);
    }

    fn score(&self) -> u32 {
        (self.floor - 1) * 100 + self.gold as u32
    }

    fn cell(i: usize) -> Rect {
        Rect::new(GX + (i % N) as i32 * CELL, GY + (i / N) as i32 * CELL, CELL, CELL)
    }

    /// Bloco de pedra (casa fechada): tom de fundo e linhas das juntas.
    fn stone(fb: &mut Framebuffer, r: Rect, tone: u8, joint: u8) {
        fb.fill(r, tone);
        let rows = 4;
        for k in 0..rows {
            let y = r.y + k * r.h / rows;
            fb.fill(Rect::new(r.x, y, r.w, 3), joint);
            let off = if k % 2 == 0 { r.w / 3 } else { r.w * 3 / 4 };
            fb.fill(Rect::new(r.x + off, y, 3, r.h / rows), joint);
        }
    }

    /// Ícones do painel.
    fn heart(fb: &mut Framebuffer, x: i32, y: i32, s: i32, v: u8) {
        fb.round_rect(Rect::new(x, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        fb.round_rect(Rect::new(x + s / 2 - 2, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        for k in 0..s / 2 {
            fb.fill(Rect::new(x + k, y + s / 4 + k, s - 2 * k, 1), v);
        }
    }

    fn sprite_of(t: Thing) -> Option<&'static str> {
        match t {
            Thing::Monster(1) => Some("rato"),
            Thing::Monster(2) => Some("morcego"),
            Thing::Monster(3) => Some("esqueleto"),
            Thing::Monster(4) => Some("ogro"),
            Thing::Monster(_) => Some("dragao"),
            Thing::Potion => Some("pocao"),
            Thing::Sword => Some("espada"),
            Thing::Gold => Some("ouro"),
            Thing::Stairs => Some("escada"),
            Thing::Empty => None,
        }
    }

    fn draw_thing(fb: &mut Framebuffer, f: &Fonts, r: Rect, t: Thing) {
        let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
        match t {
            Thing::Empty => {}
            Thing::Monster(k) => {
                let b = Rect::new(r.x + 14, r.y + 14, r.w - 28, r.h - 28);
                fb.round_rect(b, 20, INK);
                let num = k.to_string();
                let label = name(k);
                let nw = f.measure(Face::Display, 56.0, &num);
                f.draw(fb, Face::Display, 56.0, b.x + (b.w - nw) / 2, b.y + 64, PAPER, &num);
                let size = if label.chars().count() > 7 { 20.0 } else { 22.0 };
                let tw = f.measure(Face::Strong, size, label);
                f.draw(fb, Face::Strong, size, b.x + (b.w - tw) / 2, b.y + b.h - 14, PAPER, label);
            }
            Thing::Potion => {
                fb.round_rect(Rect::new(cx as i32 - 26, cy as i32 - 10, 52, 50), 24, INK);
                fb.fill(Rect::new(cx as i32 - 9, cy as i32 - 36, 18, 30), INK);
                fb.fill(Rect::new(cx as i32 - 14, cy as i32 - 40, 28, 8), INK);
                fb.round_rect(Rect::new(cx as i32 - 14, cy as i32 + 2, 12, 12), 6, PAPER);
            }
            Thing::Sword => {
                ui::stroke(fb, (cx - 34.0, cy + 34.0), (cx + 36.0, cy - 36.0), 9.0, INK);
                ui::stroke(fb, (cx - 30.0, cy + 6.0), (cx - 6.0, cy + 30.0), 9.0, INK);
                ui::stroke(fb, (cx - 44.0, cy + 44.0), (cx - 32.0, cy + 32.0), 12.0, INK);
            }
            Thing::Gold => {
                for (dx, dy) in [(-16.0, 10.0), (16.0, 10.0), (0.0, -14.0)] {
                    let c = Rect::new((cx + dx) as i32 - 22, (cy + dy) as i32 - 22, 44, 44);
                    fb.round_outline(c, 22, 6, INK, 0xDD);
                }
            }
            Thing::Stairs => {
                for k in 0..4 {
                    let y = cy as i32 + 34 - k * 22;
                    fb.fill(Rect::new(cx as i32 - 44 + k * 22, y - 22, 88 - k * 22, 22), if k % 2 == 0 { INK } else { 0x66 });
                }
            }
        }
    }
}

impl MiniGame for Cripta {
    fn summary(&self) -> String {
        if self.state == State::Menu { String::new() } else { format!("Andar {} · vida {}/{} · força {}", self.floor, self.hp, self.max_hp, self.atk) }
    }

    fn context(&self) -> String {
        if self.state == State::Menu {
            return "No menu da Cripta.".into();
        }
        let grid: Vec<String> = (0..N)
            .map(|r| {
                (0..N)
                    .map(|c| {
                        let i = r * N + c;
                        if !self.open[i] {
                            return if self.reachable(i) { '?' } else { '#' };
                        }
                        match self.thing[i] {
                            Thing::Empty => '.',
                            Thing::Monster(k) => (b'0' + k) as char,
                            Thing::Potion => 'p',
                            Thing::Sword => 's',
                            Thing::Gold => '$',
                            Thing::Stairs => '>',
                        }
                    })
                    .collect()
            })
            .collect();
        format!(
            "Cripta, andar {}: vida {}/{}, força {}, nível {}, experiência {}/{}, ouro {}, poções {}. Monstro de nível k tem vida 2+3k+(andar-1) e força k+(andar-1)/2; o herói bate primeiro.\nMapa (# coberta, ? dá para descobrir, número = monstro, p poção, s espada, $ ouro, > escada):\n{}",
            self.floor, self.hp, self.max_hp, self.atk, self.level, self.xp, self.level * 5, self.gold, self.potions, grid.join("\n")
        )
    }

    fn at_menu(&self) -> bool {
        self.state == State::Menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.state == State::Menu {
            let sub = if self.best > 0 { format!("Desça o mais fundo que puder. Recorde: {} pontos.", self.best) } else { "Desça o mais fundo que puder. Nada se mexe sem você.".into() };
            minigame::draw_menu(fb, f, "Cripta", &sub, &["Nova descida"]);
            let rules = [
                "Toque numa casa vizinha às abertas para descobri-la.",
                "Monstros não andam: bloqueiam até você lutar.",
                "Toque no monstro para ver o custo; de novo para lutar.",
                "Casa vazia descoberta recupera 1 de vida.",
                "Vencer dá experiência; subir de nível enche a vida.",
            ];
            for (k, l) in rules.iter().enumerate() {
                f.draw(fb, Face::Body, 30.0, minigame::MARGIN, 700 + k as i32 * 56, MUTED, l);
            }
            return;
        }
        let status = format!("Andar {} · {} pontos", self.floor, self.score());
        minigame::header(fb, f, "Cripta", &status);
        let frame = if self.steps % 2 == 0 { "1" } else { "2" };
        for i in 0..N * N {
            let r = Self::cell(i);
            let inner = Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6);
            if !self.open[i] {
                if self.state == State::Play && self.reachable(i) {
                    // Dá para descobrir: pedra clara com contorno.
                    Self::stone(fb, inner, 0xC4, 0xA4);
                    fb.round_outline(inner, 6, 4, INK, 0xC4);
                    Self::stone(fb, Rect::new(inner.x + 4, inner.y + 4, inner.w - 8, inner.h - 8), 0xC4, 0xA4);
                } else {
                    Self::stone(fb, inner, 0x4A, 0x30);
                }
                continue;
            }
            // Piso de lajotas.
            fb.fill(inner, 0xF2);
            fb.fill(Rect::new(inner.x + inner.w / 2, inner.y, 2, inner.h), 0xDC);
            fb.fill(Rect::new(inner.x, inner.y + inner.h / 2, inner.w, 2), 0xDC);
            let t = self.thing[i];
            let art = Rect::new(r.x + 10, r.y + 10, r.w - 20, r.h - 20);
            let drawn = Self::sprite_of(t).is_some_and(|n| {
                let name = if matches!(t, Thing::Monster(_)) { format!("{n}-{frame}") } else { n.to_string() };
                crate::sprites::draw(fb, &name, art)
            });
            if !drawn {
                Self::draw_thing(fb, f, r, t);
            } else if let Thing::Monster(k) = t {
                // Selo com o nível do monstro.
                let b = Rect::new(r.x + r.w - 46, r.y + 6, 40, 40);
                fb.round_rect(b, 20, INK);
                let num = k.to_string();
                let w = f.measure(Face::Display, 28.0, &num);
                f.draw(fb, Face::Display, 28.0, b.x + (b.w - w) / 2, b.y + 30, PAPER, &num);
            }
            if i == self.hero && !matches!(t, Thing::Monster(_)) && !crate::sprites::draw(fb, &format!("heroi-{frame}"), art) {
                // Sem sprite: um peão simples marca o herói.
                let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                fb.round_rect(Rect::new(cx - 14, cy - 34, 28, 28), 14, INK);
                fb.round_rect(Rect::new(cx - 24, cy - 2, 48, 38), 16, INK);
            }
            if self.selected == Some(i) {
                fb.round_outline(Rect::new(r.x, r.y, r.w, r.h), 10, 7, INK, PAPER);
                fb.fill(Rect::new(r.x + 7, r.y + 7, r.w - 14, r.h - 14), 0xF2);
                Self::sprite_of(t).map(|n| crate::sprites::draw(fb, &format!("{n}-{frame}"), art)).filter(|&ok| ok).unwrap_or_else(|| {
                    Self::draw_thing(fb, f, r, t);
                    true
                });
            }
        }
        // Painel: vida, força, nível e ouro, com ícones.
        let y = GY + N as i32 * CELL + 18;
        Self::heart(fb, GX, y + 4, 34, INK);
        let life = format!("{}/{}", self.hp.max(0), self.max_hp);
        let end = f.draw(fb, Face::Display, 34.0, GX + 46, y + 34, INK, &life);
        let bar = Rect::new(end + 14, y + 16, 200, 14);
        fb.round_outline(bar, 7, 2, INK, PAPER);
        fb.round_rect(Rect::new(bar.x, bar.y, (bar.w * self.hp.max(0) / self.max_hp.max(1)).max(14), bar.h), 7, INK);
        let mut x = bar.x + bar.w + 30;
        ui::stroke(fb, (x as f32, (y + 36) as f32), ((x + 26) as f32, (y + 6) as f32), 5.0, INK);
        ui::stroke(fb, (x as f32 + 2.0, (y + 20) as f32), (x as f32 + 14.0, (y + 32) as f32), 5.0, INK);
        x = f.draw(fb, Face::Display, 34.0, x + 34, y + 34, INK, &self.atk.to_string()) + 30;
        let lv = format!("Nv {}", self.level);
        x = f.draw(fb, Face::Strong, 30.0, x, y + 32, INK, &lv) + 10;
        let xb = Rect::new(x, y + 20, 90, 8);
        fb.fill(xb, 0xDD);
        fb.fill(Rect::new(xb.x, xb.y, xb.w * self.xp / (self.level * 5).max(1), xb.h), 0x5C);
        let gold = format!("{}", self.gold);
        let gw = f.measure(Face::Display, 34.0, &gold);
        fb.round_outline(Rect::new(W - GX - gw - 44, y + 6, 32, 32), 16, 5, INK, 0xDD);
        f.draw(fb, Face::Display, 34.0, W - GX - gw, y + 34, INK, &gold);
        let note = crate::apps::fit(f, Face::Body, 28.0, &self.note, W - 2 * GX);
        f.draw(fb, Face::Body, 28.0, GX, y + 88, INK, &note);
        if self.state == State::Dead {
            let t = format!("Pontos: {} (andares × 100 + ouro)", self.score());
            f.draw(fb, Face::Strong, 30.0, GX, y + 130, MUTED, &t);
            minigame::pill(fb, f, minigame::bottom_button(0, 1), "Nova descida", true);
        } else {
            let p = format!("Beber poção ({})", self.potions);
            minigame::pill(fb, f, minigame::bottom_button(0, 2), &p, self.potions > 0 && self.hp < self.max_hp);
            minigame::pill(fb, f, minigame::bottom_button(1, 2), "Recomeçar", false);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.state {
            State::Menu => vec![(900, minigame::menu_row(0))],
            State::Dead => vec![(900, minigame::bottom_button(0, 1))],
            State::Play => {
                let mut v: Vec<(u32, Rect)> = (0..N * N).map(|i| (i as u32, Self::cell(i))).collect();
                v.push((800, minigame::bottom_button(0, 2)));
                v.push((801, minigame::bottom_button(1, 2)));
                v
            }
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let board = Rect::new(0, 170, W, 1448 - 170);
        match id {
            900 | 801 => {
                self.start();
                Redraw::Full
            }
            800 => {
                if self.potions > 0 && self.hp < self.max_hp {
                    self.potions -= 1;
                    let heal = 6 + self.floor as i32;
                    self.hp = (self.hp + heal).min(self.max_hp);
                    self.note = format!("Poção: +{heal} de vida.");
                } else if self.potions == 0 {
                    self.note = "Sem poções.".into();
                } else {
                    self.note = "Vida já está cheia.".into();
                }
                Redraw::Part(board)
            }
            i if (i as usize) < N * N && self.state == State::Play => {
                let i = i as usize;
                let prev = self.selected.take();
                if !self.open[i] {
                    if self.reachable(i) {
                        self.reveal(i);
                    } else {
                        self.note = "Longe demais: abra uma casa vizinha antes.".into();
                    }
                    return Redraw::Part(board);
                }
                match self.thing[i] {
                    Thing::Monster(k) => {
                        if !neighbors(i).any(|j| self.passable(j)) {
                            self.note = "Chegue até ele antes de lutar.".into();
                        } else if prev == Some(i) {
                            self.fight(i, k);
                        } else {
                            let cost = self.fight_cost(k);
                            self.selected = Some(i);
                            self.note = if cost >= self.hp {
                                format!("{}: custaria {cost} de vida. Você morreria. Toque de novo se quiser.", name(k))
                            } else {
                                format!("{}: custa {cost} de vida. Toque de novo para lutar.", name(k))
                            };
                        }
                    }
                    Thing::Stairs => {
                        self.floor += 1;
                        self.build_floor();
                        return Redraw::Full;
                    }
                    _ => {}
                }
                Redraw::Part(board)
            }
            _ => Redraw::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn andar_tem_escada_entrada_e_custo_de_luta() {
        let mut g = Cripta::new();
        g.start();
        assert_eq!(g.thing.iter().filter(|t| **t == Thing::Stairs).count(), 1);
        assert_eq!(g.open.iter().filter(|o| **o).count(), 1);
        // Herói nível 1 (força 2) contra rato no andar 1: vida 5 → 3 golpes → perde 2×1.
        assert_eq!(g.fight_cost(1), 2);
        // Toda casa vizinha da entrada é alcançável.
        let entry = g.open.iter().position(|o| *o).unwrap();
        assert!(neighbors(entry).all(|j| g.reachable(j)));
    }

    #[test]
    fn subir_de_nivel_enche_a_vida() {
        let mut g = Cripta::new();
        g.start();
        g.hp = 3;
        g.gain_xp(5);
        assert_eq!(g.level, 2);
        assert_eq!(g.hp, g.max_hp);
        assert_eq!(g.atk, 3);
    }
}

#[cfg(test)]
mod render {
    use super::*;
    #[test]
    #[ignore]
    fn cripta_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        let mut g = Cripta::new();
        g.start();
        for _ in 0..6 {
            if let Some(i) = (0..N * N).find(|&i| g.reachable(i)) {
                g.reveal(i);
            }
        }
        g.selected = (0..N * N).find(|&i| g.open[i] && matches!(g.thing[i], Thing::Monster(_)));
        let mut fb = crate::fb::Framebuffer::memory();
        g.draw(&mut fb, &f);
        fb.save_pgm(&out.join("cripta.pgm"));
    }
}
