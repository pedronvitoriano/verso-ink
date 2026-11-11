//! Histórias: ficção interativa por escolhas. Cada história é um texto simples em
//! `data/historias/*.txt` (dá para escrever no PC ou pedir ao Claude):
//!
//! ```text
//! # Título
//! @ Uma frase de apresentação
//! == inicio                    ← cada trecho tem um nome
//! Texto do trecho, em parágrafos.
//! +chave                       ← marca algo ao passar por aqui (-chave desmarca)
//! * Abrir a porta -> porta     ← escolha e o trecho para onde leva
//! * [chave] Usar a chave -> fim     ← só aparece com a marca
//! * [!chave] Forçar a porta -> fim  ← só aparece sem a marca
//! == fim
//! Texto do final.
//! FIM
//! ```
//!
//! O ponto onde você parou fica salvo por história.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

const EXAMPLES: [(&str, &str); 1] = [("a-ultima-balsa.txt", include_str!("../assets/historias/a-ultima-balsa.txt"))];
const TEXT_TOP: i32 = 250;
const BODY: f32 = 36.0;
const LINE: i32 = 54;

#[derive(Clone, Debug)]
struct Choice {
    need: Option<(bool, String)>,
    text: String,
    to: String,
}

#[derive(Clone, Debug, Default)]
struct Passage {
    paragraphs: Vec<String>,
    set: Vec<String>,
    unset: Vec<String>,
    choices: Vec<Choice>,
    end: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Story {
    pub title: String,
    pub blurb: String,
    first: String,
    passages: HashMap<String, Passage>,
}

pub fn parse(src: &str) -> Story {
    let mut st = Story::default();
    let mut cur: Option<(String, Passage)> = None;
    let mut para = String::new();
    let flush = |para: &mut String, p: &mut Passage| {
        if !para.trim().is_empty() {
            p.paragraphs.push(para.trim().to_string());
        }
        para.clear();
    };
    for line in src.lines() {
        let l = line.trim();
        if let Some(t) = l.strip_prefix("# ") {
            st.title = t.trim().into();
        } else if let Some(t) = l.strip_prefix("@ ") {
            st.blurb = t.trim().into();
        } else if let Some(id) = l.strip_prefix("==") {
            if let Some((name, mut p)) = cur.take() {
                flush(&mut para, &mut p);
                st.passages.insert(name, p);
            }
            let id = id.trim().to_string();
            if st.first.is_empty() {
                st.first = id.clone();
            }
            cur = Some((id, Passage::default()));
        } else if let Some((_, p)) = cur.as_mut() {
            if l.is_empty() {
                flush(&mut para, p);
            } else if l == "FIM" {
                p.end = true;
            } else if let Some(c) = l.strip_prefix("* ") {
                let Some((text, to)) = c.rsplit_once("->") else { continue };
                let mut text = text.trim();
                let mut need = None;
                if let Some(rest) = text.strip_prefix('[') {
                    if let Some((cond, after)) = rest.split_once(']') {
                        let cond = cond.trim();
                        need = Some(match cond.strip_prefix('!') {
                            Some(f) => (false, f.trim().to_string()),
                            None => (true, cond.to_string()),
                        });
                        text = after.trim();
                    }
                }
                p.choices.push(Choice { need, text: text.into(), to: to.trim().into() });
            } else if let Some(f) = l.strip_prefix('+').filter(|f| !f.contains(' ')) {
                p.set.push(f.into());
            } else if let Some(f) = l.strip_prefix('-').filter(|f| !f.contains(' ') && !f.is_empty()) {
                p.unset.push(f.into());
            } else {
                if !para.is_empty() {
                    para.push(' ');
                }
                para.push_str(l);
            }
        }
    }
    if let Some((name, mut p)) = cur.take() {
        flush(&mut para, &mut p);
        st.passages.insert(name, p);
    }
    st
}

fn dir() -> PathBuf {
    crate::home_dir().join("data/historias")
}

/// Histórias guardadas; na primeira vez, copia a que vem com o Verso Ink.
fn load_all() -> Vec<(PathBuf, Story)> {
    let d = dir();
    let fresh = !d.exists();
    let _ = std::fs::create_dir_all(&d);
    if fresh {
        for (name, body) in EXAMPLES {
            let _ = std::fs::write(d.join(name), body);
        }
    }
    let mut v: Vec<_> = std::fs::read_dir(&d)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| std::fs::read_to_string(e.path()).ok().map(|s| (e.path(), parse(&s))))
        .filter(|(_, s)| !s.title.is_empty() && !s.passages.is_empty())
        .collect();
    v.sort_by(|a, b| a.1.title.cmp(&b.1.title));
    v
}

