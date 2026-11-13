//! Revisão de baralhos exportados em texto do Anki, sem rede.
//! Cada linha válida é frente<TAB>verso; o agendamento fica fora da pasta dos baralhos.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct Card { key: u64, front: String, back: String }
struct Deck { name: String, cards: Vec<Card> }

/// Hash estável de arquivo, frente e verso: reordenar linhas não perde o agendamento.
fn card_key(deck: &str, front: &str, back: &str) -> u64 {
    let mut h = 14695981039346656037u64;
    for part in [deck, front, back] {
        for &b in part.as_bytes() { h = (h ^ b as u64).wrapping_mul(1099511628211); }
        h = (h ^ 255).wrapping_mul(1099511628211);
    }
    h
}

fn parse_cards(name: &str, text: &str) -> Vec<Card> {
    let mut seen = std::collections::HashSet::new();
    let mut cards = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = if i == 0 { line.trim_start_matches('\u{feff}') } else { line };
        let Some((front, back)) = line.split_once('\t') else { continue };
        let (front, back) = (front.trim(), back.trim());
        if front.is_empty() || back.is_empty() { continue; }
        let key = card_key(name, front, back);
        if seen.insert(key) { cards.push(Card { key, front: front.to_string(), back: back.to_string() }); }
    }
    cards
}

fn load_decks(dir: &Path) -> Vec<Deck> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("txt"))).collect();
    files.sort();
    files.into_iter().filter_map(|path| {
        let filename = path.file_name()?.to_str()?;
        let text = std::fs::read_to_string(&path).ok()?;
        let cards = parse_cards(filename, &text);
        let name = path.file_stem()?.to_str()?.to_string();
        (!cards.is_empty()).then_some(Deck { name, cards })
    }).collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Schedule { due: i64, interval: u32, repetitions: u32, ease: f32 }
impl Schedule {
    fn new(day: i64) -> Self { Self { due: day, interval: 0, repetitions: 0, ease: 2.5 } }
}

#[derive(Clone, Copy)]
enum Grade { Again, Hard, Good, Easy }

/// Variante compacta do SM-2. Um erro volta ainda nesta sessão; acertos agendam em dias.
fn schedule(old: Schedule, grade: Grade, day: i64) -> Schedule {
    let ease = match grade { Grade::Again => old.ease - 0.20, Grade::Hard => old.ease - 0.15, Grade::Good => old.ease, Grade::Easy => old.ease + 0.15 }.clamp(1.3, 3.0);
    let (interval, repetitions) = match grade {
        Grade::Again => (0, 0),
        Grade::Hard => (if old.interval == 0 { 1 } else { ((old.interval as f32 * 1.2).round() as u32).max(old.interval.saturating_add(1)) }, old.repetitions.saturating_add(1)),
        Grade::Good => (match old.repetitions { 0 => 1, 1 => 6, _ => ((old.interval as f32 * ease).round() as u32).max(old.interval.saturating_add(1)) }, old.repetitions.saturating_add(1)),
        Grade::Easy => (match old.repetitions { 0 => 4, 1 => 7, _ => ((old.interval as f32 * ease * 1.3).round() as u32).max(old.interval.saturating_add(1)) }, old.repetitions.saturating_add(1)),
    };
    let interval = interval.min(365_000);
    Schedule { due: day.saturating_add(interval as i64), interval, repetitions, ease }
}

fn parse_progress(text: &str) -> HashMap<u64, Schedule> {
    text.lines().filter_map(|line| {
        let mut f = line.split('|');
        let key = f.next()?.parse().ok()?;
        let due = f.next()?.parse().ok()?;
        let interval: u32 = f.next()?.parse().ok()?;
        let repetitions = f.next()?.parse().ok()?;
        let ease: f32 = f.next()?.parse().ok()?;
        if !ease.is_finite() || !(1.3..=3.0).contains(&ease) || interval > 365_000 { return None; }
        Some((key, Schedule { due, interval, repetitions, ease }))
    }).collect()
}

