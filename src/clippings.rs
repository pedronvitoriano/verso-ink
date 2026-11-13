//! Destaques e notas do Kindle e do KOReader, agrupados por livro.

use crate::apps::{self, fit};
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::path::Path;
use std::time::SystemTime;

const MARGIN: i32 = 72;
const W: i32 = 1072;

#[derive(Clone)]
pub struct Clip {
    pub text: String,
    pub note: bool,
}

pub struct BookClips {
    pub title: String,
    pub clips: Vec<Clip>,
}

#[derive(Clone, Copy, Default)]
pub struct ImportStatus {
    /// Só aparece quando o arquivo do Kindle está velho o bastante para a pessoa não
    /// concluir, por engano, que a lista está sincronizada.
    pub kindle_age_days: Option<u64>,
    pub koreader_books: usize,
    pub koreader_clips: usize,
}

fn add_clip(books: &mut Vec<BookClips>, title: &str, text: String, note: bool) {
    let title: String = unicode_normalization::UnicodeNormalization::nfc(title).collect();
    let idx = match books.iter().position(|b| b.title == title) {
        Some(i) => i,
        None => {
            books.push(BookClips { title, clips: Vec::new() });
            books.len() - 1
        }
    };
    let clips = &mut books[idx].clips;
    // Um ajuste de seleção gera um destaque que contém (ou está contido) no anterior: fica só o maior.
    if let Some(last) = clips.last_mut() {
        if !last.note && !note && (text.contains(&last.text) || last.text.contains(&text)) {
            if text.len() > last.text.len() {
                last.text = text;
            }
            return;
        }
    }
    clips.push(Clip { text, note });
}

pub fn parse(src: &str) -> Vec<BookClips> {
    let mut books: Vec<BookClips> = Vec::new();
    for entry in src.trim_start_matches('\u{feff}').split("==========") {
        let mut lines = entry.lines().map(|l| l.trim_start_matches('\u{feff}').trim()).skip_while(|l| l.is_empty());
        let Some(title) = lines.next() else { continue };
        let meta = lines.next().unwrap_or("");
        let text: String = lines.filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        // Marcadores não têm texto; destaques repetidos (o Kindle guarda cada ajuste) também saem.
        if text.is_empty() {
            continue;
        }
        let note = meta.contains("nota") || meta.contains("Note");
        add_clip(&mut books, title, text, note);
    }
    books.sort_by_key(|b| std::cmp::Reverse(b.clips.len()));
    books
}

fn kindle_path() -> std::path::PathBuf {
    std::env::var_os("VERSO_INK_CLIPPINGS").map(Into::into).unwrap_or_else(|| apps::documents_dir().join("My Clippings.txt"))
}

/// Fecha uma tabela Lua sem executar o arquivo. O KOReader salva os metadados como Lua,
/// portanto tratamos somente o pequeno subconjunto textual de `annotations` que precisamos.
fn lua_table_end(src: &str, start: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut quote = None;
    let mut escaped = false;
    for (off, &b) in bytes[start..].iter().enumerate() {
        let i = start + off;
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == q {
                quote = None;
            }
            continue;
        }
        match b {
            b'\'' | b'"' => quote = Some(b),
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn annotations_table(src: &str) -> Option<&str> {
    let named = src.find("[\"annotations\"]").or_else(|| src.find("annotations"))?;
    let after = &src[named..];
    let eq = after.find('=')?;
    let start = named + eq + after[eq..].find('{')?;
    let end = lua_table_end(src, start)?;
    Some(&src[start..=end])
}

fn direct_lua_tables(src: &str) -> Vec<&str> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = None;
    let mut quote = None;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate() {
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == q {
                quote = None;
            }
            continue;
        }
        match b {
            b'\'' | b'"' => quote = Some(b),
            b'{' => {
                depth += 1;
                if depth == 2 {
                    start = Some(i);
                }
            }
            b'}' => {
                if depth == 2 {
                    if let Some(begin) = start.take() {
                        out.push(&src[begin..=i]);
                    }
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    out
}

fn lua_string(value: &str) -> Option<String> {
    let value = value.trim_start();
    if let Some(rest) = value.strip_prefix("[[") {
        return rest.split_once("]]" ).map(|(text, _)| text.to_string());
    }
    let quote = *value.as_bytes().first()?;
    if quote != b'\'' && quote != b'"' {
        return None;
    }
    let mut out = String::new();
    let mut chars = value[1..].chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == quote as char {
            return Some(out);
        }
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let escaped = chars.next()?;
        match escaped {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            d if d.is_ascii_digit() => {
                let mut digits = d.to_string();
                for _ in 0..2 {
                    if chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                        digits.push(chars.next().unwrap());
                    }
                }
                if let Ok(code) = digits.parse::<u32>() {
                    out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                }
            }
            other => out.push(other),
        }
    }
    None
}

fn lua_field(table: &str, key: &str) -> Option<String> {
    let quoted = format!("[\"{key}\"]");
    let at = table.find(&quoted).or_else(|| table.find(key))?;
    let rest = &table[at..];
    let eq = rest.find('=')?;
    lua_string(&rest[eq + 1..])
}

/// Lê as anotações que o KOReader persiste no `metadata.<ext>.lua` ao lado de um livro.
/// Não interpreta Lua e não tenta adivinhar bancos externos/hashdocsettings que não estejam
/// ligados ao arquivo presente na estante.
pub fn parse_koreader_annotations(src: &str) -> Vec<Clip> {
    let Some(table) = annotations_table(src) else { return Vec::new() };
    let mut clips = Vec::new();
    for item in direct_lua_tables(table) {
        // O exportador oficial do KOReader só trata como destaque uma anotação que
        // tem `drawer`; bookmarks podem ter texto/nota, mas não são trechos marcados.
        if lua_field(item, "drawer").is_none_or(|drawer| drawer.trim().is_empty()) {
            continue;
        }
        let text = lua_field(item, "text").unwrap_or_default().trim().to_string();
        let note = lua_field(item, "note").unwrap_or_default().trim().to_string();
        if !text.is_empty() {
            clips.push(Clip { text, note: false });
        }
        if !note.is_empty() {
            clips.push(Clip { text: note, note: true });
        }
    }
    clips
}

fn scan_koreader(dir: &Path, depth: u32, books: &mut Vec<BookClips>, status: &mut ImportStatus) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 && path.extension().is_none_or(|ext| ext != "sdr") {
                scan_koreader(&path, depth - 1, books, status);
            }
            continue;
        }
        let Ok(src) = std::fs::read_to_string(apps::sidecar(&path)) else { continue };
        let clips = parse_koreader_annotations(&src);
        if clips.is_empty() {
            continue;
        }
        let title = path.file_stem().and_then(|s| s.to_str()).map(apps::clean_title).unwrap_or_else(|| "Livro sem título".into());
        status.koreader_books += 1;
        status.koreader_clips += clips.len();
        for clip in clips {
            add_clip(books, &title, clip.text, clip.note);
        }
    }
}

