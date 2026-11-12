//! Módulos com função de verdade: Ler (biblioteca), Foco (Pomodoro) e Ajustes.

use crate::fb::{Framebuffer, Rect};
use crate::sys::sh;
use crate::text::{Face, Fonts};
use crate::ui::{self, Status, INK, MUTED, PAPER};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MARGIN: i32 = 72;
const W: i32 = 1072;

// ======================= Ler =======================

#[derive(Clone)]
pub struct Book {
    pub path: PathBuf,
    pub title: String,
    pub kind: String,
    pub size: u64,
    /// Última leitura no KOReader (o arquivo de progresso muda ao ler); senão, quando o arquivo chegou.
    touched: SystemTime,
    /// Fração lida (0..1) segundo o KOReader.
    pub progress: Option<f32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Recent,
    Title,
}

pub struct Library {
    /// Livros mostrados (já filtrados pela busca e ordenados).
    pub books: Vec<Book>,
    all: Vec<Book>,
    /// Livros da loja da Amazon (KFX/AZW com DRM): só abrem no leitor da Amazon.
    pub store_only: usize,
    pub page: usize,
    pub sort: Sort,
    pub query: String,
}

/// Minúsculas e sem acento, para a busca achar "joao" em "João".
pub fn fold(s: &str) -> String {
    unicode_normalization::UnicodeNormalization::nfd(s).filter(|c| c.is_ascii() || !unicode_normalization::char::is_combining_mark(*c)).collect::<String>().to_lowercase()
}

/// Arquivo de progresso do KOReader ao lado do livro.
fn sidecar(path: &Path) -> PathBuf {
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
    PathBuf::from(format!("{}.sdr", path.with_extension("").display())).join(format!("metadata.{ext}.lua"))
}

fn sort_file() -> PathBuf {
    crate::home_dir().join("settings/lib_sort")
}

pub const PER_PAGE: usize = 5;
pub const THUMB_W: u32 = 112;
pub const THUMB_H: u32 = 152;
const OPENABLE: [&str; 9] = ["epub", "pdf", "azw3", "mobi", "cbz", "fb2", "djvu", "txt", "cbr"];
const STORE: [&str; 3] = ["kfx", "azw", "azw1"];

/// Nomes gravados com UTF-8 lido como Latin-1 ("BÃ­blia" → "Bíblia").
fn fix_mojibake(s: &str) -> String {
    if !s.contains('Ã') && !s.contains('â') {
        return s.to_string();
    }
    if s.chars().all(|c| (c as u32) <= 0xFF) {
        let bytes: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
        if let Ok(fixed) = String::from_utf8(bytes) {
            return fixed;
        }
    }
    // Conserto parcial: sequências comuns de dois caracteres.
    let mut out = s.to_string();
    for (bad, good) in [("Ã¡", "á"), ("Ã©", "é"), ("Ã­", "í"), ("Ã³", "ó"), ("Ãº", "ú"), ("Ã£", "ã"), ("Ãµ", "õ"), ("Ã§", "ç"), ("Ãª", "ê"), ("Ã¢", "â"), ("Ã´", "ô"), ("Ã ", "à")] {
        out = out.replace(bad, good);
    }
    out.replace('â', "–")
}

