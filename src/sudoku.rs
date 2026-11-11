//! Sudoku: gerador com solução única, três níveis, jogo salvo e conferência.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MARGIN: i32 = 72;
const W: i32 = 1072;
pub const CELL: i32 = 96;
pub const GRID_X: i32 = (W - 9 * CELL) / 2;
pub const GRID_Y: i32 = 300;

/// Gerador pseudoaleatório simples (xorshift), semeado pelo relógio.
pub struct Rng(u64);
impl Rng {
    pub fn new() -> Self {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Rng(t | 1)
    }
    /// Sorteio que se repete (desafio do dia: a mesma partida para a mesma semente).
    pub fn seeded(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = (self.next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    }
}

fn ok(g: &[u8; 81], i: usize, n: u8) -> bool {
    let (r, c) = (i / 9, i % 9);
    for k in 0..9 {
        if g[r * 9 + k] == n || g[k * 9 + c] == n {
            return false;
        }
    }
    let (br, bc) = (r / 3 * 3, c / 3 * 3);
    for dr in 0..3 {
        for dc in 0..3 {
            if g[(br + dr) * 9 + bc + dc] == n {
                return false;
            }
        }
    }
    true
}

fn fill(g: &mut [u8; 81], rng: &mut Rng) -> bool {
    let Some(i) = g.iter().position(|&v| v == 0) else { return true };
    let mut nums = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
    rng.shuffle(&mut nums);
    for n in nums {
        if ok(g, i, n) {
            g[i] = n;
            if fill(g, rng) {
                return true;
            }
            g[i] = 0;
        }
    }
    false
}

/// Conta soluções até `limit` (para garantir solução única).
fn count(g: &mut [u8; 81], limit: u32) -> u32 {
    let Some(i) = g.iter().position(|&v| v == 0) else { return 1 };
    let mut total = 0;
    for n in 1..=9 {
        if ok(g, i, n) {
            g[i] = n;
            total += count(g, limit - total);
            g[i] = 0;
            if total >= limit {
                break;
            }
        }
    }
    total
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Easy,
    Medium,
    Hard,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Easy => "Fácil",
            Level::Medium => "Médio",
            Level::Hard => "Difícil",
        }
    }
    fn holes(self) -> usize {
        match self {
            Level::Easy => 38,
            Level::Medium => 48,
            Level::Hard => 54,
        }
    }
}


pub const LEVELS: [Level; 3] = [Level::Easy, Level::Medium, Level::Hard];

impl Level {
    fn from_name(s: &str) -> Option<Level> {
        LEVELS.into_iter().find(|l| l.name() == s)
    }
}

/// Uma jogada, para o desfazer: casa, valor e anotações de antes (e das casas que perderam anotação).
#[derive(Clone)]
struct Undo {
    cells: Vec<(usize, u8, u16)>,
}

pub struct Game {
    pub solution: [u8; 81],
    pub given: [bool; 81],
    pub cells: [u8; 81],
    /// Anotações a lápis: bit n-1 = número n.
    pub notes: [u16; 81],
    pub selected: Option<usize>,
    pub level: Level,
    pub notes_mode: bool,
    pub mistakes: u32,
    pub hints: u32,
    /// Fase (0 = treino livre).
    pub fase: u32,
    /// Segundos jogados antes desta abertura; o relógio desta sessão soma por cima.
    pub played: u64,
    pub resumed: std::time::Instant,
    undo: Vec<Undo>,
}

/// Casas vazias da fase: de 34 (bem fácil) até 52; difícil +4; desafio no máximo que der.
pub fn fase_holes(k: u32) -> usize {
    let base = 34 + (18.0 * crate::fases::ramp(k, 30)) as usize;
    match crate::fases::kind(k) {
        crate::fases::Kind::Normal => base,
        crate::fases::Kind::Hard => (base + 4).min(56),
        crate::fases::Kind::Challenge => 60,
    }
}

fn level_for_holes(h: usize) -> Level {
    match h {
        0..=40 => Level::Easy,
        41..=50 => Level::Medium,
        _ => Level::Hard,
    }
}

impl Game {
    pub fn new(level: Level) -> Self {
        Self::with_holes(level, level.holes(), 0)
    }