fn progress_path() -> PathBuf { crate::home_dir().join("data/flashcards-progresso.txt") }
fn save_progress(path: &Path, progress: &HashMap<u64, Schedule>) -> std::io::Result<()> {
    let mut rows: Vec<_> = progress.iter().collect();
    rows.sort_by_key(|(k, _)| **k);
    let mut text = rows.iter().map(|(k, s)| format!("{}|{}|{}|{}|{:.3}\n", k, s.due, s.interval, s.repetitions, s.ease)).collect::<String>();
    // Se uma linha antiga estiver danificada ou vier de outra versão, não a descarte.
    let previous = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    for line in previous.lines().filter(|line| !line.trim().is_empty() && parse_progress(line).is_empty()) {
        text.push_str(line); text.push('\n');
    }
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

fn due_count(deck: &Deck, progress: &HashMap<u64, Schedule>, day: i64) -> usize {
    deck.cards.iter().filter(|c| progress.get(&c.key).is_none_or(|s| s.due <= day)).count()
}

#[derive(Clone, Copy, PartialEq)]
enum View { Menu, Card, Done }

pub struct Flashcards {
    decks: Vec<Deck>,
    progress: HashMap<u64, Schedule>,
    view: View,
    deck: usize,
    menu_page: usize,
    queue: Vec<usize>,
    total: usize,
    learned: usize,
    revealed: bool,
    text_page: usize,
    page_count: std::cell::Cell<usize>,
    save_error: bool,
    xp: u32,
    celebration: Option<crate::aprender::Cel>,
}

impl Flashcards {
    pub fn new() -> Self {
        let decks = load_decks(&crate::home_dir().join("data/flashcards"));
        let progress = parse_progress(&std::fs::read_to_string(progress_path()).unwrap_or_default());
        Self { decks, progress, view: View::Menu, deck: 0, menu_page: 0, queue: vec![], total: 0, learned: 0, revealed: false, text_page: 0, page_count: std::cell::Cell::new(1), save_error: false, xp: 0, celebration: None }
    }
    /// Baralho fictício para os prints, sem depender de arquivos pessoais.
    #[cfg(test)]
    pub fn para_print() -> Self {
        let mut f = Self::new();
        f.decks = vec![Deck { name: "Português".into(), cards: vec![Card { key: card_key("exemplo", "O que é uma metáfora?", "Uma comparação implícita."), front: "O que é uma metáfora?".into(), back: "Uma comparação implícita.".into() }] }];
        f.progress.clear();
        f
    }
    fn start(&mut self, deck: usize) {
        if deck >= self.decks.len() { return; }
        let today = crate::aprender::today();
        self.deck = deck;
        self.queue = self.decks[deck].cards.iter().enumerate().filter(|(_, c)| self.progress.get(&c.key).is_none_or(|s| s.due <= today)).map(|(i, _)| i).collect();
        if self.queue.is_empty() { return; }
        self.total = self.queue.len(); self.learned = 0; self.revealed = false; self.text_page = 0; self.page_count.set(1); self.save_error = false; self.xp = 0; self.view = View::Card;
    }
    fn grade(&mut self, grade: Grade) {
        if !self.revealed || self.queue.is_empty() { return; }
        let i = self.queue[0];
        let key = self.decks[self.deck].cards[i].key;
        let day = crate::aprender::today();
        let old = self.progress.get(&key).copied().unwrap_or_else(|| Schedule::new(day));
        let next = schedule(old, grade, day);
        self.progress.insert(key, next);
        if save_progress(&progress_path(), &self.progress).is_err() {
            self.progress.insert(key, old);
            self.save_error = true;
            return;
        }
        self.save_error = false;
        self.queue.remove(0);
        if matches!(grade, Grade::Again) { self.queue.push(i); } else { self.learned += 1; }
        self.revealed = false; self.text_page = 0; self.page_count.set(1);
        if self.queue.is_empty() {
            self.xp = 10;
            self.celebration = crate::aprender::add(self.xp, "flashcards");
            self.view = View::Done;
        }
    }
    fn card(&self) -> Option<&Card> { self.queue.first().and_then(|&i| self.decks.get(self.deck)?.cards.get(i)) }
    fn card_lines(&self, f: &Fonts) -> Vec<String> {
        let Some(card) = self.card() else { return vec![] };
        let text = if self.revealed { &card.back } else { &card.front };
        let width = W - 2 * MARGIN - 80;
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if f.measure(Face::Body, 36.0, &candidate) <= width { line = candidate; continue; }
            if !line.is_empty() { lines.push(std::mem::take(&mut line)); }
            for c in word.chars() {
                let mut candidate = line.clone(); candidate.push(c);
                if f.measure(Face::Body, 36.0, &candidate) > width && !line.is_empty() { lines.push(std::mem::take(&mut line)); }
                line.push(c);
            }
        }
        if !line.is_empty() { lines.push(line); }
        lines
    }
    fn row(k: usize) -> Rect { Rect::new(MARGIN, 470 + k as i32 * 145, W - 2 * MARGIN, 125) }
    fn nav(k: usize) -> Rect { Rect::new(MARGIN + k as i32 * 472, 1192, 456, 80) }
    fn deck_nav(k: usize) -> Rect { Rect::new(MARGIN + k as i32 * 472, 1288, 456, 80) }
    fn action() -> Rect { Rect::new(MARGIN, 1288, W - 2 * MARGIN, 80) }
    fn grading(k: usize) -> Rect {
        let gap = 12;
        let width = (W - 2 * MARGIN - 3 * gap) / 4;
        Rect::new(MARGIN + k as i32 * (width + gap), 1288, width, 80)
    }
}

