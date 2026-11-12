//! Masmorra: roguelike por turnos (estilo Rogue/Brogue). O mundo só se move quando você dá um
//! passo, então não há animação gastando bateria. Cinco andares; o dragão guarda o último.
//!
//! Princípios (os mesmos da Cripta): nada de sorte no dano, monstros com vida e força à vista,
//! cada tipo com um traço que muda a tática, monstros dormindo ou acordados (dá para escolher a
//! briga), explorar cura, poção guardada para a hora certa e escolha ao subir de nível.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MW: usize = 14;
const MH: usize = 13;
/// Janela da câmera: o mapa é maior que a tela; ela segue o herói.
const VW: usize = 11;
const VH: usize = 9;
const CELL: i32 = 88;
const MX: i32 = (W - VW as i32 * CELL) / 2;
const MY: i32 = 262;
const SIGHT: i32 = 4;
/// Distância em que um monstro dormindo acorda.
const WAKE: i32 = 2;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Rato,
    Morcego,
    Goblin,
    Esqueleto,
    Orc,
    Ogro,
    Dragao,
}

impl Kind {
    fn name(self) -> &'static str {
        ["rato", "morcego", "goblin", "esqueleto", "orc", "ogro", "dragão"][self as usize]
    }
    fn sprite(self) -> &'static str {
        ["rato", "morcego", "goblin", "esqueleto", "orc", "ogro", "dragao"][self as usize]
    }
    /// (vida, força, armadura, experiência)
    fn base(self) -> (i32, i32, i32, i32) {
        match self {
            Kind::Rato => (3, 1, 0, 1),
            Kind::Morcego => (3, 2, 0, 2),
            Kind::Goblin => (6, 2, 0, 2),
            Kind::Esqueleto => (7, 3, 1, 3),
            Kind::Orc => (11, 4, 0, 4),
            Kind::Ogro => (16, 6, 0, 5),
            Kind::Dragao => (24, 6, 1, 0),
        }
    }
    fn trait_text(self) -> &'static str {
        match self {
            Kind::Rato => "fraco",
            Kind::Morcego => "rápido: anda duas casas",
            Kind::Goblin => "rouba ouro quando acerta",
            Kind::Esqueleto => "armadura: −1 no seu golpe",
            Kind::Orc => "forte",
            Kind::Ogro => "lento: age um turno sim, outro não",
            Kind::Dragao => "guarda a saída",
        }
    }
}

#[derive(Clone)]
struct Monster {
    kind: Kind,
    pos: usize,
    hp: i32,
    atk: i32,
    awake: bool,
}

