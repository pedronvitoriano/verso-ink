//! Um lance por dia, escolhido entre erros analisados das partidas contra treinadores.
//! A reconstrução usa `Pos` diretamente: `Game::play_uci` salvaria uma partida real.

use crate::chess::{self, Game, Kind, Mode, Move, Pos, Sprites};
use crate::coach::{self, Class, Saved};
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::path::Path;

#[derive(Clone)]
struct Puzzle {
    key: String,
    before: Pos,
    best: String,
    best_san: String,
    why: String,
    user_white: bool,
}

/// Aceita só UCI completo, com promoção explícita quando necessária.
fn apply_uci(pos: &Pos, text: &str) -> Option<Pos> {
    if !(text.len() == 4 || text.len() == 5) { return None; }
    let (m, promo) = chess::parse_uci(text)?;
    if !pos.legal_from(m.from).contains(&m) || pos.is_promotion(m) != promo.is_some() { return None; }
    if chess::uci(m, promo) != text { return None; }
    Some(pos.apply_promo(m, promo.unwrap_or(Kind::Queen)))
}

fn candidates(saved: &[Saved]) -> Vec<Puzzle> {
    let mut result = Vec::new();
    for game in saved {
        let Some(name) = game.file.file_name().and_then(|s| s.to_str()) else { continue };
        for record in &game.records {
            if !record.analyzed || !matches!(record.class, Class::Mistake | Class::Blunder) || record.ply >= game.moves.len() { continue; }
            let mut pos = Pos::start();
            let mut valid = true;
            for m in game.moves.iter().take(record.ply) {
                if let Some(next) = apply_uci(&pos, m) { pos = next; } else { valid = false; break; }
            }
            if !valid || pos.white_to_move != game.user_white || apply_uci(&pos, &game.moves[record.ply]).is_none() || apply_uci(&pos, &record.best_uci).is_none() || game.moves[record.ply] == record.best_uci { continue; }
            let best_san = if record.best_san.is_empty() { coach::best_san(&pos, &record.best_uci) } else { record.best_san.clone() };
            result.push(Puzzle { key: format!("{name}:{}", record.ply), before: pos, best: record.best_uci.clone(), best_san, why: record.why.clone(), user_white: game.user_white });
        }
    }
    result.sort_by(|a, b| a.key.cmp(&b.key));
    result.dedup_by(|a, b| a.key == b.key);
    result
}

/// A chave persistida fixa a escolha do dia mesmo se novas partidas forem salvas.
fn selected_key(path: &Path, day: i64, puzzles: &[Puzzle]) -> Option<String> {
    if let Ok(s) = std::fs::read_to_string(path) {
        let mut lines = s.lines();
        if lines.next().and_then(|s| s.parse::<i64>().ok()) == Some(day) {
            if let Some(key) = lines.next().filter(|key| puzzles.iter().any(|p| p.key == *key)) {
                return Some(key.to_string());
            }
        }
    }
    if puzzles.is_empty() { return None; }
    let i = day.rem_euclid(puzzles.len() as i64) as usize;
    let key = puzzles[i].key.clone();
    if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
    let _ = std::fs::write(path, format!("{day}\n{key}\n"));
    Some(key)
}

fn completed_days(text: &str) -> Vec<i64> {
    let mut days: Vec<i64> = text.lines().filter_map(|s| s.split('|').next()?.parse().ok()).collect();
    days.sort_unstable(); days.dedup(); days
}

fn streak(days: &[i64], day: i64) -> usize {
    let mut n = 0;
    let mut d = if days.contains(&day) { day } else { day - 1 };
    while days.contains(&d) { n += 1; d -= 1; }
    n
}

fn was_completed(text: &str, day: i64, key: &str) -> bool {
    text.lines().any(|line| {
        let Some((d, k)) = line.split_once('|') else { return false };
        d.parse::<i64>().ok() == Some(day) && k == key
    })
}

