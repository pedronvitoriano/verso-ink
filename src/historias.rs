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

const EXAMPLES: [(&str, &str); 5] = [
    ("a-ultima-balsa.txt", include_str!("../assets/historias/a-ultima-balsa.txt")),
    ("o-farol-desligado.txt", include_str!("../assets/historias/o-farol-desligado.txt")),
    ("estacao-omega.txt", include_str!("../assets/historias/estacao-omega.txt")),
    ("o-bolo-da-tia-zuleica.txt", include_str!("../assets/historias/o-bolo-da-tia-zuleica.txt")),
    ("a-biblioteca-que-nao-fecha.txt", include_str!("../assets/historias/a-biblioteca-que-nao-fecha.txt")),
];

/// Tira os caminhos que voltam para trás (um trecho que leva a um anterior dele), olhando só a
/// estrutura, sem as marcas: usado nas histórias geradas, que às vezes fazem "tente de novo".
/// Trecho que fica sem escolha vira final.
fn break_loops(s: &mut Story) {
    fn dfs(s: &Story, n: &str, on: &mut Vec<String>, done: &mut BTreeSet<String>, cut: &mut Vec<(String, String)>) {
        if done.contains(n) {
            return;
        }
        on.push(n.to_string());
        if let Some(p) = s.passages.get(n) {
            for c in &p.choices {
                if on.contains(&c.to) {
                    cut.push((n.to_string(), c.to.clone()));
                } else {
                    dfs(s, &c.to, on, done, cut);
                }
            }
        }
        on.pop();
        done.insert(n.to_string());
    }
    let mut cut = Vec::new();
    dfs(s, &s.first.clone(), &mut Vec::new(), &mut BTreeSet::new(), &mut cut);
    for (from, to) in cut {
        if let Some(p) = s.passages.get_mut(&from) {
            p.choices.retain(|c| c.to != to);
            if p.choices.is_empty() {
                p.end = true;
            }
        }
    }
}
/// Escolha especial do livro: escreve o próximo capítulo.
const NEXT: &str = "@proximo";
const BOOK_CHAPTERS: u32 = 8;
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

/// Histórias por página da lista.
const PER: usize = 6;

fn dir() -> PathBuf {
    crate::home_dir().join("data/historias")
}