/// Título de dentro do livro, se estiver legível (alguns arquivos trazem o metadado corrompido).
fn usable_title(t: &str) -> Option<String> {
    let t: String = unicode_normalization::UnicodeNormalization::nfc(t).collect();
    let t = fix_mojibake(&t);
    let broken = t.contains('\u{FFFD}') || t.contains("??") || t.contains('Ã') || t.trim().is_empty();
    (!broken).then(|| t.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// Arquivos que o próprio Kindle cria em documents e não são livros.
fn is_system_junk(name: &str) -> bool {
    name.contains("_crash_") || name == "JAILBROKEN" || name.starts_with("BootActionsApplication") || name == "My Clippings"
}

/// "Perfil de três reis - Gene Edwards PDF_E6IQT5TU…KWBM" → "Perfil de três reis - Gene Edwards PDF"
fn clean_title(stem: &str) -> String {
    let mut t = stem.to_string();
    if let Some(i) = t.rfind('_') {
        let tail = &t[i + 1..];
        if tail.len() >= 10 && tail.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
            t.truncate(i);
        }
    }
    for junk in ["-cdeKey", " (Portuguese Edition)", " (English Edition)"] {
        if let Some(i) = t.find(junk) {
            t.truncate(i);
        }
    }
    let t: String = unicode_normalization::UnicodeNormalization::nfc(t.as_str()).collect();
    let t = fix_mojibake(&t).replace('_', " ");
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() { stem.to_string() } else { t }
}

fn scan_dir(dir: &Path, depth: u32, lib: &mut Library) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() {
            // Pastas .sdr guardam anotações, não livros.
            let is_sdr = path.extension().is_some_and(|x| x == "sdr");
            if depth > 0 && !is_sdr {
                scan_dir(&path, depth - 1, lib);
            }
            continue;
        }
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_string();
        if STORE.contains(&ext.as_str()) {
            lib.store_only += 1;
        } else if OPENABLE.contains(&ext.as_str()) && !is_system_junk(&name) && !name.starts_with('.') {
            let added = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let read = std::fs::metadata(sidecar(&path)).and_then(|m| m.modified()).ok();
            lib.all.push(Book {
                title: crate::covers::cached_title(&path).and_then(|t| usable_title(&t)).unwrap_or_else(|| clean_title(&name)),
                kind: ext.to_ascii_uppercase(),
                size: meta.len(),
                touched: read.map_or(added, |r| r.max(added)),
                progress: crate::data::book_progress(&path),
                path,
            });
        }
    }
}

impl Library {
    pub fn empty() -> Self {
        Library { books: Vec::new(), all: Vec::new(), store_only: 0, page: 0, sort: Sort::Recent, query: String::new() }
    }
    pub fn scan(query: &str) -> Self {
        let mut lib = Library::empty();
        lib.sort = if std::fs::read_to_string(sort_file()).is_ok_and(|s| s.trim() == "titulo") { Sort::Title } else { Sort::Recent };
        scan_dir(Path::new("/mnt/us/documents"), 2, &mut lib);
        lib.query = query.to_string();
        lib.apply();
        lib
    }
    /// Refaz a lista mostrada a partir da ordem e da busca atuais.
    pub fn apply(&mut self) {
        match self.sort {
            // Lidos ou chegados por último primeiro.
            Sort::Recent => self.all.sort_by_key(|b| std::cmp::Reverse(b.touched)),
            Sort::Title => self.all.sort_by_cached_key(|b| fold(&b.title)),
        }
        let q = fold(self.query.trim());
        self.books = self.all.iter().filter(|b| q.is_empty() || fold(&b.title).contains(&q)).cloned().collect();
        self.page = 0;
    }
    pub fn toggle_sort(&mut self) {
        self.sort = if self.sort == Sort::Recent { Sort::Title } else { Sort::Recent };
        let _ = std::fs::write(sort_file(), if self.sort == Sort::Title { "titulo" } else { "recentes" });
        self.apply();
    }
    /// Quantos livros a busca `q` acharia (mostrado enquanto digita).
    pub fn count_matches(&self, q: &str) -> usize {
        let q = fold(q.trim());
        self.all.iter().filter(|b| fold(&b.title).contains(&q)).count()
    }
    pub fn pages(&self) -> usize {
        self.books.len().div_ceil(PER_PAGE).max(1)
    }
    pub fn visible(&self) -> &[Book] {
        let start = (self.page * PER_PAGE).min(self.books.len());
        &self.books[start..(start + PER_PAGE).min(self.books.len())]
    }
}

pub fn lib_row_rect(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 440 + i as i32 * 168, W - 2 * MARGIN + 32, 166)
}
pub fn cover_rect(i: usize) -> Rect {
    let r = lib_row_rect(i);
    Rect::new(MARGIN, r.y + 8, THUMB_W as i32, THUMB_H as i32)
}

