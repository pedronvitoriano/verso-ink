//! Cruzadinha diária gerada sem rede. As palavras usam letras sem acento na grade;
//! o progresso de cada dia fica em data/cruzadas/<dia>.txt.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

const N: usize = 11;
const CELLS: usize = N * N;
const SOURCE: &str = include_str!("../assets/cruzadas.txt");

#[derive(Clone, Debug)]
struct Entry {
    word: String,
    clue: String,
}

fn parse(source: &str) -> Vec<Entry> {
    let mut seen = std::collections::HashSet::new();
    source.lines().filter_map(|line| {
        let (word, clue) = line.split_once(';')?;
        let word = word.trim();
        let clue = clue.trim();
        if !(3..=N).contains(&word.len()) || !word.bytes().all(|c| c.is_ascii_uppercase()) || clue.is_empty() || !seen.insert(word.to_string()) {
            return None;
        }
        Some(Entry { word: word.into(), clue: clue.into() })
    }).collect()
}

#[derive(Clone, Debug)]
struct Placement {
    entry: Entry,
    row: usize,
    col: usize,
    vertical: bool,
}

impl Placement {
    fn cell(&self, k: usize) -> usize {
        (self.row + if self.vertical { k } else { 0 }) * N + self.col + if self.vertical { 0 } else { k }
    }
    fn contains(&self, cell: usize) -> bool {
        (0..self.entry.word.len()).any(|k| self.cell(k) == cell)
    }
}

#[derive(Clone)]
struct Puzzle {
    solution: [u8; CELLS],
    owners: [u8; CELLS],
    words: Vec<Placement>,
}

impl Puzzle {
    fn empty() -> Self {
        Self { solution: [0; CELLS], owners: [0; CELLS], words: Vec::new() }
    }

    fn fits(&self, word: &str, row: usize, col: usize, vertical: bool, needs_crossing: bool) -> bool {
        let len = word.len();
        if (vertical && row + len > N) || (!vertical && col + len > N) {
            return false;
        }
        let at = |r: usize, c: usize| self.solution[r * N + c] != 0;
        if vertical {
            if row > 0 && at(row - 1, col) || row + len < N && at(row + len, col) { return false; }
        } else if col > 0 && at(row, col - 1) || col + len < N && at(row, col + len) { return false; }
        let mut crossings = 0;
        let direction = if vertical { 2 } else { 1 };
        for (k, ch) in word.bytes().enumerate() {
            let r = row + if vertical { k } else { 0 };
            let c = col + if vertical { 0 } else { k };
            let i = r * N + c;
            if self.solution[i] != 0 {
                if self.solution[i] != ch || self.owners[i] & direction != 0 { return false; }
                crossings += 1;
            } else if vertical {
                if c > 0 && at(r, c - 1) || c + 1 < N && at(r, c + 1) { return false; }
            } else if r > 0 && at(r - 1, c) || r + 1 < N && at(r + 1, c) { return false; }
        }
        !needs_crossing || crossings > 0
    }

    fn add(&mut self, entry: Entry, row: usize, col: usize, vertical: bool) {
        let p = Placement { entry, row, col, vertical };
        for (k, ch) in p.entry.word.bytes().enumerate() {
            let i = p.cell(k);
            self.solution[i] = ch;
            self.owners[i] |= if vertical { 2 } else { 1 };
        }
        self.words.push(p);
    }
}

