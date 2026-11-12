//! Revisão dos destaques: cada trecho marcado no Kindle vira um cartão. Na frente, uma pergunta
//! que o Claude escreveu sobre a ideia do trecho; atrás, o trecho, o livro e uma ligação com o
//! que você destacou em outro livro. "Lembrei" empurra o cartão para mais longe (1, 3, 7, 16, 35
//! e 90 dias); "Não lembrei" traz de volta amanhã. Sem Wi-Fi, a frente esconde uma palavra-chave.
//!
//! Arquivos (em data/): revisao.txt (id|caixa|dia), revisao_q.txt (id\tpergunta\tligação).

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::collections::HashMap;

const INTERVALS: [i64; 6] = [1, 3, 7, 16, 35, 90];
/// Cartões novos por dia (os vencidos vêm todos).
const NEW_PER_DAY: usize = 5;
const SESSION_MAX: usize = 15;

#[derive(Clone)]
pub struct Card {
    pub id: String,
    pub book: String,
    pub text: String,
}

fn short_book(t: &str) -> String {
    t.split(" (").next().unwrap_or(t).split(" - ").next().unwrap_or(t).trim().to_string()
}

fn card_id(book: &str, text: &str) -> String {
    // FNV-1a: estável entre versões e entre o Kindle e o PC.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in book.bytes().chain([0u8]).chain(text.bytes()) {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// Todos os destaques (sem as notas, que são suas e não do livro), do mais antigo ao mais novo.
pub fn cards() -> Vec<Card> {
    cards_from(&crate::clippings::load())
}

fn cards_from(books: &[crate::clippings::BookClips]) -> Vec<Card> {
    let mut v = Vec::new();
    for b in books {
        for c in &b.clips {
            if !c.note && c.text.split_whitespace().count() >= 5 {
                v.push(Card { id: card_id(&b.title, &c.text), book: short_book(&b.title), text: c.text.clone() });
            }
        }
    }
    v
}

fn data(name: &str) -> std::path::PathBuf {
    let d = crate::home_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d.join(name)
}

/// Estado de cada cartão: (caixa, dia em que volta).
fn states() -> HashMap<String, (usize, i64)> {
    std::fs::read_to_string(data("revisao.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.split('|');
            Some((p.next()?.to_string(), (p.next()?.parse().ok()?, p.next()?.parse().ok()?)))
        })
        .collect()
}

fn save_states(s: &HashMap<String, (usize, i64)>) {
    let mut lines: Vec<String> = s.iter().map(|(id, (b, d))| format!("{id}|{b}|{d}")).collect();
    lines.sort();
    let _ = std::fs::write(data("revisao.txt"), lines.join("\n"));
}

/// Perguntas do Claude: id -> (pergunta, ligação).
fn questions() -> HashMap<String, (String, String)> {
    std::fs::read_to_string(data("revisao_q.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            Some((p.next()?.to_string(), (p.next()?.to_string(), p.next().unwrap_or("").to_string())))
        })
        .collect()
}

/// Novos já apresentados hoje (para não passar de NEW_PER_DAY).
fn new_today(today: i64) -> usize {
    std::fs::read_to_string(data("revisao_novos.txt")).unwrap_or_default().lines().filter(|l| l.parse::<i64>().ok() == Some(today)).count()
}

/// A sessão de hoje: vencidos primeiro (os mais atrasados antes), depois alguns novos.
pub fn due(all: &[Card], st: &HashMap<String, (usize, i64)>, today: i64, new_seen: usize) -> Vec<Card> {
    let mut old: Vec<(&Card, i64)> = all.iter().filter_map(|c| st.get(&c.id).filter(|(_, d)| *d <= today).map(|(_, d)| (c, *d))).collect();
    old.sort_by_key(|(_, d)| *d);
    let mut v: Vec<Card> = old.into_iter().map(|(c, _)| c.clone()).collect();
    // Novos: alterna livros para não ser tudo do mesmo.
    let fresh: Vec<&Card> = all.iter().filter(|c| !st.contains_key(&c.id)).collect();
    let mut by_book: Vec<Vec<&Card>> = Vec::new();
    for c in fresh {
        match by_book.iter_mut().find(|b| b[0].book == c.book) {
            Some(b) => b.push(c),
            None => by_book.push(vec![c]),
        }
    }
    let mut k = 0;
    let want = NEW_PER_DAY.saturating_sub(new_seen);
    let mut added = 0;
    while added < want && by_book.iter().any(|b| k < b.len()) {
        for b in &by_book {
            if added < want && k < b.len() {
                v.push(b[k].clone());
                added += 1;
            }
        }
        k += 1;
    }
    v.truncate(SESSION_MAX);
    v
}

pub fn today() -> i64 {
    crate::data::today(crate::offset_min())
}

/// Quantos cartões esperam hoje (para o botão da tela Destaques).
pub fn due_count() -> usize {
    due(&cards(), &states(), today(), new_today(today())).len()
}

/// Sem pergunta do Claude: esconde a palavra mais longa do trecho.
fn cloze(text: &str) -> String {
    let key = text.split_whitespace().map(|w| w.trim_matches(|c: char| !c.is_alphanumeric())).filter(|w| w.chars().count() >= 6).max_by_key(|w| w.chars().count());
    match key {
        Some(k) => text.replacen(k, "_____", 1),
        None => text.to_string(),
    }
}

/// Pede ao Claude as perguntas dos próximos cartões (roda na sincronização, em segundo plano).
pub fn prepare() {
    if !crate::chat::configured() {
        return;
    }
    let all = cards();
    if all.is_empty() {
        return;
    }
    let st = states();
    let qs = questions();
    let t = today();
    // Os de hoje e os próximos novos.
    let mut want: Vec<Card> = due(&all, &st, t, 0);
    want.extend(due(&all, &st, t + 1, 0));
    want.retain(|c| !qs.contains_key(&c.id));
    want.dedup_by(|a, b| a.id == b.id);
    want.truncate(12);
    if want.is_empty() {
        return;
    }
    // Outros trechos, de outros livros, para o Claude achar ligações.
    let others: Vec<String> = all.iter().filter(|c| !want.iter().any(|w| w.id == c.id)).rev().take(40).map(|c| format!("{{\"livro\": {}, \"texto\": {}}}", crate::chat::json_str(&c.book), crate::chat::json_str(&c.text.chars().take(240).collect::<String>()))).collect();
    let items: Vec<String> = want.iter().map(|c| format!("{{\"id\": {}, \"livro\": {}, \"texto\": {}}}", crate::chat::json_str(&c.id), crate::chat::json_str(&c.book), crate::chat::json_str(&c.text.chars().take(700).collect::<String>()))).collect();
    let body = format!("{{\"itens\": [{}], \"outros\": [{}]}}", items.join(","), others.join(","));
    let Ok(j) = crate::chat::call("POST", "/destaques/perguntas", Some(&body), 240) else { return };
    let mut out = std::fs::read_to_string(data("revisao_q.txt")).unwrap_or_default();
    for c in j.get("cartoes").map(|a| a.arr()).unwrap_or(&[]) {
        let (Some(id), Some(q)) = (c.get("id").and_then(|x| x.str()), c.get("pergunta").and_then(|x| x.str())) else { continue };
        let link = c.get("ligacao").and_then(|x| x.str()).unwrap_or("");
        let clean = |s: &str| s.replace(['\t', '\n'], " ");
        if want.iter().any(|w| w.id == id) {
            out.push_str(&format!("{}\t{}\t{}\n", id, clean(q), clean(link)));
        }
    }
    let _ = std::fs::write(data("revisao_q.txt"), out);
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Hub,
    Front,
    Back,
    Done,
}

pub struct Revisao {
    view: View,
    session: Vec<Card>,
    pos: usize,
    remembered: usize,
    total: usize,
    mastered: usize,
    questions: HashMap<String, (String, String)>,
    cel: Option<crate::aprender::Cel>,
}

impl Revisao {
    pub fn new() -> Self {
        let all = cards();
        let st = states();
        let t = today();
        let session = due(&all, &st, t, new_today(t));
        let mastered = all.iter().filter(|c| st.get(&c.id).is_some_and(|(b, _)| *b >= 4)).count();
        Revisao { view: View::Hub, session, pos: 0, remembered: 0, total: all.len(), mastered, questions: questions(), cel: None }
    }

    #[cfg(test)]
    pub fn with(cards: Vec<Card>, q: HashMap<String, (String, String)>) -> Self {
        Revisao { view: View::Hub, total: cards.len(), session: cards, pos: 0, remembered: 0, mastered: 0, questions: q, cel: None }
    }

    fn card(&self) -> Option<&Card> {
        self.session.get(self.pos)
    }

    fn front_text(&self, c: &Card) -> (String, bool) {
        match self.questions.get(&c.id) {
            Some((q, _)) if !q.is_empty() => (q.clone(), true),
            _ => (cloze(&c.text), false),
        }
    }

    fn btn(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 20) / 2;
        Rect::new(MARGIN + k as i32 * (w + 20), 1290, w, 104)
    }

    fn wide() -> Rect {
        Rect::new(MARGIN, 1290, W - 2 * MARGIN, 104)
    }

    fn grade(&mut self, ok: bool) {
        let Some(c) = self.card().cloned() else { return };
        let mut st = states();
        let t = today();
        let was_new = !st.contains_key(&c.id);
        let b = st.get(&c.id).map(|(b, _)| *b).unwrap_or(0);
        let nb = if ok { (b + usize::from(!was_new)).min(INTERVALS.len() - 1) } else { 0 };
        st.insert(c.id.clone(), (nb, t + if ok { INTERVALS[nb] } else { 1 }));
        save_states(&st);
        if was_new {
            let mut s = std::fs::read_to_string(data("revisao_novos.txt")).unwrap_or_default();
            s.push_str(&format!("{t}\n"));
            let _ = std::fs::write(data("revisao_novos.txt"), s);
        }
        if ok {
            self.remembered += 1;
        }
        // 2 XP por cartão revisado (lembrando ou não: o esforço é o mesmo).
        self.cel = crate::aprender::add(2, "revisao");
        self.pos += 1;
        self.view = if self.pos >= self.session.len() { View::Done } else { View::Front };
    }

    fn progress(&self, fb: &mut Framebuffer, f: &Fonts) {
        let n = self.session.len().max(1);
        let t = format!("{} de {}", (self.pos + 1).min(n), n);
        let w = f.measure(Face::Strong, 28.0, &t);
        f.draw(fb, Face::Strong, 28.0, W - MARGIN - w, 200, MUTED, &t);
        let bar = Rect::new(MARGIN, 226, W - 2 * MARGIN, 10);
        fb.round_rect(bar, 5, 0xC8);
        fb.round_rect(Rect::new(bar.x, bar.y, (bar.w * self.pos as i32 / n as i32).max(10), bar.h), 5, INK);
    }

    fn draw_lines(fb: &mut Framebuffer, f: &Fonts, face: Face, size: f32, x: i32, y: i32, w: i32, text: &str, max_lines: usize) -> i32 {
        let lh = (size * 1.32) as i32;
        let lines = crate::chat::wrap_words(f, face, size, text, w);
        for (k, l) in lines.iter().take(max_lines).enumerate() {
            let l = if k + 1 == max_lines && lines.len() > max_lines { format!("{}…", l.trim_end_matches(['.', ','])) } else { l.clone() };
            f.draw(fb, face, size, x, y + k as i32 * lh, INK, &l);
        }
        y + lines.len().min(max_lines) as i32 * lh
    }

    /// Escolhe o tamanho para o texto caber na altura.
    fn fit_size(f: &Fonts, face: Face, text: &str, w: i32, h: i32, max: f32, min: f32) -> f32 {
        let mut s = max;
        while s > min && crate::chat::wrap_words(f, face, s, text, w).len() as i32 * (s * 1.32) as i32 > h {
            s -= 2.0;
        }
        s
    }
}

