//! Violão: o Kindle como estante de partitura. Cifras (coladas na página "Mandar para o Kindle")
//! com os acordes sobre a letra e os diagramas de cada acorde, transposição de tom, um
//! dicionário de acordes e o treino de "um minuto de trocas" para quem está começando.
//!
//! As cifras chegam pela caixa de entrada da VPS e ficam em data/violao/cifras; o treino em
//! data/violao/trocas.txt ("dia|par|trocas").

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::path::PathBuf;

// ---------- Acordes ----------

const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

fn note_index(s: &str) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    let base = match b.first()? {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    match b.get(1) {
        Some(b'#') => Some(((base + 1) % 12, 2)),
        Some(b'b') => Some(((base + 11) % 12, 2)),
        _ => Some((base, 1)),
    }
}

/// Acorde lido: tônica, o resto do nome (m, 7, sus4…) e o baixo (C/G).
#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    pub root: usize,
    pub suffix: String,
    pub bass: Option<usize>,
}

pub fn parse_chord(s: &str) -> Option<Chord> {
    let s = s.trim().trim_matches(|c| c == '(' || c == ')');
    let (root, n) = note_index(s)?;
    let rest = &s[n..];
    let (suffix, bass) = match rest.rsplit_once('/') {
        Some((a, b)) if note_index(b).is_some_and(|(_, k)| k == b.len()) => (a, note_index(b).map(|x| x.0)),
        _ => (rest, None),
    };
    // Só caracteres de cifra no sufixo (evita confundir palavras com acordes).
    let ok = suffix.chars().all(|c| "m7M9ajsudi+°º4256b#()-1".contains(c) || c.is_ascii_digit());
    (ok && suffix.len() <= 7).then(|| Chord { root, suffix: suffix.to_string(), bass })
}

impl Chord {
    pub fn name(&self) -> String {
        let mut s = format!("{}{}", NOTES[self.root], self.suffix);
        if let Some(b) = self.bass {
            s.push('/');
            s.push_str(NOTES[b]);
        }
        s
    }
    pub fn transposed(&self, k: i32) -> Chord {
        let t = |n: usize| ((n as i32 + k).rem_euclid(12)) as usize;
        Chord { root: t(self.root), suffix: self.suffix.clone(), bass: self.bass.map(t) }
    }
    fn quality(&self) -> &'static str {
        let s = self.suffix.as_str();
        if s.contains("sus2") {
            return "sus2";
        }
        if s.contains("sus") {
            return "sus4";
        }
        let minor = s.starts_with('m') && !s.starts_with("maj");
        let maj7 = s.contains("maj7") || s.contains("7M");
        match (minor, s.contains('7'), maj7) {
            (true, true, _) => "m7",
            (true, false, _) => "m",
            (false, _, true) => "maj7",
            (false, true, false) => "7",
            _ => "",
        }
    }
}

/// Diagrama: casas por corda (Mi grave → Mi agudo; -1 abafada, 0 solta) e a casa inicial.
pub struct Shape {
    pub frets: [i8; 6],
    pub base: i8,
}

/// Posições abertas conhecidas (as que todo iniciante aprende primeiro).
fn open_shape(name: &str) -> Option<[i8; 6]> {
    Some(match name {
        "C" => [-1, 3, 2, 0, 1, 0],
        "C7" => [-1, 3, 2, 3, 1, 0],
        "Cmaj7" => [-1, 3, 2, 0, 0, 0],
        "D" => [-1, -1, 0, 2, 3, 2],
        "Dm" => [-1, -1, 0, 2, 3, 1],
        "D7" => [-1, -1, 0, 2, 1, 2],
        "Dm7" => [-1, -1, 0, 2, 1, 1],
        "Dmaj7" => [-1, -1, 0, 2, 2, 2],
        "E" => [0, 2, 2, 1, 0, 0],
        "Em" => [0, 2, 2, 0, 0, 0],
        "E7" => [0, 2, 0, 1, 0, 0],
        "Em7" => [0, 2, 0, 0, 0, 0],
        "G" => [3, 2, 0, 0, 0, 3],
        "G7" => [3, 2, 0, 0, 0, 1],
        "A" => [-1, 0, 2, 2, 2, 0],
        "Am" => [-1, 0, 2, 2, 1, 0],
        "A7" => [-1, 0, 2, 0, 2, 0],
        "Am7" => [-1, 0, 2, 0, 1, 0],
        "Amaj7" => [-1, 0, 2, 1, 2, 0],
        "B7" => [-1, 2, 1, 2, 0, 2],
        "Fmaj7" => [-1, -1, 3, 2, 1, 0],
        "Csus4" => [-1, 3, 3, 0, 1, 1],
        "Dsus4" => [-1, -1, 0, 2, 3, 3],
        "Dsus2" => [-1, -1, 0, 2, 3, 0],
        "Esus4" => [0, 2, 2, 2, 0, 0],
        "Gsus4" => [3, 3, 0, 0, 1, 3],
        "Asus4" => [-1, 0, 2, 2, 3, 0],
        "Asus2" => [-1, 0, 2, 2, 0, 0],
        _ => return None,
    })
}

pub fn shape(c: &Chord) -> Shape {
    let q = c.quality();
    let name = format!("{}{}", NOTES[c.root], q);
    if let Some(f) = open_shape(&name) {
        return Shape { frets: f, base: 1 };
    }
    // Pestana: forma de Mi (tônica na 6ª corda) ou de Lá (na 5ª), a que ficar mais perto do início.
    let e_form: [i8; 6] = match q {
        "m" => [0, 2, 2, 0, 0, 0],
        "7" => [0, 2, 0, 1, 0, 0],
        "m7" => [0, 2, 0, 0, 0, 0],
        "maj7" => [0, -1, 1, 1, 0, -1],
        "sus4" => [0, 2, 2, 2, 0, 0],
        "sus2" => [0, 2, 4, 4, 0, 0],
        _ => [0, 2, 2, 1, 0, 0],
    };
    let a_form: [i8; 6] = match q {
        "m" => [-1, 0, 2, 2, 1, 0],
        "7" => [-1, 0, 2, 0, 2, 0],
        "m7" => [-1, 0, 2, 0, 1, 0],
        "maj7" => [-1, 0, 2, 1, 2, 0],
        "sus4" => [-1, 0, 2, 2, 3, 0],
        "sus2" => [-1, 0, 2, 2, 0, 0],
        _ => [-1, 0, 2, 2, 2, 0],
    };
    let fe = ((c.root as i32 - 4).rem_euclid(12)) as i8;
    let fa = ((c.root as i32 - 9).rem_euclid(12)) as i8;
    let (form, fret) = if (fe <= fa && fe > 0) || fa == 0 { (e_form, fe) } else { (a_form, fa) };
    let fret = if fret == 0 { 12 } else { fret };
    let mut frets = form;
    for f in frets.iter_mut() {
        if *f >= 0 {
            *f += fret;
        }
    }
    Shape { frets, base: fret }
}