/// Capa de verdade (miniatura) ou, sem ela, uma capa tipográfica no estilo do Verso Ink.
pub fn draw_cover(fb: &mut Framebuffer, f: &Fonts, r: Rect, b: &Book, thumb: Option<&crate::covers::Thumb>) {
    fb.fill(r, PAPER);
    if let Some(t) = thumb {
        fb.blit(r.x, r.y, t.w as i32, t.h as i32, &t.px);
        // Moldura fina para capas claras não sumirem no papel.
        fb.round_outline(r, 6, 2, INK, PAPER);
        fb.blit(r.x + 2, r.y + 2, t.w as i32 - 4, t.h as i32 - 4, &crop_inner(t));
        return;
    }
    fb.round_rect(r, 10, INK);
    let initial: String = b.title.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let iw = f.measure(Face::Display, 84.0, &initial);
    f.draw(fb, Face::Display, 84.0, r.x + (r.w - iw) / 2, r.y + 100, PAPER, &initial);
    let kw = f.measure(Face::Strong, 20.0, &b.kind);
    f.draw(fb, Face::Strong, 20.0, r.x + (r.w - kw) / 2, r.y + r.h - 16, 0xBB, &b.kind);
}

fn crop_inner(t: &crate::covers::Thumb) -> Vec<u8> {
    let (w, h) = (t.w as usize, t.h as usize);
    let mut out = Vec::with_capacity((w - 4) * (h - 4));
    for y in 2..h - 2 {
        out.extend_from_slice(&t.px[y * w + 2..y * w + w - 2]);
    }
    out
}

/// Quebra o título em até `max_lines` linhas que caibam em `max_w`.
fn wrap(f: &Fonts, face: Face, size: f32, text: &str, max_w: i32, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if f.measure(face, size, &cand) <= max_w || cur.is_empty() {
            cur = cand;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        let rest = lines[max_lines - 1..].join(" ");
        lines.truncate(max_lines - 1);
        lines.push(rest);
    }
    lines.into_iter().map(|l| fit(f, face, size, &l, max_w)).collect()
}
/// Botão "Destaques" no topo da tela Ler.
pub fn lib_search_rect() -> Rect {
    Rect::new(W - MARGIN - 230, 236, 230, 84)
}
pub fn lib_sort_rect() -> Rect {
    Rect::new(W - MARGIN - 230 - 24 - 230, 236, 230, 84)
}
pub fn clips_rect() -> Rect {
    Rect::new(W - MARGIN - 250, 76, 250, 84)
}
/// Jogos na tela Jogar (a ordem define o índice usado pelo app).
pub const GAMES: [(&str, &str); 16] = [
    ("Sudoku", "Três níveis, anotações e dicas"),
    ("Xadrez", "Treinadores, revisão e duas pessoas"),
    ("Coroas", "Uma por linha, coluna e reino"),
    ("Lig 4", "Contra a CPU ou duas pessoas"),
    ("Caixas", "Pontos e caixas, CPU ou local"),
    ("Naval", "Batalha naval, CPU ou local"),
    ("Minas", "Campo minado em três tamanhos"),
    ("Nonograma", "Pinte pelas dicas numéricas"),
    ("UNO", "O seu UNO do AED1, CPU ou local"),
    ("Palavra", "A palavra do dia em 6 tentativas"),
    ("Sokoban", "Empurre as caixas até os alvos"),
    ("Paciência", "Klondike, comprar 1 ou 3"),
    ("Masmorra", "Roguelike por turnos, 5 andares"),
    ("Cripta", "Roguelike parado: decida cada luta"),
    ("Histórias", "Ficção interativa por escolhas"),
    ("Damas", "Regras brasileiras, CPU ou local"),
];

/// Grade de blocos 2 colunas.
/// Disposição da tela Jogar: os jogos com partida em andamento vêm primeiro, em cartões
/// maiores; depois todos os outros. Devolve (jogo, retângulo) e a altura de cada título de seção.
pub fn games_layout(live: &[bool]) -> (Vec<(usize, Rect)>, Option<i32>, i32) {
    let gap = 22;
    let w = (W - 2 * MARGIN - gap) / 2;
    let on: Vec<usize> = (0..GAMES.len()).filter(|&i| live.get(i) == Some(&true)).collect();
    let off: Vec<usize> = (0..GAMES.len()).filter(|&i| live.get(i) != Some(&true)).collect();
    let mut out = Vec::new();
    let mut y = 372;
    let mut live_head = None;
    if !on.is_empty() {
        live_head = Some(y + 34);
        y += 54;
        let h = if on.len() <= 4 { 118 } else { 84 };
        for (k, &i) in on.iter().enumerate() {
            out.push((i, Rect::new(MARGIN + (k % 2) as i32 * (w + gap), y + (k / 2) as i32 * (h + 20), w, h)));
        }
        y += on.len().div_ceil(2) as i32 * (h + 20) + 4;
    }
    let all_head = y + 34;
    y += 54;
    let rows = off.len().div_ceil(2).max(1) as i32;
    // Os cartões ocupam o que sobra da tela, entre 72 e 96 de altura.
    let h = ((1410 - y) / rows - 18).clamp(72, 96);
    for (k, &i) in off.iter().enumerate() {
        out.push((i, Rect::new(MARGIN + (k % 2) as i32 * (w + gap), y + (k / 2) as i32 * (h + 18), w, h)));
    }
    (out, live_head, all_head)
}

