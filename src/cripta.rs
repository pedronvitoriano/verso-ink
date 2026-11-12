//! Cripta: quebra-cabeça de masmorra, parado e de informação completa. Os monstros estão à
//! vista desde o começo, com vida e força; a rocha esconde tesouros e o fôlego da exploração.
//! Abrir uma casa cura você (e os monstros feridos também se recuperam um pouco); cada toque
//! num monstro é uma troca de golpes; subir de nível cura tudo. Cinco andares, o dragão no fim.
//!
//! As regras ficam em `Run` (sem tela), para o robô de testes jogar o mesmo jogo.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

pub const N: usize = 6;
const CELL: i32 = 136;
const GX: i32 = (W - N as i32 * CELL) / 2;
const GY: i32 = 316;
pub const FLOORS: u32 = 5;

// ---------- Regras ----------


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
    pub fn name(self) -> &'static str {
        match self {
            Kind::Rato => "Rato",
            Kind::Morcego => "Morcego",
            Kind::Goblin => "Goblin",
            Kind::Esqueleto => "Esqueleto",
            Kind::Orc => "Orc",
            Kind::Ogro => "Ogro",
            Kind::Dragao => "Dragão",
        }
    }
    fn sprite(self) -> &'static str {
        match self {
            Kind::Rato => "rato",
            Kind::Morcego => "morcego",
            Kind::Goblin => "goblin",
            Kind::Esqueleto => "esqueleto",
            Kind::Orc => "orc",
            Kind::Ogro => "ogro",
            Kind::Dragao => "dragao",
        }
    }
    /// Ataca antes de você.
    pub fn fast(self) -> bool {
        matches!(self, Kind::Morcego | Kind::Dragao)
    }
    /// Reduz o seu dano.
    pub fn armor(self) -> i32 {
        match self {
            Kind::Esqueleto => 1,
            Kind::Dragao => 1,
            _ => 0,
        }
    }
    /// Recupera vida quando você explora.
    pub fn regen(self) -> i32 {
        match self {
            Kind::Ogro => 2,
            _ => 1,
        }
    }
    pub fn trait_text(self) -> &'static str {
        match self {
            Kind::Rato => "comum",
            Kind::Morcego => "ataca primeiro",
            Kind::Goblin => "rouba ouro ao acertar",
            Kind::Esqueleto => "armadura: −1 no seu golpe",
            Kind::Orc => "golpe pesado",
            Kind::Ogro => "se recupera rápido",
            Kind::Dragao => "chefe final: ataca primeiro e tem armadura",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mon {
    pub kind: Kind,
    pub level: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub key: bool,
}

impl Mon {
    fn new(kind: Kind, level: i32, floor: u32) -> Mon {
        let f = floor as i32 - 1;
        let mut hp = 3 + 5 * (level - 1) + f;
        let mut atk = 1 + level * 185 / 100 + f / 2;
        if kind == Kind::Orc {
            atk += 2;
        }
        if kind == Kind::Dragao {
            hp += 14;
            atk += 3;
        }
        Mon { kind, level, hp, max_hp: hp, atk, key: false }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Loot {
    Nothing,
    Potion,
    Gold(i32),
    Power,
    Glyph(Spell),
    Stairs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spell {
    /// Dano de 2× o seu nível, sem troco.
    Raio,
    /// O próximo golpe que você levaria não acontece.
    Pedra,
    /// Dano igual ao nível e cura o mesmo tanto.
    Drenar,
}

impl Spell {
    pub fn name(self) -> &'static str {
        match self {
            Spell::Raio => "Raio",
            Spell::Pedra => "Pele de pedra",
            Spell::Drenar => "Drenar",
        }
    }
    pub fn cost(self) -> i32 {
        match self {
            Spell::Raio => 3,
            Spell::Pedra => 2,
            Spell::Drenar => 4,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cell {
    /// Rocha com algo (ou nada) por baixo.
    Rock(Loot),
    Floor(Loot),
    Mon(Mon),
}

/// Prêmio escolhido entre andares.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reward {
    Life,
    Force,
    Mana,
    Potions,
    Learn(Spell),
}

impl Reward {
    pub fn text(self) -> String {
        match self {
            Reward::Life => "+5 de vida máxima".into(),
            Reward::Force => "+1 de força".into(),
            Reward::Mana => "+2 de mana máxima".into(),
            Reward::Potions => "2 poções".into(),
            Reward::Learn(s) => format!("Aprender {}", s.name()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Play,
    /// Escolhendo o prêmio do fim do andar.
    Reward,
    Won,
    Dead,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub rng: Rng,
    pub floor: u32,
    pub cells: Vec<Cell>,
    pub hero: usize,
    pub level: i32,
    pub xp: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub mp: i32,
    pub max_mp: i32,
    pub potions: i32,
    pub gold: i32,
    pub spells: Vec<Spell>,
    pub stone: bool,
    pub key: bool,
    pub phase: Phase,
    pub offers: Vec<Reward>,
    pub turns: u32,
    pub log: String,
}

pub fn neighbors(i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = (i / N, i % N);
    [(r > 0).then(|| i - N), (r + 1 < N).then(|| i + N), (c > 0).then(|| i - 1), (c + 1 < N).then(|| i + 1)].into_iter().flatten()
}

/// Resultado previsto de uma troca de golpes e da luta inteira (sem cura no meio).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Forecast {
    /// Dano que você leva nesta troca.
    pub take_now: i32,
    /// Vida do monstro depois desta troca.
    pub mon_after: i32,
    /// Golpes e dano total para matar daqui até o fim.
    pub hits_to_kill: i32,
    pub take_total: i32,
}

impl Run {
    pub fn new(rng: Rng) -> Run {
        let mut r = Run {
            rng,
            floor: 1,
            cells: vec![],
            hero: 0,
            level: 1,
            xp: 0,
            hp: 12,
            max_hp: 12,
            atk: 3,
            mp: 3,
            max_mp: 6,
            potions: 1,
            gold: 0,
            spells: vec![Spell::Raio],
            stone: false,
            key: false,
            phase: Phase::Play,
            offers: vec![],
            turns: 0,
            log: String::new(),
        };
        r.build_floor();
        r
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.rng.next() % n as u64) as usize
    }

    /// Vida que cada casa aberta devolve (equilibrado pelo robô: 40% do nível, para cima).
    pub fn heal(&self) -> i32 {
        (2 * self.level + 4) / 5
    }

    pub fn xp_next(&self) -> i32 {
        self.level * 5
    }

    /// Monta o andar: monstros à vista (o mais forte com a chave), rocha com tesouros e a escada.
    fn build_floor(&mut self) {
        let f = self.floor;
        let entry = self.pick(N * N);
        let dist = |i: usize| (i / N).abs_diff(entry / N) + (i % N).abs_diff(entry % N);
        let mut spots: Vec<usize> = (0..N * N).filter(|&i| dist(i) >= 2).collect();
        self.rng.shuffle(&mut spots);
        let mut cells = vec![Cell::Rock(Loot::Nothing); N * N];
        cells[entry] = Cell::Floor(Loot::Nothing);
        let mut it = spots.into_iter();
        // Chefe: o dragão no último andar; antes, um monstro acima do nível esperado.
        let boss_at = it.next().unwrap();
        cells[boss_at] = Cell::Mon(if f == FLOORS {
            let mut d = Mon::new(Kind::Dragao, 7, f);
            d.key = true;
            d
        } else {
            let kinds = [Kind::Goblin, Kind::Esqueleto, Kind::Ogro, Kind::Orc];
            let mut m = Mon::new(kinds[(f as usize - 1).min(3)], f as i32 + 2, f);
            m.key = true;
            m
        });
        // Monstros comuns: uma escada de níveis (alvos fáceis e alguns ousados).
        let levels: Vec<i32> = match f {
            1 => vec![1, 1, 1, 2, 2, 3],
            2 => vec![1, 2, 2, 2, 3, 3, 4],
            3 => vec![2, 2, 3, 3, 3, 4, 5],
            4 => vec![2, 3, 3, 4, 4, 5, 5],
            _ => vec![3, 3, 4, 4, 5, 5, 6],
        };
        for lv in levels {
            let kind = match lv {
                1 => Kind::Rato,
                2 => [Kind::Rato, Kind::Morcego][self.pick(2)],
                3 => [Kind::Morcego, Kind::Goblin][self.pick(2)],
                4 => [Kind::Goblin, Kind::Esqueleto][self.pick(2)],
                5 => [Kind::Esqueleto, Kind::Orc][self.pick(2)],
                _ => [Kind::Orc, Kind::Ogro][self.pick(2)],
            };
            if let Some(i) = it.next() {
                cells[i] = Cell::Mon(Mon::new(kind, lv, f));
            }
        }
        let glyph = [Spell::Pedra, Spell::Drenar, Spell::Raio][self.pick(3)];
        let mut loot = vec![Loot::Potion, Loot::Gold(10 + 5 * f as i32), Loot::Gold(10 + 5 * f as i32), Loot::Power, Loot::Glyph(glyph)];
        if f != FLOORS {
            loot.push(Loot::Stairs);
        }
        for l in loot {
            if let Some(i) = it.next() {
                cells[i] = Cell::Rock(l);
            }
        }
        self.cells = cells;
        self.hero = entry;
        self.key = false;
        self.stone = false;
        self.phase = Phase::Play;
        self.log = if f == FLOORS { "Último andar: vença o dragão.".into() } else { format!("Andar {f}. A chave da escada está com o monstro marcado.") };
    }

    pub fn open(&self, i: usize) -> bool {
        matches!(self.cells[i], Cell::Floor(_))
    }

    /// Dá para abrir esta rocha (vizinha de uma casa aberta)?
    pub fn reachable(&self, i: usize) -> bool {
        matches!(self.cells[i], Cell::Rock(_)) && neighbors(i).any(|j| self.open(j))
    }

    /// Dá para atacar este monstro (vizinho de uma casa aberta)?
    pub fn attackable(&self, i: usize) -> bool {
        matches!(self.cells[i], Cell::Mon(_)) && neighbors(i).any(|j| self.open(j))
    }

    fn dmg_to(&self, m: &Mon) -> i32 {
        (self.atk - m.kind.armor()).max(1)
    }

    pub fn forecast(&self, m: &Mon) -> Forecast {
        let d = self.dmg_to(m);
        let mon_after = (m.hp - d).max(0);
        let take_now = if m.kind.fast() || mon_after > 0 { if self.stone { 0 } else { m.atk } } else { 0 };
        let hits = (m.hp + d - 1) / d;
        let retaliations = hits - 1 + m.kind.fast() as i32;
        let take_total = (retaliations * m.atk - if self.stone && retaliations > 0 { m.atk } else { 0 }).max(0);
        Forecast { take_now, mon_after, hits_to_kill: hits, take_total }
    }

    fn gain_xp(&mut self, m: &Mon) {
        // Matar acima do seu nível vale o dobro (ousadia).
        let bonus = if m.level > self.level { m.level } else { 0 };
        self.xp += m.level + bonus;
        while self.xp >= self.xp_next() {
            self.xp -= self.xp_next();
            self.level += 1;
            self.max_hp += 3;
            self.atk += 1;
            self.hp = self.max_hp;
            self.mp = self.max_mp;
            self.log.push_str(&format!(" Nível {}! Vida e mana cheias.", self.level));
        }
    }

    fn die(&mut self, by: &str) {
        self.hp = 0;
        self.phase = Phase::Dead;
        self.log = format!("{by} venceu você no andar {}.", self.floor);
    }

    fn kill(&mut self, i: usize, m: Mon) {
        self.cells[i] = Cell::Floor(Loot::Nothing);
        if m.key {
            self.key = true;
        }
        self.log = format!("Venceu {}.", m.kind.name());
        if m.key && self.floor < FLOORS {
            self.log.push_str(" Pegou a chave da escada!");
        }
        self.gain_xp(&m);
        if m.kind == Kind::Dragao {
            self.phase = Phase::Won;
            self.log = format!("Venceu o dragão! A cripta é sua em {} turnos.", self.turns);
        }
    }

    /// Uma troca de golpes com o monstro em `i`.
    pub fn attack(&mut self, i: usize) {
        let Cell::Mon(mut m) = self.cells[i] else { return };
        if !self.attackable(i) || self.phase != Phase::Play {
            return;
        }
        self.turns += 1;
        let d = self.dmg_to(&m);
        let mut hit_me = |run: &mut Run, m: &Mon| {
            if run.stone {
                run.stone = false;
                return;
            }
            run.hp -= m.atk;
            if m.kind == Kind::Goblin && run.gold > 0 {
                run.gold = (run.gold - 5).max(0);
            }
        };
        if m.kind.fast() {
            hit_me(self, &m);
            if self.hp <= 0 {
                return self.die(m.kind.name());
            }
        }
        m.hp -= d;
        if m.hp <= 0 {
            return self.kill(i, m);
        }
        if !m.kind.fast() {
            hit_me(self, &m);
        }
        self.cells[i] = Cell::Mon(m);
        if self.hp <= 0 {
            return self.die(m.kind.name());
        }
        self.log = format!("{}: ficou com {} de vida.", m.kind.name(), m.hp);
    }

    /// Abre a rocha em `i`: você se cura e ganha mana; monstros feridos se recuperam um pouco.
    pub fn reveal(&mut self, i: usize) {
        if !self.reachable(i) || self.phase != Phase::Play {
            return;
        }
        self.turns += 1;
        let Cell::Rock(loot) = self.cells[i] else { return };
        self.cells[i] = Cell::Floor(loot);
        self.hero = i;
        self.hp = (self.hp + self.heal()).min(self.max_hp);
        self.mp = (self.mp + 1).min(self.max_mp);
        for c in self.cells.iter_mut() {
            if let Cell::Mon(m) = c {
                m.hp = (m.hp + m.kind.regen()).min(m.max_hp);
            }
        }
        self.log = format!("+{} de vida, +1 de mana.", self.heal());
        match loot {
            Loot::Potion => {
                self.potions += 1;
                self.log.push_str(" Achou uma poção.");
                self.cells[i] = Cell::Floor(Loot::Nothing);
            }
            Loot::Gold(g) => {
                self.gold += g;
                self.log.push_str(&format!(" {g} de ouro."));
                self.cells[i] = Cell::Floor(Loot::Nothing);
            }
            Loot::Power => {
                self.atk += 1;
                self.log.push_str(&format!(" Espada melhor: força {}.", self.atk));
                self.cells[i] = Cell::Floor(Loot::Nothing);
            }
            Loot::Glyph(s) => {
                if !self.spells.contains(&s) {
                    self.spells.push(s);
                    self.log.push_str(&format!(" Aprendeu {}.", s.name()));
                } else {
                    self.max_mp += 1;
                    self.log.push_str(" Mana máxima +1.");
                }
                self.cells[i] = Cell::Floor(Loot::Nothing);
            }
            Loot::Stairs => self.log.push_str(if self.key { " A escada!" } else { " A escada, trancada." }),
            Loot::Nothing => {}
        }
    }

    pub fn drink(&mut self) {
        if self.potions > 0 && self.hp < self.max_hp && self.phase == Phase::Play {
            self.potions -= 1;
            let heal = (self.max_hp + 1) / 2;
            self.hp = (self.hp + heal).min(self.max_hp);
            self.turns += 1;
            self.log = format!("Poção: +{heal} de vida.");
        }
    }

    /// Lança a magia `s` no monstro `i` (Raio e Drenar) ou em você (Pele de pedra).
    pub fn cast(&mut self, s: Spell, target: Option<usize>) -> bool {
        if self.mp < s.cost() || !self.spells.contains(&s) || self.phase != Phase::Play {
            return false;
        }
        match s {
            Spell::Pedra => {
                if self.stone {
                    return false;
                }
                self.stone = true;
                self.log = "Pele de pedra: o próximo golpe não pega.".into();
            }
            Spell::Raio | Spell::Drenar => {
                let Some(i) = target.filter(|&i| self.attackable(i)) else { return false };
                let Cell::Mon(mut m) = self.cells[i] else { return false };
                let d = if s == Spell::Raio { 2 * self.level + 1 } else { self.level + 1 };
                m.hp -= d;
                if s == Spell::Drenar {
                    self.hp = (self.hp + d).min(self.max_hp);
                }
                self.mp -= s.cost();
                self.turns += 1;
                if m.hp <= 0 {
                    self.kill(i, m);
                } else {
                    self.cells[i] = Cell::Mon(m);
                    self.log = format!("{}: {d} de dano. {} ficou com {}.", s.name(), m.kind.name(), m.hp);
                }
                return true;
            }
        }
        self.mp -= s.cost();
        self.turns += 1;
        true
    }

    /// Descer: só com a chave. Antes do próximo andar, escolha um prêmio.
    pub fn descend(&mut self, i: usize) {
        if self.cells[i] != Cell::Floor(Loot::Stairs) || !self.key || self.phase != Phase::Play {
            return;
        }
        let mut pool = vec![Reward::Life, Reward::Force, Reward::Mana, Reward::Potions];
        for s in [Spell::Pedra, Spell::Drenar] {
            if !self.spells.contains(&s) {
                pool.push(Reward::Learn(s));
            }
        }
        self.rng.shuffle(&mut pool);
        self.offers = pool.into_iter().take(3).collect();
        self.phase = Phase::Reward;
        self.log = "Escolha um prêmio para o próximo andar.".into();
    }

    pub fn choose(&mut self, k: usize) {
        let Some(&r) = self.offers.get(k) else { return };
        match r {
            Reward::Life => {
                self.max_hp += 5;
                self.hp += 5;
            }
            Reward::Force => self.atk += 1,
            Reward::Mana => {
                self.max_mp += 2;
                self.mp += 2;
            }
            Reward::Potions => self.potions += 2,
            Reward::Learn(s) => self.spells.push(s),
        }
        self.floor += 1;
        self.offers.clear();
        self.build_floor();
    }

    pub fn score(&self) -> u32 {
        (self.floor as u32 - 1) * 100 + self.gold as u32 + if self.phase == Phase::Won { 500 } else { 0 }
    }
}

// ---------- Jogo (tela) ----------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    Menu,
    Game,
}

pub struct Cripta {
    screen: Screen,
    run: Option<Run>,
    daily: bool,
    selected: Option<usize>,
    /// Magia escolhida esperando o alvo.
    aiming: Option<Spell>,
    best: u32,
    steps: u32,
}

fn best_file() -> std::path::PathBuf {
    crate::home_dir().join("data/cripta_recorde.txt")
}

impl Cripta {
    pub fn new() -> Self {
        let best = std::fs::read_to_string(best_file()).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        Cripta { screen: Screen::Menu, run: None, daily: false, selected: None, aiming: None, best, steps: 0 }
    }

    fn start(&mut self, daily: bool) {
        self.daily = daily;
        let rng = if daily { Rng::seeded(crate::fases::today() as u64 * 7919 + 1) } else { Rng::new() };
        self.run = Some(Run::new(rng));
        self.screen = Screen::Game;
        self.selected = None;
        self.aiming = None;
    }

    fn cell(i: usize) -> Rect {
        Rect::new(GX + (i % N) as i32 * CELL, GY + (i / N) as i32 * CELL, CELL, CELL)
    }

    fn after_turn(&mut self) {
        self.steps += 1;
        if let Some(r) = &self.run {
            if matches!(r.phase, Phase::Dead | Phase::Won) && !self.daily && r.score() > self.best {
                self.best = r.score();
                let _ = std::fs::write(best_file(), self.best.to_string());
            }
        }
    }

    fn rock(fb: &mut Framebuffer, r: Rect) {
        fb.fill(r, 0x4C);
        fb.fill(Rect::new(r.x, r.y, r.w, 6), 0x66);
        fb.fill(Rect::new(r.x, r.y, 6, r.h), 0x66);
        fb.fill(Rect::new(r.x, r.y + r.h - 6, r.w, 6), 0x30);
        fb.fill(Rect::new(r.x + r.w - 6, r.y, 6, r.h), 0x30);
    }

    fn heart(fb: &mut Framebuffer, x: i32, y: i32, s: i32, v: u8) {
        fb.round_rect(Rect::new(x, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        fb.round_rect(Rect::new(x + s / 2 - 2, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        for k in 0..s / 2 {
            fb.fill(Rect::new(x + k, y + s / 4 + k, s - 2 * k, 1), v);
        }
    }

    fn key_icon(fb: &mut Framebuffer, x: i32, y: i32, v: u8, bg: u8) {
        fb.round_outline(Rect::new(x, y, 22, 22), 11, 5, v, bg);
        fb.fill(Rect::new(x + 20, y + 9, 22, 5), v);
        fb.fill(Rect::new(x + 34, y + 9, 5, 12), v);
    }

    fn stairs(fb: &mut Framebuffer, cx: i32, cy: i32, v: u8) {
        for k in 0..4 {
            fb.fill(Rect::new(cx - 40 + k * 20, cy + 22 - k * 18, 80 - k * 20, 16), v);
        }
    }

    fn draw_board(&self, fb: &mut Framebuffer, f: &Fonts, run: &Run) {
        let frame = if self.steps % 2 == 0 { "1" } else { "2" };
        fb.round_rect(Rect::new(GX - 14, GY - 14, N as i32 * CELL + 28, N as i32 * CELL + 28), 18, 0x22);
        for i in 0..N * N {
            let r = Self::cell(i);
            let inner = Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4);
            let art = Rect::new(r.x + 12, r.y + 20, r.w - 24, r.h - 32);
            match run.cells[i] {
                Cell::Rock(_) => {
                    Self::rock(fb, inner);
                    if run.reachable(i) && run.phase == Phase::Play {
                        for yy in inner.y + 8..inner.y + inner.h - 8 {
                            for xx in inner.x + 8..inner.x + inner.w - 8 {
                                fb.blend(xx, yy, PAPER, 150);
                            }
                        }
                        fb.round_outline(Rect::new(inner.x + 6, inner.y + 6, inner.w - 12, inner.h - 12), 10, 4, INK, 0xFF);
                        fb.fill(Rect::new(inner.x + 10, inner.y + 10, inner.w - 20, inner.h - 20), 0xC0);
                    }
                }
                Cell::Floor(loot) => {
                    fb.fill(inner, 0xEE);
                    fb.round_outline(Rect::new(inner.x + 6, inner.y + 6, inner.w - 12, inner.h - 12), 8, 2, 0xD4, 0xEE);
                    if loot == Loot::Stairs {
                        Self::stairs(fb, r.x + r.w / 2, r.y + r.h / 2, if run.key { INK } else { 0x99 });
                        if !run.key {
                            // Cadeado.
                            fb.round_rect(Rect::new(r.x + r.w - 46, r.y + 8, 36, 30), 6, INK);
                            fb.round_outline(Rect::new(r.x + r.w - 40, r.y - 4, 24, 26), 12, 4, INK, 0xEE);
                        }
                    }
                    if i == run.hero && !crate::sprites::draw(fb, &format!("heroi-{frame}"), art) {
                        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                        fb.round_rect(Rect::new(cx - 14, cy - 34, 28, 28), 14, INK);
                        fb.round_rect(Rect::new(cx - 24, cy - 2, 48, 38), 16, INK);
                    }
                }
                Cell::Mon(m) => {
                    // Monstro à vista: fundo de pedra escura se ainda não dá para atacar.
                    let reach = run.attackable(i);
                    fb.fill(inner, if reach { 0xEE } else { 0x9A });
                    if self.selected == Some(i) {
                        fb.round_outline(Rect::new(r.x - 2, r.y - 2, r.w + 4, r.h + 4), 12, 8, INK, 0xEE);
                    }
                    if !crate::sprites::draw(fb, &format!("{}-{frame}", m.kind.sprite()), art) {
                        fb.round_rect(Rect::new(r.x + 24, r.y + 30, r.w - 48, r.h - 50), 18, INK);
                    }
                    // Vida (barra) e força (número) do monstro, sempre à vista.
                    let bar = Rect::new(r.x + 10, r.y + r.h - 18, r.w - 20, 10);
                    fb.fill(bar, PAPER);
                    fb.round_outline(bar, 5, 2, INK, PAPER);
                    fb.fill(Rect::new(bar.x + 2, bar.y + 2, (bar.w - 4) * m.hp / m.max_hp.max(1), bar.h - 4), INK);
                    let s = format!("{}", m.hp);
                    let b = Rect::new(r.x + 4, r.y + 4, f.measure(Face::Display, 24.0, &s) + 36, 30);
                    fb.round_rect(b, 15, PAPER);
                    fb.round_outline(b, 15, 2, INK, PAPER);
                    Self::heart(fb, b.x + 7, b.y + 8, 16, INK);
                    f.draw(fb, Face::Display, 24.0, b.x + 25, b.y + 24, INK, &s);
                    // Nível do monstro (acima do seu vale o dobro de experiência).
                    let lv = format!("nv {}", m.level);
                    let above = m.level > run.level;
                    let lw = f.measure(Face::Strong, 20.0, &lv) + 16;
                    let lb = Rect::new(r.x + 6, bar.y - 30, lw, 26);
                    fb.round_rect(lb, 13, if above { INK } else { PAPER });
                    f.draw(fb, Face::Strong, 20.0, lb.x + 8, lb.y + 19, if above { PAPER } else { INK }, &lv);
                    let a = format!("{}", m.atk);
                    let aw = f.measure(Face::Display, 24.0, &a);
                    let ab = Rect::new(r.x + r.w - aw - 40, r.y + 4, aw + 36, 30);
                    fb.round_rect(ab, 15, INK);
                    // Espadinha: a força do golpe dele.
                    let (sx, sy) = (ab.x + 9, ab.y + 23);
                    ui::stroke(fb, (sx as f32, sy as f32), (sx as f32 + 12.0, sy as f32 - 15.0), 3.0, PAPER);
                    ui::stroke(fb, (sx as f32 + 1.0, sy as f32 - 8.0), (sx as f32 + 8.0, sy as f32 - 1.0), 3.0, PAPER);
                    f.draw(fb, Face::Display, 24.0, ab.x + 26, ab.y + 24, PAPER, &a);
                    if m.key {
                        let kb = Rect::new(r.x + r.w - 58, bar.y - 30, 52, 26);
                        fb.round_rect(kb, 13, INK);
                        Self::key_icon(fb, kb.x + 6, kb.y + 2, PAPER, INK);
                    }
                }
            }
        }
    }

    fn draw_panel(&self, fb: &mut Framebuffer, f: &Fonts, run: &Run) {
        let y = GY + N as i32 * CELL + 24;
        Self::heart(fb, GX, y + 4, 32, INK);
        let end = f.draw(fb, Face::Display, 30.0, GX + 42, y + 30, INK, &format!("{}/{}", run.hp.max(0), run.max_hp));
        let mut x = end + 22;
        ui::stroke(fb, (x as f32, (y + 32) as f32), ((x + 22) as f32, (y + 6) as f32), 5.0, INK);
        ui::stroke(fb, (x as f32 + 2.0, (y + 18) as f32), (x as f32 + 13.0, (y + 30) as f32), 5.0, INK);
        x = f.draw(fb, Face::Display, 30.0, x + 28, y + 30, INK, &run.atk.to_string()) + 22;
        // Mana: losangos.
        let mp = format!("{}/{}", run.mp, run.max_mp);
        for k in 0..12 {
            fb.fill(Rect::new(x + 12 - k.min(12 - k), y + 6 + k * 2, 2 * k.min(12 - k) + 1, 2), INK);
        }
        x = f.draw(fb, Face::Display, 30.0, x + 30, y + 30, INK, &mp) + 22;
        let lv = format!("Nv {}", run.level);
        x = f.draw(fb, Face::Strong, 28.0, x, y + 30, INK, &lv) + 10;
        let xb = Rect::new(x, y + 16, 80, 8);
        fb.fill(xb, 0xDD);
        fb.fill(Rect::new(xb.x, xb.y, xb.w * run.xp / run.xp_next().max(1), xb.h), 0x5C);
        if run.key {
            Self::key_icon(fb, xb.x + xb.w + 16, y + 6, INK, PAPER);
        }
        let gold = run.gold.to_string();
        let gw = f.measure(Face::Display, 30.0, &gold);
        fb.round_outline(Rect::new(W - GX - gw - 40, y + 4, 30, 30), 15, 5, INK, 0xDD);
        f.draw(fb, Face::Display, 30.0, W - GX - gw, y + 30, INK, &gold);
        // Recado: a previsão da luta quando há monstro escolhido.
        let note = match (self.selected.filter(|_| run.phase == Phase::Play), self.aiming) {
            (_, Some(s)) => format!("{}: toque no monstro alvo.", s.name()),
            (Some(i), None) => match run.cells[i] {
                Cell::Mon(m) => {
                    let fc = run.forecast(&m);
                    let now = if fc.mon_after == 0 { "Este golpe mata".to_string() } else { format!("Golpe: ele fica com {}", fc.mon_after) };
                    let risk = if fc.take_total >= run.hp { " · você morreria" } else { "" };
                    format!("{} {} ({}). {now}; você leva {}. Até o fim: −{}{risk}. Toque de novo.", m.kind.name(), m.level, m.kind.trait_text(), fc.take_now, fc.take_total)
                }
                _ => run.log.clone(),
            },
            _ => run.log.clone(),
        };
        let lines = wrap(f, &note, W - 2 * GX);
        for (k, l) in lines.iter().take(2).enumerate() {
            f.draw(fb, Face::Body, 26.0, GX, y + 70 + k as i32 * 34, INK, l);
        }
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

/// Botões de baixo: poção e magias (até 3), mais Menu.
fn btn(k: usize, n: usize) -> Rect {
    let n = n as i32;
    let gap = 12;
    let w = (W - 2 * minigame::MARGIN - (n - 1) * gap) / n;
    Rect::new(minigame::MARGIN + k as i32 * (w + gap), 1330, w, 90)
}

impl MiniGame for Cripta {
    fn at_menu(&self) -> bool {
        self.screen == Screen::Menu
    }

    fn summary(&self) -> String {
        self.run.as_ref().filter(|_| self.screen == Screen::Game).map(|r| format!("Andar {} · vida {}/{} · nível {}", r.floor, r.hp, r.max_hp, r.level)).unwrap_or_default()
    }

    fn context(&self) -> String {
        let Some(r) = self.run.as_ref().filter(|_| self.screen == Screen::Game) else { return "No menu da Cripta.".into() };
        let grid: Vec<String> = (0..N)
            .map(|row| {
                (0..N)
                    .map(|c| {
                        let i = row * N + c;
                        match r.cells[i] {
                            Cell::Rock(_) => if r.reachable(i) { "?".to_string() } else { "#".into() },
                            Cell::Floor(Loot::Stairs) => ">".into(),
                            Cell::Floor(_) => if i == r.hero { "@".into() } else { ".".into() },
                            Cell::Mon(m) => format!("[{}{} v{} f{}{}]", &m.kind.name()[..2], m.level, m.hp, m.atk, if m.key { " chave" } else { "" }),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        format!(
            "Cripta (quebra-cabeça de masmorra), andar {} de {FLOORS}. Pedro: nível {}, vida {}/{}, força {}, mana {}/{}, poções {}, magias {}, chave {}. Regras: abrir rocha cura {} de vida e +1 mana, mas monstros feridos recuperam 1 (ogro 2); cada toque no monstro é uma troca de golpes (morcego e dragão atacam primeiro; esqueleto e dragão têm armadura 1); matar acima do seu nível dá o dobro de experiência; subir de nível enche vida e mana. Mapa (# rocha, ? rocha que dá para abrir, @ Pedro, > escada, [tipo nível vida força]):\n{}",
            r.floor,
            r.level,
            r.hp,
            r.max_hp,
            r.atk,
            r.mp,
            r.max_mp,
            r.potions,
            r.spells.iter().map(|s| s.name()).collect::<Vec<_>>().join(", "),
            if r.key { "sim" } else { "não" },
            r.heal(),
            grid.join("\n")
        )
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        let Some(run) = self.run.as_ref().filter(|_| self.screen == Screen::Game) else {
            let sub = if self.best > 0 { format!("Cinco andares, o dragão no fim. Recorde: {} pontos.", self.best) } else { "Cinco andares, o dragão no fim. Tudo à vista: planeje.".into() };
            let cont = self.run.as_ref().is_some_and(|r| r.phase == Phase::Play || r.phase == Phase::Reward);
            let opts: Vec<&str> = if cont { vec!["Continuar descida", "Nova descida", "Desafio do dia"] } else { vec!["Nova descida", "Desafio do dia"] };
            minigame::draw_menu(fb, f, "Cripta", &sub, &opts);
            let rules = [
                "Os monstros estão à vista, com vida e força.",
                "Abrir rocha cura você, mas os feridos também se recuperam.",
                "Cada toque no monstro é uma troca de golpes.",
                "Subir de nível enche tudo: use isso no momento certo.",
                "Mate o monstro da chave para descer; escolha um prêmio.",
            ];
            let top = minigame::menu_row(opts.len()).y + 30;
            for (k, l) in rules.iter().enumerate() {
                f.draw(fb, Face::Body, 30.0, minigame::MARGIN, top + k as i32 * 54, MUTED, l);
            }
            return;
        };
        let mode = if self.daily { " · desafio do dia" } else { "" };
        minigame::header(fb, f, "Cripta", &format!("Andar {} de {FLOORS} · {} pontos{mode}", run.floor, run.score()));
        self.draw_board(fb, f, run);
        self.draw_panel(fb, f, run);
        match run.phase {
            Phase::Reward => {
                // Cartões de prêmio sobre o tabuleiro.
                let bx = Rect::new(GX + 30, GY + 120, N as i32 * CELL - 60, 560);
                fb.round_rect(Rect::new(bx.x - 6, bx.y - 6, bx.w + 12, bx.h + 12), 30, PAPER);
                fb.round_rect(bx, 26, INK);
                f.draw(fb, Face::Display, 44.0, bx.x + 40, bx.y + 80, PAPER, "Escolha um prêmio");
                f.draw(fb, Face::Body, 28.0, bx.x + 40, bx.y + 124, 0xCC, &format!("Andar {} vencido. O próximo é mais forte.", run.floor));
                for (k, o) in run.offers.iter().enumerate() {
                    let r = Self::offer_rect(k);
                    fb.round_rect(r, r.h / 2, PAPER);
                    let t = o.text();
                    let w = f.measure(Face::Strong, 34.0, &t);
                    f.draw(fb, Face::Strong, 34.0, r.x + (r.w - w) / 2, r.y + r.h / 2 + 12, INK, &t);
                }
            }
            Phase::Dead | Phase::Won => {
                minigame::pill(fb, f, btn(0, 2), "Menu", false);
                minigame::pill(fb, f, btn(1, 2), "Nova descida", true);
            }
            Phase::Play => {
                let n = 2 + run.spells.len();
                minigame::pill(fb, f, btn(0, n), &format!("Poção {}", run.potions), run.potions > 0 && run.hp <= run.max_hp / 2);
                for (k, s) in run.spells.iter().enumerate() {
                    let label = format!("{} {}", s.name(), s.cost());
                    let ready = run.mp >= s.cost();
                    let r = btn(k + 1, n);
                    minigame::pill(fb, f, r, &label, self.aiming == Some(*s));
                    if !ready {
                        for yy in r.y..r.y + r.h {
                            for xx in r.x..r.x + r.w {
                                fb.blend(xx, yy, PAPER, 150);
                            }
                        }
                    }
                }
                minigame::pill(fb, f, btn(n - 1, n), "Menu", false);
            }
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        let Some(run) = self.run.as_ref().filter(|_| self.screen == Screen::Game) else {
            let cont = self.run.as_ref().is_some_and(|r| r.phase == Phase::Play || r.phase == Phase::Reward);
            return if cont { vec![(903, minigame::menu_row(0)), (900, minigame::menu_row(1)), (901, minigame::menu_row(2))] } else { vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))] };
        };
        match run.phase {
            Phase::Reward => (0..run.offers.len()).map(|k| (830 + k as u32, Self::offer_rect(k))).collect(),
            Phase::Dead | Phase::Won => vec![(803, btn(0, 2)), (902, btn(1, 2))],
            Phase::Play => {
                let n = 2 + run.spells.len();
                let mut v: Vec<(u32, Rect)> = (0..N * N).map(|i| (i as u32, Self::cell(i))).collect();
                v.push((800, btn(0, n)));
                for k in 0..run.spells.len() {
                    v.push((810 + k as u32, btn(k + 1, n)));
                }
                v.push((803, btn(n - 1, n)));
                v
            }
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let board = Rect::new(0, 170, W, 1448 - 170);
        match id {
            900 => {
                self.start(false);
                return Redraw::Full;
            }
            901 => {
                self.start(true);
                return Redraw::Full;
            }
            902 => {
                let d = self.daily;
                self.start(d);
                return Redraw::Full;
            }
            903 => {
                self.screen = Screen::Game;
                return Redraw::Full;
            }
            803 => {
                self.screen = Screen::Menu;
                return Redraw::Full;
            }
            _ => {}
        }
        let Some(run) = self.run.as_mut() else { return Redraw::None };
        match id {
            830..=832 => {
                run.choose((id - 830) as usize);
                self.selected = None;
                self.aiming = None;
                self.after_turn();
                Redraw::Full
            }
            800 => {
                run.drink();
                self.after_turn();
                Redraw::Part(board)
            }
            810..=812 => {
                let Some(&s) = run.spells.get((id - 810) as usize) else { return Redraw::None };
                if run.mp < s.cost() {
                    run.log = format!("{} custa {} de mana.", s.name(), s.cost());
                } else if s == Spell::Pedra {
                    run.cast(s, None);
                    self.after_turn();
                } else {
                    // Raio e Drenar: escolhe o alvo em seguida.
                    self.aiming = if self.aiming == Some(s) { None } else { Some(s) };
                    self.selected = None;
                }
                Redraw::Part(board)
            }
            i if (i as usize) < N * N && run.phase == Phase::Play => {
                let i = i as usize;
                if let Some(s) = self.aiming.take() {
                    if run.cast(s, Some(i)) {
                        self.after_turn();
                    } else {
                        run.log = "Alvo inválido: escolha um monstro que dá para alcançar.".into();
                    }
                    return Redraw::Part(board);
                }
                let prev = self.selected.take();
                match run.cells[i] {
                    Cell::Rock(_) if run.reachable(i) => {
                        run.reveal(i);
                        self.after_turn();
                    }
                    Cell::Rock(_) => run.log = "Longe demais: abra uma casa vizinha antes.".into(),
                    Cell::Mon(_) if !run.attackable(i) => {
                        self.selected = Some(i);
                    }
                    Cell::Mon(_) => {
                        if prev == Some(i) {
                            run.attack(i);
                            // Continua selecionado para o próximo golpe.
                            if matches!(run.cells[i], Cell::Mon(_)) {
                                self.selected = Some(i);
                            }
                            self.after_turn();
                        } else {
                            self.selected = Some(i);
                        }
                    }
                    Cell::Floor(Loot::Stairs) => {
                        if run.key {
                            run.descend(i);
                            self.after_turn();
                            return Redraw::Full;
                        }
                        run.log = "Trancada: falta a chave (está com o monstro marcado).".into();
                    }
                    _ => {}
                }
                Redraw::Part(board)
            }
            _ => Redraw::None,
        }
    }
}

impl Cripta {
    fn offer_rect(k: usize) -> Rect {
        Rect::new(GX + 70, GY + 260 + k as i32 * 130, N as i32 * CELL - 140, 100)
    }
}

// ---------- Robô: joga para medir o equilíbrio ----------

/// Estratégia simples e sensata: mata o que dá com folga, explora para se curar, usa magia para
/// finalizar, bebe poção quando fica fraco. Devolve a corrida terminada.
#[cfg(test)]
pub fn bot_play(seed: u64, smart: bool) -> Run {
    let mut r = Run::new(Rng::seeded(seed));
    for _ in 0..2000 {
        match r.phase {
            Phase::Won | Phase::Dead => break,
            Phase::Reward => {
                // Prefere força, depois vida.
                let k = r.offers.iter().position(|o| *o == Reward::Force).or_else(|| r.offers.iter().position(|o| *o == Reward::Life)).unwrap_or(0);
                r.choose(k);
                continue;
            }
            Phase::Play => {}
        }
        // Desce se puder.
        if r.key {
            if let Some(s) = (0..N * N).find(|&i| r.cells[i] == Cell::Floor(Loot::Stairs)) {
                r.descend(s);
                continue;
            }
        }
        let targets: Vec<(usize, Mon)> = (0..N * N).filter_map(|i| if let Cell::Mon(m) = r.cells[i] { r.attackable(i).then_some((i, m)) } else { None }).collect();
        if !smart {
            // Ingênuo: bate no primeiro que alcança enquanto aguenta o golpe; senão explora.
            if r.potions > 0 && r.hp * 3 < r.max_hp {
                r.drink();
                continue;
            }
            if let Some(&(i, _)) = targets.iter().find(|(_, m)| r.forecast(m).take_now < r.hp) {
                r.attack(i);
                continue;
            }
            if let Some(i) = (0..N * N).find(|&i| r.reachable(i)) {
                r.reveal(i);
                continue;
            }
            if let Some(&(i, _)) = targets.first() {
                r.attack(i);
                continue;
            }
            break;
        }
        // Magia de finalização.
        if r.spells.contains(&Spell::Raio) && r.mp >= 3 {
            if let Some(&(i, _)) = targets.iter().find(|(_, m)| m.hp <= 2 * r.level + 1 && m.level >= r.level) {
                r.cast(Spell::Raio, Some(i));
                continue;
            }
        }
        // Luta que dá para vencer com folga: a que mais rende experiência.
        if let Some(&(i, _)) = targets.iter().filter(|(_, m)| r.forecast(m).take_total < r.hp).max_by_key(|(_, m)| m.level * 10 - r.forecast(m).take_total) {
            r.attack(i);
            continue;
        }
        // Explorar (cura) enquanto houver rocha.
        if let Some(i) = (0..N * N).find(|&i| r.reachable(i)) {
            r.reveal(i);
            continue;
        }
        if r.potions > 0 && r.hp < r.max_hp {
            r.drink();
            continue;
        }
        // Sem saída: ataca o mais fraco e torce.
        if let Some(&(i, _)) = targets.iter().min_by_key(|(_, m)| m.hp) {
            r.attack(i);
            continue;
        }
        break;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn andar_tem_chefe_com_chave_e_escada() {
        let r = Run::new(Rng::seeded(1));
        assert_eq!(r.cells.iter().filter(|c| matches!(c, Cell::Mon(m) if m.key)).count(), 1);
        assert_eq!(r.cells.iter().filter(|c| **c == Cell::Rock(Loot::Stairs)).count(), 1);
        assert!(neighbors(r.hero).any(|j| r.reachable(j) || r.attackable(j)));
    }

    #[test]
    fn explorar_cura_e_monstros_feridos_se_recuperam() {
        let mut r = Run::new(Rng::seeded(2));
        let (mi, mut m) = r.cells.iter().enumerate().find_map(|(i, c)| if let Cell::Mon(m) = c { Some((i, *m)) } else { None }).unwrap();
        m.hp -= 2;
        r.cells[mi] = Cell::Mon(m);
        r.hp = 5;
        let i = (0..N * N).find(|&i| r.reachable(i)).unwrap();
        r.reveal(i);
        assert_eq!(r.hp, 6);
        let Cell::Mon(after) = r.cells[mi] else { panic!() };
        assert_eq!(after.hp, m.hp + m.kind.regen());
    }

    #[test]
    fn previsao_bate_com_a_luta() {
        let mut r = Run::new(Rng::seeded(3));
        r.atk = 3;
        r.hp = 50;
        r.max_hp = 50;
        let m = Mon::new(Kind::Morcego, 2, 1);
        r.cells = vec![Cell::Floor(Loot::Nothing); N * N];
        r.cells[1] = Cell::Mon(m);
        let fc = r.forecast(&m);
        let hp0 = r.hp;
        for _ in 0..fc.hits_to_kill {
            r.attack(1);
        }
        assert!(matches!(r.cells[1], Cell::Floor(_)), "morreu no número previsto de golpes");
        assert_eq!(hp0 - r.hp, fc.take_total);
    }

    #[test]
    fn nivel_acima_vale_o_dobro_e_cura() {
        let mut r = Run::new(Rng::seeded(4));
        r.hp = 1;
        let m = Mon::new(Kind::Goblin, 3, 1);
        r.gain_xp(&m);
        assert_eq!(r.level, 2);
        assert_eq!(r.hp, r.max_hp);
    }

    #[test]
    fn desafio_do_dia_se_repete() {
        let a = Run::new(Rng::seeded(99));
        let b = Run::new(Rng::seeded(99));
        assert_eq!(a.cells, b.cells);
    }

    /// Mede o equilíbrio com o robô: `cargo test equilibrio -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn equilibrio() {
        let n = 2000;
        for smart in [true, false] {
            let mut floors = [0u32; 7];
            let mut won = 0;
            let mut turns = 0u64;
            let mut lv = 0;
            for s in 0..n {
                let r = bot_play(s * 7 + 1, smart);
                if r.phase == Phase::Won {
                    won += 1;
                } else {
                    floors[r.floor as usize] += 1;
                }
                turns += r.turns as u64;
                lv += r.level;
            }
            eprintln!("robô {}: venceu {:.1}% · morreu no andar 1..5 = {:?} · turnos {} · nível final {:.1}", if smart { "esperto" } else { "ingênuo" }, 100.0 * won as f32 / n as f32, &floors[1..6], turns / n as u64, lv as f32 / n as f32);
        }
    }

    #[test]
    #[ignore]
    fn cripta_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        let shot = |g: &Cripta, name: &str| {
            let mut fb = crate::fb::Framebuffer::memory();
            g.draw(&mut fb, &f);
            fb.save_pgm(&out.join(name));
        };
        let mut g = Cripta::new();
        g.best = 0;
        shot(&g, "cripta-menu.pgm");
        g.start(true);
        let r = g.run.as_mut().unwrap();
        for _ in 0..6 {
            if let Some(i) = (0..N * N).find(|&i| r.reachable(i)) {
                r.reveal(i);
            }
        }
        r.hp = 8;
        if let Some(i) = (0..N * N).find(|&i| r.attackable(i)) {
            r.attack(i);
            g.selected = Some(i);
        }
        g.steps = 1;
        shot(&g, "cripta.pgm");
        let r = g.run.as_mut().unwrap();
        r.key = true;
        r.spells.push(Spell::Pedra);
        r.offers = vec![Reward::Force, Reward::Learn(Spell::Drenar), Reward::Potions];
        r.phase = Phase::Reward;
        shot(&g, "cripta-premio.pgm");
    }
}
