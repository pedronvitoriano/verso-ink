//! Masmorra: roguelike por turnos em caracteres (estilo Rogue/Brogue). O mundo só se move quando
//! você dá um passo, então não há animação gastando bateria. Cinco andares; o dragão guarda o último.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MW: usize = 14;
const MH: usize = 13;
const CELL: i32 = 64;
const MX: i32 = (W - MW as i32 * CELL) / 2;
const MY: i32 = 262;
const SIGHT: i32 = 4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tile {
    Wall,
    Floor,
    Stairs,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Potion,
    Sword,
    Shield,
    Gold,
}

struct Monster {
    pos: usize,
    hp: i32,
    atk: i32,
    ch: char,
    name: &'static str,
}

pub struct Masmorra {
    menu: bool,
    map: Vec<Tile>,
    seen: Vec<bool>,
    items: Vec<(usize, Item)>,
    mons: Vec<Monster>,
    pos: usize,
    hp: i32,
    max_hp: i32,
    atk: i32,
    def: i32,
    gold: u32,
    depth: u32,
    log: String,
    dead: bool,
    won: bool,
    rng: Rng,
}

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

fn idx(r: i32, c: i32) -> Option<usize> {
    ((0..MH as i32).contains(&r) && (0..MW as i32).contains(&c)).then(|| r as usize * MW + c as usize)
}

impl Masmorra {
    pub fn new() -> Self {
        Masmorra {
            menu: true,
            map: vec![],
            seen: vec![],
            items: vec![],
            mons: vec![],
            pos: 0,
            hp: 12,
            max_hp: 12,
            atk: 2,
            def: 0,
            gold: 0,
            depth: 1,
            log: String::new(),
            dead: false,
            won: false,
            rng: Rng::new(),
        }
    }

    fn new_game(&mut self) {
        *self = Masmorra { menu: false, ..Masmorra::new() };
        self.level();
        self.log = "Você desce à masmorra. Ache a escada (>).".into();
    }

    /// Salas e corredores; jogador na primeira sala, escada na última.
    fn level(&mut self) {
        let rng = &mut self.rng;
        let mut map = vec![Tile::Wall; MW * MH];
        let mut rooms: Vec<(i32, i32, i32, i32)> = Vec::new();
        for _ in 0..60 {
            if rooms.len() >= 6 {
                break;
            }
            let (h, w) = (2 + (rng.next() % 3) as i32, 3 + (rng.next() % 3) as i32);
            let (r, c) = (1 + (rng.next() % (MH as u64 - h as u64 - 1)) as i32, 1 + (rng.next() % (MW as u64 - w as u64 - 1)) as i32);
            if rooms.iter().any(|&(rr, cc, hh, ww)| r <= rr + hh && rr <= r + h && c <= cc + ww && cc <= c + w) {
                continue;
            }
            rooms.push((r, c, h, w));
        }
        for &(r, c, h, w) in &rooms {
            for y in r..r + h {
                for x in c..c + w {
                    map[y as usize * MW + x as usize] = Tile::Floor;
                }
            }
        }
        let center = |k: usize| (rooms[k].0 + rooms[k].2 / 2, rooms[k].1 + rooms[k].3 / 2);
        for k in 1..rooms.len() {
            let (a, b) = (center(k - 1), center(k));
            for x in a.1.min(b.1)..=a.1.max(b.1) {
                map[a.0 as usize * MW + x as usize] = Tile::Floor;
            }
            for y in a.0.min(b.0)..=a.0.max(b.0) {
                map[y as usize * MW + b.1 as usize] = Tile::Floor;
            }
        }
        let (sr, sc) = center(0);
        let (er, ec) = center(rooms.len() - 1);
        self.pos = sr as usize * MW + sc as usize;
        map[er as usize * MW + ec as usize] = Tile::Stairs;
        self.map = map;
        self.seen = vec![false; MW * MH];
        self.items.clear();
        self.mons.clear();
        let floors: Vec<usize> = (0..MW * MH).filter(|&i| self.map[i] == Tile::Floor && i != self.pos).collect();
        let pick = |rng: &mut Rng| floors[(rng.next() % floors.len() as u64) as usize];
        for k in 0..4 {
            let it = [Item::Potion, Item::Gold, Item::Potion, Item::Gold][k];
            let p = pick(&mut self.rng);
            self.items.push((p, it));
        }
        if self.depth == 2 {
            let p = pick(&mut self.rng);
            self.items.push((p, Item::Sword));
        }
        if self.depth == 3 {
            let p = pick(&mut self.rng);
            self.items.push((p, Item::Shield));
        }
        let kinds: [(char, &str, i32, i32); 3] = [('r', "rato", 3, 1), ('g', "goblin", 6, 2), ('o', "orc", 10, 3)];
        let n = 2 + self.depth as usize;
        for _ in 0..n {
            let p = pick(&mut self.rng);
            let tier = ((self.rng.next() % (self.depth as u64)) as usize).min(2);
            let (ch, name, hp, atk) = kinds[tier];
            self.mons.push(Monster { pos: p, hp, atk, ch, name });
        }
        if self.depth == 5 {
            let stairs = er as usize * MW + ec as usize;
            let near = (0..MW * MH).find(|&i| self.map[i] == Tile::Floor && Self::dist(i, stairs) == 1).unwrap_or(stairs);
            self.mons.push(Monster { pos: near, hp: 24, atk: 5, ch: 'D', name: "dragão" });
        }
        self.look();
    }