pub fn draw_games(fb: &mut Framebuffer, f: &Fonts, live_games: &[Option<String>]) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let end = f.draw(fb, Face::Display, 120.0, MARGIN - 6, 300, INK, "Jogar");
    f.draw(fb, Face::Strong, 28.0, end + 24, 300, MUTED, "Tudo funciona sem Wi-Fi");
    let live: Vec<bool> = live_games.iter().map(Option::is_some).collect();
    let (cards, live_head, all_head) = games_layout(&live);
    if let Some(y) = live_head {
        f.draw(fb, Face::Strong, 34.0, MARGIN, y, INK, "Em andamento");
    }
    f.draw(fb, Face::Strong, 34.0, MARGIN, all_head, INK, if live_head.is_some() { "Todos os jogos" } else { "Jogos" });
    for (i, r) in cards {
        let (title, _) = GAMES[i];
        let on = live_games.get(i).cloned().flatten();
        ui::card(fb, r, 26, if on.is_some() { ui::SURFACE } else { PAPER }, false);
        let d = r.h - 20;
        let name = fold(title).replace(' ', "");
        let x = if ui::icon_disc(fb, r.x + 12, r.y + 10, d, &name) { r.x + 12 + d + 18 } else { r.x + 26 };
        let room = r.x + r.w - 16 - x;
        match on {
            Some(sum) if r.h > 100 => {
                f.draw(fb, Face::Display, 40.0, x, r.y + 54, INK, &fit(f, Face::Display, 40.0, title, room));
                f.draw(fb, Face::Body, 23.0, x + 2, r.y + 90, INK, &fit(f, Face::Body, 23.0, &sum, room));
            }
            _ => {
                let size = 38.0;
                f.draw(fb, Face::Display, size, x, r.y + r.h / 2 + (size * 0.36) as i32, INK, &fit(f, Face::Display, size, title, room));
            }
        }
    }
}

pub fn lib_prev_rect() -> Rect {
    Rect::new(MARGIN, 1300, 260, 88)
}
pub fn lib_next_rect() -> Rect {
    Rect::new(W - MARGIN - 260, 1300, 260, 88)
}

fn human_size(b: u64) -> String {
    if b >= 1 << 20 {
        format!("{:.1} MB", b as f64 / (1u64 << 20) as f64).replace('.', ",")
    } else {
        format!("{} KB", (b >> 10).max(1))
    }
}

/// Corta o texto com reticências para caber em `max_w`.
pub fn fit(f: &Fonts, face: Face, size: f32, text: &str, max_w: i32) -> String {
    if f.measure(face, size, text) <= max_w {
        return text.to_string();
    }
    let mut s: String = text.to_string();
    while !s.is_empty() && f.measure(face, size, &format!("{s}…")) > max_w {
        s.pop();
    }
    format!("{}…", s.trim_end())
}

fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool, _size: f32) {
    ui::button(fb, f, r, label, filled);
}