pub fn load_with_status() -> (Vec<BookClips>, ImportStatus) {
    let path = kindle_path();
    let mut status = ImportStatus::default();
    status.kindle_age_days = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|then| SystemTime::now().duration_since(then).ok())
        .map(|age| age.as_secs() / 86_400)
        .filter(|&days| days >= 45);
    let mut books = std::fs::read_to_string(path).map(|s| parse(&s)).unwrap_or_default();
    scan_koreader(&apps::documents_dir(), 2, &mut books, &mut status);
    books.sort_by_key(|b| std::cmp::Reverse(b.clips.len()));
    (books, status)
}

pub fn load() -> Vec<BookClips> {
    load_with_status().0
}

pub const BOOKS_PER_PAGE: usize = 7;

pub fn book_row(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 430 + i as i32 * 116, W - 2 * MARGIN + 32, 112)
}
pub fn prev_rect() -> Rect {
    Rect::new(MARGIN, 1300, 260, 88)
}
pub fn next_rect() -> Rect {
    Rect::new(W - MARGIN - 260, 1300, 260, 88)
}

fn pager(fb: &mut Framebuffer, f: &Fonts, page: usize, pages: usize) {
    if pages <= 1 {
        return;
    }
    for (r, label, show) in [(prev_rect(), "‹ Anterior", page > 0), (next_rect(), "Próxima ›", page + 1 < pages)] {
        if show {
            fb.round_outline(r, r.h / 2, 4, INK, PAPER);
            let w = f.measure(Face::Strong, 32.0, label);
            f.draw(fb, Face::Strong, 32.0, r.x + (r.w - w) / 2, r.y + 56, INK, label);
        }
    }
    let pg = format!("{} de {}", page + 1, pages);
    let pw = f.measure(Face::Strong, 30.0, &pg);
    f.draw(fb, Face::Strong, 30.0, (W - pw) / 2, 1356, MUTED, &pg);
}

/// Botão "Revisar" no alto da tela Destaques.
pub fn review_rect() -> Rect {
    Rect::new(W - MARGIN - 300, 236, 300, 90)
}