impl MiniGame for Revisao {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        if let Some(c) = self.cel {
            crate::aprender::draw_celebration(fb, f, c);
            return;
        }
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Hub => {
                crate::sprites::draw(fb, "cab-ler", Rect::new(W - MARGIN - 300, 96, 310, 240));
                f.draw(fb, Face::Display, 110.0, MARGIN - 4, 300, INK, "Revisar");
                let sub = "O que você destacou, de volta na hora certa para não esquecer.";
                Self::draw_lines(fb, f, Face::Strong, 30.0, MARGIN, 360, W - 2 * MARGIN - 320, sub, 3);
                // Números grandes: hoje, total e dominados.
                let stats = [(self.session.len(), "para hoje"), (self.total, "destaques"), (self.mastered, "dominados")];
                let cw = (W - 2 * MARGIN - 2 * 20) / 3;
                for (k, (n, label)) in stats.iter().enumerate() {
                    let r = Rect::new(MARGIN + k as i32 * (cw + 20), 520, cw, 200);
                    ui::card(fb, r, 28, if k == 0 { INK } else { PAPER }, false);
                    let fg = if k == 0 { PAPER } else { INK };
                    let s = n.to_string();
                    let w = f.measure(Face::Display, 96.0, &s);
                    f.draw(fb, Face::Display, 96.0, r.x + (r.w - w) / 2, r.y + 118, fg, &s);
                    let w = f.measure(Face::Strong, 28.0, label);
                    f.draw(fb, Face::Strong, 28.0, r.x + (r.w - w) / 2, r.y + 168, if k == 0 { 0xCC } else { MUTED }, label);
                }
                let how = [
                    "Na frente, uma pergunta sobre a ideia do trecho. Pense na resposta antes de virar.",
                    "Atrás, o trecho e o livro. Seja honesto: lembrou ou não?",
                    "Lembrou, o cartão volta em 3, 7, 16, 35 e 90 dias. Esqueceu, volta amanhã.",
                ];
                let mut y = 800;
                for (k, t) in how.iter().enumerate() {
                    fb.round_rect(Rect::new(MARGIN, y - 30, 44, 44), 22, INK);
                    let num = (k + 1).to_string();
                    let w = f.measure(Face::Strong, 26.0, &num);
                    f.draw(fb, Face::Strong, 26.0, MARGIN + 22 - w / 2, y + 1, PAPER, &num);
                    y = Self::draw_lines(fb, f, Face::Body, 29.0, MARGIN + 70, y, W - 2 * MARGIN - 70, t, 3) + 24;
                }
                if self.session.is_empty() {
                    let msg = if self.total == 0 { "Marque trechos nos livros do Kindle: eles aparecem aqui." } else { "Nada para hoje. Volte amanhã." };
                    ui::button(fb, f, Self::wide(), msg, false);
                } else {
                    ui::button(fb, f, Self::wide(), "Começar", true);
                }
            }
            View::Front => {
                let Some(c) = self.card() else { return };
                self.progress(fb, f);
                let (q, from_claude) = self.front_text(c);
                f.draw(fb, Face::Strong, 26.0, MARGIN, 310, MUTED, &crate::apps::fit(f, Face::Strong, 26.0, &format!("{} · {}", if from_claude { "PERGUNTA" } else { "COMPLETE O TRECHO" }, c.book.to_uppercase()), W - 2 * MARGIN));
                let face = if from_claude { Face::Display } else { Face::Body };
                let size = Self::fit_size(f, face, &q, W - 2 * MARGIN, 820, if from_claude { 54.0 } else { 40.0 }, 28.0);
                Self::draw_lines(fb, f, face, size, MARGIN, 400, W - 2 * MARGIN, &q, 16);
                f.draw(fb, Face::Body, 28.0, MARGIN, 1250, MUTED, "Pense na resposta. Depois vire o cartão.");
                ui::button(fb, f, Self::wide(), "Virar", true);
            }
            View::Back => {
                let Some(c) = self.card() else { return };
                self.progress(fb, f);
                let (q, from_claude) = self.front_text(c);
                let mut y = 300;
                if from_claude {
                    y = Self::draw_lines(fb, f, Face::Strong, 28.0, MARGIN, y, W - 2 * MARGIN, &q, 3) + 20;
                }
                // O trecho num cartão com fio à esquerda, como um destaque.
                let link = self.questions.get(&c.id).map(|(_, l)| l.clone()).unwrap_or_default();
                let avail = 1240 - y - if link.is_empty() { 0 } else { 170 };
                let size = Self::fit_size(f, Face::Body, &c.text, W - 2 * MARGIN - 80, avail - 120, 38.0, 24.0);
                let lines = crate::chat::wrap_words(f, Face::Body, size, &c.text, W - 2 * MARGIN - 80).len().min(24) as i32;
                let h = lines * (size * 1.32) as i32 + 110;
                let r = Rect::new(MARGIN - 10, y, W - 2 * MARGIN + 20, h);
                fb.round_rect(r, 26, ui::SURFACE);
                fb.round_rect(Rect::new(r.x + 26, r.y + 26, 7, h - 52), 3, INK);
                let ty = Self::draw_lines(fb, f, Face::Body, size, r.x + 58, r.y + 26 + size as i32, r.w - 90, &c.text, 24);
                f.draw(fb, Face::Strong, 26.0, r.x + 58, ty + 18, MUTED, &crate::apps::fit(f, Face::Strong, 26.0, &format!("— {}", c.book), r.w - 90));
                if !link.is_empty() {
                    let ly = r.y + r.h + 40;
                    f.draw(fb, Face::Strong, 26.0, MARGIN, ly, MUTED, "LIGA COM");
                    Self::draw_lines(fb, f, Face::Body, 29.0, MARGIN, ly + 44, W - 2 * MARGIN, &link, 3);
                }
                ui::button(fb, f, Self::btn(0), "Não lembrei", false);
                ui::button(fb, f, Self::btn(1), "Lembrei", true);
            }
            View::Done => {
                f.draw(fb, Face::Display, 100.0, MARGIN - 4, 330, INK, "Feito");
                let n = self.session.len();
                let msg = format!("{} de {} lembrados hoje. Os que escaparam voltam amanhã; os outros, mais para frente.", self.remembered, n);
                let y = Self::draw_lines(fb, f, Face::Body, 34.0, MARGIN, 420, W - 2 * MARGIN, &msg, 4);
                crate::aprender::draw_panel(fb, f, Rect::new(MARGIN, y + 60, W - 2 * MARGIN, 180), &crate::aprender::status());
                crate::aprender::week_row(fb, f, MARGIN, y + 300, W - 2 * MARGIN, &crate::aprender::status());
                ui::button(fb, f, Self::wide(), "Voltar", true);
            }
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.cel.is_some() {
            return vec![(999, crate::aprender::cel_button())];
        }
        match self.view {
            View::Hub if !self.session.is_empty() => vec![(1, Self::wide())],
            View::Front => vec![(2, Self::wide()), (2, Rect::new(0, 280, W, 960))],
            View::Back => vec![(3, Self::btn(0)), (4, Self::btn(1))],
            View::Done => vec![(5, Self::wide())],
            _ => vec![],
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
            (View::Hub, 1) => {
                self.pos = 0;
                self.remembered = 0;
                self.view = View::Front;
            }
            (View::Front, 2) => self.view = View::Back,
            (View::Back, 3) => self.grade(false),
            (View::Back, 4) => self.grade(true),
            (View::Done, 5) => {
                *self = Revisao::new();
            }
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn at_menu(&self) -> bool {
        self.view == View::Hub
    }

    fn back(&mut self) -> bool {
        if self.cel.take().is_some() {
            return true;
        }
        if matches!(self.view, View::Front | View::Back) {
            self.view = View::Hub;
            return true;
        }
        false
    }

    fn context(&self) -> String {
        match (self.view, self.card()) {
            (View::Front | View::Back, Some(c)) => format!("Revisando um destaque do livro \"{}\": \"{}\". O Pedro quer entender e lembrar essa ideia.", c.book, c.text),
            _ => "Revisão espaçada dos destaques do Kindle.".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessao_e_intervalos() {
        let books = crate::clippings::parse("Livro A (Autor)\n- Seu destaque na posição 1 | x\n\nUma ideia bem longa sobre hábitos pequenos e constantes\n==========\nLivro B\n- Seu destaque na posição 2 | y\n\nOutra ideia sobre atenção profunda no trabalho diário\n==========\nLivro A (Autor)\n- Seu destaque na posição 9 | z\n\nTerceira ideia sobre começar devagar e manter sempre\n==========\n");
        let all = cards_from(&books);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].book, "Livro A");
        let mut st = HashMap::new();
        // Sem estado: só novos, alternando livros.
        let s = due(&all, &st, 100, 0);
        assert_eq!(s.len(), 3);
        assert_ne!(s[0].book, s[1].book);
        // Limite de novos por dia.
        assert_eq!(due(&all, &st, 100, NEW_PER_DAY).len(), 0);
        // Vencido entra; futuro não.
        st.insert(all[0].id.clone(), (2, 99));
        st.insert(all[1].id.clone(), (2, 150));
        let s = due(&all, &st, 100, NEW_PER_DAY);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, all[0].id);
        assert!(cloze("Uma ideia sobre constância").contains("_____"));
        assert_eq!(card_id("a", "b"), card_id("a", "b"));
        assert_ne!(card_id("a", "b"), card_id("a", "c"));
    }
}