pub fn draw_library(fb: &mut Framebuffer, f: &Fonts, lib: &Library, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    pill(fb, f, clips_rect(), "Destaques", false, 32.0);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, "Ler");
    let n = lib.books.len();
    let has_any = !lib.all.is_empty();
    if has_any {
        pill(fb, f, lib_search_rect(), if lib.query.is_empty() { "Buscar" } else { "Limpar" }, !lib.query.is_empty(), 32.0);
        pill(fb, f, lib_sort_rect(), if lib.sort == Sort::Recent { "Recentes" } else { "A–Z" }, false, 32.0);
    }
    let plural = |k: usize| if k == 1 { "1 livro".to_string() } else { format!("{k} livros") };
    let mut summary = if !lib.query.is_empty() {
        format!("“{}” · {}", lib.query.trim(), if n == 0 { "nenhum livro".into() } else { plural(n) })
    } else {
        match n {
            0 => "Nenhum livro para abrir ainda".to_string(),
            _ if lib.sort == Sort::Title => format!("{}, de A a Z", plural(n)),
            _ => format!("{}, lidos por último primeiro", plural(n)),
        }
    };
    if lib.store_only > 0 && lib.query.is_empty() {
        summary.push_str(&format!(" · {} da loja só no Kindle original", lib.store_only));
    }
    let summary = fit(f, Face::Strong, 30.0, &summary, W - 2 * MARGIN);
    f.draw(fb, Face::Strong, 30.0, MARGIN, 400, MUTED, &summary);
    if n == 0 && has_any {
        f.draw(fb, Face::Body, 40.0, MARGIN, 560, INK, "Nenhum título com esse trecho.");
        f.draw(fb, Face::Body, 40.0, MARGIN, 620, MUTED, "Toque em Limpar para ver todos.");
    } else if n == 0 {
        let lines = [
            "Copie arquivos EPUB, PDF ou AZW3 para a",
            "pasta documents, pelo USB ou pelo Wi-Fi,",
            "e eles aparecem aqui.",
        ];
        for (k, l) in lines.iter().enumerate() {
            f.draw(fb, Face::Body, 40.0, MARGIN, 560 + k as i32 * 60, INK, l);
        }
    }
    for (i, b) in lib.visible().iter().enumerate() {
        let r = lib_row_rect(i);
        let c = cover_rect(i);
        match crate::covers::cached(&b.path, THUMB_W, THUMB_H) {
            Some(thumb) => draw_cover(fb, f, c, b, thumb.as_ref()),
            None => {
                // Ainda não extraída: moldura clara enquanto a capa é preparada.
                fb.round_rect(c, 10, 0xE6);
            }
        }
        let tx = c.x + c.w + 28;
        let lines = wrap(f, Face::Display, 42.0, &b.title, W - MARGIN - tx, 2);
        let top = r.y + 8 + if lines.len() == 1 { 62 } else { 46 };
        for (k, l) in lines.iter().enumerate() {
            f.draw(fb, Face::Display, 42.0, tx, top + k as i32 * 50, INK, l);
        }
        let meta_y = top + (lines.len() as i32 - 1) * 50 + 46;
        let meta = format!("{} · {}", b.kind, human_size(b.size));
        f.draw(fb, Face::Body, 28.0, tx + 2, meta_y, MUTED, &meta);
        if let Some(p) = b.progress {
            // Progresso do KOReader: barra curta depois do tipo e tamanho.
            let p = p.clamp(0.0, 1.0);
            let label = if p >= 0.995 { "Lido".to_string() } else { format!("{}%", (p * 100.0).round() as i32) };
            let bx = tx + 2 + f.measure(Face::Body, 28.0, &meta) + 28;
            let bar = Rect::new(bx, meta_y - 12, 160, 8);
            fb.fill(bar, 0xD0);
            fb.fill(Rect::new(bar.x, bar.y, (bar.w as f32 * p) as i32, bar.h), INK);
            f.draw(fb, Face::Strong, 28.0, bar.x + bar.w + 14, meta_y, INK, &label);
        }
    }
    if lib.pages() > 1 {
        if lib.page > 0 {
            pill(fb, f, lib_prev_rect(), "‹ Anterior", false, 32.0);
        }
        if lib.page + 1 < lib.pages() {
            pill(fb, f, lib_next_rect(), "Próxima ›", false, 32.0);
        }
        let pg = format!("{} de {}", lib.page + 1, lib.pages());
        let pw = f.measure(Face::Strong, 30.0, &pg);
        f.draw(fb, Face::Strong, 30.0, (W - pw) / 2, 1356, MUTED, &pg);
    } else {
        ui::draw_footer(fb, f, st);
    }
}

/// Momento de passagem: o KOReader leva alguns segundos para abrir.
pub fn draw_opening_book(fb: &mut Framebuffer, f: &Fonts, title: &str) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    f.draw(fb, Face::Strong, 36.0, MARGIN, 600, MUTED, "ABRINDO");
    let t = fit(f, Face::Display, 80.0, title, W - 2 * MARGIN);
    f.draw(fb, Face::Display, 80.0, MARGIN - 4, 700, INK, &t);
    f.draw(fb, Face::Body, 38.0, MARGIN, 790, INK, "no KOReader. Para voltar ao Verso Ink, toque");
    f.draw(fb, Face::Body, 38.0, MARGIN, 845, INK, "no topo da tela, abra o menu e escolha Sair.");
}

