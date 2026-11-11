//! Treinadores de xadrez: escada de 100 a 3000, rating estimado, análise de cada lance
//! e partidas guardadas para revisão. Números de rating são estimativas, não oficiais.

use crate::chess::{self, Kind, Move, Pos};
use crate::engine::Score;
use crate::sudoku::Rng;
use std::path::PathBuf;

pub struct Coach {
    pub rating: u32,
    pub name: &'static str,
    pub style: &'static str,
}

const NAMES: [&str; 30] = [
    "Pipoca", "Bia", "Téo", "Lia", "Davi", "Nina", "Caio", "Malu", "Rafa", "Iara", "Bento", "Clara", "Otávio", "Luna", "Heitor",
    "Sofia", "Gael", "Alice", "Benício", "Helena", "Ravi", "Laura", "Miguel", "Cecília", "Arthur", "Valentina", "Bernardo", "Isadora", "Teodoro", "Mestra Vera",
];

pub const LADDER: usize = 30;

pub fn coach(i: usize) -> Coach {
    let i = i.min(LADDER - 1);
    let rating = 100 + i as u32 * 100;
    let style = match rating {
        0..=500 => "Paciente: aponta só o essencial",
        501..=1200 => "Didático: explica cada deslize",
        1201..=1900 => "Exigente: cobra precisão",
        _ => "Mestre: comenta até as imprecisões",
    };
    Coach { rating, name: NAMES[i], style }
}

fn dir() -> PathBuf {
    let d = crate::home_dir().join("data/xadrez");
    let _ = std::fs::create_dir_all(d.join("partidas"));
    d
}

// ---------- Escada e rating ----------

pub fn unlocked() -> usize {
    std::fs::read_to_string(dir().join("escada.txt")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

/// (rating estimado, partidas contadas). Começa em 100, como o Pedro informou.
pub fn rating() -> (u32, u32) {
    let s = std::fs::read_to_string(dir().join("rating.txt")).unwrap_or_default();
    let mut it = s.split_whitespace().filter_map(|x| x.parse().ok());
    (it.next().unwrap_or(100), it.next().unwrap_or(0))
}

/// Atualiza o rating pela fórmula Elo contra o treinador. `score`: 1 vitória, 0.5 empate, 0 derrota.
pub fn update_rating(coach_rating: u32, score: f32) -> (u32, u32) {
    let (r, n) = rating();
    let expected = 1.0 / (1.0 + 10f32.powf((coach_rating as f32 - r as f32) / 400.0));
    let k = if n < 20 { 40.0 } else { 20.0 };
    let new = (r as f32 + k * (score - expected)).round().max(0.0) as u32;
    let _ = std::fs::write(dir().join("rating.txt"), format!("{new} {}", n + 1));
    (r, new)
}

pub fn unlock_after_win(level: usize) {
    if level + 1 > unlocked() && level + 1 < LADDER {
        let _ = std::fs::write(dir().join("escada.txt"), (level + 1).to_string());
    }
}

// ---------- Lance do computador ----------

/// Abaixo de 1400 o Stockfish (nível 0) ainda é forte demais: misturamos lances aleatórios
/// na proporção do nível (100 ≈ 85% aleatórios, 1300 ≈ 6%).
pub fn random_move_chance(rating: u32) -> f32 {
    if rating >= 1400 { 0.0 } else { ((1400 - rating) as f32 / 1300.0).clamp(0.0, 1.0) * 0.85 }
}

pub fn think_millis(rating: u32) -> u32 {
    match rating {
        0..=799 => 250,
        800..=1599 => 500,
        1600..=2399 => 1000,
        _ => 1500,
    }
}

pub fn random_legal(pos: &Pos, rng: &mut Rng) -> Option<String> {
    let moves: Vec<Move> = (0..64).flat_map(|i| pos.legal_from(i)).collect();
    if moves.is_empty() {
        return None;
    }
    let m = moves[(rng.next() % moves.len() as u64) as usize];
    Some(chess::uci(m, None))
}

// ---------- Análise de cada lance ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Best,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
}

impl Class {
    pub fn from_loss(loss: i32, best: bool) -> Class {
        match loss {
            _ if best => Class::Best,
            l if l >= 300 => Class::Blunder,
            l if l >= 150 => Class::Mistake,
            l if l >= 60 => Class::Inaccuracy,
            _ => Class::Good,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Class::Best => "Melhor lance",
            Class::Good => "Bom",
            Class::Inaccuracy => "Imprecisão",
            Class::Mistake => "Erro",
            Class::Blunder => "Erro grave",
        }
    }
    pub fn mark(self) -> &'static str {
        match self {
            Class::Best => "!",
            Class::Good => "",
            Class::Inaccuracy => "?!",
            Class::Mistake => "?",
            Class::Blunder => "??",
        }
    }
    fn code(self) -> u8 {
        self as u8
    }
    fn from_code(c: u8) -> Class {
        [Class::Best, Class::Good, Class::Inaccuracy, Class::Mistake, Class::Blunder][c.min(4) as usize]
    }
}

