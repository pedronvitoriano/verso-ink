//! Campo minado: primeiro toque sempre seguro, áreas vazias abrem sozinhas, modo bandeira.

use crate::fb::{Framebuffer, Rect};
use crate::fases;
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, PAPER};

const LEVELS: [(usize, usize, &str); 3] = [(9, 10, "Fácil · 9×9, 10 minas"), (12, 24, "Médio · 12×12, 24 minas"), (14, 40, "Difícil · 14×14, 40 minas")];

pub struct Minas {
    menu: bool,
    n: usize,
    mines: usize,
    mine: Vec<bool>,
    open: Vec<bool>,
    flag: Vec<bool>,
    placed: bool,
    flag_mode: bool,
    lost: Option<usize>,
    won: bool,
    started: std::time::Instant,
    /// Tempo da partida, parado na vitória ou na mina (segundos).
    took: Option<u64>,
    /// Fase em jogo (0 = treino livre).
    fase: u32,
    practice: bool,
    prog: fases::Progress,
}

/// Tamanho e minas da fase: cresce devagar; difícil e desafio são maiores e mais densos.
fn fase_params(k: u32) -> (usize, usize) {
    let mut n = (8 + (k.saturating_sub(1) / 3) as usize).min(13);
    let mut d = 0.12 + 0.04 * fases::ramp(k, 30);
    match fases::kind(k) {
        fases::Kind::Hard => {
            n = (n + 1).min(14);
            d += 0.025;
        }
        fases::Kind::Challenge => {
            n = (n + 2).min(14);
            d += 0.04;
        }
        fases::Kind::Normal => {}
    }
    (n, ((n * n) as f32 * d).round() as usize)
}

impl Minas {
    pub fn new() -> Self {
        Minas {
            menu: true,
            n: 0,
            mines: 0,
            mine: vec![],
            open: vec![],
            flag: vec![],
            placed: false,
            flag_mode: false,
            lost: None,
            won: false,
            started: std::time::Instant::now(),
            took: None,
            fase: 0,
            practice: false,
            prog: fases::Progress::load("minas"),
        }
    }

    fn start(&mut self, n: usize, mines: usize) {
        self.menu = false;
        self.practice = false;
        self.n = n;
        self.mines = mines;
        self.mine = vec![false; n * n];
        self.open = vec![false; n * n];
        self.flag = vec![false; n * n];
        self.placed = false;
        self.flag_mode = flag_pref();
        self.lost = None;
        self.won = false;
        self.started = std::time::Instant::now();
        self.took = None;
        self.fase = 0;
    }

    fn start_fase(&mut self) {
        let k = self.prog.level;
        let (n, m) = fase_params(k);
        self.start(n, m);
        self.fase = k;
    }

    fn neighbors(&self, i: usize) -> Vec<usize> {
        let n = self.n as i32;
        let (r, c) = ((i / self.n) as i32, (i % self.n) as i32);
        let mut v = Vec::new();
        for dr in -1..=1 {
            for dc in -1..=1 {
                let (a, b) = (r + dr, c + dc);
                if (dr, dc) != (0, 0) && (0..n).contains(&a) && (0..n).contains(&b) {
                    v.push((a * n + b) as usize);
                }
            }
        }
        v
    }

    fn count(&self, i: usize) -> usize {
        self.neighbors(i).iter().filter(|&&j| self.mine[j]).count()
    }

    /// Minas sorteadas depois do primeiro toque, longe dele (3×3 livre).
    fn place(&mut self, first: usize) {
        let mut rng = Rng::new();
        let mut safe = self.neighbors(first);
        safe.push(first);
        let mut left = self.mines;
        while left > 0 {
            let i = (rng.next() % (self.n * self.n) as u64) as usize;
            if !self.mine[i] && !safe.contains(&i) {
                self.mine[i] = true;
                left -= 1;
            }
        }
        self.placed = true;
    }

    fn reveal(&mut self, i: usize) {
        let mut stack = vec![i];
        while let Some(j) = stack.pop() {
            if self.open[j] || self.flag[j] {
                continue;
            }
            self.open[j] = true;
            if self.count(j) == 0 && !self.mine[j] {
                stack.extend(self.neighbors(j));
            }
        }
    }

    fn geom(&self) -> (i32, i32, i32) {
        minigame::grid_geom(self.n as i32, self.n as i32, 290, 940)
    }

    fn cell(&self, i: usize) -> Rect {
        let (x0, y0, s) = self.geom();
        Rect::new(x0 + (i % self.n) as i32 * s, y0 + (i / self.n) as i32 * s, s, s)
    }
}

/// Preferência do toque (guardada): `true` = tocar põe bandeira.
fn flag_pref() -> bool {
    std::fs::read_to_string(crate::home_dir().join("settings/minas_toque")).is_ok_and(|s| s.trim() == "bandeira")
}

impl Minas {
    fn toggle_flag(&mut self, i: usize) -> Redraw {
        if self.open[i] {
            return Redraw::None;
        }
        self.flag[i] = !self.flag[i];
        Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
    }