fn generate(day: i64) -> Puzzle {
    let mut pool = parse(SOURCE);
    let mut rng = Rng::seeded(day as u64 ^ 0x4352_555a_4144_4153);
    rng.shuffle(&mut pool);
    let mut puzzle = Puzzle::empty();
    if let Some(first) = pool.pop() {
        let col = (N - first.word.len()) / 2;
        puzzle.add(first, N / 2, col, false);
    }
    while puzzle.words.len() < 14 {
        let mut chosen = None;
        for (idx, entry) in pool.iter().enumerate() {
            let mut positions = Vec::new();
            for cell in 0..CELLS {
                let existing = puzzle.solution[cell];
                if existing == 0 { continue; }
                for (k, ch) in entry.word.bytes().enumerate() {
                    if ch != existing { continue; }
                    for vertical in [false, true] {
                        let (r, c) = (cell / N, cell % N);
                        let (row, col) = if vertical {
                            if r < k { continue; }
                            (r - k, c)
                        } else {
                            if c < k { continue; }
                            (r, c - k)
                        };
                        if puzzle.fits(&entry.word, row, col, vertical, true) { positions.push((row, col, vertical)); }
                    }
                }
            }
            if !positions.is_empty() {
                let pos = positions[(rng.next() as usize) % positions.len()];
                chosen = Some((idx, pos));
                break;
            }
        }
        let Some((idx, (row, col, vertical))) = chosen else { break };
        puzzle.add(pool.remove(idx), row, col, vertical);
    }
    puzzle
}

fn normalize(s: &str) -> String {
    s.nfkd().filter(|c| !is_combining_mark(*c)).flat_map(char::to_uppercase).filter(|c| c.is_ascii_uppercase()).collect()
}

fn decode_progress(text: &str, solution: &[u8; CELLS]) -> [u8; CELLS] {
    let mut grid = [0; CELLS];
    let bytes = text.trim().as_bytes();
    if bytes.len() != CELLS { return grid; }
    for (i, &ch) in bytes.iter().enumerate() {
        if solution[i] != 0 && ch.is_ascii_uppercase() { grid[i] = ch; }
    }
    grid
}

pub struct Cruzadas {
    menu: bool,
    day: i64,
    puzzle: Puzzle,
    grid: [u8; CELLS],
    selected: usize,
    selected_cell: usize,
    want_text: bool,
    checked: bool,
    message: String,
}

impl Cruzadas {
    pub fn new() -> Self {
        Self { menu: true, day: 0, puzzle: Puzzle::empty(), grid: [0; CELLS], selected: 0, selected_cell: 0, want_text: false, checked: false, message: String::new() }
    }

    fn path(&self) -> std::path::PathBuf {
        crate::home_dir().join("data/cruzadas").join(format!("{}.txt", self.day))
    }

    fn start(&mut self) {
        self.start_on(crate::data::today(crate::offset_min()));
    }

    fn start_on(&mut self, day: i64) {
        self.day = day;
        self.puzzle = generate(self.day);
        self.grid = std::fs::read_to_string(self.path()).map(|s| decode_progress(&s, &self.puzzle.solution)).unwrap_or([0; CELLS]);
        self.selected = 0;
        self.selected_cell = self.puzzle.words.first().map(|p| p.cell(0)).unwrap_or(0);
        self.message = "Toque em uma palavra para escolher.".into();
        self.checked = false;
        self.menu = false;
    }

    fn refresh_day(&mut self, day: i64) -> bool {
        if !self.menu && self.day != day { self.start_on(day); true } else { false }
    }

    fn done(&self) -> bool {
        self.puzzle.solution.iter().enumerate().all(|(i, &c)| c == 0 || self.grid[i] == c)
    }

    fn tile(i: usize) -> Rect {
        const SIDE: i32 = 68;
        Rect::new((W - SIDE * N as i32) / 2 + (i % N) as i32 * SIDE, 282 + (i / N) as i32 * SIDE, SIDE, SIDE)
    }
    fn bottom(i: usize) -> Rect { Rect::new(MARGIN + i as i32 * 314, 1288, 298, 80) }

    fn save_grid(&self) {
        let path = self.path();
        if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
        let text: String = self.grid.iter().map(|&c| if c == 0 { '.' } else { c as char }).collect();
        let _ = std::fs::write(path, text);
    }