// ======================= Foco =======================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Focus,
    Break,
}

impl Kind {
    pub fn minutes(self) -> u64 {
        match self {
            Kind::Focus => 25,
            Kind::Break => 5,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pomodoro {
    Idle,
    Running { kind: Kind, ends: SystemTime },
    Done { kind: Kind },
}

pub fn focus_primary_rect() -> Rect {
    Rect::new(MARGIN, 1010, W - 2 * MARGIN, 120)
}
pub fn focus_secondary_rect() -> Rect {
    Rect::new(MARGIN, 1156, W - 2 * MARGIN, 120)
}

pub fn remaining_min(ends: SystemTime) -> u64 {
    let left = ends.duration_since(SystemTime::now()).unwrap_or(Duration::ZERO).as_secs();
    left.div_ceil(60)
}

fn big_number(fb: &mut Framebuffer, f: &Fonts, n: &str, unit: &str) {
    fb.fill(Rect::new(0, 380, W, 330), PAPER);
    let end = f.draw(fb, Face::Display, 330.0, MARGIN - 16, 660, INK, n);
    f.draw(fb, Face::Strong, 48.0, end + 20, 660, MUTED, unit);
}

fn progress(fb: &mut Framebuffer, frac: f32) {
    let r = Rect::new(MARGIN, 760, W - 2 * MARGIN, 36);
    fb.fill(Rect::new(r.x - 4, r.y - 4, r.w + 8, r.h + 8), PAPER);
    fb.round_outline(r, 18, 4, INK, PAPER);
    let w = ((r.w - 12) as f32 * frac.clamp(0.0, 1.0)) as i32;
    if w > 24 {
        fb.round_rect(Rect::new(r.x + 6, r.y + 6, w, r.h - 12), 12, INK);
    }
}

/// Região que muda a cada minuto (número, barra e legenda), para atualização parcial.
pub fn focus_live_rect() -> Rect {
    Rect::new(0, 380, W, 520)
}

pub fn draw_focus_live(fb: &mut Framebuffer, f: &Fonts, p: Pomodoro, offset_min: i64) {
    if let Pomodoro::Running { kind, ends } = p {
        let left = remaining_min(ends);
        big_number(fb, f, &left.to_string(), if left == 1 { "minuto" } else { "minutos" });
        let total = kind.minutes() as f32 * 60.0;
        let left_s = ends.duration_since(SystemTime::now()).unwrap_or(Duration::ZERO).as_secs_f32();
        progress(fb, 1.0 - left_s / total);
        fb.fill(Rect::new(0, 820, W, 80), PAPER);
        let secs = ends.duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + offset_min * 60;
        let until = format!("{:02}:{:02}", (secs.rem_euclid(86_400)) / 3600, (secs.rem_euclid(3600)) / 60);
        let what = if kind == Kind::Focus { "Foco" } else { "Pausa" };
        f.draw(fb, Face::Body, 38.0, MARGIN, 870, INK, &format!("{what} até {until}. A tela não apaga enquanto isso."));
    }
}

pub fn draw_focus(fb: &mut Framebuffer, f: &Fonts, p: Pomodoro, done_today: u32, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let today = match done_today {
        0 => "Nenhum foco concluído hoje".to_string(),
        1 => "1 foco concluído hoje".to_string(),
        n => format!("{n} focos concluídos hoje"),
    };
    match p {
        Pomodoro::Idle => {
            f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, "Foco");
            big_number(fb, f, "25", "minutos");
            progress(fb, 0.0);
            f.draw(fb, Face::Body, 38.0, MARGIN, 870, INK, "Um bloco de atenção, depois 5 de pausa.");
            pill(fb, f, focus_primary_rect(), "Começar foco", true, 44.0);
            pill(fb, f, focus_secondary_rect(), "Pausa de 5 minutos", false, 40.0);
        }
        Pomodoro::Running { kind, .. } => {
            let title = if kind == Kind::Focus { "Focando" } else { "Pausa" };
            f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, title);
            draw_focus_live(fb, f, p, st.offset_min);
            pill(fb, f, focus_secondary_rect(), "Parar", false, 40.0);
        }
        Pomodoro::Done { kind } => {
            let (title, line, next) = if kind == Kind::Focus {
                ("Concluído", "Bom trabalho. Hora de uma pausa.", "Pausa de 5 minutos")
            } else {
                ("Pausa feita", "Pronto para mais um bloco?", "Começar foco")
            };
            f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, title);
            big_number(fb, f, &done_today.to_string(), if done_today == 1 { "foco hoje" } else { "focos hoje" });
            progress(fb, 1.0);
            f.draw(fb, Face::Body, 38.0, MARGIN, 870, INK, line);
            pill(fb, f, focus_primary_rect(), next, true, 44.0);
            pill(fb, f, focus_secondary_rect(), "Voltar ao início", false, 40.0);
        }
    }
    let tw = f.measure(Face::Strong, 30.0, &today);
    f.draw(fb, Face::Strong, 30.0, W - MARGIN - tw, 400, MUTED, &today);
}