#[derive(Clone, Debug)]
pub struct Record {
    pub ply: usize,
    pub san: String,
    pub class: Class,
    pub loss: i32,
    pub best_uci: String,
    pub best_san: String,
    pub why: String,
}

fn value(k: Kind) -> i32 {
    match k {
        Kind::Pawn => 1,
        Kind::Knight | Kind::Bishop => 3,
        Kind::Rook => 5,
        Kind::Queen => 9,
        Kind::King => 100,
    }
}

/// Peças de `white` atacadas e sem defesa (ou atacadas por peça de menor valor) na posição.
fn hanging(pos: &Pos, white: bool) -> Vec<(usize, Kind)> {
    (0..64)
        .filter_map(|i| {
            let p = pos.board[i]?;
            if p.white != white || p.kind == Kind::King || !pos.attacked(i, !white) {
                return None;
            }
            let defended = {
                // Simula a captura por qualquer atacante e pergunta se a casa fica defendida.
                let mut probe = pos.clone();
                probe.board[i] = Some(chess::Piece { kind: Kind::Pawn, white: !white });
                probe.attacked(i, white)
            };
            let cheaper_attacker = (0..64).any(|j| {
                pos.board[j].is_some_and(|a| a.white != white && value(a.kind) < value(p.kind)) && {
                    let mut t = pos.clone();
                    t.white_to_move = !white;
                    t.legal_from(j).iter().any(|m| m.to == i)
                }
            });
            (!defended || cheaper_attacker).then_some((i, p.kind))
        })
        .collect()
}

/// Explica, em português simples, por que o lance custou caro.
pub fn explain(before: &Pos, played: Move, best_uci: &str, user_white: bool, best_score: Score) -> String {
    let after = before.apply(played);
    let moved = before.board[played.from].map(|p| p.kind).unwrap_or(Kind::Pawn);
    if let Score::Mate(n) = best_score {
        if n > 0 {
            return format!("Havia mate em {n}: comece por {}.", best_san(before, best_uci));
        }
    }
    let hang = hanging(&after, user_white);
    if let Some(&(sq, k)) = hang.iter().find(|(sq, _)| *sq == played.to) {
        return format!("Seu {} em {} ficou sem defesa e pode ser capturado.", chess::piece_name(k), chess::square_name(sq));
    }
    if let Some(&(sq, k)) = hang.iter().max_by_key(|(_, k)| value(*k)) {
        if value(k) >= 3 {
            return format!("Depois do lance, seu {} em {} ficou desprotegido.", chess::piece_name(k), chess::square_name(sq));
        }
    }
    if let Some((bm, _)) = chess::parse_uci(best_uci) {
        if let Some(target) = before.board[bm.to] {
            return format!("Dava para capturar o {} em {} com {}.", chess::piece_name(target.kind), chess::square_name(bm.to), best_san(before, best_uci));
        }
        let next = before.apply(bm);
        if next.in_check(next.white_to_move) {
            return format!("Havia um xeque forte: {}.", best_san(before, best_uci));
        }
    }
    let _ = moved;
    format!("Melhor era {}.", best_san(before, best_uci))
}

pub fn best_san(pos: &Pos, best_uci: &str) -> String {
    chess::parse_uci(best_uci).map(|(m, p)| chess::san(pos, m, p)).unwrap_or_default()
}

/// Treinadores mais fortes comentam deslizes menores.
pub fn should_comment(coach_rating: u32, c: Class) -> bool {
    let min = match coach_rating {
        0..=500 => Class::Blunder,
        501..=1200 => Class::Mistake,
        _ => Class::Inaccuracy,
    };
    c.code() >= min.code()
}