fn mark_complete(path: &Path, day: i64, key: &str) -> Vec<i64> {
    let mut text = std::fs::read_to_string(path).unwrap_or_default();
    if !was_completed(&text, day, key) {
        if !text.is_empty() && !text.ends_with('\n') { text.push('\n'); }
        text.push_str(&format!("{day}|{key}\n"));
        if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
        let _ = std::fs::write(path, &text);
    }
    completed_days(&text)
}

pub struct PuzzleDia {
    day: i64,
    offset_min: i64,
    puzzle: Option<Puzzle>,
    game: Game,
    sprites: Sprites,
    solved: bool,
    sequence: usize,
    wrong: bool,
    promotion: Option<Move>,
}

impl PuzzleDia {
    pub fn new(offset_min: i64) -> Self {
        let day = crate::data::today(offset_min);
        let dir = crate::home_dir().join("data/xadrez/puzzle");
        let puzzles = candidates(&coach::list_saved());
        let selected = selected_key(&dir.join("escolha.txt"), day, &puzzles);
        let puzzle = selected.and_then(|key| puzzles.into_iter().find(|p| p.key == key));
        let completed = std::fs::read_to_string(dir.join("dias.txt")).unwrap_or_default();
        let days = completed_days(&completed);
        let solved = puzzle.as_ref().is_some_and(|p| was_completed(&completed, day, &p.key));
        let mut game = Game::new();
        game.clock = None;
        if let Some(p) = &puzzle {
            game.mode = Mode::Coach { level: 0, user_white: p.user_white };
            game.pos = if solved { apply_uci(&p.before, &p.best).unwrap_or_else(|| p.before.clone()) } else { p.before.clone() };
            if solved { game.last = chess::parse_uci(&p.best).map(|(m, _)| m); }
        }
        Self { day, offset_min, puzzle, game, sprites: Sprites::new(), solved, sequence: streak(&days, day), wrong: false, promotion: None }
    }

    /// Cenas determinísticas sem ler nem alterar arquivos de partidas ou progresso.
    #[cfg(test)]
    fn cena_print(kind: u8) -> Self {
        let mut before = Pos::start();
        before = apply_uci(&before, "e2e4").unwrap();
        before = apply_uci(&before, "e7e5").unwrap();
        let puzzle = (kind != 0).then(|| Puzzle {
            key: "exemplo:2".into(), before: before.clone(), best: "d2d4".into(), best_san: "d4".into(),
            why: "O peão no centro abre caminho para as peças e disputa espaço.".into(), user_white: true,
        });
        let solved = kind == 2;
        let game = Game {
            mode: Mode::Coach { level: 0, user_white: true }, moves: vec![],
            pos: if solved { apply_uci(&before, "d2d4").unwrap() } else { before },
            history: vec![], selected: None, targets: vec![],
            last: if solved { chess::parse_uci("d2d4").map(|(m, _)| m) } else { None },
            pending_promo: None, clock: None,
        };
        Self { day: 0, offset_min: 0, puzzle, game, sprites: Sprites::new(), solved, sequence: if solved { 3 } else { 2 }, wrong: false, promotion: None }
    }

    #[cfg(test)]
    pub fn print_vazio() -> Self { Self::cena_print(0) }
    #[cfg(test)]
    pub fn print_partida() -> Self { Self::cena_print(1) }
    #[cfg(test)]
    pub fn print_fim() -> Self { Self::cena_print(2) }

    fn try_move(&mut self, m: Move, promo: Option<Kind>) {
        let Some(p) = &self.puzzle else { return };
        let candidate = chess::uci(m, promo);
        if candidate == p.best {
            if let Some(next) = apply_uci(&p.before, &candidate) {
                self.game.pos = next;
                self.game.last = Some(m);
                self.solved = true;
                let path = crate::home_dir().join("data/xadrez/puzzle/dias.txt");
                self.sequence = streak(&mark_complete(&path, self.day, &p.key), self.day);
                self.wrong = false;
            }
        } else { self.wrong = true; }
        self.game.selected = None;
        self.game.targets.clear();
        self.promotion = None;
    }