    pub fn new_fase(k: u32) -> Self {
        let h = fase_holes(k);
        Self::with_holes(level_for_holes(h), h, k)
    }

    /// Nome para mostrar: "Fase 12" ou o nível do treino.
    pub fn label(&self) -> String {
        if self.fase > 0 { crate::fases::title(self.fase) } else { self.level.name().to_string() }
    }

    fn with_holes(level: Level, holes: usize, fase: u32) -> Self {
        let mut rng = Rng::new();
        let mut sol = [0u8; 81];
        fill(&mut sol, &mut rng);
        let mut puzzle = sol;
        let mut order: Vec<usize> = (0..81).collect();
        rng.shuffle(&mut order);
        let mut removed = 0;
        for i in order {
            if removed >= holes {
                break;
            }
            let keep = puzzle[i];
            puzzle[i] = 0;
            let mut probe = puzzle;
            if count(&mut probe, 2) == 1 {
                removed += 1;
            } else {
                puzzle[i] = keep;
            }
        }
        let given = std::array::from_fn(|i| puzzle[i] != 0);
        Game {
            solution: sol,
            given,
            cells: puzzle,
            notes: [0; 81],
            selected: None,
            level,
            notes_mode: false,
            mistakes: 0,
            hints: 0,
            fase,
            played: 0,
            resumed: std::time::Instant::now(),
            undo: Vec::new(),
        }
    }

    pub fn seconds(&self) -> u64 {
        self.played + self.resumed.elapsed().as_secs()
    }

    pub fn solved(&self) -> bool {
        self.cells == self.solution
    }

    pub fn filled(&self) -> usize {
        self.cells.iter().filter(|&&v| v != 0).count()
    }

    /// O número `n` já está em todas as 9 casas certas?
    pub fn complete(&self, n: u8) -> bool {
        (0..81).filter(|&i| self.cells[i] == n && self.solution[i] == n).count() == 9
    }

    pub fn wrong(&self, i: usize) -> bool {
        self.cells[i] != 0 && self.cells[i] != self.solution[i]
    }

    fn peers(i: usize) -> impl Iterator<Item = usize> {
        (0..81).filter(move |&j| j != i && (j / 9 == i / 9 || j % 9 == i % 9 || (j / 27 == i / 27 && j % 9 / 3 == i % 9 / 3)))
    }

    /// Toque numa tecla: número (0 apaga) ou anotação, conforme o modo.
    pub fn press(&mut self, n: u8) {
        let Some(i) = self.selected else { return };
        if self.given[i] {
            return;
        }
        let mut u = Undo { cells: vec![(i, self.cells[i], self.notes[i])] };
        if self.notes_mode && n != 0 {
            if self.cells[i] != 0 {
                return;
            }
            self.notes[i] ^= 1 << (n - 1);
        } else {
            if self.cells[i] == n {
                return;
            }
            self.cells[i] = n;
            self.notes[i] = 0;
            if n != 0 {
                if n != self.solution[i] {
                    self.mistakes += 1;
                }
                // Tira o número das anotações da linha, coluna e bloco.
                for j in Self::peers(i) {
                    if self.notes[j] & (1 << (n - 1)) != 0 {
                        u.cells.push((j, self.cells[j], self.notes[j]));
                        self.notes[j] &= !(1 << (n - 1));
                    }
                }
            }
        }
        self.undo.push(u);
    }

    pub fn undo(&mut self) -> bool {
        let Some(u) = self.undo.pop() else { return false };
        for (i, v, notes) in u.cells.into_iter().rev() {
            self.cells[i] = v;
            self.notes[i] = notes;
        }
        true
    }

    /// Dica: preenche a casa escolhida (ou a primeira vazia/errada) com o número certo.
    pub fn hint(&mut self) -> Option<usize> {
        let i = self.selected.filter(|&i| !self.given[i] && self.cells[i] != self.solution[i]).or_else(|| (0..81).find(|&i| self.cells[i] != self.solution[i]))?;
        self.selected = Some(i);
        let mode = self.notes_mode;
        self.notes_mode = false;
        let before = self.mistakes;
        self.press(self.solution[i]);
        self.mistakes = before;
        self.notes_mode = mode;
        self.hints += 1;
        Some(i)
    }