    fn dist(a: usize, b: usize) -> i32 {
        ((a / MW) as i32 - (b / MW) as i32).abs().max(((a % MW) as i32 - (b % MW) as i32).abs())
    }

    fn visible(&self, i: usize) -> bool {
        Self::dist(i, self.pos) <= SIGHT
    }

    fn look(&mut self) {
        for i in 0..MW * MH {
            if self.visible(i) {
                self.seen[i] = true;
            }
        }
    }

    fn step(&mut self, d: (i32, i32)) {
        let (r, c) = ((self.pos / MW) as i32 + d.0, (self.pos % MW) as i32 + d.1);
        let Some(t) = idx(r, c) else { return };
        if self.map[t] == Tile::Wall {
            return;
        }
        if let Some(k) = self.mons.iter().position(|m| m.pos == t) {
            // Ataque: dano com um pouco de sorte.
            let dmg = self.atk + (self.rng.next() % 2) as i32;
            self.mons[k].hp -= dmg;
            if self.mons[k].hp <= 0 {
                let m = self.mons.remove(k);
                self.log = format!("Você derrotou o {}.", m.name);
                if m.ch == 'D' {
                    self.won = true;
                    self.log = format!("Você derrotou o dragão com {} de ouro!", self.gold);
                    return;
                }
            } else {
                self.log = format!("Você acerta o {} ({dmg}).", self.mons[k].name);
            }
        } else {
            self.pos = t;
            if let Some(k) = self.items.iter().position(|&(p, _)| p == t) {
                let (_, it) = self.items.remove(k);
                self.log = match it {
                    Item::Potion => {
                        self.hp = (self.hp + 6).min(self.max_hp);
                        "Poção: +6 de vida.".into()
                    }
                    Item::Gold => {
                        let g = 5 + (self.rng.next() % 10) as u32;
                        self.gold += g;
                        format!("{g} de ouro.")
                    }
                    Item::Sword => {
                        self.atk += 2;
                        "Espada: +2 de ataque.".into()
                    }
                    Item::Shield => {
                        self.def += 1;
                        "Escudo: +1 de defesa.".into()
                    }
                };
            }
            if self.map[t] == Tile::Stairs {
                if self.depth == 5 && self.mons.iter().any(|m| m.ch == 'D') {
                    self.log = "O dragão guarda a saída. Derrote-o!".into();
                } else if self.depth < 5 {
                    self.depth += 1;
                    self.max_hp += 2;
                    self.hp = (self.hp + 3).min(self.max_hp);
                    self.level();
                    self.log = format!("Andar {}. Mais fundo, mais perigoso.", self.depth);
                    return;
                }
            }
        }
        self.monsters_act();
        self.look();
    }