pub fn draw_books(fb: &mut Framebuffer, f: &Fonts, books: &[BookClips], page: usize, due: usize, import: ImportStatus) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 130.0, MARGIN - 6, 310, INK, "Destaques");
    if !books.is_empty() {
        let label = if due > 0 { format!("Revisar · {due}") } else { "Revisar".to_string() };
        ui::button(fb, f, review_rect(), &label, due > 0);
    }
    let total: usize = books.iter().map(|b| b.clips.len()).sum();
    let sub = if books.is_empty() { "Nenhum destaque ainda. Os que você marcar no Kindle aparecem aqui.".to_string() } else { format!("{total} trechos em {} livros", books.len()) };
    f.draw(fb, Face::Strong, 30.0, MARGIN, 380, MUTED, &fit(f, Face::Strong, 30.0, &sub, W - 2 * MARGIN));
    let source = match (import.kindle_age_days, import.koreader_clips) {
        (Some(days), n) if n > 0 => format!("Kindle sem novidades há {days} dias · {n} trechos do KOReader"),
        (Some(days), _) => format!("Arquivo de destaques do Kindle sem novidades há {days} dias"),
        (None, n) if n > 0 => format!("Inclui {n} trechos do KOReader em {} livro(s)", import.koreader_books),
        _ => String::new(),
    };
    if !source.is_empty() {
        f.draw(fb, Face::Body, 23.0, MARGIN, 416, MUTED, &fit(f, Face::Body, 23.0, &source, W - 2 * MARGIN));
    }
    for (i, b) in books.iter().skip(page * BOOKS_PER_PAGE).take(BOOKS_PER_PAGE).enumerate() {
        let r = book_row(i);
        fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
        f.draw(fb, Face::Display, 44.0, MARGIN, r.y + 62, INK, &fit(f, Face::Display, 44.0, &b.title, W - 2 * MARGIN - 20));
        let n = b.clips.len();
        f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 100, MUTED, &format!("{n} {}", if n == 1 { "trecho" } else { "trechos" }));
    }
    pager(fb, f, page, books.len().div_ceil(BOOKS_PER_PAGE));
}

/// Quebra em linhas e pagina os trechos de um livro. Devolve as páginas (listas de linhas).
pub fn paginate(f: &Fonts, b: &BookClips) -> Vec<Vec<(String, bool)>> {
    let max_w = W - 2 * MARGIN - 36;
    let lines_per_page = 15;
    let mut pages: Vec<Vec<(String, bool)>> = vec![Vec::new()];
    for c in &b.clips {
        let mut block: Vec<(String, bool)> = Vec::new();
        let mut cur = String::new();
        for word in c.text.split_whitespace() {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if f.measure(Face::Body, 36.0, &cand) > max_w && !cur.is_empty() {
                block.push((std::mem::take(&mut cur), c.note));
                cur = word.to_string();
            } else {
                cur = cand;
            }
        }
        if !cur.is_empty() {
            block.push((cur, c.note));
        }
        let page = pages.last_mut().unwrap();
        if !page.is_empty() && page.len() + block.len() + 1 > lines_per_page {
            pages.push(Vec::new());
        }
        let page = pages.last_mut().unwrap();
        if !page.is_empty() {
            page.push((String::new(), false)); // espaço entre trechos
        }
        page.extend(block.into_iter().take(lines_per_page));
    }
    pages
}

pub fn draw_book(fb: &mut Framebuffer, f: &Fonts, b: &BookClips, pages: &[Vec<(String, bool)>], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 56.0, MARGIN, 260, INK, &fit(f, Face::Display, 56.0, &b.title, W - 2 * MARGIN));
    f.draw(fb, Face::Strong, 28.0, MARGIN, 310, MUTED, &format!("{} trechos", b.clips.len()));
    let mut y = 390;
    if let Some(lines) = pages.get(page) {
        for (k, (l, note)) in lines.iter().enumerate() {
            if l.is_empty() {
                y += 26;
                continue;
            }
            // Fio vertical à esquerda de cada trecho (notas: fio tracejado).
            let solid = !*note;
            if solid || k % 2 == 0 {
                fb.fill(Rect::new(MARGIN, y - 38, 6, 52), INK);
            }
            f.draw(fb, Face::Body, 36.0, MARGIN + 30, y, INK, l);
            y += 56;
        }
    }
    pager(fb, f, page, pages.len());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agrupa_e_tira_repeticoes() {
        let src = "\u{feff}Livro A\r\n- Seu destaque ou posição 10-11 | Adicionado: x\r\n\r\nUma frase\r\n==========\r\nLivro A\r\n- Seu destaque ou posição 10-12 | Adicionado: y\r\n\r\nUma frase maior\r\n==========\r\nLivro B\r\n- Seu marcador na posição 5 | Adicionado: z\r\n\r\n\r\n==========\r\nLivro B\r\n- Sua nota na posição 7 | Adicionado: w\r\n\r\nminha nota\r\n==========\r\n";
        let b = parse(src);
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].title, "Livro A");
        assert_eq!(b[0].clips.len(), 1);
        assert_eq!(b[0].clips[0].text, "Uma frase maior");
        assert!(b[1].clips[0].note);
    }

    #[test]
    fn le_anotacoes_do_koreader_sem_executar_lua() {
        let lua = r#"
return {
  ["annotations"] = {
    {
      ["drawer"] = "lighten",
      ["text"] = "Uma linha\noutra",
      ["note"] = "Uma nota minha",
      ["datetime"] = "2026-11-12 08:30:00",
    },
    { ["drawer"] = "underline", ["text"] = "Outro destaque" },
    { ["text"] = "Marcador sem destaque", ["note"] = "não deve aparecer" },
  },
}
"#;
        let clips = parse_koreader_annotations(lua);
        assert_eq!(clips.len(), 3);
        assert_eq!(clips[0].text, "Uma linha\noutra");
        assert!(!clips[0].note);
        assert_eq!(clips[1].text, "Uma nota minha");
        assert!(clips[1].note);
        assert_eq!(clips[2].text, "Outro destaque");
    }
}