    fn select(&mut self, sq: usize) {
        if let Some(from) = self.game.selected {
            let m = Move { from, to: sq };
            if self.game.targets.contains(&sq) {
                if self.game.pos.is_promotion(m) { self.promotion = Some(m); }
                else { self.try_move(m, None); }
                return;
            }
        }
        let moves = self.game.pos.legal_from(sq);
        if moves.is_empty() || self.game.selected == Some(sq) {
            self.game.selected = None; self.game.targets.clear();
        } else {
            self.game.selected = Some(sq);
            self.game.targets = moves.iter().map(|m| m.to).collect();
        }
    }

    fn promotion_rect(k: usize) -> Rect {
        let gap = 12;
        let width = (W - 2 * MARGIN - gap * 3) / 4;
        Rect::new(MARGIN + k as i32 * (width + gap), 330, width, 82)
    }

    fn explanation(&self) -> String {
        let Some(p) = &self.puzzle else { return String::new() };
        if p.why.trim().is_empty() { format!("O melhor lance era {}.", p.best_san) } else { p.why.clone() }
    }

    fn draw_wrapped(fb: &mut Framebuffer, f: &Fonts, text: &str, y: i32) {
        let mut line = String::new();
        let mut row = 0;
        for word in text.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if f.measure(Face::Body, 27.0, &next) > W - 2 * MARGIN && !line.is_empty() {
                f.draw(fb, Face::Body, 27.0, MARGIN, y + row * 34, INK, &line);
                row += 1; line = word.to_string();
                if row == 3 { return; }
            } else { line = next; }
        }
        if !line.is_empty() { f.draw(fb, Face::Body, 27.0, MARGIN, y + row * 34, INK, &line); }
    }
}

impl MiniGame for PuzzleDia {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        minigame::header(fb, f, "Xadrez", "Puzzle do dia");
        if self.puzzle.is_none() {
            f.draw(fb, Face::Display, 52.0, MARGIN, 400, INK, "Ainda não há um puzzle");
            Self::draw_wrapped(fb, f, "Jogue contra os treinadores e termine a análise da partida. Seus erros com um lance melhor vão aparecer aqui.", 480);
            return;
        }
        if self.solved {
            let title = format!("Acertou! · {} dia{}", self.sequence, if self.sequence == 1 { "" } else { "s" });
            f.draw(fb, Face::Display, 38.0, MARGIN, 286, INK, &title);
            Self::draw_wrapped(fb, f, &self.explanation(), 332);
        } else {
            f.draw(fb, Face::Display, 40.0, MARGIN, 284, INK, "Ache o lance que você não viu");
            let hint = if self.promotion.is_some() { "Escolha a peça para promover." } else if self.wrong { "Não é esse. Tente outro lance." } else if self.game.pos.white_to_move { "Brancas jogam. Toque na peça e no destino." } else { "Pretas jogam. Toque na peça e no destino." };
            f.draw(fb, Face::Body, 28.0, MARGIN, 325, MUTED, hint);
            if self.promotion.is_some() {
                for (k, label) in ["Dama", "Torre", "Bispo", "Cavalo"].iter().enumerate() {
                    minigame::pill(fb, f, Self::promotion_rect(k), label, false);
                }
            }
        }
        chess::draw_position(fb, f, &self.game, &self.sprites);
        if !self.solved && self.sequence > 0 {
            let text = format!("Sequência: {} dia{}", self.sequence, if self.sequence == 1 { "" } else { "s" });
            let width = f.measure(Face::Body, 26.0, &text);
            f.draw(fb, Face::Body, 26.0, (W - width) / 2, 1325, MUTED, &text);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.puzzle.is_none() || self.solved { return vec![]; }
        if self.promotion.is_some() { return (0..4).map(|k| (800 + k as u32, Self::promotion_rect(k))).collect(); }
        (0..64).map(|i| (i as u32, chess::square_rect(i, self.game.flipped()))).collect()
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        if self.puzzle.is_none() || self.solved { return Redraw::None; }
        if let Some(m) = self.promotion {
            if (800..804).contains(&id) { self.try_move(m, Some(chess::PROMO[(id - 800) as usize])); return Redraw::Full; }
            return Redraw::None;
        }
        if id < 64 { self.select(id as usize); return Redraw::Full; }
        Redraw::None
    }