    fn path() -> std::path::PathBuf {
        crate::home_dir().join("data/sudoku.txt")
    }

    pub fn save(&self) {
        let enc = |a: &[u8; 81]| a.iter().map(|d| (b'0' + d) as char).collect::<String>();
        let given: String = self.given.iter().map(|&g| if g { '1' } else { '0' }).collect();
        let notes: String = self.notes.iter().map(|n| format!("{n:03x}")).collect();
        let _ = std::fs::write(
            Self::path(),
            format!("{}\n{}\n{}\n{}\n{}\n{} {} {}\nfase {}\n", self.level.name(), enc(&self.solution), given, enc(&self.cells), notes, self.seconds(), self.mistakes, self.hints, self.fase),
        );
    }

    pub fn load() -> Option<Self> {
        let s = std::fs::read_to_string(Self::path()).ok()?;
        let l: Vec<&str> = s.lines().collect();
        let level = Level::from_name(l.first()?)?;
        let dec = |t: &str| -> Option<[u8; 81]> {
            let v: Vec<u8> = t.bytes().map(|b| b.wrapping_sub(b'0')).collect();
            (v.len() == 81 && v.iter().all(|&d| d <= 9)).then(|| std::array::from_fn(|i| v[i]))
        };
        let solution = dec(l.get(1)?)?;
        let given: Vec<bool> = l.get(2)?.chars().map(|c| c == '1').collect();
        let cells = dec(l.get(3)?)?;
        let notes_s = l.get(4).copied().unwrap_or("");
        let notes: [u16; 81] = std::array::from_fn(|i| notes_s.get(i * 3..i * 3 + 3).and_then(|h| u16::from_str_radix(h, 16).ok()).unwrap_or(0));
        let nums: Vec<u64> = l.get(5).map(|x| x.split_whitespace().filter_map(|n| n.parse().ok()).collect()).unwrap_or_default();
        if given.len() != 81 {
            return None;
        }
        Some(Game {
            solution,
            given: std::array::from_fn(|i| given[i]),
            cells,
            notes,
            selected: None,
            level,
            notes_mode: false,
            mistakes: nums.get(1).copied().unwrap_or(0) as u32,
            hints: nums.get(2).copied().unwrap_or(0) as u32,
            fase: l.get(6).and_then(|t| t.strip_prefix("fase ")).and_then(|n| n.trim().parse().ok()).unwrap_or(0),
            played: nums.first().copied().unwrap_or(0),
            resumed: std::time::Instant::now(),
            undo: Vec::new(),
        })
    }

    pub fn clear_saved() {
        let _ = std::fs::remove_file(Self::path());
    }
}

// ---------- Nível preferido e recordes ----------

pub fn last_level() -> Level {
    std::fs::read_to_string(crate::home_dir().join("data/sudoku-nivel.txt")).ok().and_then(|s| Level::from_name(s.trim())).unwrap_or(Level::Medium)
}

pub fn set_last_level(l: Level) {
    let _ = std::fs::write(crate::home_dir().join("data/sudoku-nivel.txt"), l.name());
}

fn records_path() -> std::path::PathBuf {
    crate::home_dir().join("data/sudoku-recordes.txt")
}

pub fn record(l: Level) -> Option<u64> {
    let s = std::fs::read_to_string(records_path()).ok()?;
    s.lines().find_map(|line| {
        let (name, secs) = line.rsplit_once(' ')?;
        (name == l.name()).then(|| secs.parse().ok()).flatten()
    })
}

/// Registra o tempo; devolve true se for recorde novo.
pub fn submit_record(l: Level, secs: u64) -> bool {
    let best = record(l);
    if best.is_some_and(|b| b <= secs) {
        return false;
    }
    let mut lines: Vec<String> = std::fs::read_to_string(records_path()).unwrap_or_default().lines().filter(|x| !x.starts_with(l.name())).map(str::to_string).collect();
    lines.push(format!("{} {secs}", l.name()));
    let _ = std::fs::write(records_path(), lines.join("\n"));
    true
}

