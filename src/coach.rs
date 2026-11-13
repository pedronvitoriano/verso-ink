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

/// O Claude como adversário especial (fora da escada): joga pela ponte da VPS e comenta.
pub const CLAUDE: usize = 99;

pub fn coach(i: usize) -> Coach {
    if i == CLAUDE {
        return Coach { rating: 2000, name: "Verso", style: "Joga e conversa com você a cada lance" };
    }
    let i = i.min(LADDER - 1);
    let rating = 100 + i as u32 * 100;
    let style = match rating {
        0..=500 => "Paciente: aponta só o essencial",
        501..=1200 => "Didático: explica cada deslize",
        1201..=1900 => "Exigente: cobra precisão",
        _ => "Mestre: comenta até as imprecisões",
    };
    // O estilo próprio de cada um, se a personalidade estiver escrita.
    let style = persona(NAMES[i]).map(|p| p.style.as_str()).filter(|s| !s.is_empty()).unwrap_or(style);
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
    /// Só é verdade quando as duas avaliações do motor chegaram. Nunca usamos um palpite
    /// de precisão para preencher uma falha de análise.
    pub analyzed: bool,
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
        s.push_str(&format!("{}|{}|{}|{}|{}|{}|{}|{}\n", r.ply, r.san, r.class.code(), r.loss, r.best_uci, r.best_san, r.why.replace('|', "/"), if r.analyzed { "a" } else { "i" }));
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
            let f: Vec<&str> = l.splitn(8, '|').collect();
            Some(Record {
                ply: f.first()?.parse().ok()?,
                san: f.get(1)?.to_string(),
                // O formato antigo não registrava se a segunda avaliação do motor chegou.
                // Não há como distinguir, depois, um diagnóstico real de um fallback: mantemos
                // os lances, mas não os usamos para precisão, alternativa ou tendência.
                analyzed: f.get(7).is_some_and(|v| *v == "a"),
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
    format!("{}|{}|{}|{}|{}|{}|{}|{}", r.ply, r.san, r.class.code(), r.loss, r.best_uci, r.best_san, r.why.replace('|', "/"), if r.analyzed { "a" } else { "i" })
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
    let analyzed: Vec<&Record> = records.iter().filter(|r| r.analyzed).collect();
    if analyzed.is_empty() {
        return 0;
    }
    let sum: f32 = analyzed.iter().map(|r| 100.0 * (-(r.loss.clamp(0, 1000) as f32) / 250.0).exp()).sum();
    (sum / analyzed.len() as f32).round() as u32
}

pub fn analyzed_count(g: &Saved) -> usize {
    g.records.iter().filter(|r| r.analyzed).count()
}

/// Quantos lances foram feitos pela pessoa, inclusive os que o motor não conseguiu medir.
pub fn player_move_count(g: &Saved) -> usize {
    let parity = if g.user_white { 0 } else { 1 };
    g.moves.iter().enumerate().filter(|(ply, _)| *ply % 2 == parity).count()
}

/// Frases de diagnóstico para a revisão, tiradas dos números da partida (e das anteriores).
pub fn insights(g: &Saved, previous: &[Saved]) -> Vec<String> {
    let mut out = Vec::new();
    let count = |c: Class| g.records.iter().filter(|r| r.analyzed && r.class == c).count();
    let (b, m, i) = (count(Class::Blunder), count(Class::Mistake), count(Class::Inaccuracy));
    let analyzed = analyzed_count(g);
    let total = player_move_count(g);
    if analyzed == 0 {
        out.push(format!("O motor não mediu nenhum dos {total} lances desta partida; não há precisão nem alternativa confiável para mostrar."));
    } else {
        out.push(format!("Precisão estimada: {}% em {analyzed} de {total} lances analisados. {} erros graves, {} erros e {} imprecisões.", accuracy(&g.records), b, m, i));
    }
    // Fase com mais deslizes.
    let phase = |r: &Record| if r.ply < 20 { 0 } else if r.ply < 60 { 1 } else { 2 };
    let mut by = [0usize; 3];
    for r in g.records.iter().filter(|r| r.analyzed && r.class as u8 >= Class::Mistake as u8) {
        by[phase(r)] += 1;
    }
    if by.iter().any(|&n| n > 0) {
        let k = (0..3).max_by_key(|&k| by[k]).unwrap();
        out.push(format!("Seus erros se concentraram {}.", ["na abertura", "no meio-jogo", "no final"][k]));
    }
    // Motivo mais comum.
    let hang = g.records.iter().filter(|r| r.analyzed && r.class as u8 >= Class::Mistake as u8 && (r.why.contains("sem defesa") || r.why.contains("desprotegido"))).count();
    let missed = g.records.iter().filter(|r| r.analyzed && r.class as u8 >= Class::Mistake as u8 && r.why.starts_with("Dava para capturar")).count();
    let mates = g.records.iter().filter(|r| r.analyzed && r.why.starts_with("Havia mate")).count();
    if hang >= 2 {
        out.push(format!("Em {hang} lances você deixou peças sem defesa. Antes de jogar, pergunte: o que o adversário pode capturar agora?"));
    }
    if missed >= 2 {
        out.push(format!("Você deixou passar {missed} capturas grátis. Olhe sempre o que está desprotegido do outro lado."));
    }
    if mates >= 1 {
        out.push(format!("Houve {mates} chance(s) de mate que passaram. Procure xeques primeiro."));
    }
    if let Some(worst) = g.records.iter().filter(|r| r.analyzed).max_by_key(|r| r.loss) {
        if worst.loss >= 150 {
            out.push(format!("O lance que mais pesou foi {} (lance {}). Melhor era {}.", worst.san, worst.ply / 2 + 1, worst.best_san));
        }
    }
    // Tendência contra as últimas partidas.
    let prev: Vec<u32> = previous.iter().filter(|p| p.file != g.file && analyzed_count(p) > 0).take(5).map(|p| accuracy(&p.records)).collect();
    if analyzed > 0 && !prev.is_empty() {
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

    #[test]
    fn partidas_antigas_e_lances_sem_motor_nao_fingem_precisao() {
        let dir = std::env::temp_dir().join(format!("verso-ink-coach-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let legacy = dir.join("legacy.txt");
        std::fs::write(&legacy, "1 0 w vitória 100 110\ne2e4\n0|e4|0|0|e2e4|e4|\n").unwrap();
        assert!(!load_saved(&legacy).unwrap().records[0].analyzed);
        let unavailable = dir.join("unavailable.txt");
        std::fs::write(&unavailable, "1 0 w vitória 100 110\ne2e4\n0|e4|1|0||||i\n").unwrap();
        let game = load_saved(&unavailable).unwrap();
        assert!(!game.records[0].analyzed);
        assert_eq!(accuracy(&game.records), 0);
        assert_eq!(analyzed_count(&game), 0);
        let verified = dir.join("verified.txt");
        std::fs::write(&verified, "1 0 w vitória 100 110\ne2e4\n0|e4|0|0|e2e4|e4||a\n").unwrap();
        assert!(load_saved(&verified).unwrap().records[0].analyzed);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tendencia_ignora_partidas_sem_medicao_confirmada() {
        fn saved(file: &str, analyzed: bool, loss: i32) -> Saved {
            Saved {
                file: file.into(),
                when: 0,
                level: 0,
                user_white: true,
                result: "derrota".into(),
                rating_before: 100,
                rating_after: 90,
                moves: vec!["e2e4".into()],
                records: vec![Record {
                    ply: 0,
                    san: "e4".into(),
                    analyzed,
                    class: Class::Mistake,
                    loss,
                    best_uci: "d2d4".into(),
                    best_san: "d4".into(),
                    why: String::new(),
                }],
            }
        }
        let current_without_motor = saved("current", false, 0);
        let valid_previous = saved("previous", true, 20);
        let notes = insights(&current_without_motor, &[valid_previous]);
        assert!(notes.iter().any(|line| line.contains("não mediu")));
        assert!(!notes.iter().any(|line| line.contains("média recente")));

        let current_measured = saved("current-measured", true, 20);
        let unavailable_previous = saved("previous-unavailable", false, 0);
        let notes = insights(&current_measured, &[unavailable_previous]);
        assert!(!notes.iter().any(|line| line.contains("média recente")));
    }
}

// ---------- Reações do treinador (falas de gente, sem análise) ----------

/// Expressão do treinador na partida; cada uma é uma variação do mesmo busto
/// (`busto-<nome>-<humor>.png`); sem a imagem, aparece o busto normal.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mood {
    #[default]
    Normal,
    Feliz,
    Preocupado,
    Chocado,
    Triste,
}

impl Mood {
    pub fn slug(self) -> Option<&'static str> {
        match self {
            Mood::Normal => None,
            Mood::Feliz => Some("feliz"),
            Mood::Preocupado => Some("preocupado"),
            Mood::Chocado => Some("chocado"),
            Mood::Triste => Some("triste"),
        }
    }
}

fn pick(list: &[&'static str], roll: u64) -> &'static str {
    list[(roll % list.len() as u64) as usize]
}

/// O que aconteceu no lance de quem joga contra o treinador, para ele reagir.
pub struct Moment {
    pub class: Class,
    /// Vantagem de quem joga, em centipeões, depois do lance (None sem análise).
    pub user_eval: Option<i32>,
    /// Peça do treinador que acabou de ser capturada.
    pub took: Option<crate::chess::Kind>,
}

/// Reação ao lance da pessoa: a frase e a expressão. Nunca explica o lance (isso fica na revisão).
pub fn react_to_user(m: &Moment, roll: u64) -> (&'static str, Mood) {
    use crate::chess::Kind;
    match m.took {
        Some(Kind::Queen) => return (pick(&["Nãoo, minha rainha!", "Minha dama! Não acredito…", "Ai, ai… lá se foi a rainha.", "Minha dama, volta aqui!", "Isso doeu até em mim!", "Pera, cadê minha rainha?", "Ah não, justo ela!", "Vou sentir saudade dela…"], roll), Mood::Chocado),
        Some(Kind::Rook) => return (pick(&["Ai, minha torre…", "Perdi a torre? Sério?", "Minha torre, não!", "Lá se foi minha torre…", "Poxa, eu gostava dela!", "Essa torre vai fazer falta.", "Nem deu tempo de dizer tchau!", "Me devolve, vai?"], roll), Mood::Preocupado),
        Some(Kind::Bishop) => return (pick(&["Meu bispo, não!", "Ih, levaram meu bispo…", "Poxa, justo meu bispo!", "Bispo, volta aqui!", "Já tô com saudade do bispo.", "Meu bispo foi de arrasta…", "Essa perda eu senti!", "Cadê meu bispo? Ah, não…"], roll), Mood::Preocupado),
        Some(Kind::Knight) => return (pick(&["Meu cavalo, não!", "Ih, levaram meu cavalo…", "Poxa, meu cavalinho!", "Cavalo, volta aqui!", "Já tô com saudade dele.", "Meu cavalo saiu galopando…", "Essa perda eu senti!", "Cadê meu cavalo? Ah, não…"], roll), Mood::Preocupado),
        _ => {}
    }
    match m.class {
        Class::Blunder => return (pick(&["Eita, vacilou…", "Opa! Essa eu não deixo passar.", "Obrigado pelo presente!", "Xiii… olha o que você fez.", "Ih, essa foi pra mim?", "Opa, aceito com carinho!", "Essa eu vou guardar!", "Você me mimou agora!"], roll), Mood::Feliz),
        Class::Mistake => return (pick(&["Hmm, deu uma brecha aí.", "Opa, gostei dessa… pra mim.", "Interessante… vou aproveitar.", "Opa, essa eu gostei!", "Ih, bobeou um pouquinho!", "Deixa comigo agora!", "Olha só o que apareceu…", "Aí você me anima!"], roll), Mood::Feliz),
        _ => {}
    }
    let eval = m.user_eval.unwrap_or(0);
    if eval > 300 {
        return (pick(&["Tô ficando em apuros…", "Calma, calma, ainda dá…", "Você tá apertando, hein.", "Eita, pega leve comigo!", "Assim você me assusta…", "Opa, tô suando aqui.", "Preciso de um cafezinho…", "Deixa eu respirar!"], roll), Mood::Preocupado);
    }
    if eval < -300 {
        return (pick(&["Tá ficando bom pra mim!", "Hehe, gosto dessa posição.", "Vai ser difícil sair dessa.", "Hoje eu tô inspirado!", "Hehe, agora eu gostei!", "Tô me divertindo, viu?", "O sorriso veio sozinho!", "Opa, o dia tá bom!"], roll), Mood::Feliz);
    }
    match m.class {
        Class::Best => (pick(&["Uau, boa jogada!", "Essa foi forte.", "Tá jogando sério hoje, hein?", "Opa, mandou bem!", "Aí sim, gostei de ver!", "Você veio com tudo!", "Rapaz, essa foi bonita!", "Olha você dando show!"], roll), if eval > 100 { Mood::Preocupado } else { Mood::Normal }),
        Class::Inaccuracy => (pick(&["Hmm, ok…", "Dava pra mais, hein.", "Certo, certo…", "Opa, entendi…", "Foi por aí, né?", "Hmm, curioso isso…", "Tá bom, minha vez!", "Vamos nessa, então."], roll), Mood::Normal),
        _ => (pick(&["Hmm, entendi.", "Certo…", "Vamos ver…", "Deixa eu pensar…", "Opa, minha vez!", "Tá, tô de olho!", "Então tá bom…", "Agora deixa comigo."], roll), Mood::Normal),
    }
}

/// Reação ao próprio lance do treinador, quando ele captura algo grande.
pub fn react_to_own(took: Option<crate::chess::Kind>, roll: u64) -> Option<(&'static str, Mood)> {
    use crate::chess::Kind;
    match took? {
        Kind::Queen => Some((pick(&["Peguei sua rainha!", "Essa dama agora é minha!", "Opa, ganhei uma rainha!", "Sua dama veio comigo!", "Hehe, essa eu levo!", "Olha quem veio me visitar!", "Essa rainha é minha agora!", "Adorei esse presente!"], roll), Mood::Feliz)),
        Kind::Rook => Some((pick(&["Torre no bolso.", "Obrigado pela torre!", "Opa, ganhei uma torre!", "Essa torre veio comigo!", "Hehe, essa eu levo!", "Uma torre pra chamar de minha!", "Gostei dessa torre, viu?", "Adorei esse presente!"], roll), Mood::Feliz)),
        Kind::Bishop => Some((pick(&["Peguei seu bispo!", "O bispo veio comigo!", "Opa, ganhei um bispo!", "Seu bispo é meu agora!", "Hehe, esse eu levo!", "Gostei desse bispo, viu?", "Um bispo pro meu bolso!", "Esse bispo fez as malas!"], roll), Mood::Feliz)),
        Kind::Knight => Some((pick(&["Peguei seu cavalo!", "O cavalo veio comigo!", "Opa, ganhei um cavalo!", "Seu cavalo é meu agora!", "Hehe, esse eu levo!", "Gostei do cavalinho!", "Um cavalo pro meu bolso!", "Esse cavalo vai comigo!"], roll), Mood::Feliz)),
        _ => None,
    }
}

/// Fim de partida: (frase, expressão) do ponto de vista do treinador.
pub fn react_to_end(user_score: f32, roll: u64) -> (&'static str, Mood) {
    if user_score >= 1.0 {
        (pick(&["Ah, você venceu. Boa partida!", "Perdi… mas quero revanche!", "Você ganhou! Foi bonito, viu?", "Hoje a vitória foi sua!", "Essa eu perdi, mas adorei!", "Muito bem! Quero outra!", "Tá bom, você mereceu!", "Foi boa! Na próxima eu volto."], roll), Mood::Triste)
    } else if user_score <= 0.0 {
        (pick(&["Ganhei! Boa partida, viu?", "Essa foi minha! Bora outra?", "Hoje eu tava inspirado!", "Gostei! Vamos de novo?", "Hehe, dessa vez deu eu!", "Essa vitória eu comemoro!", "Boa! Já quero a próxima.", "Foi divertido demais!"], roll), Mood::Feliz)
    } else {
        ("Empate! Ninguém cedeu.", Mood::Normal)
    }
}

#[cfg(test)]
mod reacoes {
    use super::*;
    use crate::chess::Kind;

    #[test]
    fn rainha_perdida_choca_e_vacilo_alegra() {
        let m = Moment { class: Class::Good, user_eval: Some(0), took: Some(Kind::Queen) };
        assert_eq!(react_to_user(&m, 0), ("Nãoo, minha rainha!", Mood::Chocado));
        let m = Moment { class: Class::Blunder, user_eval: Some(-500), took: None };
        assert_eq!(react_to_user(&m, 0).1, Mood::Feliz);
        let m = Moment { class: Class::Good, user_eval: Some(450), took: None };
        assert_eq!(react_to_user(&m, 1).1, Mood::Preocupado);
        // Nenhuma reação traz análise (nada de "Erro grave:" nem "Melhor era").
        for c in [Class::Best, Class::Good, Class::Inaccuracy, Class::Mistake, Class::Blunder] {
            for roll in 0..6 {
                let (t, _) = react_to_user(&Moment { class: c, user_eval: None, took: None }, roll);
                assert!(!t.contains(':') && !t.contains("Melhor"), "{t}");
            }
        }
    }
}

// ---------- Personalidade de cada treinador ----------

/// Quem é o treinador e as falas dele por situação (assets/treinadores/personalidades.txt).
pub struct Persona {
    pub name: String,
    pub who: String,
    pub style: String,
    lines: std::collections::HashMap<String, Vec<String>>,
}

const PERSONAS_SRC: &str = include_str!("../assets/treinadores/personalidades.txt");

fn parse_personas(src: &str) -> Vec<Persona> {
    let mut v: Vec<Persona> = Vec::new();
    for l in src.lines().map(str::trim) {
        if let Some(n) = l.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            v.push(Persona { name: n.trim().to_string(), who: String::new(), style: String::new(), lines: Default::default() });
        } else if let (Some(p), Some((k, val))) = (v.last_mut(), l.split_once(':')) {
            let (k, val) = (k.trim(), val.trim());
            match k {
                "quem" => p.who = val.to_string(),
                "estilo" => p.style = val.to_string(),
                _ => {
                    let list: Vec<String> = val.split(" | ").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                    if !list.is_empty() {
                        p.lines.insert(k.to_string(), list);
                    }
                }
            }
        }
    }
    v
}

fn personas() -> &'static [Persona] {
    static P: std::sync::OnceLock<Vec<Persona>> = std::sync::OnceLock::new();
    P.get_or_init(|| parse_personas(PERSONAS_SRC))
}

pub fn persona(name: &str) -> Option<&'static Persona> {
    personas().iter().find(|p| p.name == name)
}

/// A fala do treinador nesta situação, no jeito dele (None: usar a frase geral).
pub fn persona_line(name: &str, situation: &str, roll: u64) -> Option<&'static str> {
    let l = persona(name)?.lines.get(situation)?;
    Some(l[(roll % l.len() as u64) as usize].as_str())
}

/// Situação do lance da pessoa, com a mesma prioridade das reações gerais.
pub fn situation_user(m: &Moment) -> &'static str {
    use crate::chess::Kind;
    match m.took {
        Some(Kind::Queen) => return "perdeu_rainha",
        Some(Kind::Rook) => return "perdeu_torre",
        Some(Kind::Bishop | Kind::Knight) => return "perdeu_peca",
        _ => {}
    }
    match m.class {
        Class::Blunder => return "vacilo",
        Class::Mistake => return "erro",
        _ => {}
    }
    let eval = m.user_eval.unwrap_or(0);
    if eval > 300 {
        return "apuros";
    }
    if eval < -300 {
        return "vantagem";
    }
    match m.class {
        Class::Best => "otimo",
        Class::Inaccuracy => "neutro",
        _ => "bom",
    }
}

pub fn situation_own(took: Option<crate::chess::Kind>) -> Option<&'static str> {
    use crate::chess::Kind;
    match took? {
        Kind::Queen => Some("pegou_rainha"),
        Kind::Rook => Some("pegou_torre"),
        Kind::Bishop | Kind::Knight => Some("pegou_peca"),
        _ => None,
    }
}