/// Diagrama de um acorde: nome em cima, seis cordas, cinco casas, bolinhas, X e O.
pub fn draw_chord(fb: &mut Framebuffer, f: &Fonts, r: Rect, c: &Chord, big: bool) {
    let sh = shape(c);
    let name = c.name();
    let fs = if big { 40.0 } else { 26.0 };
    let w = f.measure(Face::Display, fs, &name);
    f.draw(fb, Face::Display, fs, r.x + (r.w - w) / 2, r.y + fs as i32, INK, &name);
    let top = r.y + fs as i32 + if big { 56 } else { 30 };
    let (gw, gh) = (r.w * 3 / 4, r.h - (top - r.y) - 8);
    let x0 = r.x + (r.w - gw) / 2;
    let (sx, fy) = (gw / 5, gh / 5);
    let start = if sh.base <= 1 { 1 } else { sh.frets.iter().filter(|&&f| f > 0).min().copied().unwrap_or(1) };
    // Pestana mais perto do início: pestana grossa no topo quando a casa inicial é 1.
    if start == 1 {
        fb.fill(Rect::new(x0, top - 3, gw + 2, 6), INK);
    } else {
        let t = format!("{start}");
        f.draw(fb, Face::Strong, if big { 26.0 } else { 18.0 }, x0 - if big { 36 } else { 24 }, top + fy / 2 + 8, INK, &t);
    }
    for k in 0..6 {
        fb.fill(Rect::new(x0 + k * sx, top, 2, 5 * fy), INK);
    }
    for k in 0..=5 {
        fb.fill(Rect::new(x0, top + k * fy, gw + 2, 2), if k == 0 { INK } else { 0x88 });
    }
    let dot = (sx * 3 / 5).max(10);
    for (s, &fr) in sh.frets.iter().enumerate() {
        let cx = x0 + s as i32 * sx + 1;
        match fr {
            -1 => {
                let d = dot as f32 * 0.35;
                let cy = (top - dot / 2 - 6) as f32;
                ui::stroke(fb, (cx as f32 - d, cy - d), (cx as f32 + d, cy + d), 3.0, INK);
                ui::stroke(fb, (cx as f32 + d, cy - d), (cx as f32 - d, cy + d), 3.0, INK);
            }
            0 => fb.round_outline(Rect::new(cx - dot / 3, top - dot / 2 - 6 - dot / 3, 2 * (dot / 3), 2 * (dot / 3)), dot / 3, 2, INK, PAPER),
            n => {
                let row = (n - start) as i32;
                if (0..5).contains(&row) {
                    let cy = top + row * fy + fy / 2;
                    fb.round_rect(Rect::new(cx - dot / 2, cy - dot / 2, dot, dot), dot / 2, INK);
                }
            }
        }
    }
}

// ---------- Cifra ----------

enum Line {
    /// Acordes (coluna, acorde) sobre a letra.
    Sung(Vec<(usize, Chord)>, String),
    Text(String),
    Blank,
}

pub struct Song {
    pub path: PathBuf,
    pub title: String,
    lines: Vec<Line>,
}

fn chords_of(line: &str) -> Option<Vec<(usize, Chord)>> {
    let mut out = Vec::new();
    let mut col = 0;
    for part in line.split(' ') {
        if !part.is_empty() {
            let clean = part.trim_matches(|c| c == '|' || c == '-' || c == '.');
            if clean.is_empty() || clean.eq_ignore_ascii_case("x2") || clean.eq_ignore_ascii_case("(2x)") {
                // marcações de compasso e repetição
            } else {
                out.push((line[..col].chars().count(), parse_chord(clean)?));
            }
        }
        col += part.len() + 1;
    }
    (!out.is_empty()).then_some(out)
}

impl Song {
    pub fn parse(path: PathBuf, text: &str) -> Song {
        let mut it = text.lines();
        let title = it.next().unwrap_or("Música").trim().to_string();
        let raw: Vec<&str> = it.collect();
        let mut lines = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            let l = raw[i].trim_end();
            if l.trim().is_empty() {
                if !matches!(lines.last(), Some(Line::Blank) | None) {
                    lines.push(Line::Blank);
                }
            } else if let Some(ch) = chords_of(l) {
                // A letra é a linha seguinte, se não for outra linha de acordes.
                let next = raw.get(i + 1).map(|s| s.trim_end()).filter(|s| !s.trim().is_empty() && chords_of(s).is_none());
                lines.push(Line::Sung(ch, next.unwrap_or("").to_string()));
                if next.is_some() {
                    i += 1;
                }
            } else {
                lines.push(Line::Text(l.to_string()));
            }
            i += 1;
        }
        Song { path, title, lines }
    }

    /// Acordes da música, na ordem em que aparecem (sem repetir).
    pub fn chords(&self) -> Vec<Chord> {
        let mut v: Vec<Chord> = Vec::new();
        for l in &self.lines {
            if let Line::Sung(cs, _) = l {
                for (_, c) in cs {
                    if !v.contains(c) {
                        v.push(c.clone());
                    }
                }
            }
        }
        v
    }
}

fn dir() -> PathBuf {
    let d = crate::home_dir().join("data/violao/cifras");
    let _ = std::fs::create_dir_all(&d);
    d
}

pub fn songs() -> Vec<Song> {
    let mut v: Vec<(std::time::SystemTime, Song)> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| {
            let t = e.metadata().ok()?.modified().ok()?;
            Some((t, Song::parse(e.path(), &std::fs::read_to_string(e.path()).ok()?)))
        })
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().map(|(_, s)| s).collect()
}

/// Música de exemplo (exercício autoral, sem letra de terceiros), para a primeira vez.
const EXEMPLO: &str = "Primeiros acordes (exercício)\n\nEm               G\nUm, dois, três, quatro, troca devagar\nD                A\nOlha a mão esquerda, deixa o som soar\n\nEm      C        G        D\nQuatro batidas em cada acorde\nEm      C        D\nRepete até sair sem olhar\n";

// ---------- Treino: um minuto de trocas ----------

pub const PAIRS: [(&str, &str); 8] = [("Em", "G"), ("Am", "C"), ("C", "G"), ("G", "D"), ("D", "A"), ("A", "E"), ("Am", "Dm"), ("Em", "Am")];

fn log_path() -> PathBuf {
    let d = crate::home_dir().join("data/violao");
    let _ = std::fs::create_dir_all(&d);
    d.join("trocas.txt")
}

fn today() -> i64 {
    crate::fases::today()
}

/// Registro: (dia, par, trocas).
fn log() -> Vec<(i64, usize, u32)> {
    std::fs::read_to_string(log_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.split('|');
            Some((p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?))
        })
        .collect()
}

fn best(pair: usize) -> Option<u32> {
    log().iter().filter(|(_, p, _)| *p == pair).map(|(_, _, n)| *n).max()
}

/// Dias seguidos de treino (até hoje ou ontem).
pub fn streak() -> u32 {
    let days: std::collections::BTreeSet<i64> = log().iter().map(|(d, _, _)| *d).collect();
    let mut d = today();
    if !days.contains(&d) {
        d -= 1;
    }
    let mut n = 0;
    while days.contains(&d) {
        n += 1;
        d -= 1;
    }
    n
}

/// Dias com treino de trocas entre `from` e `to` (inclusive).
pub fn practice_days(from: i64, to: i64) -> usize {
    let days: std::collections::BTreeSet<i64> = log().iter().map(|(d, _, _)| *d).filter(|d| (from..=to).contains(d)).collect();
    days.len()
}

pub fn practiced_today() -> bool {
    log().iter().any(|(d, _, _)| *d == today())
}

/// O par sugerido hoje: o de pior recorde (ou nunca treinado).
fn suggested() -> usize {
    (0..PAIRS.len()).min_by_key(|&k| best(k).unwrap_or(0)).unwrap_or(0)
}

// ---------- Telas ----------

#[derive(Clone, Copy, PartialEq)]
enum View {
    Hub,
    List,
    Song,
    Dict,
    Practice,
    Course,
    Module,
    Lesson,
    Check,
    Quiz,
}