    /// Monstros que te veem chegam perto e atacam quando estão do lado.
    fn monsters_act(&mut self) {
        for k in 0..self.mons.len() {
            let m = &self.mons[k];
            let d = Self::dist(m.pos, self.pos);
            if d == 1 {
                let dmg = (m.atk - self.def).max(1);
                self.hp -= dmg;
                self.log.push_str(&format!(" O {} te acerta ({dmg}).", m.name));
                if self.hp <= 0 {
                    self.dead = true;
                    self.log = format!("O {} venceu você no andar {}.", m.name, self.depth);
                    return;
                }
            } else if d <= SIGHT + 1 {
                let (mr, mc) = ((m.pos / MW) as i32, (m.pos % MW) as i32);
                let (pr, pc) = ((self.pos / MW) as i32, (self.pos % MW) as i32);
                let opts = [((pr - mr).signum(), 0), (0, (pc - mc).signum())];
                for (dr, dc) in opts {
                    if (dr, dc) == (0, 0) {
                        continue;
                    }
                    if let Some(t) = idx(mr + dr, mc + dc) {
                        if self.map[t] != Tile::Wall && t != self.pos && !self.mons.iter().any(|o| o.pos == t) {
                            self.mons[k].pos = t;
                            break;
                        }
                    }
                }
            }
        }
    }

    fn arrow_rect(k: usize) -> Rect {
        let (cx, cy, s) = (W / 2 + 150, 1236, 100);
        match k {
            0 => Rect::new(cx - s / 2, cy - s - 4, s, s),
            1 => Rect::new(cx - s / 2, cy + 4, s, s),
            2 => Rect::new(cx - s / 2 - s - 10, cy - s / 2, s, s),
            _ => Rect::new(cx + s / 2 + 10, cy - s / 2, s, s),
        }
    }
}