pub fn mmss(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}h{:02}", secs / 3600, secs % 3600 / 60)
    } else {
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

// ---------- Desenho ----------

pub fn cell_rect(i: usize) -> Rect {
    Rect::new(GRID_X + (i % 9) as i32 * CELL, GRID_Y + (i / 9) as i32 * CELL, CELL, CELL)
}

/// Teclado numérico: 1–9 e apagar (0).
pub fn key_rect(n: u8) -> Rect {
    let (w, gap) = (84, 8);
    let total = 10 * w + 9 * gap;
    let x0 = (W - total) / 2;
    let k = if n == 0 { 9 } else { n as i32 - 1 };
    Rect::new(x0 + k * (w + gap), GRID_Y + 9 * CELL + 28, w, 100)
}

/// Barra de ações: Anotar, Dica, Desfazer, Novo.
pub fn action_rect(k: usize) -> Rect {
    let w = (W - 2 * MARGIN - 3 * 16) / 4;
    Rect::new(MARGIN + k as i32 * (w + 16), 1322, w, 96)
}
pub fn continue_rect() -> Rect {
    Rect::new(MARGIN, 520, W - 2 * MARGIN, 140)
}
fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    fb.fill(r, PAPER);
    let fg = if filled {
        fb.round_rect(r, r.h / 2, INK);
        PAPER
    } else {
        fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        INK
    };
    let size = if f.measure(Face::Strong, 38.0, label) > r.w - 30 { 30.0 } else { 38.0 };
    let tw = f.measure(Face::Strong, size, label);
    f.draw(fb, Face::Strong, size, r.x + (r.w - tw) / 2, r.y + r.h / 2 + (size * 0.36) as i32, fg, label);
}

/// Abrir o Sudoku com uma partida pela metade: continuar ou começar outra.
pub fn draw_resume(fb: &mut Framebuffer, f: &Fonts, saved: Option<&Game>, prog: &crate::fases::Progress, practice: bool) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, if practice { "Treino" } else { "Sudoku" });
    if practice {
        f.draw(fb, Face::Strong, 34.0, MARGIN, 420, MUTED, "Escolha o nível. Não conta para as fases.");
        for (k, lv) in LEVELS.iter().enumerate() {
            let label = match record(*lv) {
                Some(s) => format!("{} · recorde {}", lv.name(), mmss(s)),
                None => lv.name().to_string(),
            };
            pill(fb, f, resume_level_rect(k, false), &label, false);
        }
        return;
    }
    if let Some(g) = saved {
        f.draw(fb, Face::Strong, 34.0, MARGIN, 420, MUTED, "Você tem uma partida pela metade");
        let r = continue_rect();
        fb.round_rect(r, 50, INK);
        f.draw(fb, Face::Display, 56.0, r.x + 44, r.y + 70, PAPER, "Continuar");
        let sub = format!("{} · {} de 81 · {}", g.label(), g.filled(), mmss(g.seconds()));
        f.draw(fb, Face::Body, 30.0, r.x + 46, r.y + 116, 0xCC, &sub);
        f.draw(fb, Face::Strong, 30.0, MARGIN, 740, MUTED, "OU COMECE");
    } else {
        f.draw(fb, Face::Strong, 34.0, MARGIN, 420, MUTED, "A dificuldade sobe a cada fase vencida");
    }
    let below = saved.is_some();
    pill(fb, f, resume_level_rect(0, below), &format!("Fase {}", prog.level), !below);
    pill(fb, f, resume_level_rect(1, below), "Treino livre", false);
    let streak = prog.current_streak(crate::fases::today());
    let y = resume_level_rect(1, below).y + 200;
    f.draw(fb, Face::Display, 56.0, MARGIN, y, INK, &format!("Sequência: {streak} dia{}", if streak == 1 { "" } else { "s" }));
    f.draw(fb, Face::Body, 30.0, MARGIN, y + 60, MUTED, &format!("Melhor: {} · Fases vencidas: {} · de 5 em 5, uma difícil", prog.best, prog.solved));
}

/// Botões do menu: abaixo do "Continuar" quando há partida salva; senão, logo no alto.
pub fn resume_level_rect(k: usize, below_continue: bool) -> Rect {
    Rect::new(MARGIN, if below_continue { 780 } else { 470 } + k as i32 * 140, W - 2 * MARGIN, 116)
}