// ======================= Ajustes =======================

pub struct SysInfo {
    pub wifi_on: bool,
    pub connected: bool,
    pub essid: Option<String>,
    pub ip: Option<String>,
    pub firmware: String,
}

impl SysInfo {
    pub fn read() -> Self {
        let wifi_on = sh("lipc-get-prop", &["com.lab126.cmd", "wirelessEnable"]).is_some_and(|s| s == "1");
        let connected = sh("lipc-get-prop", &["com.lab126.wifid", "cmState"]).is_some_and(|s| s == "CONNECTED");
        let essid = sh("lipc-get-prop", &["com.lab126.wifid", "currentEssid"]).filter(|s| !s.is_empty());
        let ip = sh("ifconfig", &["wlan0"]).and_then(|out| {
            out.split_whitespace().find_map(|w| w.strip_prefix("addr:").map(str::to_string))
        });
        let firmware = std::fs::read_to_string("/etc/prettyversion.txt")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
            .unwrap_or_else(|| "?".into());
        SysInfo { wifi_on, connected, essid, ip, firmware }
    }
}

pub fn set_wifi_row() -> Rect {
    Rect::new(MARGIN - 16, 400, W - 2 * MARGIN + 32, 170)
}
pub fn set_reboot_rect() -> Rect {
    Rect::new(MARGIN, 1140, (W - 2 * MARGIN - 28) / 2, 110)
}
pub fn set_update_rect() -> Rect {
    Rect::new(MARGIN, 1280, W - 2 * MARGIN, 110)
}

/// Tela de atualização: título, linhas de estado e, ao final, um botão.
pub fn draw_update(fb: &mut Framebuffer, f: &Fonts, title: &str, lines: &[String], button: Option<&str>) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    f.draw(fb, Face::Display, 110.0, MARGIN - 4, 640, INK, title);
    for (k, l) in lines.iter().enumerate() {
        f.draw(fb, Face::Body, 40.0, MARGIN, 740 + k as i32 * 60, INK, l);
    }
    if let Some(b) = button {
        pill(fb, f, update_ok_rect(), b, true, 44.0);
    }
}
pub fn update_ok_rect() -> Rect {
    Rect::new(MARGIN, 1150, W - 2 * MARGIN, 120)
}
pub fn set_off_rect() -> Rect {
    let r = set_reboot_rect();
    Rect::new(r.x + r.w + 28, r.y, r.w, r.h)
}

fn info_row(fb: &mut Framebuffer, f: &Fonts, y: i32, label: &str, value: &str) {
    fb.fill(Rect::new(MARGIN, y, W - 2 * MARGIN, 3), INK);
    f.draw(fb, Face::Strong, 30.0, MARGIN, y + 56, MUTED, label);
    let v = fit(f, Face::Display, 44.0, value, W - 2 * MARGIN - 260);
    let vw = f.measure(Face::Display, 44.0, &v);
    f.draw(fb, Face::Display, 44.0, W - MARGIN - vw, y + 60, INK, &v);
}