impl MiniGame for Masmorra {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Masmorra", "Cinco andares, um dragão. O mundo anda quando você anda.", &["Nova aventura"]);
            return;
        }
        let status = format!("Andar {} · vida {}/{} · ataque {} · ouro {}", self.depth, self.hp.max(0), self.max_hp, self.atk, self.gold);
        minigame::header(fb, f, "Masmorra", &status);
        for i in 0..MW * MH {
            let r = Rect::new(MX + (i % MW) as i32 * CELL, MY + (i / MW) as i32 * CELL, CELL, CELL);
            if !self.seen[i] {
                continue;
            }
            let vis = self.visible(i);
            match self.map[i] {
                Tile::Wall => fb.fill(r, if vis { 0x88 } else { 0xC8 }),
                Tile::Floor => {
                    fb.fill(r, if vis { PAPER } else { 0xEE });
                    fb.fill(Rect::new(r.x + CELL / 2 - 3, r.y + CELL / 2 - 3, 6, 6), 0xBB);
                }
                Tile::Stairs => {
                    let t = ">";
                    let tw = f.measure(Face::Display, 50.0, t);
                    f.draw(fb, Face::Display, 50.0, r.x + (CELL - tw) / 2, r.y + 48, INK, t);
                }
            }
        }
        let glyph = |fb: &mut Framebuffer, i: usize, ch: &str, ink: u8| {
            let r = Rect::new(MX + (i % MW) as i32 * CELL, MY + (i / MW) as i32 * CELL, CELL, CELL);
            fb.fill(r, PAPER);
            let tw = f.measure(Face::Display, 50.0, ch);
            f.draw(fb, Face::Display, 50.0, r.x + (CELL - tw) / 2, r.y + 48, ink, ch);
        };
        for &(p, it) in &self.items {
            if self.visible(p) {
                let ch = match it {
                    Item::Potion => "!",
                    Item::Sword => "/",
                    Item::Shield => "[",
                    Item::Gold => "$",
                };
                glyph(fb, p, ch, MUTED);
            }
        }
        for m in &self.mons {
            if self.visible(m.pos) {
                glyph(fb, m.pos, &m.ch.to_string(), INK);
            }
        }
        glyph(fb, self.pos, "@", INK);
        // Moldura fina do mapa (quatro traços, sem pintar o miolo).
        let (bw, bh) = (MW as i32 * CELL, MH as i32 * CELL);
        for r in [Rect::new(MX - 4, MY - 4, bw + 8, 3), Rect::new(MX - 4, MY + bh + 1, bw + 8, 3), Rect::new(MX - 4, MY - 4, 3, bh + 8), Rect::new(MX + bw + 1, MY - 4, 3, bh + 8)] {
            fb.fill(r, INK);
        }
        for (k, l) in wrap(f, &self.log, 560).iter().take(3).enumerate() {
            f.draw(fb, Face::Body, 28.0, MARGIN - 20, 1150 + k as i32 * 40, INK, l);
        }
        if self.dead || self.won {
            minigame::pill(fb, f, Rect::new(MARGIN - 20, 1300, 420, 100), "Nova aventura", true);
            return;
        }
        for k in 0..4 {
            let r = Self::arrow_rect(k);
            fb.round_outline(r, 22, 4, INK, PAPER);
            let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
            let (dx, dy) = (DIRS[k].1 as f32, DIRS[k].0 as f32);
            let tip = (cx + dx * 24.0, cy + dy * 24.0);
            ui::stroke(fb, (cx - dx * 24.0, cy - dy * 24.0), tip, 7.0, INK);
            ui::stroke(fb, tip, (tip.0 - dx * 16.0 - dy * 16.0, tip.1 - dy * 16.0 - dx * 16.0), 7.0, INK);
            ui::stroke(fb, tip, (tip.0 - dx * 16.0 + dy * 16.0, tip.1 - dy * 16.0 + dx * 16.0), 7.0, INK);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu {
            return vec![(900, minigame::menu_row(0))];
        }
        if self.dead || self.won {
            return vec![(900, Rect::new(MARGIN - 20, 1300, 420, 100))];
        }
        (0..4).map(|k| (810 + k as u32, Self::arrow_rect(k))).collect()
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 => {
                self.new_game();
                Redraw::Full
            }
            810..=813 => {
                let before = self.depth;
                self.step(DIRS[(id - 810) as usize]);
                if self.depth != before || self.dead || self.won { Redraw::Full } else { Redraw::Part(Rect::new(0, 170, W, 1448 - 170)) }
            }
            _ => Redraw::None,
        }
    }
}

fn wrap(f: &Fonts, text: &str, max_w: i32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for w in text.split_whitespace() {
        let cur = lines.last().unwrap().clone();
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if f.measure(Face::Body, 28.0, &cand) > max_w && !cur.is_empty() {
            lines.push(w.to_string());
        } else {
            *lines.last_mut().unwrap() = cand;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn andar_tem_escada_alcancavel() {
        let mut g = Masmorra::new();
        g.new_game();
        let stairs = (0..MW * MH).find(|&i| g.map[i] == Tile::Stairs).unwrap();
        let mut seen = vec![false; MW * MH];
        let mut stack = vec![g.pos];
        while let Some(i) = stack.pop() {
            if seen[i] || g.map[i] == Tile::Wall {
                continue;
            }
            seen[i] = true;
            for d in DIRS {
                if let Some(j) = idx((i / MW) as i32 + d.0, (i % MW) as i32 + d.1) {
                    stack.push(j);
                }
            }
        }
        assert!(seen[stairs]);
    }
}