pub fn situation_end(user_score: f32) -> &'static str {
    if user_score >= 1.0 {
        "perdeu"
    } else if user_score <= 0.0 {
        "venceu"
    } else {
        "empate"
    }
}

#[cfg(test)]
mod personalidades {
    use super::*;

    #[test]
    fn le_o_formato_e_escolhe_pela_situacao() {
        let p = parse_personas("[Bia]\nquem: Estudante de música\nestilo: Calma e caprichosa\nvacilo: Ih, deu ruim | Opa, presente\n");
        assert_eq!(p[0].style, "Calma e caprichosa");
        assert_eq!(p[0].lines["vacilo"].len(), 2);
        assert_eq!(situation_user(&Moment { class: Class::Blunder, user_eval: Some(0), took: None }), "vacilo");
        assert_eq!(situation_user(&Moment { class: Class::Good, user_eval: Some(500), took: None }), "apuros");
    }

    /// O arquivo real, quando preenchido: os 30 treinadores, todas as situações, frases curtas e sem análise.
    #[test]
    fn personalidades_completas() {
        let ps = personas();
        if ps.is_empty() {
            return;
        }
        let need = ["abertura", "vacilo", "erro", "bom", "otimo", "apuros", "vantagem", "perdeu_rainha", "perdeu_torre", "perdeu_peca", "pegou_rainha", "pegou_torre", "pegou_peca", "venceu", "perdeu", "empate", "neutro"];
        assert_eq!(ps.len(), 30, "faltam treinadores");
        for (i, p) in ps.iter().enumerate() {
            assert_eq!(p.name, NAMES[i], "ordem/nome");
            assert!(!p.who.is_empty() && !p.style.is_empty() && p.style.chars().count() <= 48, "{}: quem/estilo", p.name);
            for k in need {
                let l = p.lines.get(k).unwrap_or_else(|| panic!("{}: falta {k}", p.name));
                assert!(l.len() >= 4, "{}: poucas frases em {k}", p.name);
                for t in l {
                    assert!(t.chars().count() <= 40 && !t.contains(':') && !t.contains("Melhor"), "{}/{k}: {t}", p.name);
                }
            }
        }
    }
}