/// `confirm`: qual botão pediu confirmação (0 = reiniciar, 1 = desligar).
pub fn draw_settings(fb: &mut Framebuffer, f: &Fonts, s: &SysInfo, st: &Status, confirm: Option<u8>) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 120.0, MARGIN - 4, 330, INK, "Ajustes");
    // Wi-Fi com chave.
    let r = set_wifi_row();
    f.draw(fb, Face::Display, 64.0, MARGIN, r.y + 82, INK, "Wi-Fi");
    let sub = match (s.wifi_on, s.connected, &s.essid) {
        (false, _, _) => "Desligado".to_string(),
        (true, true, Some(n)) => format!("Conectado a {n}"),
        (true, true, None) => "Conectado".to_string(),
        (true, false, _) => "Procurando rede…".to_string(),
    };
    f.draw(fb, Face::Body, 32.0, MARGIN, r.y + 134, MUTED, &sub);
    let sw = Rect::new(W - MARGIN - 180, r.y + 40, 180, 96);
    if s.wifi_on {
        fb.round_rect(sw, 48, INK);
        let c = (sw.x + sw.w - 48, sw.y + 48);
        fb.round_rect(Rect::new(c.0 - 36, c.1 - 36, 72, 72), 36, PAPER);
    } else {
        fb.round_outline(sw, 48, 5, INK, PAPER);
        fb.round_rect(Rect::new(sw.x + 18, sw.y + 18, 60, 60), 30, INK);
    }
    let ip = s.ip.clone().unwrap_or_else(|| "sem rede".into());
    info_row(fb, f, 590, "ENDEREÇO", &ip);
    info_row(fb, f, 680, "SSH", if s.ip.is_some() { "porta 2223, só chave" } else { "indisponível" });
    let bat = match st.battery {
        Some(p) => format!("{p}%{}", if st.charging { ", carregando" } else { "" }),
        None => "?".into(),
    };
    info_row(fb, f, 770, "BATERIA", &bat);
    let version = match crate::backup::update_available() {
        Some(v) => format!("{} · nova: {v}", env!("CARGO_PKG_VERSION")),
        None => format!("Verso Ink {} · Kindle {}", env!("CARGO_PKG_VERSION"), s.firmware),
    };
    info_row(fb, f, 860, "VERSÃO", &version);
    info_row(fb, f, 950, "CÓPIA", &crate::backup::summary());
    fb.fill(Rect::new(MARGIN, 1040, W - 2 * MARGIN, 3), INK);
    let (rl, ol) = match confirm {
        Some(0) => ("Toque de novo", "Desligar"),
        Some(1) => ("Reiniciar", "Toque de novo"),
        _ => ("Reiniciar", "Desligar"),
    };
    if confirm.is_some() {
        f.draw(fb, Face::Body, 32.0, MARGIN, 1098, INK, "Confirme tocando no mesmo botão em até 5 segundos.");
    }
    pill(fb, f, set_reboot_rect(), rl, confirm == Some(0), 40.0);
    pill(fb, f, set_off_rect(), ol, confirm == Some(1), 40.0);
    let up = crate::backup::update_available().map(|v| format!("Instalar a versão {v}"));
    pill(fb, f, set_update_rect(), up.as_deref().unwrap_or("Procurar atualização"), up.is_some(), 40.0);
}

/// Fica na tela com o aparelho desligado (o e-ink mantém a imagem sem energia).
pub fn draw_powered_off(fb: &mut Framebuffer, f: &Fonts, rebooting: bool) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    let (title, line) = if rebooting {
        ("Reiniciando", "Volta sozinho em cerca de um minuto e meio.")
    } else {
        ("Desligado", "Segure o botão de ligar para ligar.")
    };
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 760, INK, title);
    f.draw(fb, Face::Body, 40.0, MARGIN, 850, INK, line);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn titulos_limpos() {
        assert_eq!(clean_title("BÃ\u{AD}blia Sagrada_6VYJXAZDOS3KFJGN56HOCTTKCO66RDLO"), "Bíblia Sagrada");
        assert_eq!(clean_title("Rick-Warren-Uma-Vida-com-Propósitos-cdeKey_KWTUCPKUZVNZPFFA3W4WRY3EXLLD7FIE"), "Rick-Warren-Uma-Vida-com-Propósitos");
        assert_eq!(clean_title("Kindle_Users_Guide"), "Kindle Users Guide");
        assert_eq!(usable_title("BÃ\u{AD}blia Sagrada â???? NVI"), None);
        assert_eq!(usable_title("Livro de teste com capa").as_deref(), Some("Livro de teste com capa"));
        assert!(is_system_junk("KPPMainAppV2_6215_002-juno_16020101_muscat_wario-409747_crash_Nov_11_07.21.53_2026"));
    }
}