    fn open_cell(&mut self, i: usize) -> Redraw {
        if self.flag[i] || self.open[i] {
            return Redraw::None;
        }
        if !self.placed {
            self.place(i);
            // O relógio começa no primeiro toque, não ao abrir o jogo.
            self.started = std::time::Instant::now();
        }
        if self.mine[i] {
            self.lost = Some(i);
            self.took = Some(self.started.elapsed().as_secs());
            return Redraw::Full;
        }
        self.reveal(i);
        self.won = (0..self.n * self.n).all(|j| self.mine[j] || self.open[j]);
        if self.won {
            self.took = Some(self.started.elapsed().as_secs());
        }
        if self.won && self.fase > 0 {
            self.prog.record_win(self.fase, fases::today());
        }
        if self.won { Redraw::Full } else { Redraw::Part(Rect::new(0, 170, W, 1448 - 170)) }
    }
}

/// Tempo em minutos e segundos (ou só segundos, se for menos de um minuto).
fn mmss(secs: u64) -> String {
    if secs < 60 { format!("{secs} s") } else { format!("{}:{:02}", secs / 60, secs % 60) }
}

impl MiniGame for Minas {
    /// Bandeira mais rápida: segurar só um instante já marca (toque rápido continua abrindo).
    fn hold_ms(&self) -> u64 {
        280
    }

    fn risky(&self, id: u32) -> bool {
        id == 801 && self.fase == 0 && !self.won && self.lost.is_none() && self.placed
    }

    fn summary(&self) -> String {
        if self.menu { String::new() } else { format!("{}×{} · {} minas", self.n, self.n, self.mines) }
    }

    fn context(&self) -> String {
        if self.menu {
            return "No menu do Campo minado.".into();
        }
        let n = self.n;
        let rows: Vec<String> = (0..n)
            .map(|r| {
                (0..n)
                    .map(|c| {
                        let i = r * n + c;
                        if self.flag[i] {
                            'F'
                        } else if !self.open[i] {
                            '#'
                        } else {
                            match self.count(i) {
                                0 => '.',
                                k => (b'0' + k as u8) as char,
                            }
                        }
                    })
                    .collect()
            })
            .collect();
        format!("Campo minado {n}×{n}, {} minas (# fechada, F bandeira, número = minas vizinhas).\n{}", self.mines, rows.join("\n"))
    }