#[derive(Clone, Copy, PartialEq)]
enum Timer {
    Idle,
    Running(std::time::Instant),
    Count(u32),
}

pub struct Violao {
    view: View,
    songs: Vec<Song>,
    song: usize,
    page: usize,
    shift: i32,
    list_page: usize,
    dict_root: usize,
    pair: usize,
    timer: Timer,
    shown_secs: u64,
    module: usize,
    lesson: usize,
    answered: Option<usize>,
    /// Aberto direto no curso (tópico Partitura): voltar do curso sai do módulo.
    course_root: bool,
    quiz: Quiz,
    /// Comemoração por cima (ofensiva cresceu, meta batida).
    cel: Option<crate::aprender::Cel>,
}

/// Treinos de leitura: modo (notas, ritmo, armaduras, intervalos), nível, pergunta, acertos
/// seguidos e o último erro.
struct Quiz {
    mode: usize,
    level: usize,
    q: crate::partitura::Q,
    streak: u32,
    wrong: Option<&'static str>,
    rng: crate::minigame::Rng,
}

impl Quiz {
    fn next(&mut self) {
        self.q = crate::partitura::Q::new(self.mode, self.level, &mut self.rng, Some(self.q));
    }
}

const SECS: u64 = 60;

impl Violao {
    pub fn new() -> Self {
        let mut songs = songs();
        if songs.is_empty() {
            let p = dir().join("primeiros-acordes.txt");
            let _ = std::fs::write(&p, EXEMPLO);
            songs = self::songs();
        }
        let mut rng = crate::minigame::Rng::new();
        let q = crate::partitura::Q::new(0, 0, &mut rng, None);
        let quiz = Quiz { mode: 0, level: 0, q, streak: 0, wrong: None, rng };
        let lesson = crate::partitura::next_lesson();
        Violao { view: View::Hub, songs, song: 0, page: 0, shift: 0, list_page: 0, dict_root: 0, pair: suggested(), timer: Timer::Idle, shown_secs: 0, module: crate::partitura::locate(lesson).0, lesson, answered: None, course_root: false, quiz, cel: None }
    }

    /// Abre direto no curso de partitura (para o tópico Partitura da seção Aprender).
    pub fn course() -> Self {
        let mut v = Self::new();
        v.view = View::Course;
        v.course_root = true;
        v
    }

    fn hub_card(k: usize) -> Rect {
        Rect::new(MARGIN, 450 + k as i32 * 218, W - 2 * MARGIN, 196)
    }

    fn draw_hub(&self, fb: &mut Framebuffer, f: &Fonts) {
        crate::sprites::draw(fb, "cab-violao", Rect::new(W - MARGIN - 330, 90, 340, 270));
        f.draw(fb, Face::Display, 130.0, MARGIN - 6, 300, INK, "Violão");
        let s = streak();
        let sub = match s {
            0 => "Um pouco todo dia vale mais que muito de vez em quando.".to_string(),
            1 => "1 dia de treino seguido. Amanhã vira sequência.".to_string(),
            n => format!("{n} dias seguidos de treino. Continua!"),
        };
        for (k, l) in crate::chat::wrap_words(f, Face::Strong, 28.0, &sub, W - 2 * MARGIN - 340).iter().take(2).enumerate() {
            f.draw(fb, Face::Strong, 28.0, MARGIN, 350 + k as i32 * 36, MUTED, l);
        }
        let (a, b) = PAIRS[self.pair];
        let practice_sub = if practiced_today() { format!("Feito hoje. Quer mais um? {a} ↔ {b}") } else { format!("Hoje: {a} ↔ {b}, um minuto") };
        let cards = [
            ("Treino do dia", practice_sub, true),
            ("Músicas", format!("{} cifra{} · mande pelo celular", self.songs.len(), if self.songs.len() == 1 { "" } else { "s" }), false),
            ("Acordes", "Diagramas de todos os tons".to_string(), false),
            ("Aprender partitura", {
                let st = crate::aprender::status();
                let fogo = if st.streak > 0 { format!("Ofensiva de {} dia{}{} · ", st.streak, if st.streak == 1 { "" } else { "s" }, if st.today_done { "" } else { ", falta hoje" }) } else { String::new() };
                format!("{fogo}{} de {} lições", crate::partitura::done().len().min(crate::partitura::total()), crate::partitura::total())
            }, false),
        ];
        for (k, (t, s, main)) in cards.iter().enumerate() {
            let r = Self::hub_card(k);
            ui::card(fb, r, 32, if *main { INK } else { PAPER }, false);
            let (fg, sub) = if *main { (PAPER, 0xCC) } else { (INK, MUTED) };
            f.draw(fb, Face::Display, 58.0, r.x + 40, r.y + 92, fg, t);
            f.draw(fb, Face::Body, 28.0, r.x + 42, r.y + 142, sub, &crate::apps::fit(f, Face::Body, 28.0, s, r.w - 340));
            // Os acordes do treino desenhados no cartão.
            if *main {
                for (j, n) in [a, b].iter().enumerate() {
                    let cr = Rect::new(r.x + r.w - 300 + j as i32 * 140, r.y + 12, 130, 172);
                    fb.round_rect(cr, 18, PAPER);
                    draw_chord(fb, f, Rect::new(cr.x + 6, cr.y + 4, cr.w - 12, cr.h - 10), &parse_chord(n).unwrap(), false);
                }
            }
        }
    }

    // ----- lista de músicas -----

    fn list_row(k: usize) -> Rect {
        Rect::new(MARGIN, 300 + k as i32 * 130, W - 2 * MARGIN, 112)
    }

