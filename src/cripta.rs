//! Cripta: roguelike parado, de decisões. Cada andar é uma grade coberta; você descobre casas
//! vizinhas às abertas gastando tochas. Casas vazias mostram quantos monstros há em volta.
//! Monstros não se mexem: bloqueiam a passagem até você decidir lutar, e o custo da luta aparece
//! antes. A escada está trancada; a chave fica com o monstro mais forte do andar.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 6;
const CELL: i32 = 136;
const GX: i32 = (W - N as i32 * CELL) / 2;
const GY: i32 = 316;
const NAMES: [&str; 5] = ["Rato", "Morcego", "Esqueleto", "Ogro", "Dragão"];
const TORCHES: i32 = 14;

fn name(k: u8) -> &'static str {
    NAMES[(k.clamp(1, 5) - 1) as usize]
}

/// Traço de cada monstro, para o Pedro decidir com quem lutar.
fn trait_of(k: u8) -> &'static str {
    match k {
        1 => "fraco",
        2 => "ataca primeiro",
        3 => "armadura 1",
        4 => "bate forte",
        _ => "chefe: ataca primeiro e aguenta muito",
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Thing {
    Empty,
    Monster(u8),
    Potion,
    Sword,
    Shield,
    Gold,
    Chest,
    Scroll,
    Stairs,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Menu,
    Play,
    LevelUp,
    Dead,
}

pub struct Cripta {
    state: State,
    daily: bool,
    rng: Rng,
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
    /// Escudo: absorve esse tanto de dano nas lutas.
    shield: i32,
    torches: i32,
    /// Casa do monstro com a chave; true quando a chave já é sua.
    key_at: usize,
    key: bool,
    /// O pergaminho mostrou onde estão a escada e a chave.
    mapped: bool,
    selected: Option<usize>,
    note: String,
    best: u32,
    hero: usize,
    steps: u32,
}

fn neighbors(i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = (i / N, i % N);
    [(r > 0).then(|| i - N), (r + 1 < N).then(|| i + N), (c > 0).then(|| i - 1), (c + 1 < N).then(|| i + 1)].into_iter().flatten()
}

fn around(i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = ((i / N) as i32, (i % N) as i32);
    (-1..=1)
        .flat_map(move |dr| (-1..=1).map(move |dc| (r + dr, c + dc)))
        .filter(move |&(rr, cc)| (rr, cc) != (r, c) && (0..N as i32).contains(&rr) && (0..N as i32).contains(&cc))
        .map(|(rr, cc)| rr as usize * N + cc as usize)
}

fn best_file() -> std::path::PathBuf {
    crate::home_dir().join("data/cripta_recorde.txt")
}

/// Vida, força, armadura e se ataca primeiro, do monstro de nível `k` no andar `floor`.
fn monster(k: u8, floor: u32) -> (i32, i32, i32, bool) {
    let (k, f) = (k as i32, floor as i32 - 1);
    let (mut hp, mut atk, mut armor, mut fast) = (2 + 3 * k + f, k + f / 2, 0, false);
    match k {
        2 => fast = true,
        3 => armor = 1,
        4 => atk += 2,
        5 => {
            hp += 4;
            fast = true;
        }
        _ => {}
    }
    (hp, atk.max(1), armor, fast)
}

impl Cripta {
    pub fn new() -> Self {
        let best = std::fs::read_to_string(best_file()).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        Cripta {
            state: State::Menu,
            daily: false,
            rng: Rng::new(),
            floor: 1,
            thing: vec![Thing::Empty; N * N],
            open: vec![false; N * N],
            hp: 12,
            max_hp: 12,
            atk: 2,
            level: 1,
            xp: 0,
            gold: 0,
            potions: 1,
            shield: 0,
            torches: TORCHES,
            key_at: 0,
            key: false,
            mapped: false,
            selected: None,
            note: String::new(),
            best,
            hero: 0,
            steps: 0,
        }
    }

    fn start(&mut self, daily: bool) {
        let best = self.best;
        *self = Cripta::new();
        self.best = best;
        self.daily = daily;
        if daily {
            self.rng = Rng::seeded(crate::fases::today() as u64 * 7919 + 1);
        }
        self.state = State::Play;
        self.build_floor();
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.rng.next() % n as u64) as usize
    }

    /// Monta o andar: entrada aberta, escada longe, chefe com a chave, monstros e itens.
    fn build_floor(&mut self) {
        let f = self.floor;
        loop {
            let mut thing = vec![Thing::Empty; N * N];
            let entry = self.pick(N * N);
            let dist = |i: usize| (i / N).abs_diff(entry / N) + (i % N).abs_diff(entry % N);
            let far: Vec<usize> = (0..N * N).filter(|&i| dist(i) >= 5).collect();
            if far.len() < 2 {
                continue;
            }
            let stairs = far[self.pick(far.len())];
            thing[stairs] = Thing::Stairs;
            let mut free: Vec<usize> = (0..N * N).filter(|&i| i != entry && i != stairs && dist(i) > 1).collect();
            self.rng.shuffle(&mut free);
            let mut it = free.into_iter();
            // Chefe do andar, com a chave: o mais forte (dragão a partir do 4º andar).
            let boss = if f >= 4 { 5 } else { (f as u8 + 2).min(4) };
            let key_at = it.next().unwrap();
            thing[key_at] = Thing::Monster(boss);
            for _ in 0..6 + f as usize {
                let top = (f as u8 + 1).min(4);
                let a = 1 + (self.rng.next() % top as u64) as u8;
                let b = 1 + (self.rng.next() % top as u64) as u8;
                thing[it.next().unwrap()] = Thing::Monster(a.min(b));
            }
            for t in [Thing::Potion, Thing::Potion, Thing::Sword, Thing::Shield, Thing::Gold, Thing::Gold, Thing::Chest, Thing::Scroll] {
                if let Some(i) = it.next() {
                    thing[i] = t;
                }
            }
            self.thing = thing;
            self.open = vec![false; N * N];
            self.open[entry] = true;
            self.hero = entry;
            self.key_at = key_at;
            self.key = false;
            self.mapped = false;
            self.torches = TORCHES;
            self.selected = None;
            self.note = format!("Andar {f}. A escada está trancada: a chave é do monstro mais forte.");
            return;
        }
    }

    fn passable(&self, i: usize) -> bool {
        self.open[i] && !matches!(self.thing[i], Thing::Monster(_))
    }

    fn reachable(&self, i: usize) -> bool {
        !self.open[i] && neighbors(i).any(|j| self.passable(j))
    }

    /// Monstros vivos nas 8 casas em volta (o número das casas vazias).
    fn danger(&self, i: usize) -> usize {
        around(i).filter(|&j| matches!(self.thing[j], Thing::Monster(_))).count()
    }

    /// Quanto de vida a luta custa e quanto o escudo segura.
    fn fight_cost(&self, k: u8) -> (i32, i32) {
        let (mhp, matk, armor, fast) = monster(k, self.floor);
        let dmg = (self.atk - armor).max(1);
        let rounds = (mhp + dmg - 1) / dmg;
        let raw = (rounds - 1 + fast as i32) * matk;
        let absorbed = raw.min(self.shield);
        (raw - absorbed, absorbed)
    }

    fn gain_xp(&mut self, k: u8) {
        self.xp += k as i32 * 2;
        if self.xp >= self.level * 5 {
            self.xp -= self.level * 5;
            self.level += 1;
            self.state = State::LevelUp;
            self.note = format!("Nível {}! Escolha: mais vida ou mais força.", self.level);
        }
    }

    fn die(&mut self, why: String) {
        self.hp = 0;
        self.state = State::Dead;
        let score = self.score();
        if score > self.best && !self.daily {
            self.best = score;
            let _ = std::fs::write(best_file(), score.to_string());
        }
        self.note = why;
    }

    fn reveal(&mut self, i: usize) {
        // Tocha: sem ela, abrir no escuro custa vida.
        if self.torches > 0 {
            self.torches -= 1;
        } else {
            self.hp -= 1;
            if self.hp <= 0 {
                return self.die("As tochas acabaram e o escuro venceu.".into());
            }
        }
        self.open[i] = true;
        self.steps += 1;
        if !matches!(self.thing[i], Thing::Monster(_)) {
            self.hero = i;
        }
        let dark = if self.torches == 0 { " Sem tochas: cada casa custa 1 de vida." } else { "" };
        self.note = match self.thing[i] {
            Thing::Empty => {
                let before = self.hp;
                self.hp = (self.hp + 1).min(self.max_hp);
                let around = match self.danger(i) {
                    0 => "Nenhum monstro em volta.".to_string(),
                    1 => "1 monstro em volta.".to_string(),
                    n => format!("{n} monstros em volta."),
                };
                format!("{around}{}{dark}", if self.hp > before { " +1 de vida." } else { "" })
            }
            Thing::Monster(k) => {
                let (mhp, matk, _, _) = monster(k, self.floor);
                let key = if i == self.key_at { " Está com a chave." } else { "" };
                format!("{} (vida {mhp}, força {matk}, {}).{key}", name(k), trait_of(k))
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
            Thing::Shield => {
                self.shield += 4;
                self.thing[i] = Thing::Empty;
                format!("Escudo: segura {} de dano.", self.shield)
            }
            Thing::Gold => {
                let g = 5 + 5 * self.floor as i32;
                self.gold += g;
                self.thing[i] = Thing::Empty;
                format!("{g} de ouro.")
            }
            Thing::Chest => {
                self.thing[i] = Thing::Empty;
                match self.pick(4) {
                    0 => {
                        self.potions += 1;
                        "Baú: uma poção.".into()
                    }
                    1 => {
                        self.shield += 3;
                        format!("Baú: escudo (segura {}).", self.shield)
                    }
                    2 => {
                        self.torches += 4;
                        "Baú: 4 tochas.".into()
                    }
                    _ => {
                        let g = 15 + 5 * self.floor as i32;
                        self.gold += g;
                        format!("Baú: {g} de ouro.")
                    }
                }
            }
            Thing::Scroll => {
                self.thing[i] = Thing::Empty;
                self.mapped = true;
                "Pergaminho: agora você sabe onde estão a escada e a chave.".into()
            }
            Thing::Stairs => {
                if self.key { "A escada! Toque nela para descer.".into() } else { "A escada, trancada. Falta a chave.".into() }
            }
        };
    }

    fn fight(&mut self, i: usize, k: u8) {
        let (cost, absorbed) = self.fight_cost(k);
        self.shield -= absorbed;
        self.hp -= cost;
        if self.hp <= 0 {
            return self.die(format!("{} venceu no andar {}.", name(k), self.floor));
        }
        self.thing[i] = Thing::Empty;
        self.hero = i;
        self.steps += 1;
        let key = if i == self.key_at {
            self.key = true;
            " Pegou a chave da escada!"
        } else {
            ""
        };
        let shield = if absorbed > 0 { format!(" O escudo segurou {absorbed}.") } else { String::new() };
        self.note = format!("Venceu {} perdendo {cost} de vida.{shield}{key}", name(k));
        self.gain_xp(k);
    }

    fn score(&self) -> u32 {
        (self.floor - 1) * 100 + self.gold as u32
    }

    fn cell(i: usize) -> Rect {
        Rect::new(GX + (i % N) as i32 * CELL, GY + (i / N) as i32 * CELL, CELL, CELL)
    }

    // ---------- Desenho ----------

    /// Casa fechada: textura de rocha (sprite) ou pedra lisa com relevo.
    fn rock(fb: &mut Framebuffer, r: Rect) {
        if crate::sprites::draw(fb, "pedra", r) {
            return;
        }
        fb.fill(r, 0x4C);
        fb.fill(Rect::new(r.x, r.y, r.w, 6), 0x66);
        fb.fill(Rect::new(r.x, r.y, 6, r.h), 0x66);
        fb.fill(Rect::new(r.x, r.y + r.h - 6, r.w, 6), 0x30);
        fb.fill(Rect::new(r.x + r.w - 6, r.y, 6, r.h), 0x30);
    }

    fn lighten(fb: &mut Framebuffer, r: Rect, a: u8) {
        for yy in r.y..r.y + r.h {
            for xx in r.x..r.x + r.w {
                fb.blend(xx, yy, PAPER, a);
            }
        }
    }

    fn heart(fb: &mut Framebuffer, x: i32, y: i32, s: i32, v: u8) {
        fb.round_rect(Rect::new(x, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        fb.round_rect(Rect::new(x + s / 2 - 2, y, s / 2 + 2, s / 2 + 2), s / 4 + 1, v);
        for k in 0..s / 2 {
            fb.fill(Rect::new(x + k, y + s / 4 + k, s - 2 * k, 1), v);
        }
    }

    fn key_icon(fb: &mut Framebuffer, x: i32, y: i32, v: u8) {
        fb.round_outline(Rect::new(x, y, 22, 22), 11, 5, v, PAPER);
        fb.fill(Rect::new(x + 20, y + 9, 22, 5), v);
        fb.fill(Rect::new(x + 34, y + 9, 5, 12), v);
    }

    fn torch_icon(fb: &mut Framebuffer, x: i32, y: i32, v: u8) {
        fb.fill(Rect::new(x + 8, y + 14, 7, 22), v);
        fb.round_rect(Rect::new(x + 2, y, 19, 18), 9, v);
        fb.round_rect(Rect::new(x + 8, y + 5, 7, 9), 3, PAPER);
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
            Thing::Shield => Some("escudo"),
            Thing::Gold => Some("ouro"),
            Thing::Chest => Some("bau"),
            Thing::Scroll => Some("pergaminho"),
            Thing::Stairs => Some("escada"),
            Thing::Empty => None,
        }
    }

    /// Desenho de reserva (sem sprite): formas simples.
    fn draw_fallback(fb: &mut Framebuffer, f: &Fonts, r: Rect, t: Thing) {
        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
        match t {
            Thing::Monster(k) => {
                let b = Rect::new(r.x + 18, r.y + 18, r.w - 36, r.h - 36);
                fb.round_rect(b, 22, INK);
                let label = name(k);
                let w = f.measure(Face::Strong, 22.0, label);
                f.draw(fb, Face::Strong, 22.0, b.x + (b.w - w) / 2, b.y + b.h / 2 + 8, PAPER, label);
            }
            Thing::Potion => {
                fb.round_rect(Rect::new(cx - 24, cy - 8, 48, 46), 22, INK);
                fb.fill(Rect::new(cx - 8, cy - 32, 16, 26), INK);
            }
            Thing::Sword => {
                ui::stroke(fb, (cx as f32 - 30.0, cy as f32 + 30.0), (cx as f32 + 32.0, cy as f32 - 32.0), 8.0, INK);
                ui::stroke(fb, (cx as f32 - 26.0, cy as f32 + 4.0), (cx as f32 - 4.0, cy as f32 + 26.0), 8.0, INK);
            }
            Thing::Shield => {
                fb.round_rect(Rect::new(cx - 28, cy - 34, 56, 50), 12, INK);
                for k in 0..24 {
                    fb.fill(Rect::new(cx - 28 + k, cy + 14 + k, 56 - 2 * k, 1), INK);
                }
            }
            Thing::Gold => {
                for (dx, dy) in [(-16, 10), (16, 10), (0, -14)] {
                    fb.round_outline(Rect::new(cx + dx - 20, cy + dy - 20, 40, 40), 20, 6, INK, 0xDD);
                }
            }
            Thing::Chest => {
                fb.round_rect(Rect::new(cx - 36, cy - 24, 72, 50), 8, INK);
                fb.fill(Rect::new(cx - 36, cy - 6, 72, 4), PAPER);
                fb.fill(Rect::new(cx - 5, cy - 10, 10, 14), PAPER);
            }
            Thing::Scroll => {
                fb.round_rect(Rect::new(cx - 30, cy - 26, 60, 52), 10, 0x88);
                fb.round_rect(Rect::new(cx - 36, cy - 30, 72, 14), 7, INK);
                fb.round_rect(Rect::new(cx - 36, cy + 18, 72, 14), 7, INK);
            }
            Thing::Stairs => {
                for k in 0..4 {
                    fb.fill(Rect::new(cx - 44 + k * 22, cy + 12 - k * 22, 88 - k * 22, 22), if k % 2 == 0 { INK } else { 0x66 });
                }
            }
            Thing::Empty => {}
        }
    }

    fn draw_board(&self, fb: &mut Framebuffer, f: &Fonts) {
        let frame = if self.steps % 2 == 0 { "1" } else { "2" };
        let (bx, by, bw) = (GX - 14, GY - 14, N as i32 * CELL + 28);
        fb.round_rect(Rect::new(bx, by, bw, bw), 18, 0x22);
        for i in 0..N * N {
            let r = Self::cell(i);
            let inner = Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4);
            if !self.open[i] {
                Self::rock(fb, inner);
                if self.state == State::Play && self.reachable(i) {
                    Self::lighten(fb, inner, 140);
                    fb.round_outline(Rect::new(inner.x + 4, inner.y + 4, inner.w - 8, inner.h - 8), 10, 4, INK, 0xFF);
                    let core = Rect::new(inner.x + 8, inner.y + 8, inner.w - 16, inner.h - 16);
                    Self::rock(fb, core);
                    Self::lighten(fb, core, 150);
                }
                // Pergaminho: escada e chave aparecem por baixo da rocha.
                if self.mapped && (i == self.key_at || self.thing[i] == Thing::Stairs) {
                    let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                    fb.round_rect(Rect::new(cx - 28, cy - 28, 56, 56), 28, PAPER);
                    if i == self.key_at {
                        Self::key_icon(fb, cx - 20, cy - 11, INK);
                    } else {
                        for k in 0..3 {
                            fb.fill(Rect::new(cx - 18 + k * 12, cy + 10 - k * 10, 36 - k * 12, 8), INK);
                        }
                    }
                }
                continue;
            }
            if !crate::sprites::draw(fb, "piso", inner) {
                fb.fill(inner, 0xEE);
                fb.round_outline(Rect::new(inner.x + 6, inner.y + 6, inner.w - 12, inner.h - 12), 8, 2, 0xD4, 0xEE);
            }
            let t = self.thing[i];
            let art = Rect::new(r.x + 10, r.y + 10, r.w - 20, r.h - 20);
            let sprite = |fb: &mut Framebuffer| {
                Self::sprite_of(t).is_some_and(|n| {
                    let name = if matches!(t, Thing::Monster(_)) { format!("{n}-{frame}") } else { n.to_string() };
                    crate::sprites::draw(fb, &name, art)
                })
            };
            if self.selected == Some(i) {
                fb.round_outline(Rect::new(r.x - 2, r.y - 2, r.w + 4, r.h + 4), 12, 8, INK, 0xEE);
            }
            if !sprite(fb) {
                Self::draw_fallback(fb, f, r, t);
            }
            if let Thing::Monster(k) = t {
                // Selo do nível; a chave em cima do chefe.
                let b = Rect::new(r.x + r.w - 44, r.y + 6, 38, 38);
                fb.round_rect(b, 19, INK);
                let num = k.to_string();
                let w = f.measure(Face::Display, 26.0, &num);
                f.draw(fb, Face::Display, 26.0, b.x + (b.w - w) / 2, b.y + 28, PAPER, &num);
                if i == self.key_at {
                    fb.round_rect(Rect::new(r.x + 4, r.y + 6, 50, 32), 16, PAPER);
                    Self::key_icon(fb, r.x + 8, r.y + 11, INK);
                }
            } else if t == Thing::Empty && i != self.hero {
                // Número de perigo (monstros nas 8 casas em volta).
                let d = self.danger(i);
                if d > 0 {
                    let num = d.to_string();
                    let w = f.measure(Face::Display, 44.0, &num);
                    f.draw(fb, Face::Display, 44.0, r.x + (r.w - w) / 2, r.y + r.h / 2 + 16, if d >= 3 { INK } else { 0x55 }, &num);
                }
            }
            if i == self.hero && !matches!(t, Thing::Monster(_)) && !crate::sprites::draw(fb, &format!("heroi-{frame}"), art) {
                let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
                fb.round_rect(Rect::new(cx - 14, cy - 34, 28, 28), 14, INK);
                fb.round_rect(Rect::new(cx - 24, cy - 2, 48, 38), 16, INK);
            }
        }
    }

    fn draw_panel(&self, fb: &mut Framebuffer, f: &Fonts) {
        let y = GY + N as i32 * CELL + 26;
        Self::heart(fb, GX, y + 4, 34, INK);
        let end = f.draw(fb, Face::Display, 32.0, GX + 44, y + 32, INK, &format!("{}/{}", self.hp.max(0), self.max_hp));
        let bar = Rect::new(end + 12, y + 14, 150, 14);
        fb.round_outline(bar, 7, 2, INK, PAPER);
        fb.round_rect(Rect::new(bar.x, bar.y, (bar.w * self.hp.max(0) / self.max_hp.max(1)).max(14), bar.h), 7, INK);
        let mut x = bar.x + bar.w + 26;
        ui::stroke(fb, (x as f32, (y + 34) as f32), ((x + 24) as f32, (y + 6) as f32), 5.0, INK);
        ui::stroke(fb, (x as f32 + 2.0, (y + 20) as f32), (x as f32 + 14.0, (y + 32) as f32), 5.0, INK);
        x = f.draw(fb, Face::Display, 32.0, x + 30, y + 32, INK, &self.atk.to_string()) + 22;
        if self.shield > 0 {
            fb.round_rect(Rect::new(x, y + 4, 24, 22), 6, INK);
            for k in 0..10 {
                fb.fill(Rect::new(x + k, y + 24 + k, 24 - 2 * k, 1), INK);
            }
            x = f.draw(fb, Face::Display, 32.0, x + 30, y + 32, INK, &self.shield.to_string()) + 22;
        }
        Self::torch_icon(fb, x, y, if self.torches > 0 { INK } else { 0xAA });
        x = f.draw(fb, Face::Display, 32.0, x + 28, y + 32, if self.torches > 0 { INK } else { MUTED }, &self.torches.to_string()) + 22;
        if self.key {
            Self::key_icon(fb, x, y + 8, INK);
        }
        let gold = self.gold.to_string();
        let gw = f.measure(Face::Display, 32.0, &gold);
        fb.round_outline(Rect::new(W - GX - gw - 42, y + 6, 30, 30), 15, 5, INK, 0xDD);
        f.draw(fb, Face::Display, 32.0, W - GX - gw, y + 32, INK, &gold);
        // Experiência: barra fina sob o painel.
        let xb = Rect::new(GX, y + 50, W - 2 * GX, 6);
        fb.fill(xb, 0xDD);
        fb.fill(Rect::new(xb.x, xb.y, xb.w * self.xp / (self.level * 5).max(1), xb.h), 0x5C);
        let note = crate::apps::fit(f, Face::Body, 28.0, &self.note, W - 2 * GX);
        f.draw(fb, Face::Body, 28.0, GX, y + 96, INK, &note);
    }
}

impl MiniGame for Cripta {
    fn at_menu(&self) -> bool {
        self.state == State::Menu
    }

    fn summary(&self) -> String {
        if self.state == State::Menu {
            String::new()
        } else {
            format!("Andar {} · vida {}/{} · tochas {}", self.floor, self.hp, self.max_hp, self.torches)
        }
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
                            Thing::Empty => char::from_digit(self.danger(i) as u32, 10).unwrap_or('.'),
                            Thing::Monster(k) => (b'a' + k - 1) as char,
                            Thing::Potion => 'p',
                            Thing::Sword => 's',
                            Thing::Shield => 'e',
                            Thing::Gold => '$',
                            Thing::Chest => 'b',
                            Thing::Scroll => 'm',
                            Thing::Stairs => '>',
                        }
                    })
                    .collect()
            })
            .collect();
        format!(
            "Cripta, andar {}: vida {}/{}, força {}, escudo {}, tochas {}, nível {}, ouro {}, poções {}, chave: {}. Monstros: a rato, b morcego (ataca primeiro), c esqueleto (armadura 1), d ogro (bate forte), e dragão (chefe). Casas vazias mostram quantos monstros há nas 8 em volta. Mapa (# coberta, ? dá para abrir, número = perigo, p poção, s espada, e escudo, $ ouro, b baú, m pergaminho, > escada):\n{}",
            self.floor,
            self.hp,
            self.max_hp,
            self.atk,
            self.shield,
            self.torches,
            self.level,
            self.gold,
            self.potions,
            if self.key { "sim" } else { "não" },
            grid.join("\n")
        )
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.state == State::Menu {
            let sub = if self.best > 0 { format!("Desça o mais fundo que puder. Recorde: {} pontos.", self.best) } else { "Desça o mais fundo que puder. Nada se mexe sem você.".into() };
            minigame::draw_menu(fb, f, "Cripta", &sub, &["Descida livre", "Desafio do dia"]);
            let rules = [
                "Cada casa aberta gasta uma tocha; sem tochas, custa vida.",
                "Casas vazias mostram quantos monstros há em volta.",
                "Toque no monstro para ver o custo; de novo para lutar.",
                "A escada está trancada: a chave é do monstro mais forte.",
                "Desafio do dia: a mesma cripta para todo mundo hoje.",
            ];
            for (k, l) in rules.iter().enumerate() {
                f.draw(fb, Face::Body, 30.0, minigame::MARGIN, 830 + k as i32 * 56, MUTED, l);
            }
            return;
        }
        let mode = if self.daily { " · desafio do dia" } else { "" };
        minigame::header(fb, f, "Cripta", &format!("Andar {} · {} pontos{mode}", self.floor, self.score()));
        self.draw_board(fb, f);
        self.draw_panel(fb, f);
        match self.state {
            State::Dead => {
                minigame::pill(fb, f, minigame::bottom_button(0, 2), "Menu", false);
                minigame::pill(fb, f, minigame::bottom_button(1, 2), "Nova descida", true);
            }
            State::LevelUp => {
                minigame::pill(fb, f, minigame::bottom_button(0, 2), "+4 de vida", true);
                minigame::pill(fb, f, minigame::bottom_button(1, 2), "+1 de força", true);
            }
            _ => {
                let p = format!("Beber poção ({})", self.potions);
                minigame::pill(fb, f, minigame::bottom_button(0, 2), &p, self.potions > 0 && self.hp < self.max_hp);
                minigame::pill(fb, f, minigame::bottom_button(1, 2), "Menu", false);
            }
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.state {
            State::Menu => vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))],
            State::Dead => vec![(803, minigame::bottom_button(0, 2)), (902, minigame::bottom_button(1, 2))],
            State::LevelUp => vec![(820, minigame::bottom_button(0, 2)), (821, minigame::bottom_button(1, 2))],
            State::Play => {
                let mut v: Vec<(u32, Rect)> = (0..N * N).map(|i| (i as u32, Self::cell(i))).collect();
                v.push((800, minigame::bottom_button(0, 2)));
                v.push((803, minigame::bottom_button(1, 2)));
                v
            }
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let board = Rect::new(0, 170, W, 1448 - 170);
        match id {
            900 => {
                self.start(false);
                Redraw::Full
            }
            901 => {
                self.start(true);
                Redraw::Full
            }
            902 => {
                let d = self.daily;
                self.start(d);
                Redraw::Full
            }
            803 => {
                self.state = State::Menu;
                Redraw::Full
            }
            820 | 821 => {
                if id == 820 {
                    self.max_hp += 4;
                } else {
                    self.atk += 1;
                }
                self.hp = self.max_hp;
                self.state = State::Play;
                self.note = if id == 820 { format!("Vida máxima {}.", self.max_hp) } else { format!("Força {}.", self.atk) };
                Redraw::Part(board)
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
                            let (cost, absorbed) = self.fight_cost(k);
                            self.selected = Some(i);
                            let shield = if absorbed > 0 { format!(" (o escudo segura {absorbed})") } else { String::new() };
                            self.note = if cost >= self.hp {
                                format!("{}: custaria {cost} de vida{shield}. Você morreria. Toque de novo se quiser.", name(k))
                            } else {
                                format!("{}: custa {cost} de vida{shield}. Toque de novo para lutar.", name(k))
                            };
                        }
                    }
                    Thing::Stairs if self.key => {
                        self.floor += 1;
                        self.build_floor();
                        return Redraw::Full;
                    }
                    Thing::Stairs => {
                        let boss = match self.thing[self.key_at] {
                            Thing::Monster(k) => name(k),
                            _ => "chefe",
                        };
                        self.note = format!("Trancada. A chave está com o {}.", boss.to_lowercase());
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
    fn andar_tem_escada_chefe_e_entrada() {
        let mut g = Cripta::new();
        g.start(false);
        assert_eq!(g.thing.iter().filter(|t| **t == Thing::Stairs).count(), 1);
        assert!(matches!(g.thing[g.key_at], Thing::Monster(_)));
        assert_eq!(g.open.iter().filter(|o| **o).count(), 1);
        let entry = g.open.iter().position(|o| *o).unwrap();
        assert!(neighbors(entry).all(|j| g.reachable(j)));
    }

    #[test]
    fn tracos_mudam_o_custo() {
        let mut g = Cripta::new();
        g.start(false);
        g.atk = 2;
        // Rato no andar 1: vida 5 → 3 golpes → 2 de dano.
        assert_eq!(g.fight_cost(1), (2, 0));
        // Morcego ataca primeiro: um golpe a mais.
        let (hp2, atk2, _, _) = monster(2, 1);
        assert_eq!(g.fight_cost(2).0, ((hp2 + 1) / 2) * atk2);
        // Esqueleto com armadura: o dano do herói cai de 2 para 1.
        let (hp3, atk3, _, _) = monster(3, 1);
        assert_eq!(g.fight_cost(3).0, (hp3 - 1) * atk3);
        g.shield = 3;
        assert_eq!(g.fight_cost(1), (0, 2));
    }

    #[test]
    fn tochas_acabam_e_cobram_vida() {
        let mut g = Cripta::new();
        g.start(false);
        g.torches = 0;
        let hp = g.hp;
        let i = (0..N * N).find(|&i| g.reachable(i)).unwrap();
        g.thing[i] = Thing::Gold;
        g.reveal(i);
        assert_eq!(g.hp, hp - 1);
    }

    #[test]
    fn escada_so_com_a_chave_e_desafio_do_dia_se_repete() {
        let mut a = Cripta::new();
        a.start(true);
        let mut b = Cripta::new();
        b.start(true);
        assert_eq!(a.thing, b.thing);
        let s = a.thing.iter().position(|t| *t == Thing::Stairs).unwrap();
        a.open[s] = true;
        for j in neighbors(s) {
            a.open[j] = true;
            a.thing[j] = Thing::Empty;
        }
        a.tap(s as u32, 0, 0);
        assert_eq!(a.floor, 1, "sem a chave não desce");
        a.key = true;
        a.tap(s as u32, 0, 0);
        assert_eq!(a.floor, 2);
    }

    #[test]
    fn subir_de_nivel_pede_escolha() {
        let mut g = Cripta::new();
        g.start(false);
        g.hp = 3;
        g.gain_xp(3);
        assert_eq!(g.state, State::LevelUp);
        g.tap(821, 0, 0);
        assert_eq!((g.atk, g.hp, g.state), (3, g.max_hp, State::Play));
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
        g.start(true);
        for _ in 0..8 {
            if let Some(i) = (0..N * N).find(|&i| g.reachable(i)) {
                g.reveal(i);
            }
        }
        g.mapped = true;
        g.shield = 4;
        g.selected = (0..N * N).find(|&i| g.open[i] && matches!(g.thing[i], Thing::Monster(_)));
        let mut fb = crate::fb::Framebuffer::memory();
        g.draw(&mut fb, &f);
        fb.save_pgm(&out.join("cripta.pgm"));
        let mut m = Cripta::new();
        let mut fb = crate::fb::Framebuffer::memory();
        m.draw(&mut fb, &f);
        fb.save_pgm(&out.join("cripta-menu.pgm"));
        m.start(false);
        m.state = State::LevelUp;
        let mut fb = crate::fb::Framebuffer::memory();
        m.draw(&mut fb, &f);
        fb.save_pgm(&out.join("cripta-nivel.pgm"));
    }
}