    fn select(&mut self, cell: usize) {
        let matches: Vec<usize> = self.puzzle.words.iter().enumerate().filter_map(|(i, p)| p.contains(cell).then_some(i)).collect();
        if matches.is_empty() { return; }
        self.selected = if cell == self.selected_cell {
            let old = matches.iter().position(|&i| i == self.selected).unwrap_or(matches.len() - 1);
            matches[(old + 1) % matches.len()]
        } else if matches.contains(&self.selected) {
            self.selected
        } else { matches[0] };
        self.selected_cell = cell;
        self.message.clear();
    }

    fn fill(&mut self, text: &str) {
        let letters = normalize(text);
        let Some(p) = self.puzzle.words.get(self.selected) else { return };
        let len = p.entry.word.len();
        if letters.len() != len {
            self.message = format!("Digite {len} letras, sem espaços.");
            return;
        }
        for (k, ch) in letters.bytes().enumerate() { self.grid[p.cell(k)] = ch; }
        self.checked = false;
        self.message = if self.done() { "Cruzadinha completa!".into() } else { "Resposta preenchida.".into() };
        self.save_grid();
    }

    #[cfg(test)]
    pub fn screenshot_start(&mut self) { self.start(); }

    #[cfg(test)]
    pub fn screenshot_finish(&mut self) {
        self.screenshot_start();
        self.grid = self.puzzle.solution;
        self.message = "Cruzadinha completa!".into();
    }
}