    fn draw_list(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Display, 90.0, MARGIN - 4, 250, INK, "Músicas");
        for (k, s) in self.songs.iter().skip(self.list_page * 7).take(7).enumerate() {
            let r = Self::list_row(k);
            ui::card(fb, r, 26, PAPER, false);
            f.draw(fb, Face::Display, 36.0, r.x + 28, r.y + 50, INK, &crate::apps::fit(f, Face::Display, 36.0, &s.title, r.w - 56));
            let cs: Vec<String> = s.chords().iter().take(8).map(Chord::name).collect();
            f.draw(fb, Face::Body, 24.0, r.x + 30, r.y + 88, MUTED, &crate::apps::fit(f, Face::Body, 24.0, &cs.join("  "), r.w - 60));
        }
        let tip = "Para mandar: no celular, abra a página Mandar para o Kindle e cole a cifra (acordes e letra).";
        for (k, l) in crate::chat::wrap_words(f, Face::Body, 26.0, tip, W - 2 * MARGIN).iter().take(2).enumerate() {
            f.draw(fb, Face::Body, 26.0, MARGIN, 1250 + k as i32 * 34, MUTED, l);
        }
        let pages = self.songs.len().div_ceil(7);
        if self.list_page > 0 {
            ui::button(fb, f, Self::nav(0), "‹ Anterior", false);
        }
        if self.list_page + 1 < pages {
            ui::button(fb, f, Self::nav(1), "Próxima ›", false);
        }
    }

    fn nav(k: usize) -> Rect {
        Rect::new(if k == 0 { MARGIN } else { W - MARGIN - 250 }, 1330, 250, 86)
    }

    // ----- cifra -----

    fn tone_rect(k: usize) -> Rect {
        Rect::new(W - MARGIN - 2 * 86 - 12 + k as i32 * (86 + 12), 176, 86, 70)
    }

    /// Páginas da cifra: (linha inicial, final) com as alturas reais.
    fn song_pages(&self, f: &Fonts, s: &Song, size: f32) -> Vec<(usize, usize)> {
        let lh = (size * 1.35) as i32;
        let (top, bottom) = (500, 1300);
        let mut pages = vec![];
        let (mut start, mut y) = (0, top);
        for (i, l) in s.lines.iter().enumerate() {
            let h = match l {
                Line::Sung(..) => 2 * lh - 4,
                Line::Text(_) => lh,
                Line::Blank => lh / 2,
            };
            if y + h > bottom && i > start {
                pages.push((start, i));
                start = i;
                y = top;
            }
            y += h;
        }
        pages.push((start, s.lines.len()));
        let _ = f;
        pages
    }

    /// Tamanho de letra que faz a linha mais longa caber na largura.
    fn song_size(f: &Fonts, s: &Song) -> f32 {
        let longest = s.lines.iter().map(|l| match l {
            Line::Sung(_, t) | Line::Text(t) => f.measure(Face::Body, 30.0, t),
            Line::Blank => 0,
        });
        let w = longest.max().unwrap_or(0).max(1) as f32;
        (30.0 * (W - 2 * MARGIN) as f32 / w).clamp(20.0, 34.0)
    }

    fn draw_song(&self, fb: &mut Framebuffer, f: &Fonts) {
        let Some(s) = self.songs.get(self.song) else { return };
        let title = crate::apps::fit(f, Face::Display, 46.0, &s.title, W - 2 * MARGIN - 220);
        f.draw(fb, Face::Display, 46.0, MARGIN, 226, INK, &title);
        let tone = match self.shift {
            0 => "tom original".to_string(),
            k => format!("{}{} semitom{}", if k > 0 { "+" } else { "" }, k, if k.abs() == 1 { "" } else { "s" }),
        };
        f.draw(fb, Face::Strong, 24.0, MARGIN, 266, MUTED, &tone);
        ui::button(fb, f, Self::tone_rect(0), "−", false);
        ui::button(fb, f, Self::tone_rect(1), "+", false);
        // Faixa de diagramas dos acordes da música (até 7).
        let chords: Vec<Chord> = s.chords().into_iter().map(|c| c.transposed(self.shift)).take(7).collect();
        let cw = (W - 2 * MARGIN) / 7;
        fb.round_rect(Rect::new(MARGIN - 12, 286, W - 2 * MARGIN + 24, 190), 20, ui::SURFACE);
        for (k, c) in chords.iter().enumerate() {
            draw_chord(fb, f, Rect::new(MARGIN + k as i32 * cw, 292, cw, 180), c, false);
        }
        let size = Self::song_size(f, s);
        let lh = (size * 1.35) as i32;
        let pages = self.song_pages(f, s, size);
        let page = self.page.min(pages.len() - 1);
        let (a, b) = pages[page];
        let mut y = 500;
        for l in &s.lines[a..b] {
            match l {
                Line::Sung(cs, lyric) => {
                    for (col, c) in cs {
                        // A posição do acorde é a largura da letra até aquela coluna.
                        let prefix: String = lyric.chars().take(*col).collect();
                        let pad = col.saturating_sub(lyric.chars().count());
                        let x = MARGIN + f.measure(Face::Body, size, &prefix) + pad as i32 * f.measure(Face::Body, size, " ");
                        f.draw(fb, Face::Strong, size * 0.92, x, y + lh - 8, INK, &c.transposed(self.shift).name());
                    }
                    f.draw(fb, Face::Body, size, MARGIN, y + 2 * lh - 12, 0x33, lyric);
                    y += 2 * lh - 4;
                }
                Line::Text(t) => {
                    f.draw(fb, Face::Strong, size, MARGIN, y + lh - 8, MUTED, t);
                    y += lh;
                }
                Line::Blank => y += lh / 2,
            }
        }
        if pages.len() > 1 {
            if page > 0 {
                ui::button(fb, f, Self::nav(0), "‹ Anterior", false);
            }
            if page + 1 < pages.len() {
                ui::button(fb, f, Self::nav(1), "Próxima ›", true);
            }
            let t = format!("{} de {}", page + 1, pages.len());
            let w = f.measure(Face::Strong, 26.0, &t);
            f.draw(fb, Face::Strong, 26.0, (W - w) / 2, 1384, MUTED, &t);
        }
    }

    // ----- dicionário -----

    fn root_rect(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 5 * 10) / 6;
        Rect::new(MARGIN + (k % 6) as i32 * (w + 10), 280 + (k / 6) as i32 * 90, w, 76)
    }

    fn draw_dict(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Display, 90.0, MARGIN - 4, 236, INK, "Acordes");
        for k in 0..12 {
            ui::button(fb, f, Self::root_rect(k), NOTES[k], k == self.dict_root);
        }
        let quals = ["", "m", "7", "m7", "maj7", "sus4"];
        for (k, q) in quals.iter().enumerate() {
            let r = Rect::new(MARGIN + (k % 3) as i32 * ((W - 2 * MARGIN) / 3), 490 + (k / 3) as i32 * 400, (W - 2 * MARGIN) / 3, 380);
            let c = Chord { root: self.dict_root, suffix: q.to_string(), bass: None };
            draw_chord(fb, f, Rect::new(r.x + 20, r.y + 10, r.w - 40, r.h - 20), &c, true);
        }
    }

    // ----- treino -----

    fn pair_rect(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 3 * 12) / 4;
        Rect::new(MARGIN + (k % 4) as i32 * (w + 12), 250 + (k / 4) as i32 * 92, w, 78)
    }

    fn practice_btn(k: usize) -> Rect {
        match k {
            0 => Rect::new(MARGIN, 1290, W - 2 * MARGIN, 110),
            1 => Rect::new(MARGIN, 1160, 200, 110),
            _ => Rect::new(W - MARGIN - 200, 1160, 200, 110),
        }
    }

    fn draw_practice(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Strong, 28.0, MARGIN, 220, MUTED, "UM MINUTO DE TROCAS · ESCOLHA O PAR");
        for (k, (a, b)) in PAIRS.iter().enumerate() {
            ui::button(fb, f, Self::pair_rect(k), &format!("{a} ↔ {b}"), k == self.pair);
        }
        let (a, b) = PAIRS[self.pair];
        for (j, n) in [a, b].iter().enumerate() {
            let r = Rect::new(MARGIN + j as i32 * ((W - 2 * MARGIN) / 2), 460, (W - 2 * MARGIN) / 2, 440);
            draw_chord(fb, f, Rect::new(r.x + 40, r.y, r.w - 80, r.h), &parse_chord(n).unwrap(), true);
        }
        let rec = best(self.pair).map(|n| format!("Seu recorde neste par: {n} trocas")).unwrap_or_else(|| "Primeira vez neste par.".into());
        match self.timer {
            Timer::Idle => {
                for (k, l) in crate::chat::wrap_words(f, Face::Body, 30.0, "Troque entre os dois acordes o mais rápido que conseguir, sem pressa de soar perfeito. Conte as trocas.", W - 2 * MARGIN).iter().enumerate() {
                    f.draw(fb, Face::Body, 30.0, MARGIN, 960 + k as i32 * 42, INK, l);
                }
                f.draw(fb, Face::Strong, 30.0, MARGIN, 1100, INK, &rec);
                ui::button(fb, f, Self::practice_btn(0), "Começar 1 minuto", true);
            }
            Timer::Running(t) => {
                let left = SECS.saturating_sub(t.elapsed().as_secs());
                let s = format!("{left}");
                let w = f.measure(Face::Display, 200.0, &s);
                f.draw(fb, Face::Display, 200.0, (W - w) / 2, 1140, INK, &s);
                let t2 = "segundos · troque, troque, troque";
                let w2 = f.measure(Face::Strong, 30.0, t2);
                f.draw(fb, Face::Strong, 30.0, (W - w2) / 2, 1200, MUTED, t2);
                ui::button(fb, f, Self::practice_btn(0), "Parar", false);
            }
            Timer::Count(n) => {
                let t = "Quantas trocas você fez?";
                f.draw(fb, Face::Display, 50.0, MARGIN, 990, INK, t);
                let s = format!("{n}");
                let w = f.measure(Face::Display, 150.0, &s);
                f.draw(fb, Face::Display, 150.0, (W - w) / 2, 1250, INK, &s);
                ui::button(fb, f, Self::practice_btn(1), "−", false);
                ui::button(fb, f, Self::practice_btn(2), "+", false);
                f.draw(fb, Face::Strong, 26.0, MARGIN, 1060, MUTED, &rec);
                ui::button(fb, f, Self::practice_btn(0), "Salvar", true);
            }
        }
    }
}