    fn tick(&mut self) -> Redraw {
        if crate::data::today(self.offset_min) != self.day { *self = Self::new(self.offset_min); Redraw::Full } else { Redraw::None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::Record;
    fn saved() -> Saved {
        Saved { file: "123.txt".into(), when: 123, level: 0, user_white: true, result: "derrota".into(), rating_before: 100, rating_after: 90,
            moves: ["e2e4", "e7e5", "d1h5"].into_iter().map(str::to_string).collect(),
            records: vec![Record { ply: 2, san: "Dh5".into(), analyzed: true, class: Class::Mistake, loss: 180, best_uci: "d2d4".into(), best_san: "d4".into(), why: "Abra o centro.".into() }] }
    }
    #[test]
    fn cenas_de_print_reproduzem_a_posicao_sem_persistir() {
        let empty = PuzzleDia::print_vazio();
        let before = PuzzleDia::print_partida();
        let after = PuzzleDia::print_fim();
        assert!(empty.puzzle.is_none());
        let p = before.puzzle.as_ref().unwrap();
        assert_eq!(before.game.pos, p.before);
        assert!(!before.solved && after.solved);
        assert_eq!(after.game.pos, apply_uci(&p.before, &p.best).unwrap());
    }
    #[test]
    fn posicao_anterior_e_alternativa_legal() {
        let g = saved();
        let p = candidates(&[g]);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].before.board[chess::parse_uci("e2e4").unwrap().0.to].unwrap().kind, Kind::Pawn);
        assert!(apply_uci(&p[0].before, &p[0].best).is_some());
        let mut bad = saved(); bad.records[0].best_uci = "e2e4".into();
        assert!(candidates(&[bad]).is_empty());
        let mut bad = saved(); bad.moves[1] = "e7e3".into();
        assert!(candidates(&[bad]).is_empty());
        let mut bad = saved(); bad.user_white = false;
        assert!(candidates(&[bad]).is_empty());
    }
    #[test]
    fn somente_erros_analisados_e_promocoes_validas() {
        let mut bad = saved(); bad.records[0].analyzed = false;
        assert!(candidates(&[bad]).is_empty());
        let mut bad = saved(); bad.records[0].class = Class::Good;
        assert!(candidates(&[bad]).is_empty());
        let start = Pos::start();
        assert!(apply_uci(&start, "e2e4q").is_none());
        assert!(apply_uci(&start, "e2e4l").is_none());
        assert!(apply_uci(&start, "e2e4extra").is_none());
    }
    #[test]
    fn selecao_fica_estavel_e_sequencia_conta_dias_distintos() {
        let root = std::env::temp_dir().join(format!("verso-puzzle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let puzzles = candidates(&[saved()]);
        let a = selected_key(&root.join("escolha"), 42, &puzzles).unwrap();
        let mut more = puzzles.clone();
        let mut p = more[0].clone(); p.key = "outra:2".into(); more.push(p);
        assert_eq!(selected_key(&root.join("escolha"), 42, &more), Some(a.clone()));
        let only_new = vec![more[1].clone()];
        assert_eq!(selected_key(&root.join("escolha"), 42, &only_new), Some("outra:2".into()));
        assert_eq!(selected_key(&root.join("escolha"), 42, &[]), None);
        let path = root.join("dias");
        mark_complete(&path, 40, "a"); mark_complete(&path, 41, "b"); mark_complete(&path, 41, "b");
        assert_eq!(streak(&completed_days(&std::fs::read_to_string(&path).unwrap()), 42), 2);
        assert_eq!(streak(&mark_complete(&path, 42, "c"), 42), 3);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(was_completed(&text, 42, "c"));
        assert!(!was_completed(&text, 42, "outra:2"));
        assert_eq!(streak(&completed_days(&std::fs::read_to_string(&path).unwrap()), 44), 0);
        let _ = std::fs::remove_dir_all(root);
    }
}