impl MiniGame for Cruzadas {
    fn at_menu(&self) -> bool { self.menu }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Cruzadas", "Uma cruzadinha diferente a cada dia.", &["Cruzadinha do dia"]);
            minigame::menu_art_ate(fb, "Palavra", minigame::menu_row(1).y + 40, 1368);
            return;
        }
        let title = if self.done() { "Cruzadinha completa!" } else { "Cruzadinha do dia" };
        minigame::header(fb, f, "Cruzadas", title);
        for i in 0..CELLS {
            if self.puzzle.solution[i] == 0 { continue; }
            let r = Self::tile(i);
            let selected = self.puzzle.words.get(self.selected).is_some_and(|p| p.contains(i));
            let fill = if i == self.selected_cell { 0xB8 } else if selected { 0xE2 } else { PAPER };
            fb.fill(r, fill);
            fb.round_outline(r, 0, 2, INK, fill);
            let ch = self.grid[i];
            if ch != 0 {
                let s = (ch as char).to_string();
                let tone = if self.checked && ch != self.puzzle.solution[i] { MUTED } else { INK };
                let tw = f.measure(Face::Display, 39.0, &s);
                f.draw(fb, Face::Display, 39.0, r.x + (r.w - tw) / 2, r.y + 49, tone, &s);
                if self.checked && ch != self.puzzle.solution[i] { fb.fill(Rect::new(r.x + 12, r.y + 58, r.w - 24, 3), INK); }
            }
        }
        let card = Rect::new(MARGIN, 1052, W - 2 * MARGIN, 214);
        ui::card(fb, card, 24, PAPER, false);
        if let Some(p) = self.puzzle.words.get(self.selected) {
            let label = format!("{} · {} letras", if p.vertical { "Vertical" } else { "Horizontal" }, p.entry.word.len());
            f.draw(fb, Face::Strong, 28.0, card.x + 26, card.y + 48, MUTED, &label);
            for (i, line) in crate::chat::wrap_words(f, Face::Strong, 31.0, &p.entry.clue, card.w - 52).iter().take(2).enumerate() {
                f.draw(fb, Face::Strong, 31.0, card.x + 26, card.y + 91 + i as i32 * 38, INK, line);
            }
            let msg = if self.message.is_empty() { "Toque novamente no cruzamento para mudar." } else { &self.message };
            let msg = crate::apps::fit(f, Face::Body, 24.0, msg, card.w - 52);
            f.draw(fb, Face::Body, 24.0, card.x + 26, card.y + 186, MUTED, &msg);
        }
        for (i, label) in ["Escrever", "Conferir", "Revelar letra"].iter().enumerate() {
            minigame::pill(fb, f, Self::bottom(i), label, i == 0);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu { return vec![(900, minigame::menu_row(0))]; }
        let mut v: Vec<_> = (0..CELLS).filter(|&i| self.puzzle.solution[i] != 0).map(|i| (i as u32, Self::tile(i))).collect();
        v.extend((0..3).map(|i| (901 + i as u32, Self::bottom(i))));
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 => self.start(),
            901 => self.want_text = true,
            902 => {
                self.checked = true;
                let errors = self.grid.iter().enumerate().filter(|(i, c)| **c != 0 && **c != self.puzzle.solution[*i]).count();
                self.message = if self.done() { "Cruzadinha completa!".into() } else if errors == 0 { "As letras preenchidas estão certas.".into() } else { format!("{errors} letra(s) precisam de atenção.") };
            }
            903 => {
                self.grid[self.selected_cell] = self.puzzle.solution[self.selected_cell];
                self.checked = false;
                self.message = if self.done() { "Cruzadinha completa!".into() } else { "Letra revelada.".into() };
                self.save_grid();
            }
            i if (i as usize) < CELLS => self.select(i as usize),
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn wants_text(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.want_text) { return None; }
        self.puzzle.words.get(self.selected).map(|p| format!("Resposta: {} letras", p.entry.word.len()))
    }

    fn text_input(&mut self, text: &str) -> Redraw { self.fill(text); Redraw::Full }

    fn save(&self) { if !self.menu { self.save_grid(); } }

    fn tick(&mut self) -> Redraw {
        if self.refresh_day(crate::data::today(crate::offset_min())) { Redraw::Full } else { Redraw::None }
    }

    fn resume(&mut self) { self.refresh_day(crate::data::today(crate::offset_min())); }

    fn back(&mut self) -> bool {
        if self.menu { false } else { self.save_grid(); self.menu = true; true }
    }

    fn summary(&self) -> String {
        if self.menu { String::new() } else { format!("{} de {} letras", self.grid.iter().filter(|&&c| c != 0).count(), self.puzzle.solution.iter().filter(|&&c| c != 0).count()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_valida_sem_duplicatas() {
        let entries = parse(SOURCE);
        assert!(entries.len() >= 400);
        assert_eq!(entries.len(), SOURCE.lines().count());
        assert_eq!(parse("ABC;dica\nABC;outra\nÁRVORE;dica\nVAZIA;").len(), 1);
        assert_eq!(normalize("Açúcar"), "ACUCAR");
    }

    #[test]
    fn grade_repetivel_conectada_e_sem_palavras_coladas() {
        for day in 20_000..20_050 {
            let p = generate(day);
            assert_eq!(p.solution, generate(day).solution);
            assert!(p.words.len() >= 6, "dia {day}: {} palavras", p.words.len());
            let mut replay = Puzzle::empty();
            for (i, word) in p.words.iter().enumerate() {
                assert!(replay.fits(&word.entry.word, word.row, word.col, word.vertical, i > 0));
                replay.add(word.entry.clone(), word.row, word.col, word.vertical);
            }
            assert_eq!(replay.solution, p.solution);
            assert!(p.owners.contains(&3));
        }
    }

    #[test]
    fn recupera_progresso_e_ignora_casas_fora_da_grade() {
        let p = generate(21_000);
        let encoded: String = p.solution.iter().map(|&c| if c == 0 { 'X' } else { c as char }).collect();
        assert_eq!(decode_progress(&encoded, &p.solution), p.solution);
        assert_eq!(decode_progress("inválido", &p.solution), [0; CELLS]);
    }

    #[test]
    fn troca_a_grade_quando_muda_o_dia() {
        let mut g = Cruzadas::new();
        g.start_on(20_000);
        assert!(!g.refresh_day(20_000));
        assert!(g.refresh_day(20_001));
        assert_eq!(g.day, 20_001);
        assert_eq!(g.puzzle.solution, generate(20_001).solution);
    }
}