    fn at_menu(&self) -> bool {
        self.menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu && self.practice {
            let opts: Vec<&str> = LEVELS.iter().map(|l| l.2).collect();
            minigame::draw_menu(fb, f, "Treino", "Escolha o tamanho. Não conta para as fases.", &opts);
            return;
        }
        if self.menu {
            let main = format!("Fase {}", self.prog.level);
            fases::draw_menu(fb, f, "Minas", "Abra tudo que não é mina. Números contam as vizinhas.", &self.prog, &main, &["Treino livre"]);
            return;
        }
        let flags = self.flag.iter().filter(|&&x| x).count();
        let status = if self.won {
            format!("Campo limpo em {}!", mmss(self.took.unwrap_or(0)))
        } else if self.lost.is_some() {
            format!("Boom. Era uma mina. ({})", mmss(self.took.unwrap_or(0)))
        } else {
            format!("{} minas · {} bandeira{}", self.mines, flags, if flags == 1 { "" } else { "s" })
        };
        let title = if self.fase > 0 { fases::title(self.fase) } else { "Campo minado".to_string() };
        minigame::header(fb, f, &title, &status);
        let (_, _, s) = self.geom();
        let size = s as f32 * 0.6;
        for i in 0..self.n * self.n {
            let r = self.cell(i);
            let inner = Rect::new(r.x + 2, r.y + 2, r.w - 4, r.h - 4);
            let show_mine = self.mine[i] && (self.lost.is_some() || self.won);
            if self.open[i] || show_mine {
                fb.fill(inner, if Some(i) == self.lost { INK } else { PAPER });
                fb.round_outline(inner, 4, 1, 0xBB, if Some(i) == self.lost { INK } else { PAPER });
                if self.mine[i] {
                    let m = s / 8;
                    if !crate::sprites::draw(fb, "peca-mina", Rect::new(r.x + m, r.y + m, s - 2 * m, s - 2 * m)) {
                        let d = (s / 4).max(6);
                        fb.round_rect(Rect::new(r.x + s / 2 - d, r.y + s / 2 - d, 2 * d, 2 * d), d, if Some(i) == self.lost { PAPER } else { INK });
                    }
                } else {
                    let k = self.count(i);
                    if k > 0 {
                        let t = k.to_string();
                        let w = f.measure(Face::Display, size, &t);
                        f.draw(fb, Face::Display, size, r.x + (s - w) / 2, r.y + s / 2 + (size * 0.36) as i32, if k >= 3 { INK } else { 0x44 }, &t);
                    }
                }
            } else {
                fb.round_rect(inner, 6, 0xC8);
                if self.flag[i] && crate::sprites::draw(fb, "peca-bandeira", Rect::new(r.x + s / 7, r.y + s / 7, s - 2 * (s / 7), s - 2 * (s / 7))) {
                } else if self.flag[i] {
                    let (x, y) = (r.x as f32 + s as f32 * 0.38, r.y as f32 + s as f32 * 0.22);
                    ui::stroke(fb, (x, y), (x, y + s as f32 * 0.56), 4.0, INK);
                    for k in 0..6 {
                        let yy = y + k as f32 * s as f32 * 0.04;
                        ui::stroke(fb, (x, yy), (x + s as f32 * 0.3, y + s as f32 * 0.12), 3.0, INK);
                    }
                }
            }
        }
        if self.fase > 0 && self.won {
            return fases::draw_win(fb, f, &self.prog);
        }
        if self.fase > 0 && self.lost.is_some() {
            return fases::draw_retry(fb, f, self.fase);
        }
        // A troca: por padrão toque abre e segurar põe bandeira; invertido, o contrário.
        let mode = if self.flag_mode { "Inverter: toque abre" } else { "Inverter: toque marca" };
        minigame::pill(fb, f, minigame::bottom_button(0, 2), mode, self.flag_mode);
        minigame::pill(fb, f, minigame::bottom_button(1, 2), if self.fase > 0 { "Menu" } else { "Novo jogo" }, false);
        minigame::hint(fb, f, if self.flag_mode { "Invertido: toque põe bandeira, segurar abre." } else { "Toque abre, segurar põe bandeira." });
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu && self.practice {
            return (0..LEVELS.len()).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.menu {
            return vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))];
        }
        if self.fase > 0 && (self.won || self.lost.is_some()) {
            return fases::win_targets();
        }
        let mut v = vec![(800u32, minigame::bottom_button(0, 2)), (801, minigame::bottom_button(1, 2))];
        if self.lost.is_none() && !self.won {
            v.extend((0..self.n * self.n).filter(|&i| !self.open[i]).map(|i| (i as u32, self.cell(i))));
        }
        v
    }

    /// Segurar faz o contrário do toque: abre no modo bandeira, marca no modo abrir.
    fn back(&mut self) -> bool {
        // Só o submenu de treino volta ao menu do jogo; da partida, Início sai e o app guarda
        // a partida para continuar depois.
        if self.menu && self.practice {
            self.practice = false;
            return true;
        }
        false
    }

    fn hold(&mut self, id: u32) -> Option<Redraw> {
        let i = id as usize;
        if self.menu || self.lost.is_some() || self.won || i >= self.n * self.n {
            return None;
        }
        Some(if self.flag_mode { self.open_cell(i) } else { self.toggle_flag(i) })
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        let all = Rect::new(0, 170, W, 1448 - 170);
        match id {
            900 | fases::WIN_NEXT => {
                // Vitória já avançou a fase; derrota repete a mesma.
                self.start_fase();
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            fases::WIN_MENU => {
                self.menu = true;
                Redraw::Full
            }
            910..=912 => {
                let (n, m, _) = LEVELS[(id - 910) as usize];
                self.start(n, m);
                Redraw::Full
            }
            800 => {
                self.flag_mode = !self.flag_mode;
                let _ = std::fs::write(crate::home_dir().join("settings/minas_toque"), if self.flag_mode { "bandeira" } else { "abrir" });
                Redraw::Part(all)
            }
            801 => {
                self.menu = true;
                Redraw::Full
            }
            i if (i as usize) < self.n * self.n => {
                let i = i as usize;
                if self.flag_mode { self.toggle_flag(i) } else { self.open_cell(i) }
            }
            _ => Redraw::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vencer_a_fase_avanca() {
        let file = std::env::temp_dir().join(format!("minas-fase-{}.txt", std::process::id()));
        let mut g = Minas { prog: fases::Progress::load_from(file.clone()), ..Minas::new() };
        g.tap(900, 0, 0);
        assert_eq!((g.fase, g.n), (1, fase_params(1).0));
        g.tap(0, 0, 0);
        for i in 0..g.n * g.n {
            if !g.mine[i] && !g.open[i] {
                g.tap(i as u32, 0, 0);
            }
        }
        assert!(g.won);
        assert_eq!(g.prog.level, 2);
        assert_eq!(fases::Progress::load_from(file.clone()).level, 2);
        assert_eq!(g.targets().len(), 2); // Menu e Fase 2
        g.tap(fases::WIN_NEXT, 0, 0);
        assert_eq!(g.fase, 2);
        let _ = std::fs::remove_file(file);
    }
    #[test]
    fn primeiro_toque_seguro_e_abre_vazio() {
        let mut g = Minas::new();
        g.start(9, 10);
        g.tap(40, 0, 0);
        assert!(!g.mine[40] && g.open[40]);
        assert_eq!(g.mine.iter().filter(|&&m| m).count(), 10);
        assert!(g.neighbors(40).iter().all(|&j| !g.mine[j]));
    }
}
