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
        Violao { view: View::Hub, songs, song: 0, page: 0, shift: 0, list_page: 0, dict_root: 0, pair: suggested(), timer: Timer::Idle, shown_secs: 0 }
    }

    fn hub_card(k: usize) -> Rect {
        Rect::new(MARGIN, 470 + k as i32 * 250, W - 2 * MARGIN, 220)
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
        ];
        for (k, (t, s, main)) in cards.iter().enumerate() {
            let r = Self::hub_card(k);
            ui::card(fb, r, 32, if *main { INK } else { PAPER }, false);
            let (fg, sub) = if *main { (PAPER, 0xCC) } else { (INK, MUTED) };
            f.draw(fb, Face::Display, 64.0, r.x + 40, r.y + 100, fg, t);
            f.draw(fb, Face::Body, 30.0, r.x + 42, r.y + 156, sub, &crate::apps::fit(f, Face::Body, 30.0, s, r.w - 340));
            // Os acordes do treino desenhados no cartão.
            if *main {
                for (j, n) in [a, b].iter().enumerate() {
                    let cr = Rect::new(r.x + r.w - 300 + j as i32 * 140, r.y + 20, 130, 180);
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
            _ => "No módulo de violão do Kindle (o Pedro está aprendendo violão).".into(),
        }
    }

    fn keep_awake(&self) -> bool {
        // Na estante, a tela não dorme enquanto toca; no treino, enquanto o minuto corre.
        matches!(self.view, View::Song) || matches!(self.timer, Timer::Running(_))
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Hub => self.draw_hub(fb, f),
            View::List => self.draw_list(fb, f),
            View::Song => self.draw_song(fb, f),
            View::Dict => self.draw_dict(fb, f),
            View::Practice => self.draw_practice(fb, f),
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.view {
            View::Hub => (0..3).map(|k| (700 + k as u32, Self::hub_card(k))).collect(),
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
        if self.view == View::Hub {
            return false;
        }
        self.timer = Timer::Idle;
        self.view = match self.view {
            View::Song => View::List,
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