impl Violao {
    fn module_row(k: usize) -> Rect {
        Rect::new(MARGIN, 478 + k as i32 * 126, W - 2 * MARGIN, 112)
    }

    fn lesson_row(k: usize) -> Rect {
        Rect::new(MARGIN, 300 + k as i32 * 76, W - 2 * MARGIN, 66)
    }

    fn course_btn() -> Rect {
        Rect::new(MARGIN, 1250, W - 2 * MARGIN, 110)
    }

    fn draw_course(&self, fb: &mut Framebuffer, f: &Fonts) {
        use crate::partitura as p;
        f.draw(fb, Face::Display, 80.0, MARGIN - 4, 236, INK, "Partitura");
        let seen = p::done().len().min(p::total());
        let t = format!("{seen} de {} lições", p::total());
        let w = f.measure(Face::Strong, 26.0, &t);
        f.draw(fb, Face::Strong, 26.0, W - MARGIN - w, 232, MUTED, &t);
        crate::aprender::draw_panel(fb, f, Rect::new(MARGIN, 270, W - 2 * MARGIN, 180), &crate::aprender::status());
        for (k, m) in p::MODULES.iter().enumerate() {
            let r = Self::module_row(k);
            let (n, total) = (p::done_in(k), m.lessons.len());
            let complete = n == total;
            ui::card(fb, r, 28, if complete { ui::SURFACE } else { PAPER }, false);
            // Número do módulo num disco.
            let d = 70;
            fb.round_rect(Rect::new(r.x + 22, r.y + (r.h - d) / 2, d, d), d / 2, INK);
            let num = format!("{}", k + 1);
            let nw = f.measure(Face::Display, 40.0, &num);
            f.draw(fb, Face::Display, 40.0, r.x + 22 + (d - nw) / 2, r.y + r.h / 2 + 15, PAPER, &num);
            let tx = r.x + 22 + d + 24;
            f.draw(fb, Face::Display, 38.0, tx, r.y + 52, INK, m.title);
            f.draw(fb, Face::Body, 24.0, tx, r.y + 88, MUTED, &crate::apps::fit(f, Face::Body, 24.0, m.sub, r.x + r.w - 150 - tx));
            // Progresso: número e barrinha.
            let prog = format!("{n}/{total}");
            let pw = f.measure(Face::Strong, 28.0, &prog);
            f.draw(fb, Face::Strong, 28.0, r.x + r.w - 28 - pw, r.y + 52, INK, &prog);
            let bar = Rect::new(r.x + r.w - 128, r.y + 72, 100, 12);
            fb.round_rect(bar, 6, 0xC8);
            if n > 0 {
                fb.round_rect(Rect::new(bar.x, bar.y, (bar.w * n as i32 / total as i32).max(12), bar.h), 6, INK);
            }
        }
        ui::button(fb, f, Self::course_btn(), "Treinos de leitura", true);
    }

    fn draw_module(&self, fb: &mut Framebuffer, f: &Fonts) {
        use crate::partitura as p;
        let m = &p::MODULES[self.module];
        f.draw(fb, Face::Strong, 26.0, MARGIN, 196, MUTED, &format!("MÓDULO {} DE {}", self.module + 1, p::MODULES.len()));
        f.draw(fb, Face::Display, 64.0, MARGIN - 2, 264, INK, m.title);
        let done = p::done();
        let base = p::first(self.module);
        let next = (0..m.lessons.len()).find(|k| !done.contains(&(base + k)));
        // Trilha: bolinhas em zigue-zague ligadas por um traço, o título ao lado.
        let centers: Vec<(i32, i32)> = (0..m.lessons.len()).map(|k| {
            let r = Self::lesson_row(k);
            const ZIG: [i32; 6] = [0, 44, 88, 110, 88, 44];
            (r.x + 40 + ZIG[k % 6], r.y + r.h / 2)
        }).collect();
        for w in centers.windows(2) {
            ui::stroke(fb, (w[0].0 as f32, w[0].1 as f32), (w[1].0 as f32, w[1].1 as f32), 6.0, 0xB8);
        }
        for (k, l) in m.lessons.iter().enumerate() {
            let (cx, cy) = centers[k];
            let seen = done.contains(&(base + k));
            let is_next = next == Some(k);
            let d = if is_next { 70 } else { 58 };
            let nr = Rect::new(cx - d / 2, cy - d / 2, d, d);
            if is_next {
                fb.round_rect(Rect::new(nr.x - 8, nr.y - 8, nr.w + 16, nr.h + 16), d / 2 + 8, 0xC8);
                fb.round_rect(nr, d / 2, INK);
                let t = format!("{}", k + 1);
                let tw = f.measure(Face::Display, 32.0, &t);
                f.draw(fb, Face::Display, 32.0, cx - tw / 2, cy + 12, PAPER, &t);
            } else if seen {
                fb.round_rect(nr, d / 2, INK);
                ui::stroke(fb, ((cx - 13) as f32, (cy + 1) as f32), ((cx - 3) as f32, (cy + 11) as f32), 6.0, PAPER);
                ui::stroke(fb, ((cx - 3) as f32, (cy + 11) as f32), ((cx + 15) as f32, (cy - 10) as f32), 6.0, PAPER);
            } else {
                fb.round_outline(nr, d / 2, 4, 0x88, PAPER);
                let t = format!("{}", k + 1);
                let tw = f.measure(Face::Strong, 26.0, &t);
                f.draw(fb, Face::Strong, 26.0, cx - tw / 2, cy + 10, MUTED, &t);
            }
            let tx = Self::lesson_row(0).x + 40 + 110 + 64;
            let face = if is_next { Face::Display } else { Face::Strong };
            let size = if is_next { 32.0 } else { 28.0 };
            let title = crate::apps::fit(f, face, size, l.title, W - MARGIN - tx - if is_next { 150 } else { 0 });
            f.draw(fb, face, size, tx, cy + 10, if seen || is_next { INK } else { MUTED }, &title);
            if is_next {
                let r = Rect::new(W - MARGIN - 140, cy - 26, 140, 52);
                fb.round_rect(r, 26, INK);
                let t = if seen { "rever" } else { "começar" };
                let tw = f.measure(Face::Strong, 22.0, t);
                f.draw(fb, Face::Strong, 22.0, r.x + (r.w - tw) / 2, r.y + 34, PAPER, t);
            }
        }
    }