fn save_path(story: &std::path::Path) -> PathBuf {
    story.with_extension("salvo")
}

/// Onde parou: (trecho, marcas).
fn load_progress(story: &std::path::Path) -> Option<(String, BTreeSet<String>)> {
    let t = std::fs::read_to_string(save_path(story)).ok()?;
    let mut l = t.lines();
    let at = l.next()?.trim().to_string();
    let flags = l.next().unwrap_or("").split(',').filter(|s| !s.is_empty()).map(str::to_string).collect();
    Some((at, flags))
}

struct Reading {
    k: usize,
    at: String,
    flags: BTreeSet<String>,
    page: usize,
}

pub struct Historias {
    list: Vec<(PathBuf, Story)>,
    read: Option<Reading>,
    /// Calculado ao desenhar (precisa das fontes): alvos de toque da tela atual.
    hits: RefCell<Vec<(u32, Rect)>>,
}

fn wrap(f: &Fonts, face: Face, size: f32, text: &str, max_w: i32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for w in text.split_whitespace() {
        let cur = lines.last().unwrap().clone();
        let cand = if cur.is_empty() { w.to_string() } else { format!("{cur} {w}") };
        if f.measure(face, size, &cand) > max_w && !cur.is_empty() {
            lines.push(w.to_string());
        } else {
            *lines.last_mut().unwrap() = cand;
        }
    }
    lines
}

impl Historias {
    pub fn new() -> Self {
        Historias { list: load_all(), read: None, hits: RefCell::new(Vec::new()) }
    }

    fn open(&mut self, k: usize) {
        let (path, story) = &self.list[k];
        let (at, flags) = load_progress(path).filter(|(at, _)| story.passages.contains_key(at)).unwrap_or_else(|| (story.first.clone(), BTreeSet::new()));
        self.read = Some(Reading { k, at, flags, page: 0 });
    }

    fn go_to(&mut self, to: &str) {
        let Some(r) = self.read.as_mut() else { return };
        let (path, story) = &self.list[r.k];
        let Some(p) = story.passages.get(to) else { return };
        if to == story.first {
            r.flags.clear();
        }
        for f in &p.set {
            r.flags.insert(f.clone());
        }
        for f in &p.unset {
            r.flags.remove(f);
        }
        r.at = to.to_string();
        r.page = 0;
        let flags: Vec<&str> = r.flags.iter().map(String::as_str).collect();
        let _ = std::fs::write(save_path(path), format!("{to}\n{}\n", flags.join(",")));
    }

    fn visible_choices(&self) -> Vec<Choice> {
        let Some(r) = &self.read else { return vec![] };
        let Some(p) = self.list[r.k].1.passages.get(&r.at) else { return vec![] };
        p.choices.iter().filter(|c| c.need.as_ref().is_none_or(|(want, f)| r.flags.contains(f) == *want)).cloned().collect()
    }