impl Monster {
    fn new(kind: Kind, pos: usize, depth: u32) -> Monster {
        let (hp, atk, _, _) = kind.base();
        let hp = hp + (depth as i32 - 1);
        Monster { kind, pos, hp, atk, awake: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Menu,
    Play,
    /// Subiu de nível: escolhendo o bônus.
    LevelUp,
    Over,
}

pub struct Masmorra {
    state: State,
    daily: bool,
    map: Vec<Tile>,
    seen: Vec<bool>,
    items: Vec<(usize, Item)>,
    mons: Vec<Monster>,
    pos: usize,
    hp: i32,
    max_hp: i32,
    atk: i32,
    def: i32,
    level: i32,
    xp: i32,
    potions: i32,
    gold: u32,
    depth: u32,
    log: String,
    won: bool,
    rng: Rng,
    /// Passos dados (alterna o quadro da animação dos sprites e o ritmo do ogro).
    steps: u32,
    best: u32,
}

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

fn idx(r: i32, c: i32) -> Option<usize> {
    ((0..MH as i32).contains(&r) && (0..MW as i32).contains(&c)).then(|| r as usize * MW + c as usize)
}

fn best_file() -> std::path::PathBuf {
    crate::home_dir().join("data/masmorra_recorde.txt")
}

impl Masmorra {
    pub fn new() -> Self {
        let best = std::fs::read_to_string(best_file()).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        Masmorra {
            state: State::Menu,
            daily: false,
            map: vec![],
            seen: vec![],
            items: vec![],
            mons: vec![],
            pos: 0,
            hp: 18,
            max_hp: 18,
            atk: 3,
            def: 0,
            level: 1,
            xp: 0,
            potions: 1,
            gold: 0,
            depth: 1,
            log: String::new(),
            won: false,
            rng: Rng::new(),
            steps: 0,
            best,
        }
    }

    fn new_game(&mut self, daily: bool, rng: Rng) {
        let best = self.best;
        *self = Masmorra { state: State::Play, daily, rng, best, ..Masmorra::new() };
        self.level();
        self.log = "Você desce à masmorra. Ache a escada; os monstros com z estão dormindo.".into();
    }

    fn score(&self) -> u32 {
        (self.depth - 1) * 100 + self.gold + if self.won { 500 } else { 0 }
    }

    fn xp_next(&self) -> i32 {
        self.level * 3
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
        let mut floors: Vec<usize> = (0..MW * MH).filter(|&i| self.map[i] == Tile::Floor && Self::dist(i, self.pos) > 2).collect();
        self.rng.shuffle(&mut floors);
        let mut spots = floors.into_iter();
        let d = self.depth;
        let mut items = vec![Item::Potion, Item::Gold, Item::Gold];
        if d % 2 == 0 {
            items.push(Item::Potion);
        }
        if d == 2 || d == 4 {
            items.push(Item::Sword);
        }
        if d == 3 {
            items.push(Item::Shield);
        }
        for it in items {
            if let Some(p) = spots.next() {
                self.items.push((p, it));
            }
        }
        // Monstros do andar: os mais fundos trazem tipos novos.
        let pool: &[Kind] = match d {
            1 => &[Kind::Rato, Kind::Rato, Kind::Morcego],
            2 => &[Kind::Rato, Kind::Morcego, Kind::Goblin],
            3 => &[Kind::Morcego, Kind::Goblin, Kind::Esqueleto],
            4 => &[Kind::Goblin, Kind::Esqueleto, Kind::Orc],
            _ => &[Kind::Esqueleto, Kind::Orc, Kind::Ogro],
        };
        for _ in 0..3 + d as usize {
            let k = pool[(self.rng.next() % pool.len() as u64) as usize];
            if let Some(p) = spots.next() {
                self.mons.push(Monster::new(k, p, d));
            }
        }
        if d == 5 {
            let stairs = er as usize * MW + ec as usize;
            let near = (0..MW * MH).find(|&i| self.map[i] == Tile::Floor && Self::dist(i, stairs) == 1 && !self.mons.iter().any(|m| m.pos == i)).unwrap_or(stairs);
            self.mons.push(Monster::new(Kind::Dragao, near, d));
        }
        self.look();
    }

    fn dist(a: usize, b: usize) -> i32 {
        ((a / MW) as i32 - (b / MW) as i32).abs().max(((a % MW) as i32 - (b % MW) as i32).abs())
    }

    fn visible(&self, i: usize) -> bool {
        Self::dist(i, self.pos) <= SIGHT
    }

    /// Marca o que está à vista. Ver chão novo cura 1 (explorar é o fôlego da masmorra).
    fn look(&mut self) -> bool {
        let mut new = false;
        for i in 0..MW * MH {
            if self.visible(i) && !self.seen[i] {
                self.seen[i] = true;
                new |= self.map[i] != Tile::Wall;
            }
        }
        new
    }

    fn end(&mut self, won: bool) {
        self.won = won;
        self.state = State::Over;
        if !self.daily && self.score() > self.best {
            self.best = self.score();
            let _ = std::fs::write(best_file(), self.best.to_string());
        }
    }

    fn drink(&mut self) {
        if self.potions == 0 || self.hp >= self.max_hp || self.state != State::Play {
            return;
        }
        self.potions -= 1;
        let heal = self.max_hp / 2;
        self.hp = (self.hp + heal).min(self.max_hp);
        self.log = format!("Poção: +{heal} de vida.");
        self.steps += 1;
        self.monsters_act();
    }

    fn step(&mut self, d: (i32, i32)) {
        if self.state != State::Play {
            return;
        }
        let (r, c) = ((self.pos / MW) as i32 + d.0, (self.pos % MW) as i32 + d.1);
        let Some(t) = idx(r, c) else { return };
        if self.map[t] == Tile::Wall {
            return;
        }
        self.steps += 1;
        if let Some(k) = self.mons.iter().position(|m| m.pos == t) {
            // Ataque sem sorte: força menos a armadura do monstro.
            let (_, _, armor, xp) = self.mons[k].kind.base();
            let dmg = (self.atk - armor).max(1);
            self.mons[k].hp -= dmg;
            self.mons[k].awake = true;
            if self.mons[k].hp <= 0 {
                let m = self.mons.remove(k);
                self.log = format!("Você derrotou o {}.", m.kind.name());
                if m.kind == Kind::Dragao {
                    self.log = format!("Você derrotou o dragão com {} de ouro!", self.gold);
                    return self.end(true);
                }
                self.xp += xp;
                if self.xp >= self.xp_next() {
                    self.xp -= self.xp_next();
                    self.level += 1;
                    self.state = State::LevelUp;
                    self.log = format!("Nível {}! Escolha o que melhorar.", self.level);
                    return;
                }
            } else {
                let m = &self.mons[k];
                self.log = format!("Você acerta o {} ({dmg}); ele fica com {}.", m.kind.name(), m.hp);
            }
        } else {
            self.pos = t;
            if let Some(k) = self.items.iter().position(|&(p, _)| p == t) {
                let (_, it) = self.items.remove(k);
                self.log = match it {
                    Item::Potion => {
                        self.potions += 1;
                        "Pegou uma poção (use quando precisar).".into()
                    }
                    Item::Gold => {
                        let g = 10 + 5 * self.depth;
                        self.gold += g;
                        format!("{g} de ouro.")
                    }
                    Item::Sword => {
                        self.atk += 1;
                        format!("Espada: força {}.", self.atk)
                    }
                    Item::Shield => {
                        self.def += 1;
                        "Escudo: cada golpe que você leva tira 1 a menos.".into()
                    }
                };
            } else {
                self.log.clear();
            }
            if self.map[t] == Tile::Stairs {
                if self.depth == 5 && self.mons.iter().any(|m| m.kind == Kind::Dragao) {
                    self.log = "O dragão guarda a saída. Derrote-o!".into();
                } else if self.depth < 5 {
                    self.depth += 1;
                    self.level();
                    self.log = format!("Andar {}. Mais fundo, mais perigoso.", self.depth);
                    return;
                } else {
                    return self.end(true);
                }
            }
            if self.look() && self.hp < self.max_hp {
                self.hp += 1;
            }
        }
        self.monsters_act();
    }

    /// Monstros acordados chegam perto e atacam do lado; os dormindo acordam quando você chega
    /// perto. O morcego anda duas casas; o ogro age um turno sim, outro não.
    fn monsters_act(&mut self) {
        for k in 0..self.mons.len() {
            let d = Self::dist(self.mons[k].pos, self.pos);
            if !self.mons[k].awake {
                if d <= WAKE {
                    self.mons[k].awake = true;
                    let n = self.mons[k].kind.name();
                    self.log.push_str(&format!(" O {n} acordou!"));
                }
                continue;
            }
            let kind = self.mons[k].kind;
            if kind == Kind::Ogro && self.steps % 2 == 1 {
                continue;
            }
            let moves = if kind == Kind::Morcego { 2 } else { 1 };
            for _ in 0..moves {
                let m = &self.mons[k];
                if Self::dist(m.pos, self.pos) == 1 && Self::adjacent4(m.pos, self.pos) {
                    let dmg = (m.atk - self.def).max(1);
                    self.hp -= dmg;
                    self.log.push_str(&format!(" O {} te acerta ({dmg}).", m.kind.name()));
                    if kind == Kind::Goblin && self.gold > 0 {
                        self.gold = self.gold.saturating_sub(5);
                    }
                    if self.hp <= 0 {
                        self.hp = 0;
                        self.log = format!("O {} venceu você no andar {}.", kind.name(), self.depth);
                        return self.end(false);
                    }
                    break;
                }
                if kind == Kind::Dragao {
                    break; // Guarda a escada: não sai do lugar.
                }
                self.chase(k);
            }
        }
    }

    fn adjacent4(a: usize, b: usize) -> bool {
        let (dr, dc) = ((a / MW) as i32 - (b / MW) as i32, (a % MW) as i32 - (b % MW) as i32);
        dr.abs() + dc.abs() == 1
    }

    fn chase(&mut self, k: usize) {
        let m = &self.mons[k];
        let (mr, mc) = ((m.pos / MW) as i32, (m.pos % MW) as i32);
        let (pr, pc) = ((self.pos / MW) as i32, (self.pos % MW) as i32);
        let (dr, dc) = (pr - mr, pc - mc);
        let mut opts = [(dr.signum(), 0), (0, dc.signum())];
        if dc.abs() > dr.abs() {
            opts.swap(0, 1);
        }
        for (r, c) in opts {
            if (r, c) == (0, 0) {
                continue;
            }
            if let Some(t) = idx(mr + r, mc + c) {
                if self.map[t] != Tile::Wall && t != self.pos && !self.mons.iter().any(|o| o.pos == t) {
                    self.mons[k].pos = t;
                    return;
                }
            }
        }
    }

    fn choose(&mut self, life: bool) {
        if self.state != State::LevelUp {
            return;
        }
        if life {
            self.max_hp += 5;
        } else {
            self.atk += 1;
        }
        self.hp = self.max_hp;
        self.state = State::Play;
        self.log = if life { format!("Vida máxima {}. Vida cheia.", self.max_hp) } else { format!("Força {}. Vida cheia.", self.atk) };
    }

    fn arrow_rect(k: usize) -> Rect {
        let (cx, cy, s) = (W / 2 + 230, 1262, 96);
        match k {
            0 => Rect::new(cx - s / 2, cy - s - 4, s, s),
            1 => Rect::new(cx - s / 2, cy + 4, s, s),
            2 => Rect::new(cx - s / 2 - s - 10, cy - s / 2, s, s),
            _ => Rect::new(cx + s / 2 + 10, cy - s / 2, s, s),
        }
    }

    fn potion_rect() -> Rect {
        Rect::new(MARGIN - 20, 1312, 300, 92)
    }

    fn choice_rect(k: usize) -> Rect {
        Rect::new(MX + 90, MY + 370 + k as i32 * 125, VW as i32 * CELL - 180, 100)
    }

    /// Canto da câmera (linha, coluna), centrada no herói e presa às bordas do mapa.
    fn view(&self) -> (i32, i32) {
        let (hr, hc) = ((self.pos / MW) as i32, (self.pos % MW) as i32);
        ((hr - VH as i32 / 2).clamp(0, (MH - VH) as i32), (hc - VW as i32 / 2).clamp(0, (MW - VW) as i32))
    }

    /// Casa na tela, se estiver dentro da câmera.
    fn cell(&self, i: usize) -> Option<Rect> {
        let (r0, c0) = self.view();
        let (r, c) = ((i / MW) as i32 - r0, (i % MW) as i32 - c0);
        ((0..VH as i32).contains(&r) && (0..VW as i32).contains(&c)).then(|| Rect::new(MX + c * CELL, MY + r * CELL, CELL, CELL))
    }

    fn heart(fb: &mut Framebuffer, x: i32, y: i32, s: i32, v: u8) {
        fb.round_rect(Rect::new(x, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        fb.round_rect(Rect::new(x + s / 2 - 2, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        for k in 0..s / 2 {
            fb.fill(Rect::new(x + k, y + s / 4 + k, s - 2 * k, 1), v);
        }
    }
}

impl MiniGame for Masmorra {
    fn at_menu(&self) -> bool {
        self.state == State::Menu
    }

    fn summary(&self) -> String {
        if self.map.is_empty() || self.state == State::Over {
            return String::new();
        }
        format!("Andar {} · vida {}/{} · nível {}", self.depth, self.hp, self.max_hp, self.level)
    }

    fn context(&self) -> String {
        if self.state == State::Menu || self.map.is_empty() {
            return "No menu da Masmorra.".into();
        }
        let near: Vec<String> = self
            .mons
            .iter()
            .filter(|m| self.visible(m.pos))
            .map(|m| format!("{} (vida {}, força {}, {}, {})", m.kind.name(), m.hp, m.atk, m.kind.trait_text(), if m.awake { "acordado" } else { "dormindo" }))
            .collect();
        format!(
            "Masmorra (roguelike por turnos), andar {} de 5. Pedro: nível {}, vida {}/{}, força {}, defesa {}, poções {}. Monstros à vista: {}. Regras: dano sem sorte; monstros dormindo acordam a {WAKE} casas; ver chão novo cura 1; subir de nível enche a vida.",
            self.depth,
            self.level,
            self.hp,
            self.max_hp,
            self.atk,
            self.def,
            self.potions,
            if near.is_empty() { "nenhum".into() } else { near.join("; ") }
        )
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.state == State::Menu || self.map.is_empty() {
            let sub = if self.best > 0 { format!("Cinco andares, um dragão. Recorde: {} pontos.", self.best) } else { "Cinco andares, um dragão. O mundo anda quando você anda.".into() };
            let cont = !self.map.is_empty() && self.state != State::Over;
            let opts: Vec<&str> = if cont { vec!["Continuar", "Nova aventura", "Desafio do dia"] } else { vec!["Nova aventura", "Desafio do dia"] };
            minigame::draw_menu(fb, f, "Masmorra", &sub, &opts);
            return;
        }
        let mode = if self.daily { " · desafio do dia" } else { "" };
        minigame::header(fb, f, "Masmorra", &format!("Andar {} de 5 · {} pontos{mode}", self.depth, self.score()));
        // Fundo: rocha escura onde você ainda não esteve (a masmorra não é papel em branco).
        fb.fill(Rect::new(MX, MY, VW as i32 * CELL, VH as i32 * CELL), 0x2A);
        for i in 0..MW * MH {
            let Some(r) = self.cell(i) else { continue };
            if !self.seen[i] {
                // Veios na rocha, fixos por casa.
                let h = (i as i32 * 7919) % 97;
                fb.fill(Rect::new(r.x + 10 + h % 40, r.y + 18 + h % 30, 26, 3), 0x3A);
                fb.fill(Rect::new(r.x + 40 - h % 20, r.y + 58 - h % 16, 20, 3), 0x3A);
                continue;
            }
            let vis = self.visible(i);
            match self.map[i] {
                Tile::Wall => {
                    if !crate::sprites::draw(fb, "parede", r) {
                        // Parede de pedra: blocos desencontrados.
                        fb.fill(r, 0x5C);
                        for k in 0..4 {
                            let y = r.y + k * CELL / 4;
                            fb.fill(Rect::new(r.x, y, CELL, 3), 0x3A);
                            let off = if k % 2 == 0 { CELL / 3 } else { CELL * 5 / 6 };
                            fb.fill(Rect::new(r.x + off, y, 3, CELL / 4), 0x3A);
                        }
                    }
                }
                Tile::Floor | Tile::Stairs => {
                    if !crate::sprites::draw(fb, "chao", r) {
                        fb.fill(r, 0xF2);
                        fb.round_outline(Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6), 6, 2, 0xD8, 0xF2);
                    }
                    if self.map[i] == Tile::Stairs && !crate::sprites::draw(fb, "escada", Rect::new(r.x + 6, r.y + 6, r.w - 12, r.h - 12)) {
                        for k in 0..3 {
                            fb.fill(Rect::new(r.x + 14 + k * 20, r.y + 62 - k * 20, CELL - 28 - k * 20, 16), INK);
                        }
                    }
                }
            }
            if !vis {
                // Lembrança: o que já viu, mas está longe, fica na penumbra.
                for yy in r.y..r.y + r.h {
                    for xx in r.x..r.x + r.w {
                        fb.blend(xx, yy, 0x40, 110);
                    }
                }
            }
        }
        // Personagens alternam dois quadros a cada passo.
        let frame = if self.steps % 2 == 0 { "1" } else { "2" };
        let sprite = |fb: &mut Framebuffer, i: usize, name: &str| {
            let Some(r) = self.cell(i) else { return };
            if !crate::sprites::draw(fb, name, Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8)) {
                fb.round_rect(Rect::new(r.x + 14, r.y + 14, r.w - 28, r.h - 28), 12, INK);
            }
        };
        for &(p, it) in &self.items {
            if self.visible(p) {
                let name = match it {
                    Item::Potion => "pocao",
                    Item::Sword => "espada",
                    Item::Shield => "escudo",
                    Item::Gold => "ouro",
                };
                sprite(fb, p, name);
            }
        }
        for m in &self.mons {
            if !self.visible(m.pos) {
                continue;
            }
            sprite(fb, m.pos, &format!("{}-{frame}", m.kind.sprite()));
            let Some(r) = self.cell(m.pos) else { continue };
            // Vida do monstro, sempre à vista; z quando dorme.
            let s = m.hp.to_string();
            let w = f.measure(Face::Display, 20.0, &s) + 12;
            let b = Rect::new(r.x, r.y + r.h - 24, w, 24);
            fb.round_rect(b, 12, INK);
            f.draw(fb, Face::Display, 20.0, b.x + 6, b.y + 19, PAPER, &s);
            if !m.awake {
                let zb = Rect::new(r.x + r.w - 24, r.y, 24, 24);
                fb.round_rect(zb, 12, PAPER);
                fb.round_outline(zb, 12, 2, INK, PAPER);
                f.draw(fb, Face::Strong, 18.0, zb.x + 7, zb.y + 18, INK, "z");
            }
        }
        sprite(fb, self.pos, &format!("heroi-{frame}"));
        // Moldura fina do mapa (quatro traços, sem pintar o miolo).
        let (bw, bh) = (VW as i32 * CELL, VH as i32 * CELL);
        for r in [Rect::new(MX - 4, MY - 4, bw + 8, 3), Rect::new(MX - 4, MY + bh + 1, bw + 8, 3), Rect::new(MX - 4, MY - 4, 3, bh + 8), Rect::new(MX + bw + 1, MY - 4, 3, bh + 8)] {
            fb.fill(r, INK);
        }
        // Painel: vida, força, defesa, nível.
        let y = MY + bh + 22;
        Self::heart(fb, MARGIN - 20, y + 4, 30, INK);
        let mut x = f.draw(fb, Face::Display, 30.0, MARGIN + 18, y + 30, INK, &format!("{}/{}", self.hp.max(0), self.max_hp)) + 24;
        ui::stroke(fb, (x as f32, (y + 32) as f32), ((x + 22) as f32, (y + 6) as f32), 5.0, INK);
        ui::stroke(fb, (x as f32 + 2.0, (y + 18) as f32), (x as f32 + 13.0, (y + 30) as f32), 5.0, INK);
        x = f.draw(fb, Face::Display, 30.0, x + 28, y + 30, INK, &self.atk.to_string()) + 24;
        if self.def > 0 {
            fb.round_outline(Rect::new(x, y + 4, 24, 28), 8, 4, INK, PAPER);
            x = f.draw(fb, Face::Display, 30.0, x + 32, y + 30, INK, &self.def.to_string()) + 24;
        }
        x = f.draw(fb, Face::Strong, 28.0, x, y + 30, INK, &format!("Nv {}", self.level)) + 10;
        let xb = Rect::new(x, y + 16, 90, 8);
        fb.fill(xb, 0xDD);
        fb.fill(Rect::new(xb.x, xb.y, xb.w * self.xp / self.xp_next().max(1), xb.h), 0x5C);
        // Recado e monstros por perto.
        let text_w = Self::arrow_rect(2).x - MARGIN;
        let mut ly = y + 74;
        for l in wrap(f, &self.log, text_w).iter().take(2) {
            f.draw(fb, Face::Body, 26.0, MARGIN - 20, ly, INK, l);
            ly += 34;
        }
        let mut near: Vec<&Monster> = self.mons.iter().filter(|m| self.visible(m.pos)).collect();
        near.sort_by_key(|m| Self::dist(m.pos, self.pos));
        if let Some(m) = near.first() {
            let n = m.kind.name();
            let t = format!("{}{}: vida {}, força {}, {}{}.", n[..1].to_uppercase(), &n[1..], m.hp, m.atk, m.kind.trait_text(), if m.awake { "" } else { ", dormindo" });
            for l in wrap(f, &t, text_w).iter().take(2) {
                f.draw(fb, Face::Body, 24.0, MARGIN - 20, ly + 4, MUTED, l);
                ly += 32;
            }
        }
        match self.state {
            State::Over => {
                minigame::pill(fb, f, Rect::new(MARGIN - 20, 1312, 300, 92), "Menu", false);
                minigame::pill(fb, f, Rect::new(MARGIN + 300, 1312, 400, 92), "Nova aventura", true);
                return;
            }
            State::LevelUp => {
                let bx = Rect::new(MX + 40, MY + 200, bw - 80, 430);
                fb.round_rect(Rect::new(bx.x - 6, bx.y - 6, bx.w + 12, bx.h + 12), 30, PAPER);
                fb.round_rect(bx, 26, INK);
                f.draw(fb, Face::Display, 44.0, bx.x + 40, bx.y + 76, PAPER, &format!("Nível {}!", self.level));
                f.draw(fb, Face::Body, 28.0, bx.x + 40, bx.y + 118, 0xCC, "A vida enche. Escolha o que melhorar:");
                for (k, t) in ["+5 de vida máxima", "+1 de força"].iter().enumerate() {
                    let r = Self::choice_rect(k);
                    fb.round_rect(r, r.h / 2, PAPER);
                    let w = f.measure(Face::Strong, 34.0, t);
                    f.draw(fb, Face::Strong, 34.0, r.x + (r.w - w) / 2, r.y + r.h / 2 + 12, INK, t);
                }
                return;
            }
            _ => {}
        }
        minigame::pill(fb, f, Self::potion_rect(), &format!("Poção {}", self.potions), self.potions > 0 && self.hp * 3 <= self.max_hp);
        if self.potions == 0 {
            let r = Self::potion_rect();
            for yy in r.y..r.y + r.h {
                for xx in r.x..r.x + r.w {
                    fb.blend(xx, yy, PAPER, 150);
                }
            }
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
        match self.state {
            _ if self.state == State::Menu || self.map.is_empty() => {
                let cont = !self.map.is_empty();
                if cont {
                    vec![(903, minigame::menu_row(0)), (900, minigame::menu_row(1)), (901, minigame::menu_row(2))]
                } else {
                    vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))]
                }
            }
            State::Over => vec![(803, Rect::new(MARGIN - 20, 1312, 300, 92)), (902, Rect::new(MARGIN + 300, 1312, 400, 92))],
            State::LevelUp => vec![(820, Self::choice_rect(0)), (821, Self::choice_rect(1))],
            _ => {
                let mut v: Vec<(u32, Rect)> = (0..4).map(|k| (810 + k as u32, Self::arrow_rect(k))).collect();
                v.push((800, Self::potion_rect()));
                v
            }
        }
    }

    fn back(&mut self) -> bool {
        if self.state == State::Menu {
            return false;
        }
        if self.state == State::Over {
            self.map.clear();
        }
        self.state = State::Menu;
        true
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let area = Redraw::Part(Rect::new(0, 170, W, 1448 - 170));
        match id {
            900 => self.new_game(false, Rng::new()),
            901 => self.new_game(true, Rng::seeded(crate::fases::today() as u64 * 6151 + 3)),
            902 => {
                let d = self.daily;
                let rng = if d { Rng::seeded(crate::fases::today() as u64 * 6151 + 3) } else { Rng::new() };
                self.new_game(d, rng);
            }
            903 => self.state = State::Play,
            803 => {
                self.map.clear();
                self.state = State::Menu;
            }
            820 | 821 => {
                self.choose(id == 820);
                return Redraw::Full;
            }
            800 => {
                self.drink();
                return if self.state == State::Over { Redraw::Full } else { area };
            }
            810..=813 => {
                let before = self.depth;
                self.step(DIRS[(id - 810) as usize]);
                return if self.depth != before || self.state != State::Play { Redraw::Full } else { area };
            }
            _ => return Redraw::None,
        }
        Redraw::Full
    }
}

fn wrap(f: &Fonts, text: &str, max_w: i32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for w in text.split_whitespace() {
        let cur = lines.last().unwrap().clone();
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if f.measure(Face::Body, 26.0, &cand) > max_w && !cur.is_empty() {
            lines.push(w.to_string());
        } else {
            *lines.last_mut().unwrap() = cand;
        }
    }
    lines
}

/// Robô de testes: segue até a escada pelo caminho mais curto, pegando itens à vista, luta com o
/// que encosta. `smart` bebe poção na hora certa e alterna força/vida ao subir de nível.
#[cfg(test)]
fn bot_play(seed: u64, smart: bool) -> Masmorra {
    let mut g = Masmorra::new();
    g.new_game(false, Rng::seeded(seed));
    for _ in 0..3000 {
        match g.state {
            State::Over => break,
            State::LevelUp => {
                g.choose(if smart { g.level % 2 == 1 } else { true });
                continue;
            }
            _ => {}
        }
        if smart && g.potions > 0 && g.hp * 3 <= g.max_hp && g.mons.iter().any(|m| m.awake && Masmorra::dist(m.pos, g.pos) <= 2) {
            g.drink();
            continue;
        }
        if let Some(m) = g.mons.iter().find(|m| m.awake && Masmorra::adjacent4(m.pos, g.pos)) {
            let t = m.pos;
            let d = DIRS.iter().position(|&(r, c)| idx((g.pos / MW) as i32 + r, (g.pos % MW) as i32 + c) == Some(t)).unwrap();
            g.step(DIRS[d]);
            continue;
        }
        // Alvo: item à vista, senão a escada.
        let mut goal: Vec<usize> = g.items.iter().map(|&(p, _)| p).filter(|&p| g.visible(p)).collect();
        if smart {
            // Caça o que vence com folga (experiência é o que faz crescer).
            for m in g.mons.iter().filter(|m| g.visible(m.pos) && m.kind != Kind::Dragao) {
                let (_, _, armor, _) = m.kind.base();
                let hits = (m.hp + (g.atk - armor).max(1) - 1) / (g.atk - armor).max(1);
                if (hits - 1 + (m.kind == Kind::Morcego) as i32) * (m.atk - g.def).max(1) < g.hp - 2 {
                    goal.extend(DIRS.iter().filter_map(|&(r, c)| idx((m.pos / MW) as i32 + r, (m.pos % MW) as i32 + c)).filter(|&j| g.map[j] != Tile::Wall));
                }
            }
            if g.depth == 5 && goal.is_empty() && g.hp < g.max_hp && (0..MW * MH).any(|i| !g.seen[i] && g.map[i] != Tile::Wall) {
                // Antes do dragão: explora o que falta (cura).
                goal.extend((0..MW * MH).filter(|&i| !g.seen[i] && g.map[i] != Tile::Wall));
            }
        }
        let goal = if goal.is_empty() { (0..MW * MH).filter(|&i| g.map[i] == Tile::Stairs).collect() } else { goal };
        // BFS a partir dos alvos até o herói: o passo é o vizinho mais perto do alvo.
        let mut dist = vec![i32::MAX; MW * MH];
        let mut q = std::collections::VecDeque::new();
        for &t in &goal {
            dist[t] = 0;
            q.push_back(t);
        }
        while let Some(i) = q.pop_front() {
            for (r, c) in DIRS {
                if let Some(j) = idx((i / MW) as i32 + r, (i % MW) as i32 + c) {
                    if g.map[j] != Tile::Wall && dist[j] == i32::MAX {
                        dist[j] = dist[i] + 1;
                        q.push_back(j);
                    }
                }
            }
        }
        let best = DIRS.iter().copied().filter_map(|(r, c)| idx((g.pos / MW) as i32 + r, (g.pos % MW) as i32 + c).map(|j| ((r, c), dist[j]))).min_by_key(|&(_, d)| d);
        match best {
            Some((d, _)) => g.step(d),
            None => break,
        }
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn andar_tem_escada_alcancavel() {
        let mut g = Masmorra::new();
        g.new_game(false, Rng::seeded(5));
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

    /// `cargo test --release equilibrio_masmorra -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn equilibrio_masmorra() {
        let n = 2000;
        for smart in [true, false] {
            let (mut won, mut floors, mut lv) = (0, [0u32; 6], 0);
            for s in 0..n {
                let g = bot_play(s * 13 + 7, smart);
                if g.won {
                    won += 1;
                } else {
                    floors[g.depth as usize] += 1;
                }
                lv += g.level;
            }
            eprintln!("robô {}: venceu {:.1}% · morreu no andar 1..5 = {:?} · nível {:.1}", if smart { "esperto" } else { "ingênuo" }, 100.0 * won as f32 / n as f32, &floors[1..6], lv as f32 / n as f32);
        }
    }

    #[test]
    #[ignore]
    fn masmorra_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        let shot = |g: &Masmorra, name: &str| {
            let mut fb = crate::fb::Framebuffer::memory();
            g.draw(&mut fb, &f);
            fb.save_pgm(&out.join(name));
        };
        // Joga com o robô até ter monstros à vista.
        let mut g = Masmorra::new();
        g.new_game(false, Rng::seeded(20));
        g.depth = 3;
        g.level();
        for k in 0..40 {
            if g.mons.iter().filter(|m| g.visible(m.pos)).count() >= 2 {
                break;
            }
            g.step(DIRS[k % 4]);
            g.state = State::Play;
        }
        shot(&g, "masmorra.pgm");
        g.state = State::LevelUp;
        shot(&g, "masmorra-nivel.pgm");
        g.state = State::Menu;
        g.map.clear();
        shot(&g, "masmorra-menu.pgm");
    }
}