    fn draw_lesson(&self, fb: &mut Framebuffer, f: &Fonts) {
        use crate::partitura as p;
        let l = p::lesson(self.lesson);
        let (m, k) = p::locate(self.lesson);
        f.draw(fb, Face::Strong, 26.0, MARGIN, 210, MUTED, &format!("MÓDULO {} · LIÇÃO {} DE {}", m + 1, k + 1, p::MODULES[m].lessons.len()));
        let mut tsize = 52.0;
        while f.measure(Face::Display, tsize, l.title) > W - 2 * MARGIN && tsize > 36.0 {
            tsize -= 2.0;
        }
        f.draw(fb, Face::Display, tsize, MARGIN, 276, INK, l.title);
        // Texto: diminui um pouco se não couber junto com a ilustração.
        let nav_top = Self::nav(0).y;
        let mut size = 31.0;
        let mut lines: Vec<Vec<String>>;
        loop {
            lines = l.text.iter().map(|t| crate::chat::wrap_words(f, Face::Body, size, t, W - 2 * MARGIN)).collect();
            let lh = (size * 1.35) as i32;
            let h: i32 = lines.iter().map(|ls| ls.len() as i32 * lh + 14).sum();
            if 316 + h + 40 + 330 <= nav_top - 24 || size <= 26.0 {
                break;
            }
            size -= 1.0;
        }
        let lh = (size * 1.35) as i32;
        let mut y = 316;
        for ls in &lines {
            for line in ls {
                f.draw(fb, Face::Body, size, MARGIN, y + lh - 10, INK, line);
                y += lh;
            }
            y += 14;
        }
        let top = y + 24;
        let h = (nav_top - 34 - top).clamp(300, 380);
        let demo = Rect::new(MARGIN, top + (nav_top - 34 - top - h).max(0) / 2, W - 2 * MARGIN, h);
        fb.round_rect(Rect::new(demo.x - 14, demo.y - 10, demo.w + 28, demo.h + 20), 24, ui::SURFACE);
        p::draw_demo(fb, f, Rect::new(demo.x + 6, demo.y, demo.w - 12, demo.h), &l.demo);
        if k > 0 {
            ui::button(fb, f, Self::nav(0), "‹ Anterior", false);
        }
        ui::button(fb, f, Self::nav(1), "Pergunta ›", true);
    }

    fn check_next() -> Rect {
        Rect::new(W - MARGIN - 340, Self::nav(1).y, 340, Self::nav(1).h)
    }

    fn option_rect(k: usize) -> Rect {
        Rect::new(MARGIN, 560 + k as i32 * 128, W - 2 * MARGIN, 108)
    }

    fn draw_check(&self, fb: &mut Framebuffer, f: &Fonts) {
        use crate::partitura as p;
        let l = p::lesson(self.lesson);
        let c = &l.check;
        f.draw(fb, Face::Strong, 26.0, MARGIN, 210, MUTED, &format!("PERGUNTA · {}", l.title.to_uppercase()));
        for (k, line) in crate::chat::wrap_words(f, Face::Display, 48.0, c.q, W - 2 * MARGIN).iter().take(4).enumerate() {
            f.draw(fb, Face::Display, 48.0, MARGIN, 300 + k as i32 * 62, INK, line);
        }
        for (k, o) in c.opts.iter().enumerate() {
            let r = Self::option_rect(k);
            match self.answered {
                Some(_) if k == c.ok => ui::button(fb, f, r, o, true),
                Some(a) if a == k => {
                    ui::card(fb, r, 40, ui::SURFACE, false);
                    let t = format!("{o}  (sua resposta)");
                    let w = f.measure(Face::Strong, 32.0, &t);
                    f.draw(fb, Face::Strong, 32.0, r.x + (r.w - w) / 2, r.y + r.h / 2 + 12, MUTED, &t);
                }
                _ => ui::button(fb, f, r, o, false),
            }
        }
        if let Some(a) = self.answered {
            let head = if a == c.ok { "Isso! " } else { "Quase. A certa está em preto. " };
            let msg = format!("{head}{}", c.why);
            let y0 = Self::option_rect(c.opts.len()).y + 20;
            for (k, line) in crate::chat::wrap_words(f, Face::Body, 31.0, &msg, W - 2 * MARGIN).iter().take(4).enumerate() {
                f.draw(fb, if k == 0 { Face::Strong } else { Face::Body }, 31.0, MARGIN, y0 + 30 + k as i32 * 44, INK, line);
            }
            let (m, k) = p::locate(self.lesson);
            let last_in_module = k + 1 == p::MODULES[m].lessons.len();
            let label = if self.lesson + 1 == p::total() { "Treinos ›" } else if last_in_module { "Fim do módulo ›" } else { "Próxima lição ›" };
            ui::button(fb, f, Self::check_next(), label, true);
        } else {
            let t = "Toque na resposta.";
            f.draw(fb, Face::Strong, 28.0, MARGIN, Self::option_rect(c.opts.len()).y + 50, MUTED, t);
        }
        ui::button(fb, f, Self::nav(0), "‹ Lição", false);
    }

    /// Depois da pergunta: a próxima lição do módulo, ou de volta à lista ao fechar o módulo.
    fn advance(&mut self) {
        use crate::partitura as p;
        let (m, k) = p::locate(self.lesson);
        self.answered = None;
        if self.lesson + 1 == p::total() {
            self.view = View::Quiz;
        } else if k + 1 == p::MODULES[m].lessons.len() {
            self.module = (m + 1).min(p::MODULES.len() - 1);
            self.lesson += 1;
            self.view = View::Course;
        } else {
            self.lesson += 1;
            p::mark_done(self.lesson);
            self.view = View::Lesson;
        }
    }