    fn draw_list(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Display, 130.0, MARGIN - 6, 330, INK, "Histórias");
        f.draw(fb, Face::Strong, 30.0, MARGIN, 400, MUTED, "Ficção interativa: você escolhe o caminho.");
        let mut hits = Vec::new();
        for (k, (path, s)) in self.list.iter().enumerate().take(6) {
            let r = Rect::new(MARGIN - 16, 450 + k as i32 * 150, W - 2 * MARGIN + 32, 136);
            fb.round_outline(r, 28, 4, INK, PAPER);
            let started = load_progress(path).is_some_and(|(at, _)| at != s.first);
            f.draw(fb, Face::Display, 44.0, r.x + 32, r.y + 58, INK, &crate::apps::fit(f, Face::Display, 44.0, &s.title, r.w - 64));
            let sub = if started { "Continuar de onde parou".to_string() } else { s.blurb.clone() };
            f.draw(fb, Face::Body, 28.0, r.x + 34, r.y + 106, MUTED, &crate::apps::fit(f, Face::Body, 28.0, &sub, r.w - 68));
            hits.push((900 + k as u32, r));
        }
        let tip = ["Escreva novas histórias em data/historias,", "pelo PC ou pedindo ao Claude."];
        for (k, l) in tip.iter().enumerate() {
            f.draw(fb, Face::Body, 28.0, MARGIN, 1340 + k as i32 * 42, MUTED, l);
        }
        *self.hits.borrow_mut() = hits;
    }

    fn draw_reading(&self, fb: &mut Framebuffer, f: &Fonts, r: &Reading) {
        let story = &self.list[r.k].1;
        let Some(p) = story.passages.get(&r.at) else { return };
        let tw = f.measure(Face::Strong, 30.0, &story.title).min(W - 2 * MARGIN - 260);
        f.draw(fb, Face::Strong, 30.0, W - MARGIN - tw, 128, MUTED, &crate::apps::fit(f, Face::Strong, 30.0, &story.title, W - 2 * MARGIN - 260));
        // Escolhas de baixo para cima; o texto usa o que sobra.
        let width = W - 2 * MARGIN;
        let mut options: Vec<(u32, String)> = if p.end {
            vec![(702, "Outras histórias".into()), (701, "Ler de novo desde o começo".into())]
        } else {
            self.visible_choices().into_iter().enumerate().map(|(i, c)| (i as u32, c.text)).collect()
        };
        // Texto todo em linhas (parágrafos separados por uma linha meia).
        let mut lines: Vec<Option<String>> = Vec::new();
        for (k, para) in p.paragraphs.iter().enumerate() {
            if k > 0 {
                lines.push(None);
            }
            lines.extend(wrap(f, Face::Body, BODY, para, width).into_iter().map(Some));
        }
        let choice_h = |text: &str| 34 + wrap(f, Face::Strong, 32.0, text, width - 64).len() as i32 * 42;
        let layout_top = |opts: &[(u32, String)]| 1430 - opts.iter().map(|(_, t)| choice_h(t) + 14).sum::<i32>() - if p.end { 60 } else { 0 };
        // Páginas: se o texto não cabe acima das escolhas, mostra só "Continuar" até a última.
        let cap = |top: i32| (((top - 40 - TEXT_TOP) / LINE).max(3)) as usize;
        let full_cap = cap(layout_top(&options));
        let more_cap = cap(layout_top(&[(700, "Continuar ›".into())]));
        let mut pages: Vec<(usize, usize)> = Vec::new();
        let mut start = 0;
        while lines.len() - start > full_cap {
            let end = (start + more_cap).min(lines.len());
            pages.push((start, end));
            start = end;
        }
        pages.push((start, lines.len()));
        let page = r.page.min(pages.len() - 1);
        if page + 1 < pages.len() {
            options = vec![(700, "Continuar ›".into())];
        }
        let (a, b) = pages[page];
        let mut y = TEXT_TOP;
        for l in &lines[a..b] {
            match l {
                Some(t) => {
                    f.draw(fb, Face::Body, BODY, MARGIN, y + 40, INK, t);
                    y += LINE;
                }
                None => y += LINE / 2,
            }
        }
        if p.end && page + 1 == pages.len() {
            f.draw(fb, Face::Display, 56.0, MARGIN, y + 70, INK, "Fim.");
        }
        let mut hits = Vec::new();
        let mut cy = layout_top(&options) + if p.end { 60 } else { 0 };
        for (id, text) in &options {
            let h = choice_h(text);
            let rr = Rect::new(MARGIN - 16, cy, W - 2 * MARGIN + 32, h);
            let filled = *id == 700 || *id == 701;
            if filled {
                fb.round_rect(rr, 26, INK);
            } else {
                fb.round_outline(rr, 26, 4, INK, PAPER);
            }
            for (k, l) in wrap(f, Face::Strong, 32.0, text, width - 64).iter().enumerate() {
                f.draw(fb, Face::Strong, 32.0, rr.x + 32, rr.y + 52 + k as i32 * 42, if filled { PAPER } else { INK }, l);
            }
            hits.push((*id, rr));
            cy += h + 14;
        }
        if pages.len() > 1 {
            let t = format!("{} de {}", page + 1, pages.len());
            let w = f.measure(Face::Body, 26.0, &t);
            f.draw(fb, Face::Body, 26.0, (W - w) / 2, 206, MUTED, &t);
        }
        *self.hits.borrow_mut() = hits;
    }
}