/// Histórias guardadas; na primeira vez, copia a que vem com o Verso Ink.
fn load_all() -> Vec<(PathBuf, Story)> {
    let d = dir();
    let fresh = !d.exists();
    let _ = std::fs::create_dir_all(&d);
    let _ = fresh;
    // As histórias que vêm com o Verso Ink: novas são copiadas e as corrigidas, atualizadas.
    for (name, body) in EXAMPLES {
        let p = d.join(name);
        if std::fs::read_to_string(&p).ok().as_deref() != Some(body) {
            let _ = std::fs::write(p, body);
        }
    }
    let mut v: Vec<_> = std::fs::read_dir(&d)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| {
            let bundled = EXAMPLES.iter().any(|(n, _)| e.file_name() == *n);
            std::fs::read_to_string(e.path()).ok().map(|s| {
                let mut st = parse(&s);
                if !bundled {
                    break_loops(&mut st);
                }
                (e.path(), st)
            })
        })
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
    /// Página da lista de histórias.
    list_page: usize,
    /// Pediu ao app o texto do rumo novo.
    want_text: bool,
    /// O texto pedido é o tema de uma história nova (e não o rumo da atual).
    want_new: bool,
    /// Escolhendo o tamanho da história a gerar (antes de pedir o tema).
    picking: bool,
    /// Tamanho escolhido: 0 conto, 1 novela, 2 livro.
    size: u8,
    /// Recado na leitura (erro ao gerar, por exemplo).
    note: Option<String>,
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
        Historias { list: load_all(), read: None, hits: RefCell::new(Vec::new()), want_text: false, want_new: false, picking: false, size: 0, note: None, list_page: 0 }
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
        let gen = gen_rect();
        ui::button(fb, f, gen, "Gerar história", true);
        let n = self.list.len();
        f.draw(fb, Face::Strong, 30.0, MARGIN, 400, MUTED, &format!("{n} {} · você escolhe o caminho ou cria outro", if n == 1 { "história" } else { "histórias" }));
        let mut hits = Vec::new();
        let pages = n.div_ceil(PER).max(1);
        let page = self.list_page.min(pages - 1);
        for (j, (path, s)) in self.list.iter().enumerate().skip(page * PER).take(PER) {
            let k = j - page * PER;
            let r = Rect::new(MARGIN, 446 + k as i32 * 140, W - 2 * MARGIN, 120);
            ui::card(fb, r, 28, PAPER, false);
            let started = load_progress(path).is_some_and(|(at, _)| at != s.first);
            f.draw(fb, Face::Display, 44.0, r.x + 32, r.y + 58, INK, &crate::apps::fit(f, Face::Display, 44.0, &s.title, r.w - 64 - 170));
            let sub = if started { "Continuar de onde parou".to_string() } else { s.blurb.clone() };
            f.draw(fb, Face::Body, 28.0, r.x + 34, r.y + 106, MUTED, &crate::apps::fit(f, Face::Body, 28.0, &sub, r.w - 68));
            // Histórias criadas (não as que vêm com o Verso Ink) podem ser apagadas.
            let bundled = EXAMPLES.iter().any(|(name, _)| path.file_name().is_some_and(|n| n == *name));
            if !bundled {
                let del = Rect::new(r.x + r.w - 168, r.y + (r.h - 60) / 2, 148, 60);
                ui::button(fb, f, del, "Apagar", false);
                hits.push((1900 + j as u32, del));
            }
            hits.push((900 + j as u32, r));
        }
        if pages > 1 {
            let (pr, nr) = (Rect::new(MARGIN, 1300, 260, 88), Rect::new(W - MARGIN - 260, 1300, 260, 88));
            if page > 0 {
                ui::button(fb, f, pr, "‹ Anterior", false);
                hits.push((720, pr));
            }
            if page + 1 < pages {
                ui::button(fb, f, nr, "Próxima ›", false);
                hits.push((721, nr));
            }
            let t = format!("{} de {pages}", page + 1);
            let w = f.measure(Face::Strong, 28.0, &t);
            f.draw(fb, Face::Strong, 28.0, (W - w) / 2, 1356, MUTED, &t);
        } else {
            let tip = "Em Gerar, o Verso escreve uma história nova sobre o que você pedir.";
            f.draw(fb, Face::Body, 26.0, MARGIN, 1356, MUTED, &crate::apps::fit(f, Face::Body, 26.0, tip, W - 2 * MARGIN));
        }
        hits.push((730, gen));
        if let Some(n) = &self.note {
            f.draw(fb, Face::Strong, 28.0, MARGIN, 1256, INK, &crate::apps::fit(f, Face::Strong, 28.0, n, W - 2 * MARGIN));
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
        let with_new: Vec<(u32, String)> = options.iter().cloned().chain([(710, "+ Seguir outro rumo…".to_string())]).collect();
        let full_cap = cap(layout_top(&with_new));
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
        } else {
            // Com o Claude: sair do roteiro e criar uma história nova a partir daqui.
            options.push((710, "+ Seguir outro rumo…".into()));
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
        if let Some(n) = &self.note {
            f.draw(fb, Face::Strong, 26.0, MARGIN, 206, INK, &crate::apps::fit(f, Face::Strong, 26.0, n, W - 2 * MARGIN));
        }
        let mut hits = Vec::new();
        let mut cy = layout_top(&options) + if p.end { 60 } else { 0 };
        for (id, text) in &options {
            let h = choice_h(text);
            let rr = Rect::new(MARGIN, cy, W - 2 * MARGIN, h);
            let filled = *id == 700 || *id == 701;
            if *id == 710 {
                // O rumo novo: cartão cinza, diferente das escolhas do roteiro.
                fb.round_rect(rr, 26, ui::SURFACE);
                fb.round_outline(rr, 26, 3, 0x88, ui::SURFACE);
            } else {
                ui::card(fb, Rect::new(rr.x, rr.y, rr.w, rr.h - ui::SH), 26, if filled { INK } else { PAPER }, false);
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
    fn risky(&self, id: u32) -> bool {
        (1900..=2899).contains(&id)
    }

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
        self.read.is_none() && !self.picking
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match &self.read {
            None if self.picking => self.draw_sizes(fb, f),
            None => self.draw_list(fb, f),
            Some(r) => self.draw_reading(fb, f, r),
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        self.hits.borrow().clone()
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            720 | 721 => {
                self.list_page = if id == 720 { self.list_page.saturating_sub(1) } else { self.list_page + 1 };
                Redraw::Full
            }
            1900..=2899 => {
                // Apaga a história criada e o ponto onde parou.
                if let Some((path, _)) = self.list.get((id - 1900) as usize) {
                    let _ = std::fs::remove_file(save_path(path));
                    let _ = std::fs::remove_file(path);
                }
                self.list = load_all();
                self.list_page = self.list_page.min(self.list.len().saturating_sub(1) / PER);
                Redraw::Full
            }
            900..=1899 if ((id - 900) as usize) < self.list.len() => {
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
            710 => {
                self.want_text = true;
                self.want_new = false;
                Redraw::None
            }
            730 => {
                self.picking = true;
                Redraw::Full
            }
            740..=742 => {
                self.size = (id - 740) as u8;
                self.picking = false;
                self.want_text = true;
                self.want_new = true;
                Redraw::Full
            }
            749 => {
                self.picking = false;
                Redraw::Full
            }
            i => {
                let Some(c) = self.visible_choices().get(i as usize).cloned() else { return Redraw::None };
                if c.to == NEXT {
                    // Livro: o próximo capítulo é escrito agora pelo Verso.
                    self.note = self.next_chapter().err();
                } else {
                    self.go_to(&c.to);
                }
                Redraw::Full
            }
        }
    }

    fn back(&mut self) -> bool {
        if self.picking {
            self.picking = false;
            return true;
        }
        if self.read.is_some() {
            self.read = None;
            return true;
        }
        false
    }

    fn keep_awake(&self) -> bool {
        false
    }

    fn wants_text(&mut self) -> Option<String> {
        let new = self.want_new;
        std::mem::take(&mut self.want_text).then(|| if new { "Sobre o que é a história? (tema, gênero ou uma frase)".to_string() } else { "Para onde a história vai?".to_string() })
    }

    fn text_input(&mut self, text: &str) -> Redraw {
        let r = if std::mem::take(&mut self.want_new) { self.new_story(text) } else { self.new_branch(text) };
        self.note = match r {
            Ok(()) => None,
            Err(e) => Some(e),
        };
        Redraw::Full
    }
}

impl Historias {
    /// Pede ao Claude uma história nova que parte do trecho atual e segue o rumo pedido; salva
    /// como arquivo próprio (a original não muda) e abre no começo dela.
    fn new_branch(&mut self, direction: &str) -> Result<(), String> {
        let r = self.read.as_ref().ok_or("Abra uma história antes.")?;
        let (_, story) = &self.list[r.k];
        let here = story.passages.get(&r.at).map(|p| p.paragraphs.join("\n\n")).unwrap_or_default();
        let context = format!("História \"{}\". Trecho onde o leitor está:\n{here}", story.title);
        let prompt = format!(
            "Escreva uma história interativa NOVA que continua exatamente do trecho do contexto, mas segue este rumo pedido pelo leitor: «{direction}». \
Português do Brasil, prosa literária enxuta (trechos de 60 a 140 palavras), de 8 a 14 trechos, 2 ou 3 escolhas por trecho, pelo menos 3 finais diferentes. \
Responda SÓ com o texto da história neste formato (sem comentários, sem cercas de código):\n\
# Título novo\n@ Uma frase de resumo\n\n== inicio\nTexto do trecho.\n\n* Texto da escolha -> nome_do_trecho\n* Outra escolha -> outro_trecho\n\n== nome_do_trecho\nTexto...\n\n\
A história sempre anda para a frente: nenhuma escolha volta a um trecho anterior. \
Nomes de trecho em minúsculas, sem acento e sem espaço. Trecho final não tem escolhas. Toda escolha leva a um trecho que existe."
        );
        self.generate(&prompt, Some(&context), "")
    }

    /// História nova do zero, sobre o tema pedido (vazio: o Verso escolhe).
    fn new_story(&mut self, theme: &str) -> Result<(), String> {
        let theme = theme.trim();
        let about = if theme.is_empty() { "um tema e um gênero que você escolher (surpreenda)".to_string() } else { format!("«{theme}»") };
        let shape = match self.size {
            0 => "um CONTO: trechos de 80 a 160 palavras, de 10 a 14 trechos, de 3 a 5 finais diferentes e marcantes".to_string(),
            1 => "uma NOVELA: trechos de 120 a 220 palavras, de 30 a 45 trechos em 3 ou 4 partes, de 5 a 8 finais diferentes e marcantes, personagens com nome e reviravoltas".to_string(),
            _ => format!("o CAPÍTULO 1 de {BOOK_CHAPTERS} de um LIVRO: trechos de 150 a 260 palavras, de 12 a 16 trechos com nomes começando por cap1_ (o primeiro se chama inicio), personagens com nome e motivação. O capítulo termina em 2 ou 3 pontos de virada, cada um com uma única escolha \"* Continuar no capítulo 2 -> {NEXT}\"; pode ter também 1 ou 2 finais antecipados (FIM)"),
        };
        let prompt = format!(
            "Escreva uma história interativa NOVA sobre {about}. \
Português do Brasil, prosa literária viva, com diálogos. Tamanho: {shape}. 2 ou 3 escolhas por trecho. \
A história SEMPRE anda para a frente: nenhuma escolha volta a um trecho anterior (nada de \"tente de novo\" ou \"recomeçar\"); um erro vira um final próprio. \
Use marcas para lembrar escolhas: a linha +nome marca algo, e uma escolha com [nome] só aparece com a marca ([!nome], sem ela). \
Responda SÓ com o texto da história neste formato (sem comentários, sem cercas de código):\n\
# Título\n@ Uma frase de resumo\n\n== inicio\nTexto do trecho.\n\n+marca\n* Texto da escolha -> nome_do_trecho\n* [marca] Escolha só com a marca -> outro_trecho\n\n== nome_do_trecho\n...\n\n== um_final\nTexto do final.\n\nFIM\n\
Nomes de trecho em minúsculas, sem acento e sem espaço. Trecho final termina com a linha FIM e não tem escolhas. Toda escolha leva a um trecho que existe."
        );
        let meta = if self.size >= 2 { format!("% livro {BOOK_CHAPTERS} 1\n") } else { String::new() };
        self.generate(&prompt, None, &meta)
    }

    /// Escreve o próximo capítulo do livro aberto, a partir do ponto de virada atual, e o
    /// acrescenta ao mesmo arquivo.
    fn next_chapter(&mut self) -> Result<(), String> {
        let r = self.read.as_ref().ok_or("Abra uma história antes.")?;
        let (path, story) = &self.list[r.k];
        let src = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let (total, cur) = src
            .lines()
            .find_map(|l| l.strip_prefix("% livro ").map(|m| {
                let v: Vec<u32> = m.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (v.first().copied().unwrap_or(BOOK_CHAPTERS), v.get(1).copied().unwrap_or(1))
            }))
            .unwrap_or((BOOK_CHAPTERS, 1));
        let n = cur + 1;
        let here = story.passages.get(&r.at).map(|p| p.paragraphs.join("\n\n")).unwrap_or_default();
        let flags: Vec<&str> = r.flags.iter().map(String::as_str).collect();
        // O resumo do que veio antes: a apresentação e o fim de cada trecho já escrito, curto.
        let earlier: String = story.blurb.clone();
        let last = n >= total;
        let ending = if last {
            "Este é o ÚLTIMO capítulo: todos os caminhos terminam em finais (de 4 a 7 finais diferentes, cada um com a linha FIM)".to_string()
        } else {
            format!("O capítulo termina em 2 ou 3 pontos de virada, cada um com uma única escolha \"* Continuar no capítulo {} -> {NEXT}\"; pode ter 1 final antecipado (FIM)", n + 1)
        };
        let context = format!("Livro \"{}\" ({earlier}). Marcas que o leitor tem: {:?}. Trecho onde o leitor parou (fim do capítulo {cur}):\n{here}", story.title, flags);
        let prompt = format!(
            "Escreva o CAPÍTULO {n} de {total} deste livro interativo, continuando exatamente do trecho do contexto, com os mesmos personagens e o mesmo tom. \
Português do Brasil, prosa literária viva com diálogos, trechos de 150 a 260 palavras, de 12 a 16 trechos. O primeiro trecho se chama cap{n}_inicio e todos os nomes começam por cap{n}_. \
2 ou 3 escolhas por trecho; use as marcas do leitor ([marca] e [!marca]) para mudar cenas; pode criar marcas novas (+nome). {ending}. \
A história sempre anda para a frente: nenhuma escolha volta a um trecho anterior. \
Responda SÓ com os trechos, sem título e sem cercas de código, no formato:\n== cap{n}_inicio\nTexto.\n\n* Escolha -> cap{n}_outro\n\n== cap{n}_outro\n...\n\
Trecho final termina com a linha FIM. Toda escolha leva a um trecho que existe neste capítulo."
        );
        let id = crate::chat::send(None, &prompt, Some(&context))?;
        let chat = crate::chat::open(&id)?;
        let reply = chat.messages.iter().rev().find(|m| !m.user).map(|m| m.text.clone()).ok_or("O Verso não respondeu.")?;
        let body: String = reply.lines().filter(|l| !l.trim_start().starts_with("```") && !l.starts_with("# ") && !l.starts_with("@ ")).collect::<Vec<_>>().join("\n");
        let start = format!("cap{n}_inicio");
        if !body.contains(&format!("== {start}")) {
            return Err("O capítulo não veio no formato certo. Toque de novo para tentar.".into());
        }
        // Os pontos de virada passam a apontar para o começo do novo capítulo.
        let updated = src.replace(&format!("% livro {total} {cur}"), &format!("% livro {total} {n}")).replace(&format!("-> {NEXT}"), &format!("-> {start}"));
        let path = path.clone();
        std::fs::write(&path, format!("{}\n\n{body}\n", updated.trim_end())).map_err(|e| e.to_string())?;
        let k = r.k;
        self.list = load_all();
        let k = self.list.iter().position(|(p, _)| *p == path).unwrap_or(k);
        let flags = self.read.as_ref().map(|r| r.flags.clone()).unwrap_or_default();
        self.read = Some(Reading { k, at: start.clone(), flags, page: 0 });
        self.go_to(&start);
        Ok(())
    }

    fn draw_sizes(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Display, 100.0, MARGIN - 4, 320, INK, "Gerar história");
        f.draw(fb, Face::Strong, 30.0, MARGIN, 392, MUTED, "De que tamanho? Depois você diz o tema.");
        let opts = [
            ("Conto", "10 a 14 trechos · uns 15 minutos"),
            ("Novela", "30 a 45 trechos · uma noite de leitura"),
            ("Livro", "8 capítulos, escritos enquanto você lê"),
        ];
        let mut hits = Vec::new();
        for (k, (t, d)) in opts.iter().enumerate() {
            let r = Rect::new(MARGIN, 460 + k as i32 * 190, W - 2 * MARGIN, 160);
            ui::card(fb, r, 30, PAPER, false);
            f.draw(fb, Face::Display, 56.0, r.x + 36, r.y + 76, INK, t);
            f.draw(fb, Face::Body, 30.0, r.x + 38, r.y + 128, MUTED, d);
            hits.push((740 + k as u32, r));
        }
        hits.push((749, ui::back_rect()));
        *self.hits.borrow_mut() = hits;
    }

    /// Manda o pedido ao Verso, confere o formato, tira loops, salva e abre a história.
    fn generate(&mut self, prompt: &str, context: Option<&str>, meta: &str) -> Result<(), String> {
        let id = crate::chat::send(None, prompt, context)?;
        let chat = crate::chat::open(&id)?;
        let reply = chat.messages.iter().rev().find(|m| !m.user).map(|m| m.text.clone()).ok_or("O Verso não respondeu.")?;
        let body: String = reply.lines().filter(|l| !l.trim_start().starts_with("```")).collect::<Vec<_>>().join("\n");
        let mut new = parse(&body);
        break_loops(&mut new);
        // Escolhas para trechos que não existem saem; o que ficar sem saída vira final.
        let names: BTreeSet<String> = new.passages.keys().cloned().collect();
        for p in new.passages.values_mut() {
            p.choices.retain(|c| names.contains(&c.to) || c.to == NEXT);
            if p.choices.is_empty() {
                p.end = true;
            }
        }
        if new.title.is_empty() || new.passages.len() < 2 || !new.passages.contains_key(&new.first) {
            return Err("A resposta não veio no formato de história. Tente de novo.".into());
        }
        let slug: String = crate::apps::fold(&new.title).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir().join(format!("{}-{stamp}.txt", slug.trim_matches('-')));
        std::fs::write(&path, format!("{meta}{body}")).map_err(|e| e.to_string())?;
        self.list = load_all();
        let k = self.list.iter().position(|(p, _)| *p == path).ok_or("História salva, mas não abriu.")?;
        self.open(k);
        Ok(())
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

    /// Toda história que vem com o Verso Ink anda sempre para a frente: nenhum caminho volta a um
    /// trecho já visto (com as mesmas marcas), todo trecho é alcançável, todo caminho termina num
    /// final, e há pelo menos 3 finais alcançáveis.
    #[test]
    fn historias_sem_loop_e_com_finais() {
        for (file, src) in EXAMPLES {
            let s = parse(src);
            assert!(!s.title.is_empty() && s.passages.contains_key(&s.first), "{file}: sem título ou início");
            for (name, p) in &s.passages {
                assert!(!p.paragraphs.is_empty(), "{file}/{name} sem texto");
                assert!(p.end || !p.choices.is_empty(), "{file}/{name} não tem saída");
                assert!(!(p.end && !p.choices.is_empty()), "{file}/{name}: final com escolhas");
                for c in &p.choices {
                    assert!(s.passages.contains_key(&c.to), "{file}/{name} → {} não existe", c.to);
                }
            }
            // Busca em profundidade pelos estados (trecho, marcas): achar um estado já na pilha é loop.
            type St = (String, BTreeSet<String>);
            fn enter(s: &Story, n: &str, mut f: BTreeSet<String>) -> St {
                let p = &s.passages[n];
                f.extend(p.set.iter().cloned());
                for u in &p.unset {
                    f.remove(u);
                }
                (n.to_string(), f)
            }
            fn walk(s: &Story, st: St, path: &mut Vec<St>, seen: &mut std::collections::HashSet<St>, ends: &mut BTreeSet<String>, nodes: &mut BTreeSet<String>, file: &str) {
                assert!(!path.contains(&st), "{file}: loop ao voltar para '{}'", st.0);
                nodes.insert(st.0.clone());
                if !seen.insert(st.clone()) {
                    return;
                }
                let p = &s.passages[&st.0];
                if p.end {
                    ends.insert(st.0.clone());
                    return;
                }
                let open: Vec<_> = p.choices.iter().filter(|c| c.need.as_ref().is_none_or(|(want, f)| st.1.contains(f) == *want)).collect();
                assert!(!open.is_empty(), "{file}/{}: nenhuma escolha disponível com {:?}", st.0, st.1);
                path.push(st.clone());
                for c in open {
                    let next = enter(s, &c.to, st.1.clone());
                    walk(s, next, path, seen, ends, nodes, file);
                }
                path.pop();
            }
            let (mut ends, mut nodes) = (BTreeSet::new(), BTreeSet::new());
            let start = enter(&s, &s.first, BTreeSet::new());
            walk(&s, start, &mut Vec::new(), &mut std::collections::HashSet::new(), &mut ends, &mut nodes, file);
            assert!(ends.len() >= 3, "{file}: só {} finais alcançáveis", ends.len());
            let missing: Vec<_> = s.passages.keys().filter(|k| !nodes.contains(*k)).collect();
            assert!(missing.is_empty(), "{file}: trechos inalcançáveis {missing:?}");
        }
    }

    #[test]
    fn historia_gerada_com_loop_vira_linha_reta() {
        let src = "# T\n@ r\n\n== inicio\nA.\n\n* ir -> meio\n\n== meio\nB.\n\n* voltar -> inicio\n* seguir -> fim\n\n== erro\nC.\n\n* tente de novo -> inicio\n\n== fim\nD.\n\nFIM\n";
        let mut s = parse(src);
        break_loops(&mut s);
        assert!(s.passages["meio"].choices.iter().all(|c| c.to != "inicio"));
        assert_eq!(s.passages["meio"].choices.len(), 1);
        // "erro" não é alcançável a partir do início: fica como está.
        assert_eq!(s.passages["erro"].choices.len(), 1);
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

pub fn gen_rect() -> Rect {
    // Ao lado do título (o canto de cima é do "Como jogar").
    Rect::new(W - MARGIN - 330, 248, 330, 92)
}