    fn answer_rect(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 3 * 14) / 4;
        Rect::new(MARGIN + (k % 4) as i32 * (w + 14), 1100 + (k / 4) as i32 * 118, w, 100)
    }

    fn mode_rect(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 3 * 12) / 4;
        Rect::new(MARGIN + k as i32 * (w + 12), 236, w, 74)
    }

    fn level_rect(k: usize) -> Rect {
        let n = crate::partitura::LEVELS.len() as i32;
        let w = (W - 2 * MARGIN - (n - 1) * 10) / n;
        Rect::new(MARGIN + k as i32 * (w + 10), 330, w, 62)
    }

    fn draw_quiz(&self, fb: &mut Framebuffer, f: &Fonts) {
        use crate::partitura as p;
        let q = &self.quiz;
        f.draw(fb, Face::Strong, 26.0, MARGIN, 210, MUTED, &format!("TREINO · {}", p::prompt(q.mode)));
        for (k, name) in p::MODES.iter().enumerate() {
            ui::button(fb, f, Self::mode_rect(k), name, k == q.mode);
        }
        if q.mode == 0 {
            for (k, (name, _, _)) in p::LEVELS.iter().enumerate() {
                let r = Self::level_rect(k);
                if k == q.level {
                    ui::card(fb, r, 31, ui::SURFACE, false);
                    fb.round_rect(Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8), 27, ui::SURFACE);
                    fb.round_outline(Rect::new(r.x + 4, r.y + 4, r.w - 8, r.h - 8), 27, 3, INK, ui::SURFACE);
                } else {
                    ui::card(fb, r, 31, PAPER, false);
                }
                let mut size = 24.0;
                while f.measure(Face::Strong, size, name) > r.w - 14 && size > 16.0 {
                    size -= 1.0;
                }
                let w = f.measure(Face::Strong, size, name);
                f.draw(fb, Face::Strong, size, r.x + (r.w - w) / 2, r.y + r.h / 2 + 9, INK, name);
            }
        } else {
            let t = p::tip(q.mode);
            f.draw(fb, Face::Body, 27.0, MARGIN, 370, MUTED, &crate::apps::fit(f, Face::Body, 27.0, t, W - 2 * MARGIN));
        }
        let r = Rect::new(MARGIN, 420, W - 2 * MARGIN, 520);
        fb.round_rect(r, 30, ui::SURFACE);
        q.q.draw(fb, r);
        let key = q.q.record_key();
        let msg = match q.wrong {
            Some(picked) => format!("Era {}. Você tocou {picked}. {}", q.q.answer(), p::tip(q.mode)),
            None if q.streak > 0 => format!("{} acerto{} seguido{} · recorde {}", q.streak, if q.streak == 1 { "" } else { "s" }, if q.streak == 1 { "" } else { "s" }, p::best(key).max(q.streak)),
            None => format!("Recorde aqui: {} seguidos", p::best(key)),
        };
        for (k, l) in crate::chat::wrap_words(f, Face::Strong, 30.0, &msg, W - 2 * MARGIN).iter().take(2).enumerate() {
            f.draw(fb, Face::Strong, 30.0, MARGIN, 994 + k as i32 * 40, if q.wrong.is_some() { INK } else { MUTED }, l);
        }
        for (k, a) in p::answers(q.mode).iter().enumerate() {
            ui::button(fb, f, Self::answer_rect(k), a, false);
        }
    }

    /// Para os testes de tela: abre uma lição (e a pergunta dela, já respondida).
    #[cfg(test)]
    pub fn open_lesson(&mut self, g: usize, check: Option<usize>) {
        self.lesson = g;
        self.module = crate::partitura::locate(g).0;
        self.answered = check;
        self.view = if check.is_some() { View::Check } else { View::Lesson };
    }

    #[cfg(test)]
    pub fn open_quiz(&mut self, mode: usize, level: usize) {
        self.quiz.mode = mode;
        self.quiz.level = level;
        self.quiz.next();
        self.view = View::Quiz;
    }
}

impl MiniGame for Violao {
    fn at_menu(&self) -> bool {
        self.view == View::Hub
    }