pub fn draw_status(fb: &mut Framebuffer, f: &Fonts, g: &Game, note: &str) {
    let r = Rect::new(0, 170, W, GRID_Y - 176);
    fb.fill(r, PAPER);
    let mins = g.seconds() / 60;
    let time = if mins == 0 { "menos de 1 min".to_string() } else { format!("{mins} min") };
    let errs = match g.mistakes {
        0 => "sem erros".to_string(),
        1 => "1 erro".to_string(),
        n => format!("{n} erros"),
    };
    f.draw(fb, Face::Display, 48.0, GRID_X, 228, INK, &format!("{} · {time} · {errs}", g.label()));
    f.draw(fb, Face::Body, 30.0, GRID_X, 272, MUTED, note);
}

pub fn draw_board(fb: &mut Framebuffer, f: &Fonts, g: &Game, note: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    draw_status(fb, f, g, note);
    for i in 0..81 {
        draw_cell(fb, f, g, i);
    }
    draw_grid_lines(fb);
    draw_keys(fb, f, g);
    draw_actions(fb, f, g);
}

pub fn draw_keys(fb: &mut Framebuffer, f: &Fonts, g: &Game) {
    for n in 0..=9u8 {
        let r = key_rect(n);
        fb.fill(r, PAPER);
        // Número já completo: tecla apagada.
        let done = n != 0 && g.complete(n);
        fb.round_outline(r, 18, if done { 2 } else { 4 }, if done { 0xBB } else { INK }, PAPER);
        if n == 0 {
            let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
            ui::stroke(fb, (cx - 16.0, cy - 16.0), (cx + 16.0, cy + 16.0), 6.0, INK);
            ui::stroke(fb, (cx + 16.0, cy - 16.0), (cx - 16.0, cy + 16.0), 6.0, INK);
        } else {
            let s = n.to_string();
            let face = if g.notes_mode { Face::Body } else { Face::Display };
            let size = if g.notes_mode { 40.0 } else { 56.0 };
            let w = f.measure(face, size, &s);
            f.draw(fb, face, size, r.x + (r.w - w) / 2, r.y + if g.notes_mode { 64 } else { 70 }, if done { 0xBB } else { INK }, &s);
        }
    }
}

pub fn draw_actions(fb: &mut Framebuffer, f: &Fonts, g: &Game) {
    let labels = [if g.notes_mode { "Anotando" } else { "Anotar" }, "Dica", "Desfazer", "Novo"];
    for (k, l) in labels.iter().enumerate() {
        pill(fb, f, action_rect(k), l, k == 0 && g.notes_mode);
    }
}

pub fn draw_cell(fb: &mut Framebuffer, f: &Fonts, g: &Game, i: usize) {
    let r = cell_rect(i);
    let inner = Rect::new(r.x + 3, r.y + 3, r.w - 6, r.h - 6);
    let sel = g.selected == Some(i);
    // Destaque leve para a linha, a coluna e o bloco da casa escolhida, e mais forte para o mesmo número.
    let related = g.selected.is_some_and(|s| s / 9 == i / 9 || s % 9 == i % 9 || (s / 27 == i / 27 && s % 9 / 3 == i % 9 / 3));
    let same = g.selected.is_some_and(|s| g.cells[s] != 0 && g.cells[s] == g.cells[i]);
    let bg = if sel { INK } else if same { 0xC8 } else if related { 0xEA } else { PAPER };
    fb.fill(inner, bg);
    let v = g.cells[i];
    let fg = |dark: u8| if sel { PAPER } else { dark };
    if v != 0 {
        let s = v.to_string();
        let (face, ink) = if g.given[i] { (Face::Display, fg(INK)) } else { (Face::Body, fg(0x33)) };
        let w = f.measure(face, 62.0, &s);
        f.draw(fb, face, 62.0, r.x + (r.w - w) / 2, r.y + 71, ink, &s);
        if g.wrong(i) {
            // Erro marcado na hora com um traço embaixo do número.
            fb.fill(Rect::new(r.x + 26, r.y + r.h - 16, r.w - 52, 6), fg(INK));
        }
    } else if g.notes[i] != 0 {
        for n in 1..=9u8 {
            if g.notes[i] & (1 << (n - 1)) != 0 {
                let (cx, cy) = (r.x + 18 + ((n - 1) % 3) as i32 * 30, r.y + 30 + ((n - 1) / 3) as i32 * 30);
                let s = n.to_string();
                let w = f.measure(Face::Strong, 24.0, &s);
                f.draw(fb, Face::Strong, 24.0, cx - w / 2 + 12, cy, fg(0x44), &s);
            }
        }
    }
}