impl MiniGame for Historias {
    fn summary(&self) -> String {
        self.read.as_ref().map(|r| self.list[r.k].1.title.clone()).unwrap_or_default()
    }

    fn context(&self) -> String {
        let Some(r) = &self.read else { return "Escolhendo uma história.".into() };
        let (_, story) = &self.list[r.k];
        let text = story.passages.get(&r.at).map(|p| p.paragraphs.join("\n\n")).unwrap_or_default();
        let choices: Vec<String> = self.visible_choices().iter().map(|c| format!("- {}", c.text)).collect();
        format!("Lendo a história interativa \"{}\". Trecho atual:\n{text}\nEscolhas na tela:\n{}", story.title, choices.join("\n"))
    }

    fn at_menu(&self) -> bool {
        self.read.is_none()
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match &self.read {
            None => self.draw_list(fb, f),
            Some(r) => self.draw_reading(fb, f, r),
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        self.hits.borrow().clone()
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900..=905 => {
                self.open((id - 900) as usize);
                Redraw::Full
            }
            700 => {
                if let Some(r) = self.read.as_mut() {
                    r.page += 1;
                }
                Redraw::Full
            }
            701 => {
                let first = self.read.as_ref().map(|r| self.list[r.k].1.first.clone()).unwrap_or_default();
                self.go_to(&first);
                Redraw::Full
            }
            702 => {
                self.read = None;
                Redraw::Full
            }
            i => {
                let Some(c) = self.visible_choices().get(i as usize).cloned() else { return Redraw::None };
                self.go_to(&c.to);
                Redraw::Full
            }
        }
    }

    fn back(&mut self) -> bool {
        if self.read.is_some() {
            self.read = None;
            return true;
        }
        false
    }

    fn keep_awake(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn historia_incluida_e_coerente() {
        let s = parse(EXAMPLES[0].1);
        assert_eq!(s.title, "A última balsa");
        assert_eq!(s.first, "inicio");
        // Toda escolha leva a um trecho que existe; todo trecho sem escolha é final.
        for (name, p) in &s.passages {
            for c in &p.choices {
                assert!(s.passages.contains_key(&c.to), "{name} → {} não existe", c.to);
            }
            assert!(p.end || !p.choices.is_empty(), "{name} não tem saída");
            assert!(!p.paragraphs.is_empty(), "{name} sem texto");
        }
        assert!(s.passages.values().filter(|p| p.end).count() >= 2);
        let proa = &s.passages["proa"];
        assert_eq!(proa.choices[0].need, Some((true, "chave".into())));
        assert_eq!(proa.choices[1].need, Some((false, "chave".into())));
    }

    #[test]
    fn todo_final_e_alcancavel() {
        let s = parse(EXAMPLES[0].1);
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![s.first.clone()];
        while let Some(n) = stack.pop() {
            if seen.insert(n.clone()) {
                stack.extend(s.passages[&n].choices.iter().map(|c| c.to.clone()));
            }
        }
        assert_eq!(seen.len(), s.passages.len());
    }
}