    fn summary(&self) -> String {
        match self.view {
            View::Song => self.songs.get(self.song).map(|s| s.title.clone()).unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn context(&self) -> String {
        match self.view {
            View::Song => {
                let s = &self.songs[self.song];
                let cs: Vec<String> = s.chords().iter().map(|c| c.transposed(self.shift).name()).collect();
                format!("Tocando violão: a cifra \"{}\" (acordes: {}), transposta {} semitons. O Pedro está aprendendo violão.", s.title, cs.join(", "), self.shift)
            }
            View::Practice => {
                let (a, b) = PAIRS[self.pair];
                format!("Treinando trocas de acorde no violão: {a} e {b}, um minuto. Recorde: {:?}. O Pedro está aprendendo.", best(self.pair))
            }
            View::Lesson | View::Check => {
                let l = crate::partitura::lesson(self.lesson);
                let (m, _) = crate::partitura::locate(self.lesson);
                format!("Curso de partitura no Kindle, módulo \"{}\", lição \"{}\": {} O Pedro lê cifra e está aprendendo partitura do zero; explique simples, com exemplos no violão.", crate::partitura::MODULES[m].title, l.title, l.text.join(" "))
            }
            View::Quiz => format!("Treino de leitura de partitura ({}), no Kindle. O Pedro está aprendendo partitura do zero.", crate::partitura::MODES[self.quiz.mode]),
            _ => "No módulo de violão do Kindle (o Pedro está aprendendo violão).".into(),
        }
    }

    fn keep_awake(&self) -> bool {
        // Na estante, a tela não dorme enquanto toca; no treino, enquanto o minuto corre.
        matches!(self.view, View::Song) || matches!(self.timer, Timer::Running(_))
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        if let Some(c) = self.cel {
            crate::aprender::draw_celebration(fb, f, c);
            return;
        }
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Hub => self.draw_hub(fb, f),
            View::List => self.draw_list(fb, f),
            View::Song => self.draw_song(fb, f),
            View::Dict => self.draw_dict(fb, f),
            View::Practice => self.draw_practice(fb, f),
            View::Course => self.draw_course(fb, f),
            View::Module => self.draw_module(fb, f),
            View::Lesson => self.draw_lesson(fb, f),
            View::Check => self.draw_check(fb, f),
            View::Quiz => self.draw_quiz(fb, f),
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.cel.is_some() {
            return vec![(999, crate::aprender::cel_button())];
        }
        match self.view {
            View::Hub => (0..4).map(|k| (700 + k as u32, Self::hub_card(k))).collect(),
            View::Course => {
                let mut v: Vec<(u32, Rect)> = (0..crate::partitura::MODULES.len()).map(|k| (810 + k as u32, Self::module_row(k))).collect();
                v.push((790, Self::course_btn()));
                v
            }
            View::Module => (0..crate::partitura::MODULES[self.module].lessons.len()).map(|k| (900 + k as u32, Self::lesson_row(k))).collect(),
            View::Lesson => {
                let mut v = vec![(721, Self::nav(1))];
                if crate::partitura::locate(self.lesson).1 > 0 {
                    v.push((720, Self::nav(0)));
                }
                v
            }
            View::Check => {
                let mut v: Vec<(u32, Rect)> = vec![(720, Self::nav(0))];
                if self.answered.is_some() {
                    v.push((721, Self::check_next()));
                } else {
                    v.extend((0..crate::partitura::lesson(self.lesson).check.opts.len()).map(|k| (870 + k as u32, Self::option_rect(k))));
                }
                v
            }
            View::Quiz => {
                let mut v: Vec<(u32, Rect)> = (0..4).map(|k| (840 + k as u32, Self::mode_rect(k))).collect();
                if self.quiz.mode == 0 {
                    v.extend((0..crate::partitura::LEVELS.len()).map(|k| (850 + k as u32, Self::level_rect(k))));
                }
                v.extend((0..7).map(|k| (860 + k as u32, Self::answer_rect(k))));
                v
            }
            View::List => {
                let mut v: Vec<(u32, Rect)> = (0..self.songs.len().saturating_sub(self.list_page * 7).min(7)).map(|k| (100 + (self.list_page * 7 + k) as u32, Self::list_row(k))).collect();
                v.push((720, Self::nav(0)));
                v.push((721, Self::nav(1)));
                v
            }
            View::Song => vec![(730, Self::tone_rect(0)), (731, Self::tone_rect(1)), (720, Self::nav(0)), (721, Self::nav(1)), (722, Rect::new(W / 2, 500, W / 2, 790)), (723, Rect::new(0, 500, W / 2, 790))],
            View::Dict => (0..12).map(|k| (740 + k as u32, Self::root_rect(k))).collect(),
            View::Practice => {
                let mut v: Vec<(u32, Rect)> = (0..PAIRS.len()).map(|k| (760 + k as u32, Self::pair_rect(k))).collect();
                v.push((780, Self::practice_btn(0)));
                if matches!(self.timer, Timer::Count(_)) {
                    v.push((781, Self::practice_btn(1)));
                    v.push((782, Self::practice_btn(2)));
                }
                v
            }
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        if self.cel.is_some() {
            if id == 999 {
                self.cel = None;
                return Redraw::Full;
            }
            return Redraw::None;
        }
        match (self.view, id) {
            (View::Hub, 700) => {
                self.pair = suggested();
                self.view = View::Practice;
            }
            (View::Hub, 701) => {
                self.songs = songs();
                self.view = View::List;
            }
            (View::Hub, 702) => self.view = View::Dict,
            (View::Hub, 703) => self.view = View::Course,
            (View::Course, k @ 810..=819) => {
                self.module = (k - 810) as usize;
                self.view = View::Module;
            }
            (View::Course, 790) => self.view = View::Quiz,
            (View::Module, k @ 900..=919) => {
                self.lesson = crate::partitura::first(self.module) + (k - 900) as usize;
                crate::partitura::mark_done(self.lesson);
                self.view = View::Lesson;
            }
            (View::Lesson, 720) => {
                if crate::partitura::locate(self.lesson).1 > 0 {
                    self.lesson -= 1;
                }
            }
            (View::Lesson, 721) => {
                self.answered = None;
                self.view = View::Check;
            }
            (View::Check, 720) => {
                self.answered = None;
                self.view = View::Lesson;
            }
            (View::Check, k @ 870..=873) => {
                let k = (k - 870) as usize;
                let ok = k == crate::partitura::lesson(self.lesson).check.ok;
                self.answered = Some(k);
                self.cel = crate::aprender::add(if ok { 10 } else { 5 }, "licao");
            }
            (View::Check, 721) => self.advance(),
            (View::Quiz, k @ 840..=843) => {
                self.quiz.mode = (k - 840) as usize;
                self.quiz.streak = 0;
                self.quiz.wrong = None;
                self.quiz.next();
            }
            (View::Quiz, k @ 850..=855) => {
                self.quiz.level = (k - 850) as usize;
                self.quiz.streak = 0;
                self.quiz.wrong = None;
                self.quiz.next();
            }
            (View::Quiz, k @ 860..=866) => {
                let picked = crate::partitura::answers(self.quiz.mode)[(k - 860) as usize];
                if (k - 860) as usize == self.quiz.q.correct() {
                    self.quiz.streak += 1;
                    // 1 XP por acerto e 5 de bônus a cada 10 seguidos.
                    let bonus = if self.quiz.streak % 10 == 0 { 5 } else { 0 };
                    if let Some(c) = crate::aprender::add(1 + bonus, "treino") {
                        self.cel = Some(c);
                        crate::partitura::save_best(self.quiz.q.record_key(), self.quiz.streak);
                        self.quiz.wrong = None;
                        self.quiz.next();
                        return Redraw::Full;
                    }
                    crate::partitura::save_best(self.quiz.q.record_key(), self.quiz.streak);
                    self.quiz.wrong = None;
                    self.quiz.next();
                } else {
                    self.quiz.wrong = Some(picked);
                    self.quiz.streak = 0;
                    // A mesma pergunta fica na tela para olhar de novo.
                    return Redraw::Part(Rect::new(0, 950, W, 110));
                }
                return Redraw::Part(Rect::new(0, 410, W, 660));
            }
            (View::List, 720) => self.list_page = self.list_page.saturating_sub(1),
            (View::List, 721) if (self.list_page + 1) * 7 < self.songs.len() => self.list_page += 1,
            (View::List, k @ 100..=699) => {
                self.song = (k - 100) as usize;
                self.page = 0;
                self.shift = 0;
                self.view = View::Song;
            }
            (View::Song, 730) => self.shift -= 1,
            (View::Song, 731) => self.shift += 1,
            (View::Song, 720 | 723) => self.page = self.page.saturating_sub(1),
            (View::Song, 721 | 722) => self.page += 1,
            (View::Dict, k @ 740..=751) => self.dict_root = (k - 740) as usize,
            (View::Practice, k @ 760..=767) if matches!(self.timer, Timer::Idle) => self.pair = (k - 760) as usize,
            (View::Practice, 780) => {
                self.timer = match self.timer {
                    Timer::Idle => Timer::Running(std::time::Instant::now()),
                    Timer::Running(_) => Timer::Count(0),
                    Timer::Count(n) => {
                        let _ = std::fs::OpenOptions::new().create(true).append(true).open(log_path()).and_then(|mut f| {
                            use std::io::Write;
                            writeln!(f, "{}|{}|{n}", today(), self.pair)
                        });
                        self.cel = crate::aprender::add(10, "violao");
                        Timer::Idle
                    }
                };
                self.shown_secs = SECS;
            }
            (View::Practice, 781) => {
                if let Timer::Count(n) = self.timer {
                    self.timer = Timer::Count(n.saturating_sub(1));
                }
            }
            (View::Practice, 782) => {
                if let Timer::Count(n) = self.timer {
                    self.timer = Timer::Count(n + 1);
                }
            }
            _ => return Redraw::None,
        }
        // A cifra ajusta a página ao número real de páginas no desenho.
        Redraw::Full
    }

    fn tick(&mut self) -> Redraw {
        let Timer::Running(t) = self.timer else { return Redraw::None };
        let secs = t.elapsed().as_secs();
        if secs >= SECS {
            self.timer = Timer::Count(0);
            return Redraw::Full;
        }
        // Atualiza o número a cada 5 segundos (o e-ink não precisa de mais).
        let left = SECS - secs;
        if left / 5 != self.shown_secs / 5 {
            self.shown_secs = left;
            return Redraw::Part(Rect::new(0, 960, W, 280));
        }
        Redraw::None
    }

    fn back(&mut self) -> bool {
        if self.cel.take().is_some() {
            return true;
        }
        if self.view == View::Hub || (self.view == View::Course && self.course_root) {
            return false;
        }
        self.timer = Timer::Idle;
        self.view = match self.view {
            View::Song => View::List,
            View::Lesson | View::Check => View::Module,
            View::Module | View::Quiz => View::Course,
            _ => View::Hub,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acordes_lidos_e_transpostos() {
        let c = parse_chord("F#m7").unwrap();
        assert_eq!(c.name(), "F#m7");
        assert_eq!(parse_chord("Bb").unwrap().name(), "A#");
        assert_eq!(parse_chord("C/G").unwrap().transposed(2).name(), "D/A");
        assert!(parse_chord("Olha").is_none());
        assert!(parse_chord("Em").is_some());
        assert_eq!(chords_of("Em               G").unwrap().len(), 2);
        assert!(chords_of("Um, dois, três").is_none());
    }

    #[test]
    fn formas_abertas_e_pestanas() {
        assert_eq!(shape(&parse_chord("C").unwrap()).frets, [-1, 3, 2, 0, 1, 0]);
        // F: pestana na 1ª casa, forma de Mi.
        assert_eq!(shape(&parse_chord("F").unwrap()).frets, [1, 3, 3, 2, 1, 1]);
        // Bm: pestana na 2ª, forma de Lá menor.
        assert_eq!(shape(&parse_chord("Bm").unwrap()).frets, [-1, 2, 4, 4, 3, 2]);
    }

    #[test]
    fn suspensos() {
        assert_eq!(shape(&parse_chord("Dsus4").unwrap()).frets, [-1, -1, 0, 2, 3, 3]);
        assert_eq!(shape(&parse_chord("Bsus4").unwrap()).frets, [-1, 2, 4, 4, 5, 2]);
    }

    #[test]
    fn cifra_de_exemplo() {
        let s = Song::parse(PathBuf::from("x"), EXEMPLO);
        assert_eq!(s.title, "Primeiros acordes (exercício)");
        let names: Vec<String> = s.chords().iter().map(Chord::name).collect();
        assert_eq!(names, ["Em", "G", "D", "A", "C"]);
    }
}