pub fn praise(coach_rating: u32, c: Class) -> Option<&'static str> {
    match (c, coach_rating) {
        (Class::Best, 0..=1200) => Some("Ótimo lance! Era o melhor."),
        (Class::Best, _) => Some("Preciso. Melhor lance."),
        _ => None,
    }
}

// ---------- Partidas guardadas ----------

pub struct Saved {
    pub file: PathBuf,
    pub when: i64,
    pub level: usize,
    pub user_white: bool,
    /// "vitória", "derrota" ou "empate"
    pub result: String,
    pub rating_before: u32,
    pub rating_after: u32,
    pub moves: Vec<String>,
    pub records: Vec<Record>,
}

pub fn save_game(level: usize, user_white: bool, result: &str, ratings: (u32, u32), moves: &[String], records: &[Record]) {
    let when = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let mut s = format!("{when} {level} {} {result} {} {}\n{}\n", if user_white { "w" } else { "b" }, ratings.0, ratings.1, moves.join(" "));
    for r in records {
        s.push_str(&format!("{}|{}|{}|{}|{}|{}|{}\n", r.ply, r.san, r.class.code(), r.loss, r.best_uci, r.best_san, r.why.replace('|', "/")));
    }
    let _ = std::fs::write(dir().join("partidas").join(format!("{when}.txt")), s);
}

pub fn load_saved(file: &std::path::Path) -> Option<Saved> {
    let s = std::fs::read_to_string(file).ok()?;
    let mut lines = s.lines();
    let h: Vec<&str> = lines.next()?.split_whitespace().collect();
    let moves = lines.next().unwrap_or("").split_whitespace().map(str::to_string).collect();
    let records = lines
        .filter_map(|l| {
            let f: Vec<&str> = l.splitn(7, '|').collect();
            Some(Record {
                ply: f.first()?.parse().ok()?,
                san: f.get(1)?.to_string(),
                class: Class::from_code(f.get(2)?.parse().ok()?),
                loss: f.get(3)?.parse().ok()?,
                best_uci: f.get(4)?.to_string(),
                best_san: f.get(5)?.to_string(),
                why: f.get(6).unwrap_or(&"").to_string(),
            })
        })
        .collect();
    Some(Saved {
        file: file.to_path_buf(),
        when: h.first()?.parse().ok()?,
        level: h.get(1)?.parse().ok()?,
        user_white: h.get(2) != Some(&"b"),
        result: h.get(3)?.to_string(),
        rating_before: h.get(4).and_then(|x| x.parse().ok()).unwrap_or(0),
        rating_after: h.get(5).and_then(|x| x.parse().ok()).unwrap_or(0),
        moves,
        records,
    })
}

/// Partidas mais recentes primeiro.
pub fn list_saved() -> Vec<Saved> {
    let mut v: Vec<Saved> = std::fs::read_dir(dir().join("partidas")).into_iter().flatten().flatten().filter_map(|e| load_saved(&e.path())).collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.when));
    v
}

fn record_line(r: &Record) -> String {
    format!("{}|{}|{}|{}|{}|{}|{}", r.ply, r.san, r.class.code(), r.loss, r.best_uci, r.best_san, r.why.replace('|', "/"))
}

/// Análises da partida em andamento (para continuar depois sem perder a revisão).
pub fn save_current(records: &[Record]) {
    let s: Vec<String> = records.iter().map(record_line).collect();
    let _ = std::fs::write(dir().join("atual.txt"), s.join("\n"));
}

pub fn load_current() -> Vec<Record> {
    let path = dir().join("atual.txt");
    let fake = format!("0 0 w x 0 0\n\n{}", std::fs::read_to_string(&path).unwrap_or_default());
    let tmp = dir().join("atual.tmp");
    let _ = std::fs::write(&tmp, fake);
    let r = load_saved(&tmp).map(|s| s.records).unwrap_or_default();
    let _ = std::fs::remove_file(tmp);
    r
}

pub fn clear_current() {
    let _ = std::fs::remove_file(dir().join("atual.txt"));
}