pub fn draw_grid_lines(fb: &mut Framebuffer) {
    for k in 0..=9 {
        let t = if k % 3 == 0 { 6 } else { 2 };
        fb.fill(Rect::new(GRID_X + k * CELL - t / 2, GRID_Y - 3, t, 9 * CELL + 6), INK);
        fb.fill(Rect::new(GRID_X - 3, GRID_Y + k * CELL - t / 2, 9 * CELL + 6, t), INK);
    }
}

/// Vitória: tempo, erros, dicas e recorde do nível.
pub fn draw_won(fb: &mut Framebuffer, f: &Fonts, g: &Game, secs: u64, new_record: bool, prog: &crate::fases::Progress) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 360, INK, "Resolvido!");
    let sub = if g.fase > 0 {
        let streak = prog.current_streak(crate::fases::today());
        format!("{} · sequência de {streak} dia{}", g.label(), if streak == 1 { "" } else { "s" })
    } else {
        format!("Sudoku {}", g.level.name())
    };
    f.draw(fb, Face::Strong, 40.0, MARGIN, 440, MUTED, &sub);
    let rows = [
        ("TEMPO", mmss(secs)),
        ("ERROS", g.mistakes.to_string()),
        ("DICAS", g.hints.to_string()),
        (
            "RECORDE",
            if new_record {
                "este é o novo!".to_string()
            } else if g.hints > 0 {
                "não vale com dicas".to_string()
            } else {
                record(g.level).map(mmss).unwrap_or_default()
            },
        ),
    ];
    for (k, (label, value)) in rows.iter().enumerate() {
        let y = 540 + k as i32 * 110;
        fb.fill(Rect::new(MARGIN, y, W - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Strong, 30.0, MARGIN, y + 66, MUTED, label);
        let vw = f.measure(Face::Display, 56.0, value);
        f.draw(fb, Face::Display, 56.0, W - MARGIN - vw, y + 74, INK, value);
    }
    let next = if g.fase > 0 { format!("Fase {}", prog.level) } else { "Jogo novo".to_string() };
    pill(fb, f, won_new_rect(), &next, true);
}

pub fn won_new_rect() -> Rect {
    Rect::new(MARGIN, 1150, W - 2 * MARGIN, 120)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gera_jogo_valido_com_solucao_unica() {
        let g = Game::new(Level::Medium);
        let mut p = g.cells;
        assert_eq!(count(&mut p, 2), 1);
        for i in 0..81 {
            if g.given[i] {
                assert_eq!(g.cells[i], g.solution[i]);
            }
        }
        assert!(81 - g.filled() >= 40);
    }

    #[test]
    fn anotar_errar_desfazer_e_dica() {
        let mut g = Game::new(Level::Easy);
        let i = (0..81).find(|&i| !g.given[i]).unwrap();
        let right = g.solution[i];
        let wrong = right % 9 + 1;
        g.selected = Some(i);
        g.notes_mode = true;
        g.press(right);
        assert_eq!(g.notes[i], 1 << (right - 1));
        g.notes_mode = false;
        g.press(wrong);
        assert_eq!((g.cells[i], g.mistakes), (wrong, 1));
        assert!(g.undo());
        assert_eq!((g.cells[i], g.notes[i]), (0, 1 << (right - 1)));
        g.hint();
        assert_eq!((g.cells[i], g.hints, g.mistakes), (right, 1, 1));
        // Dois jogos seguidos não são iguais.
        assert_ne!(Game::new(Level::Easy).cells, Game::new(Level::Easy).cells);
    }
}