impl MiniGame for Flashcards {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        if let Some(c) = self.celebration.filter(|_| self.view == View::Menu) { return crate::aprender::draw_celebration(fb, f, c); }
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Menu => {
                minigame::draw_menu(fb, f, "Flashcards", "Revise seus baralhos no seu ritmo.", &[]);
                if self.decks.is_empty() {
                    for (k, line) in crate::chat::wrap_words(f, Face::Body, 34.0, "Nenhum baralho encontrado. Exporte suas cartas do Anki em texto e coloque os arquivos na pasta data/flashcards.", W - 2 * MARGIN).iter().enumerate() {
                        f.draw(fb, Face::Body, 34.0, MARGIN, 535 + k as i32 * 48, INK, line);
                    }
                } else {
                    let today = crate::aprender::today();
                    for (k, deck) in self.decks.iter().skip(self.menu_page * 5).take(5).enumerate() {
                        let r = Self::row(k);
                        ui::card(fb, r, 32, PAPER, false);
                        f.draw(fb, Face::Display, 38.0, r.x + 28, r.y + 51, INK, &crate::apps::fit(f, Face::Display, 38.0, &deck.name, r.w - 56));
                        let n = due_count(deck, &self.progress, today);
                        let text = if n == 0 { "Tudo em dia".to_string() } else { format!("{n} carta{} para revisar", if n == 1 { "" } else { "s" }) };
                        f.draw(fb, Face::Body, 27.0, r.x + 30, r.y + 96, MUTED, &text);
                    }
                    if self.menu_page > 0 { ui::button(fb, f, Self::deck_nav(0), "Anteriores", false); }
                    if (self.menu_page + 1) * 5 < self.decks.len() { ui::button(fb, f, Self::deck_nav(1), "Mais baralhos", false); }
                }
            }
            View::Card => {
                minigame::header(fb, f, "Flashcards", if self.revealed { "Resposta" } else { "Pense na resposta" });
                let heading = format!("{} · {} de {} revisadas", self.decks[self.deck].name, self.learned, self.total);
                f.draw(fb, Face::Strong, 26.0, MARGIN, 290, MUTED, &crate::apps::fit(f, Face::Strong, 26.0, &heading, W - 2 * MARGIN));
                let panel = Rect::new(MARGIN, 335, W - 2 * MARGIN, 770);
                ui::card(fb, panel, 32, PAPER, false);
                let lines = self.card_lines(f);
                self.page_count.set(lines.len().div_ceil(14).max(1));
                for (k, line) in lines.iter().skip(self.text_page * 14).take(14).enumerate() {
                    f.draw(fb, Face::Body, 36.0, panel.x + 40, panel.y + 70 + k as i32 * 45, INK, line);
                }
                if self.save_error { f.draw(fb, Face::Strong, 27.0, MARGIN, 1150, INK, "Não consegui salvar. Toque na resposta para tentar de novo."); }
                if self.text_page > 0 { ui::button(fb, f, Self::nav(0), "Anterior", false); }
                if (self.text_page + 1) * 14 < lines.len() { ui::button(fb, f, Self::nav(1), "Continuar lendo", false); }
                if self.revealed {
                    for (k, name) in ["Errei", "Difícil", "Bom", "Fácil"].iter().enumerate() { ui::button(fb, f, Self::grading(k), name, k == 2); }
                } else { ui::button(fb, f, Self::action(), "Mostrar resposta", true); }
            }
            View::Done => {
                minigame::header(fb, f, "Flashcards", "Revisão concluída");
                f.draw(fb, Face::Display, 88.0, MARGIN, 410, INK, "Tudo em dia!");
                f.draw(fb, Face::Display, 70.0, MARGIN, 530, INK, &format!("+{} XP", self.xp));
                f.draw(fb, Face::Strong, 34.0, MARGIN, 610, MUTED, &format!("{} carta{} revisada{}", self.total, if self.total == 1 { "" } else { "s" }, if self.total == 1 { "" } else { "s" }));
                let status = crate::aprender::status();
                crate::aprender::draw_panel(fb, f, Rect::new(MARGIN, 730, W - 2 * MARGIN, 180), &status);
                crate::aprender::week_row(fb, f, MARGIN, 990, W - 2 * MARGIN, &status);
                ui::button(fb, f, Self::action(), "Continuar", true);
            }
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.celebration.is_some() && self.view == View::Menu { return vec![(999, crate::aprender::cel_button())]; }
        match self.view {
            View::Menu => {
                let mut v: Vec<_> = self.decks.iter().enumerate().skip(self.menu_page * 5).take(5).filter(|(_, d)| due_count(d, &self.progress, crate::aprender::today()) > 0).map(|(i, _)| (900 + i as u32, Self::row(i % 5))).collect();
                if self.menu_page > 0 { v.push((890, Self::deck_nav(0))); }
                if (self.menu_page + 1) * 5 < self.decks.len() { v.push((891, Self::deck_nav(1))); }
                v
            }
            View::Card => {
                let mut v = Vec::new();
                if self.text_page > 0 { v.push((700, Self::nav(0))); }
                if self.text_page + 1 < self.page_count.get() { v.push((701, Self::nav(1))); }
                if self.revealed { v.extend((0..4).map(|k| (800 + k as u32, Self::grading(k)))); }
                else { v.push((810, Self::action())); }
                v
            }
            View::Done => vec![(810, Self::action())],
        }
    }
    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        if self.celebration.is_some() && self.view == View::Menu {
            if id == 999 { self.celebration = None; return Redraw::Full; }
            return Redraw::None;
        }
        match (self.view, id) {
            (View::Menu, 890) => self.menu_page = self.menu_page.saturating_sub(1),
            (View::Menu, 891) if (self.menu_page + 1) * 5 < self.decks.len() => self.menu_page += 1,
            (View::Menu, n) if n >= 900 => self.start((n - 900) as usize),
            (View::Card, 700) => self.text_page = self.text_page.saturating_sub(1),
            (View::Card, 701) if self.text_page + 1 < self.page_count.get() => self.text_page += 1,
            (View::Card, 810) if !self.revealed => { self.revealed = true; self.text_page = 0; self.page_count.set(1); }
            (View::Card, n @ 800..=803) if self.revealed => self.grade([Grade::Again, Grade::Hard, Grade::Good, Grade::Easy][(n - 800) as usize]),
            (View::Done, 810) => self.view = View::Menu,
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn back(&mut self) -> bool {
        if self.view == View::Menu && self.celebration.take().is_some() { return true; }
        if self.view != View::Menu { self.view = View::Menu; self.celebration = None; return true; }
        false
    }
    fn resume(&mut self) {
        if self.view == View::Menu {
            self.decks = load_decks(&crate::home_dir().join("data/flashcards"));
            self.progress = parse_progress(&std::fs::read_to_string(progress_path()).unwrap_or_default());
            self.menu_page = self.menu_page.min(self.decks.len().saturating_sub(1) / 5);
        }
    }
    fn at_menu(&self) -> bool { self.view == View::Menu && self.celebration.is_none() }
    fn summary(&self) -> String { if self.view == View::Card { format!("Revisando {}", self.decks[self.deck].name) } else { String::new() } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn importa_tab_e_ignora_linhas_ruins_ou_repetidas() {
        let cards = parse_cards("baralho.txt", "\u{feff}Frente\tVerso\nsem tab\n\tvazio\nfrente\t\nFrente\tVerso\nOutra\tResposta\n");
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].front, "Frente");
        assert_eq!(cards[0].back, "Verso");
        let reordered = parse_cards("baralho.txt", "Outra\tResposta\nFrente\tVerso\n");
        assert_eq!(cards[0].key, reordered[1].key);
        assert_ne!(cards[0].key, parse_cards("outro.txt", "Frente\tVerso")[0].key);
    }
    #[test]
    fn sm2_escala_intervalo_e_erro_recomeca() {
        let initial = Schedule::new(100);
        let one = schedule(initial, Grade::Good, 100);
        assert_eq!((one.due, one.interval, one.repetitions), (101, 1, 1));
        let two = schedule(one, Grade::Good, 101);
        assert_eq!((two.due, two.interval, two.repetitions), (107, 6, 2));
        let easy = schedule(two, Grade::Easy, 107);
        assert!(easy.interval > two.interval && easy.ease > two.ease);
        let again = schedule(easy, Grade::Again, 110);
        assert_eq!((again.due, again.interval, again.repetitions), (110, 0, 0));
        assert!(again.ease < easy.ease);
        assert_eq!(schedule(initial, Grade::Hard, 100).due, 101);
        assert_eq!(schedule(initial, Grade::Easy, 100).due, 104);
        let long = Schedule { due: 100, interval: 365_000, repetitions: u32::MAX, ease: 3.0 };
        let next = schedule(long, Grade::Good, 100);
        assert_eq!(next.interval, 365_000);
        assert_eq!(next.repetitions, u32::MAX);
    }
    #[test]
    fn progresso_persiste_apos_reordenar_baralho() {
        let root = std::env::temp_dir().join(format!("verso-flashcards-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let p = root.join("progresso.txt");
        let cards = parse_cards("exemplo.txt", "Um\t1\nDois\t2\n");
        let mut progress = HashMap::new();
        progress.insert(cards[0].key, schedule(Schedule::new(100), Grade::Good, 100));
        save_progress(&p, &progress).unwrap();
        let mut raw = std::fs::read_to_string(&p).unwrap();
        raw.push_str("registro-antigo-ilegivel\n");
        std::fs::write(&p, raw).unwrap();
        save_progress(&p, &progress).unwrap();
        let persisted = std::fs::read_to_string(&p).unwrap();
        assert!(persisted.contains("registro-antigo-ilegivel"));
        let recovered = parse_progress(&persisted);
        let reordered = parse_cards("exemplo.txt", "Dois\t2\nUm\t1\n");
        assert_eq!(recovered[&reordered[1].key].due, 101);
        assert!(!recovered.contains_key(&reordered[0].key));
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn progresso_recusa_linhas_invalidas_e_mantem_datas() {
        let parsed = parse_progress("123|100|6|2|2.500\nruim\n456|100|1|1|NaN\n789|100|1|1|0.2\n");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[&123].due, 100);
        let deck = Deck { name: "Teste".into(), cards: vec![Card { key: 123, front: "a".into(), back: "b".into() }] };
        assert_eq!(due_count(&deck, &parsed, 99), 0);
        assert_eq!(due_count(&deck, &parsed, 100), 1);
    }
}
