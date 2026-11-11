//! Destaques e notas do Kindle ("My Clippings.txt"), agrupados por livro.

use crate::apps::fit;
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MARGIN: i32 = 72;
const W: i32 = 1072;

pub struct Clip {
    pub text: String,
    pub note: bool,
}

pub struct BookClips {
    pub title: String,
    pub clips: Vec<Clip>,
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
                continue;
            }
        }
        clips.push(Clip { text, note });
    }
    books.sort_by_key(|b| std::cmp::Reverse(b.clips.len()));
    books
}

pub fn load() -> Vec<BookClips> {
    std::fs::read_to_string("/mnt/us/documents/My Clippings.txt").map(|s| parse(&s)).unwrap_or_default()
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

pub fn draw_books(fb: &mut Framebuffer, f: &Fonts, books: &[BookClips], page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 130.0, MARGIN - 6, 310, INK, "Destaques");
    let total: usize = books.iter().map(|b| b.clips.len()).sum();
    let sub = if books.is_empty() { "Nenhum destaque ainda. Os que você marcar no Kindle aparecem aqui.".to_string() } else { format!("{total} trechos em {} livros", books.len()) };
    f.draw(fb, Face::Strong, 30.0, MARGIN, 380, MUTED, &fit(f, Face::Strong, 30.0, &sub, W - 2 * MARGIN));
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
}