/// Precisão estimada (0–100) a partir da perda média por lance.
pub fn accuracy(records: &[Record]) -> u32 {
    if records.is_empty() {
        return 0;
    }
    let sum: f32 = records.iter().map(|r| 100.0 * (-(r.loss.clamp(0, 1000) as f32) / 250.0).exp()).sum();
    (sum / records.len() as f32).round() as u32
}

/// Frases de diagnóstico para a revisão, tiradas dos números da partida (e das anteriores).
pub fn insights(g: &Saved, previous: &[Saved]) -> Vec<String> {
    let mut out = Vec::new();
    let count = |c: Class| g.records.iter().filter(|r| r.class == c).count();
    let (b, m, i) = (count(Class::Blunder), count(Class::Mistake), count(Class::Inaccuracy));
    out.push(format!("Precisão estimada: {}%. {} erros graves, {} erros e {} imprecisões.", accuracy(&g.records), b, m, i));
    // Fase com mais deslizes.
    let phase = |r: &Record| if r.ply < 20 { 0 } else if r.ply < 60 { 1 } else { 2 };
    let mut by = [0usize; 3];
    for r in g.records.iter().filter(|r| r.class as u8 >= Class::Mistake as u8) {
        by[phase(r)] += 1;
    }
    if by.iter().any(|&n| n > 0) {
        let k = (0..3).max_by_key(|&k| by[k]).unwrap();
        out.push(format!("Seus erros se concentraram {}.", ["na abertura", "no meio-jogo", "no final"][k]));
    }
    // Motivo mais comum.
    let hang = g.records.iter().filter(|r| r.class as u8 >= Class::Mistake as u8 && (r.why.contains("sem defesa") || r.why.contains("desprotegido"))).count();
    let missed = g.records.iter().filter(|r| r.class as u8 >= Class::Mistake as u8 && r.why.starts_with("Dava para capturar")).count();
    let mates = g.records.iter().filter(|r| r.why.starts_with("Havia mate")).count();
    if hang >= 2 {
        out.push(format!("Em {hang} lances você deixou peças sem defesa. Antes de jogar, pergunte: o que o adversário pode capturar agora?"));
    }
    if missed >= 2 {
        out.push(format!("Você deixou passar {missed} capturas grátis. Olhe sempre o que está desprotegido do outro lado."));
    }
    if mates >= 1 {
        out.push(format!("Houve {mates} chance(s) de mate que passaram. Procure xeques primeiro."));
    }
    if let Some(worst) = g.records.iter().max_by_key(|r| r.loss) {
        if worst.loss >= 150 {
            out.push(format!("O lance que mais pesou foi {} (lance {}). Melhor era {}.", worst.san, worst.ply / 2 + 1, worst.best_san));
        }
    }
    // Tendência contra as últimas partidas.
    let prev: Vec<u32> = previous.iter().filter(|p| p.file != g.file).take(5).map(|p| accuracy(&p.records)).collect();
    if !prev.is_empty() {
        let avg = prev.iter().sum::<u32>() / prev.len() as u32;
        let now = accuracy(&g.records);
        out.push(if now > avg + 3 {
            format!("Melhor que a sua média recente ({avg}%). Continue assim.")
        } else if now + 3 < avg {
            format!("Abaixo da sua média recente ({avg}%). Jogue com mais calma nos lances de troca.")
        } else {
            format!("Na linha da sua média recente ({avg}%).")
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escada_e_elo() {
        assert_eq!(coach(0).rating, 100);
        assert_eq!(coach(29).rating, 3000);
        assert!(random_move_chance(100) > 0.8 && random_move_chance(1300) < 0.1 && random_move_chance(1500) == 0.0);
        assert_eq!(Class::from_loss(400, false), Class::Blunder);
        assert_eq!(Class::from_loss(20, false), Class::Good);
    }
    #[test]
    fn explica_peca_pendurada() {
        // 1.e4 e5 2.Bc4 Nf6 3.Qh5?? Nxh5: a dama em h5 fica sem defesa e pode ser capturada.
        let mut g = chess::Game::new();
        for m in ["e2e4", "e7e5", "f1c4", "g8f6"] {
            g.play_uci(m).unwrap();
        }
        let (m, _) = chess::parse_uci("d1h5").unwrap();
        let why = explain(&g.pos, m, "d2d3", true, Score::Cp(30));
        assert!(why.contains("dama em h5"), "{why}");
    }
}
