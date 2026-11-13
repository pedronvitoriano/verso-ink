//! Aprender: a seção de estudo no jeito do Duolingo. No topo, a ofensiva geral (todos os
//! hábitos); embaixo, um cartão por tópico, cada um com a sua chama: Violão, Partitura e
//! Destaques (que já existem) e tópicos com lições escritas pelo Claude (Inglês para devs,
//! Xadrez, Rust e os que você criar). Cada lição: 2 ou 3 cartões curtos e 6 exercícios de toque;
//! o que você erra volta no fim da lição e reforça as próximas.
//!
//! Arquivos: data/aprender/topicos/<id>/ com tema.txt, curriculo.json, licoes/<u>-<l>.json,
//! feitas.txt e erros.txt. As lições são preparadas na sincronização, para estudar sem Wi-Fi.

use crate::chat::Json;
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

/// Tópicos que já vêm prontos para o Claude montar.
const SEEDS: [(&str, &str); 3] = [("ingles", "Inglês para desenvolvedores"), ("xadrez", "Xadrez: aberturas e táticas"), ("rust", "Rust do zero")];

fn root() -> PathBuf {
    let d = crate::home_dir().join("data/aprender/topicos");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Um tópico com lições do Claude.
#[derive(Clone)]
pub struct Topic {
    pub id: String,
    pub theme: String,
    pub title: String,
    pub units: Vec<(String, Vec<String>)>,
}

impl Topic {
    fn dir(&self) -> PathBuf {
        root().join(&self.id)
    }
    fn total(&self) -> usize {
        self.units.iter().map(|u| u.1.len()).sum()
    }
    fn done(&self) -> Vec<(usize, usize)> {
        std::fs::read_to_string(self.dir().join("feitas.txt"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (a, b) = l.split_once('-')?;
                Some((a.parse().ok()?, b.parse().ok()?))
            })
            .collect()
    }
    fn mark(&self, u: usize, l: usize) {
        let mut d = self.done();
        if !d.contains(&(u, l)) {
            d.push((u, l));
            let _ = std::fs::write(self.dir().join("feitas.txt"), d.iter().map(|(a, b)| format!("{a}-{b}")).collect::<Vec<_>>().join("\n"));
        }
    }
    /// A próxima lição a fazer (primeira não feita, em ordem).
    fn next(&self) -> Option<(usize, usize)> {
        let d = self.done();
        self.units.iter().enumerate().flat_map(|(u, (_, ls))| (0..ls.len()).map(move |l| (u, l))).find(|x| !d.contains(x))
    }
    fn lesson_path(&self, u: usize, l: usize) -> PathBuf {
        let d = self.dir().join("licoes");
        let _ = std::fs::create_dir_all(&d);
        d.join(format!("{u}-{l}.json"))
    }
    fn source(&self) -> String {
        format!("t:{}", self.id)
    }
    fn errors(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir().join("erros.txt")).unwrap_or_default().lines().rev().take(6).map(str::to_string).collect()
    }
    fn add_error(&self, q: &str) {
        let mut s = std::fs::read_to_string(self.dir().join("erros.txt")).unwrap_or_default();
        s.push_str(&q.replace('\n', " "));
        s.push('\n');
        let _ = std::fs::write(self.dir().join("erros.txt"), s);
    }
}

/// Lê os tópicos do disco (com ou sem currículo ainda).
pub fn topics() -> Vec<(String, String, Option<Topic>)> {
    // Primeira vez: cria as sementes.
    for (id, theme) in SEEDS {
        let d = root().join(id);
        if !d.exists() {
            let _ = std::fs::create_dir_all(&d);
            let _ = std::fs::write(d.join("tema.txt"), theme);
        }
    }
    let mut v: Vec<(String, String, Option<Topic>)> = std::fs::read_dir(root())
        .map(|it| {
            it.flatten()
                .filter_map(|e| {
                    let id = e.file_name().to_string_lossy().to_string();
                    let theme = std::fs::read_to_string(e.path().join("tema.txt")).ok()?.trim().to_string();
                    let t = load_topic(&id, &theme);
                    Some((id, theme, t))
                })
                .collect()
        })
        .unwrap_or_default();
    // Sementes primeiro, na ordem delas; depois os criados, por nome.
    v.sort_by_key(|(id, theme, _)| (SEEDS.iter().position(|s| s.0 == id).unwrap_or(99), theme.clone()));
    v
}

fn load_topic(id: &str, theme: &str) -> Option<Topic> {
    let s = std::fs::read_to_string(root().join(id).join("curriculo.json")).ok()?;
    let j = crate::chat::parse(&s)?;
    let units: Vec<(String, Vec<String>)> = j
        .get("unidades")?
        .arr()
        .iter()
        .filter_map(|u| Some((u.get("titulo")?.str()?.to_string(), u.get("licoes")?.arr().iter().filter_map(|l| l.str().map(str::to_string)).collect())))
        .collect();
    (!units.is_empty()).then(|| Topic { id: id.to_string(), theme: theme.to_string(), title: j.get("titulo").and_then(Json::str).unwrap_or(theme).to_string(), units })
}

fn slug(s: &str) -> String {
    let base: String = crate::apps::fold(s).chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let base: String = base.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-").chars().take(30).collect();
    if base.is_empty() { "topico".into() } else { base }
}

/// Pede à ponte o currículo de um tema e grava. Devolve erro em texto.
fn fetch_curriculum(id: &str, theme: &str) -> Result<(), String> {
    let raw = crate::chat::call_raw("POST", "/aprender/curriculo", Some(&format!("{{\"tema\": {}}}", crate::chat::json_str(theme))), 300)?;
    std::fs::write(root().join(id).join("curriculo.json"), raw).map_err(|e| e.to_string())
}

/// Pede à ponte uma lição e grava.
fn fetch_lesson(t: &Topic, u: usize, l: usize) -> Result<(), String> {
    let before: Vec<String> = t.units.iter().enumerate().flat_map(|(ui, (_, ls))| ls.iter().enumerate().filter(move |(li, _)| (ui, *li) < (u, l)).map(|(_, s)| crate::chat::json_str(s))).collect();
    let errs: Vec<String> = t.errors().iter().map(|e| crate::chat::json_str(e)).collect();
    let body = format!(
        "{{\"tema\": {}, \"unidade\": {}, \"licao\": {}, \"anteriores\": [{}], \"erros\": [{}]}}",
        crate::chat::json_str(&t.theme),
        crate::chat::json_str(&t.units[u].0),
        crate::chat::json_str(&t.units[u].1[l]),
        before.join(","),
        errs.join(",")
    );
    let raw = crate::chat::call_raw("POST", "/aprender/licao", Some(&body), 300)?;
    std::fs::write(t.lesson_path(u, l), raw).map_err(|e| e.to_string())
}

/// Sincronização em segundo plano: currículos que faltam e as duas próximas lições de cada tópico.
pub fn prepare() {
    if !crate::chat::configured() {
        return;
    }
    for (id, theme, t) in topics() {
        let t = match t {
            Some(t) => t,
            None => {
                if fetch_curriculum(&id, &theme).is_err() {
                    continue;
                }
                match load_topic(&id, &theme) {
                    Some(t) => t,
                    None => continue,
                }
            }
        };
        let done = t.done();
        let todo: Vec<(usize, usize)> = t.units.iter().enumerate().flat_map(|(u, (_, ls))| (0..ls.len()).map(move |l| (u, l))).filter(|x| !done.contains(x)).take(2).collect();
        for (u, l) in todo {
            if !t.lesson_path(u, l).exists() {
                let _ = fetch_lesson(&t, u, l);
            }
        }
    }
}

// ---------- Lição ----------

struct Exercise {
    q: String,
    opts: Vec<String>,
    ok: usize,
    why: String,
}

struct Lesson {
    cards: Vec<(String, String)>,
    exs: Vec<Exercise>,
}

fn load_lesson(p: &std::path::Path) -> Option<Lesson> {
    let j = crate::chat::parse(&std::fs::read_to_string(p).ok()?)?;
    let cards = j.get("cartoes")?.arr().iter().filter_map(|c| Some((c.get("titulo").and_then(Json::str).unwrap_or("").to_string(), c.get("texto")?.str()?.to_string()))).collect::<Vec<_>>();
    let exs = j
        .get("exercicios")?
        .arr()
        .iter()
        .filter_map(|e| {
            let opts: Vec<String> = e.get("opcoes")?.arr().iter().filter_map(|o| o.str().map(str::to_string)).collect();
            let ok = e.get("certa")?.num()? as usize;
            let q = e.get("pergunta")?.str()?.to_string();
            (ok < opts.len()).then(|| Exercise { q, opts, ok, why: e.get("explicacao").and_then(Json::str).unwrap_or("").to_string() })
        })
        .collect::<Vec<_>>();
    (!cards.is_empty() && !exs.is_empty()).then_some(Lesson { cards, exs })
}

/// Em que passo da lição: cartão k, ou exercício (índice na fila) com a resposta dada.
#[derive(Clone, Copy, PartialEq)]
enum Step {
    Card(usize),
    Ex(Option<usize>),
    Done,
}

struct Run {
    topic: Topic,
    unit: usize,
    lesson: usize,
    data: Lesson,
    step: Step,
    /// Fila de exercícios: os errados voltam para o fim.
    queue: Vec<usize>,
    first_try_ok: usize,
    tried: Vec<bool>,
    xp: u32,
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Hub,
    Topic,
    Loading,
    Lesson,
}

pub struct Estudo {
    view: View,
    /// Tópico embutido aberto (Violão, Partitura, Destaques): o módulo dele cuida da tela.
    child: Option<Box<dyn MiniGame>>,
    topics: Vec<(String, String, Option<Topic>)>,
    page: usize,
    cur: usize,
    unit: usize,
    run: Option<Run>,
    /// Trabalho em segundo plano (currículo ou lição) e o que fazer quando terminar.
    job: Option<(Receiver<Result<(), String>>, usize, Option<(usize, usize)>)>,
    error: Option<String>,
    want_text: bool,
    cel: Option<crate::aprender::Cel>,
    /// Comemoração guardada para depois do resumo da lição (primeiro o resultado, depois a chama).
    cel_after: Option<crate::aprender::Cel>,
    last_poll: std::time::Instant,
}

/// Os três tópicos embutidos: (nome, fontes de XP, ícone).
const BUILTIN: [(&str, &[&str], &str); 6] = [("Violão", &["violao"], "violao"), ("Partitura", &["licao", "treino"], "partitura"), ("Destaques", &["revisao"], "destaques"), ("Geografia", &["geografia"], "geografia"), ("Matemática mental", &["matematica"], "matematica"), ("Flashcards", &["flashcards"], "flashcards")];
const PER_PAGE: usize = 8;

impl Estudo {
    pub fn new() -> Self {
        Estudo { view: View::Hub, child: None, topics: topics(), page: 0, cur: 0, unit: 0, run: None, job: None, error: None, want_text: false, cel: None, cel_after: None, last_poll: std::time::Instant::now() }
    }

    /// Quantos cartões no hub: embutidos + tópicos + "Novo tópico".
    fn n_cards(&self) -> usize {
        BUILTIN.len() + self.topics.len() + 1
    }

    fn card_rect(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 20) / 2;
        let k = k % PER_PAGE;
        Rect::new(MARGIN + (k % 2) as i32 * (w + 20), 560 + (k / 2) as i32 * 180, w, 162)
    }

    fn nav(k: usize) -> Rect {
        Rect::new(if k == 0 { MARGIN } else { W - MARGIN - 250 }, 1300, 250, 86)
    }

    fn wide() -> Rect {
        Rect::new(MARGIN, 1290, W - 2 * MARGIN, 104)
    }

    fn start_job(&mut self, topic: usize, lesson: Option<(usize, usize)>) {
        let (tx, rx) = channel();
        let (id, theme, t) = self.topics[topic].clone();
        std::thread::spawn(move || {
            let r = match (lesson, t) {
                (Some((u, l)), Some(t)) => fetch_lesson(&t, u, l),
                _ => fetch_curriculum(&id, &theme),
            };
            let _ = tx.send(r);
        });
        self.job = Some((rx, topic, lesson));
        self.error = None;
    }

    /// Abre a lição (baixando antes, se ainda não estiver no Kindle).
    fn open_lesson(&mut self, u: usize, l: usize) {
        let Some(t) = self.topics[self.cur].2.clone() else { return };
        match load_lesson(&t.lesson_path(u, l)) {
            Some(data) => {
                let n = data.exs.len();
                self.run = Some(Run { topic: t, unit: u, lesson: l, data, step: Step::Card(0), queue: (0..n).collect(), first_try_ok: 0, tried: vec![false; n], xp: 0 });
                self.view = View::Lesson;
            }
            None => {
                let _ = std::fs::remove_file(t.lesson_path(u, l));
                self.view = View::Loading;
                self.start_job(self.cur, Some((u, l)));
            }
        }
    }

    fn streak_of(&self, k: usize) -> (u32, bool) {
        if k < BUILTIN.len() {
            crate::aprender::topic_streak(BUILTIN[k].1)
        } else if let Some((id, _, _)) = self.topics.get(k - BUILTIN.len()) {
            crate::aprender::topic_streak(&[&format!("t:{id}")])
        } else {
            (0, false)
        }
    }

    fn draw_hub(&self, fb: &mut Framebuffer, f: &Fonts) {
        if !crate::sprites::draw(fb, "icone-aprender", Rect::new(W - MARGIN - 200, 110, 200, 200)) {
            crate::sprites::draw(fb, "cab-violao", Rect::new(W - MARGIN - 300, 96, 310, 240));
        }
        f.draw(fb, Face::Display, 120.0, MARGIN - 6, 300, INK, "Aprender");
        let st = crate::aprender::status();
        crate::aprender::draw_panel(fb, f, Rect::new(MARGIN, 340, W - 2 * MARGIN, 180), &st);
        let start = self.page * PER_PAGE;
        for k in start..(start + PER_PAGE).min(self.n_cards()) {
            let r = Self::card_rect(k);
            ui::card(fb, r, 28, PAPER, false);
            let (title, sub, icon): (String, String, Option<String>) = if k < BUILTIN.len() {
                let sub = match k {
                    0 => if crate::violao::practiced_today() { "Treino feito hoje".to_string() } else { "Trocas, cifras, acordes".to_string() },
                    1 => format!("{} de {} lições", crate::partitura::done().len().min(crate::partitura::total()), crate::partitura::total()),
                    2 => format!("{} para hoje", crate::revisao::due_count()),
                    3 => "Capitais e países".to_string(),
                    4 => "Contas rápidas por nível".to_string(),
                    _ => "Revise seus baralhos".to_string(),
                };
                (BUILTIN[k].0.to_string(), sub, Some(BUILTIN[k].2.to_string()))
            } else if let Some((id, theme, t)) = self.topics.get(k - BUILTIN.len()) {
                // Ícone do tópico (os conhecidos têm desenho próprio; os criados, o do Aprender).
                let icon = [("ingles", "ingles"), ("xadrez", "xadrez"), ("rust", "rust")].iter().find(|(i, _)| i == id).map(|(_, n)| n.to_string()).unwrap_or("aprender".into());
                match t {
                    Some(t) => (t.title.clone(), format!("{} de {} lições", t.done().len(), t.total()), Some(icon)),
                    None => (theme.clone(), "O Verso está montando…".to_string(), Some(icon)),
                }
            } else {
                ("+ Novo tópico".to_string(), "Qualquer assunto".to_string(), None)
            };
            let is_new = k == self.n_cards() - 1;
            // Ícone: o desenho do tópico, ou a inicial num disco.
            let d = 64;
            let ir = Rect::new(r.x + 22, r.y + 22, d, d);
            let drawn = match &icon {
                Some(name) => ui::icon_disc(fb, ir.x, ir.y, d, name),
                None => false,
            };
            if !drawn {
                fb.round_rect(ir, d / 2, if is_new { 0xC8 } else { INK });
                let c: String = if is_new { "+".into() } else { title.chars().next().unwrap_or('?').to_uppercase().collect() };
                let w = f.measure(Face::Display, 36.0, &c);
                f.draw(fb, Face::Display, 36.0, ir.x + (d - w) / 2, ir.y + 46, if is_new { INK } else { PAPER }, &c);
            }
            // Chama do tópico, no canto.
            if !is_new {
                let (n, done) = self.streak_of(k);
                if n > 0 || done {
                    crate::aprender::flame(fb, Rect::new(r.x + r.w - 92, r.y + 20, 40, 40), done);
                    f.draw(fb, Face::Display, 34.0, r.x + r.w - 48, r.y + 54, if done { INK } else { MUTED }, &n.to_string());
                }
            }
            let title = if is_new { title.trim_start_matches("+ ").to_string() } else { title };
            // Título inteiro: diminui a letra (até 26) antes de cortar.
            let mut ts = 34.0;
            while f.measure(Face::Display, ts, &title) > r.w - 44 && ts > 26.0 {
                ts -= 2.0;
            }
            f.draw(fb, Face::Display, ts, r.x + 22, r.y + 122, INK, &crate::apps::fit(f, Face::Display, ts, &title, r.w - 44));
            f.draw(fb, Face::Body, 22.0, r.x + 24, r.y + 150, MUTED, &crate::apps::fit(f, Face::Body, 22.0, &sub, r.w - 44));
        }
        let pages = self.n_cards().div_ceil(PER_PAGE);
        if self.page > 0 {
            ui::button(fb, f, Self::nav(0), "‹ Anterior", false);
        }
        if self.page + 1 < pages {
            ui::button(fb, f, Self::nav(1), "Próxima ›", false);
        }
    }

    fn node(k: usize) -> (i32, i32) {
        const ZIG: [i32; 6] = [0, 70, 130, 70, 0, -40];
        (MARGIN + 110 + ZIG[k % 6], 470 + k as i32 * 150)
    }

    fn draw_topic(&self, fb: &mut Framebuffer, f: &Fonts) {
        let Some(t) = &self.topics[self.cur].2 else { return };
        let (u, (ut, ls)) = (self.unit, &t.units[self.unit]);
        f.draw(fb, Face::Strong, 26.0, MARGIN, 196, MUTED, &format!("{} · UNIDADE {} DE {}", t.title.to_uppercase(), u + 1, t.units.len()));
        f.draw(fb, Face::Display, 58.0, MARGIN - 2, 262, INK, &crate::apps::fit(f, Face::Display, 58.0, ut, W - 2 * MARGIN));
        let (n, done_today) = crate::aprender::topic_streak(&[&t.source()]);
        crate::aprender::flame(fb, Rect::new(MARGIN, 300, 52, 52), done_today);
        f.draw(fb, Face::Strong, 28.0, MARGIN + 62, 338, if done_today { INK } else { MUTED }, &format!("{n} dia{} neste tópico{}", if n == 1 { "" } else { "s" }, if done_today { "" } else { " · falta hoje" }));
        let done = t.done();
        let next = t.next();
        let centers: Vec<(i32, i32)> = (0..ls.len()).map(Self::node).collect();
        for w in centers.windows(2) {
            ui::stroke(fb, (w[0].0 as f32, w[0].1 as f32), (w[1].0 as f32, w[1].1 as f32), 7.0, 0xB8);
        }
        for (k, title) in ls.iter().enumerate() {
            let (cx, cy) = centers[k];
            let seen = done.contains(&(u, k));
            let is_next = next == Some((u, k));
            let d = if is_next { 110 } else { 92 };
            let nr = Rect::new(cx - d / 2, cy - d / 2, d, d);
            if is_next {
                fb.round_rect(Rect::new(nr.x - 10, nr.y - 10, nr.w + 20, nr.h + 20), d / 2 + 10, 0xC8);
            }
            if seen || is_next {
                fb.round_rect(nr, d / 2, INK);
            } else {
                fb.round_outline(nr, d / 2, 5, 0x88, PAPER);
            }
            if seen {
                ui::stroke(fb, ((cx - 20) as f32, (cy + 2) as f32), ((cx - 5) as f32, (cy + 17) as f32), 8.0, PAPER);
                ui::stroke(fb, ((cx - 5) as f32, (cy + 17) as f32), ((cx + 22) as f32, (cy - 14) as f32), 8.0, PAPER);
            } else {
                let s = (k + 1).to_string();
                let w = f.measure(Face::Display, 40.0, &s);
                f.draw(fb, Face::Display, 40.0, cx - w / 2, cy + 14, if is_next { PAPER } else { MUTED }, &s);
            }
            let tx = MARGIN + 110 + 130 + 90;
            f.draw(fb, if is_next { Face::Display } else { Face::Strong }, if is_next { 34.0 } else { 30.0 }, tx, cy + 4, if seen || is_next { INK } else { MUTED }, &crate::apps::fit(f, Face::Strong, 30.0, title, W - MARGIN - tx));
            let cached = t.lesson_path(u, k).exists();
            let tag = if is_next { "começar" } else if seen { "refazer" } else if cached { "pronta" } else { "" };
            f.draw(fb, Face::Body, 22.0, tx, cy + 38, MUTED, tag);
        }
        if u > 0 {
            ui::button(fb, f, Self::nav(0), "‹ Unidade", false);
        }
        if u + 1 < t.units.len() {
            ui::button(fb, f, Self::nav(1), "Unidade ›", false);
        }
    }

    fn option_rect(k: usize) -> Rect {
        Rect::new(MARGIN, 580 + k as i32 * 108, W - 2 * MARGIN, 96)
    }

    fn progress_bar(fb: &mut Framebuffer, r: &Run) {
        // Cartões lidos + exercícios acertados (os errados só contam quando acertar na volta).
        let total = r.data.cards.len() + r.data.exs.len();
        let pos = match r.step {
            Step::Card(k) => k,
            Step::Ex(_) => r.data.cards.len() + r.data.exs.len() - r.queue.len(),
            Step::Done => total,
        };
        let bar = Rect::new(MARGIN, 200, W - 2 * MARGIN, 16);
        fb.round_rect(bar, 8, 0xC8);
        fb.round_rect(Rect::new(bar.x, bar.y, (bar.w * pos as i32 / total.max(1) as i32).max(16), bar.h), 8, INK);
    }

    fn draw_lesson(&self, fb: &mut Framebuffer, f: &Fonts) {
        let Some(r) = &self.run else { return };
        Self::progress_bar(fb, r);
        let lesson_title = &r.topic.units[r.unit].1[r.lesson];
        match r.step {
            Step::Card(k) => {
                let (t, text) = &r.data.cards[k];
                f.draw(fb, Face::Strong, 26.0, MARGIN, 280, MUTED, &crate::apps::fit(f, Face::Strong, 26.0, &lesson_title.to_uppercase(), W - 2 * MARGIN));
                let mut y = 360;
                for l in crate::chat::wrap_words(f, Face::Display, 54.0, t, W - 2 * MARGIN).iter().take(3) {
                    f.draw(fb, Face::Display, 54.0, MARGIN, y, INK, l);
                    y += 66;
                }
                y += 30;
                for l in crate::chat::wrap_words(f, Face::Body, 36.0, text, W - 2 * MARGIN).iter().take(16) {
                    f.draw(fb, Face::Body, 36.0, MARGIN, y, INK, l);
                    y += 50;
                }
                ui::button(fb, f, Self::wide(), "Continuar", true);
            }
            Step::Ex(answer) => {
                let e = &r.data.exs[r.queue[0]];
                let retry = r.tried[r.queue[0]] && answer.is_none();
                f.draw(fb, Face::Strong, 26.0, MARGIN, 280, MUTED, if retry { "DE NOVO: ESSA VOCÊ ERROU" } else { "ESCOLHA A RESPOSTA" });
                let mut size = 44.0;
                let mut ql = crate::chat::wrap_words(f, Face::Display, size, &e.q, W - 2 * MARGIN);
                while ql.len() > 4 && size > 32.0 {
                    size -= 2.0;
                    ql = crate::chat::wrap_words(f, Face::Display, size, &e.q, W - 2 * MARGIN);
                }
                for (k, l) in ql.iter().take(4).enumerate() {
                    f.draw(fb, Face::Display, size, MARGIN, 350 + k as i32 * (size * 1.25) as i32, INK, l);
                }
                for (k, o) in e.opts.iter().enumerate() {
                    let rr = Self::option_rect(k);
                    match answer {
                        Some(_) if k == e.ok => {
                            ui::button(fb, f, rr, o, true);
                            crate::aprender::badge(fb, rr, true, true);
                        }
                        Some(a) if a == k => {
                            ui::card(fb, rr, rr.h / 2, ui::SURFACE, false);
                            let t = crate::apps::fit(f, Face::Strong, 32.0, o, rr.w - 140);
                            let w = f.measure(Face::Strong, 32.0, &t);
                            f.draw(fb, Face::Strong, 32.0, rr.x + (rr.w - w) / 2, rr.y + rr.h / 2 + 12, MUTED, &t);
                            crate::aprender::badge(fb, rr, false, false);
                        }
                        _ => ui::button(fb, f, rr, o, false),
                    }
                }
                if let Some(a) = answer {
                    let ok = a == e.ok;
                    let title = if ok { ["Correto!", "Isso!", "Mandou bem!", "Exato!"][r.queue[0] % 4].to_string() } else { format!("Resposta certa: {}", e.opts[e.ok]) };
                    crate::aprender::draw_feedback(fb, f, ok, &title, &e.why);
                }
            }
            Step::Done => {
                f.draw(fb, Face::Display, 100.0, MARGIN - 4, 380, INK, "Lição feita!");
                let total = r.data.exs.len();
                let lines = [format!("+{} XP", r.xp), format!("{} de {} certas de primeira", r.first_try_ok, total), lesson_title.clone()];
                f.draw(fb, Face::Display, 80.0, MARGIN, 520, INK, &lines[0]);
                f.draw(fb, Face::Strong, 34.0, MARGIN, 600, MUTED, &lines[1]);
                f.draw(fb, Face::Body, 30.0, MARGIN, 660, MUTED, &crate::apps::fit(f, Face::Body, 30.0, &lines[2], W - 2 * MARGIN));
                let st = crate::aprender::status();
                crate::aprender::draw_panel(fb, f, Rect::new(MARGIN, 740, W - 2 * MARGIN, 180), &st);
                crate::aprender::week_row(fb, f, MARGIN, 990, W - 2 * MARGIN, &st);
                ui::button(fb, f, Self::wide(), "Continuar", true);
            }
        }
    }

    fn draw_loading(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Display, 70.0, MARGIN, 420, INK, if self.error.is_some() { "Não deu agora" } else { "Preparando…" });
        let msg = match &self.error {
            Some(e) => format!("{e} As lições ficam prontas sozinhas quando houver Wi-Fi."),
            None => "O Verso está escrevendo esta lição para você. Leva uns 30 segundos; da próxima vez ela já estará no Kindle.".to_string(),
        };
        for (k, l) in crate::chat::wrap_words(f, Face::Body, 34.0, &msg, W - 2 * MARGIN).iter().take(5).enumerate() {
            f.draw(fb, Face::Body, 34.0, MARGIN, 500 + k as i32 * 48, MUTED, l);
        }
        if self.error.is_some() {
            ui::button(fb, f, Self::wide(), "Tentar de novo", true);
        }
    }

    /// Resposta a um exercício.
    fn answer(&mut self, k: usize) {
        let Some(r) = &mut self.run else { return };
        let i = r.queue[0];
        let first = !r.tried[i];
        r.tried[i] = true;
        let ok = k == r.data.exs[i].ok;
        if ok && first {
            r.first_try_ok += 1;
        }
        if !ok {
            r.topic.add_error(&r.data.exs[i].q);
        }
        r.step = Step::Ex(Some(k));
    }

    fn advance(&mut self) {
        let Some(r) = &mut self.run else { return };
        match r.step {
            Step::Card(k) if k + 1 < r.data.cards.len() => r.step = Step::Card(k + 1),
            Step::Card(_) => r.step = Step::Ex(None),
            Step::Ex(Some(a)) => {
                let i = r.queue.remove(0);
                if a != r.data.exs[i].ok {
                    r.queue.push(i); // errou: volta no fim
                }
                if r.queue.is_empty() {
                    r.step = Step::Done;
                    r.xp = 10 + if r.first_try_ok == r.data.exs.len() { 5 } else { 0 };
                    r.topic.mark(r.unit, r.lesson);
                    self.cel_after = crate::aprender::add(r.xp, &r.topic.source());
                } else {
                    r.step = Step::Ex(None);
                }
            }
            Step::Ex(None) => {}
            Step::Done => {
                self.run = None;
                self.topics = topics();
                self.view = View::Topic;
                // Depois do resumo da lição, a comemoração (se a chama cresceu ou a meta bateu).
                self.cel = self.cel_after.take();
            }
        }
    }

    /// Para os testes de tela: tópico com currículo e uma lição já gravados.
    #[cfg(test)]
    pub fn open_first_topic(&mut self) {
        self.topics = topics();
        self.cur = 0;
        self.view = View::Topic;
    }

    #[cfg(test)]
    pub fn start_lesson(&mut self, u: usize, l: usize) {
        self.open_lesson(u, l);
    }
}

impl MiniGame for Estudo {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        if let Some(c) = &self.child {
            return c.draw(fb, f);
        }
        if let Some(c) = self.cel {
            return crate::aprender::draw_celebration(fb, f, c);
        }
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Hub => self.draw_hub(fb, f),
            View::Topic => self.draw_topic(fb, f),
            View::Loading => self.draw_loading(fb, f),
            View::Lesson => self.draw_lesson(fb, f),
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if let Some(c) = &self.child {
            return c.targets();
        }
        if self.cel.is_some() {
            return vec![(999, crate::aprender::cel_button())];
        }
        match self.view {
            View::Hub => {
                let start = self.page * PER_PAGE;
                let mut v: Vec<(u32, Rect)> = (start..(start + PER_PAGE).min(self.n_cards())).map(|k| (600 + k as u32, Self::card_rect(k))).collect();
                if self.page > 0 {
                    v.push((660, Self::nav(0)));
                }
                if self.page + 1 < self.n_cards().div_ceil(PER_PAGE) {
                    v.push((661, Self::nav(1)));
                }
                v
            }
            View::Topic => {
                let Some(t) = &self.topics[self.cur].2 else { return vec![] };
                let mut v: Vec<(u32, Rect)> = (0..t.units[self.unit].1.len())
                    .map(|k| {
                        let (cx, cy) = Self::node(k);
                        (700 + k as u32, Rect::new(cx - 60, cy - 60, W - MARGIN - cx + 60, 120))
                    })
                    .collect();
                if self.unit > 0 {
                    v.push((720, Self::nav(0)));
                }
                if self.unit + 1 < t.units.len() {
                    v.push((721, Self::nav(1)));
                }
                v
            }
            View::Loading => {
                if self.error.is_some() {
                    vec![(750, Self::wide())]
                } else {
                    vec![]
                }
            }
            View::Lesson => match self.run.as_ref().map(|r| r.step) {
                Some(Step::Ex(None)) => {
                    let r = self.run.as_ref().unwrap();
                    (0..r.data.exs[r.queue[0]].opts.len()).map(|k| (740 + k as u32, Self::option_rect(k))).collect()
                }
                Some(Step::Ex(Some(_))) => vec![(730, crate::aprender::feedback_button())],
                Some(_) => vec![(730, Self::wide())],
                None => vec![],
            },
        }
    }

    fn tap(&mut self, id: u32, x: i32, y: i32) -> Redraw {
        if let Some(c) = &mut self.child {
            return c.tap(id, x, y);
        }
        if self.cel.is_some() {
            if id == 999 {
                self.cel = None;
                return Redraw::Full;
            }
            return Redraw::None;
        }
        match (self.view, id) {
            (View::Hub, 660) => self.page = self.page.saturating_sub(1),
            (View::Hub, 661) => self.page += 1,
            (View::Hub, k @ 600..=659) => {
                let k = (k - 600) as usize;
                if k < BUILTIN.len() {
                    self.child = Some(match k {
                        0 => Box::new(crate::violao::Violao::new()),
                        1 => Box::new(crate::violao::Violao::course()),
                        2 => Box::new(crate::revisao::Revisao::new()),
                        3 => Box::new(crate::geografia::Geografia::new()),
                        4 => Box::new(crate::matematica::Matematica::new()),
                        _ => Box::new(crate::flashcards::Flashcards::new()),
                    });
                } else if k == self.n_cards() - 1 {
                    self.want_text = true;
                    return Redraw::None;
                } else {
                    self.cur = k - BUILTIN.len();
                    match &self.topics[self.cur].2 {
                        Some(t) => {
                            self.unit = t.next().map(|(u, _)| u).unwrap_or(0);
                            self.view = View::Topic;
                        }
                        None => {
                            self.view = View::Loading;
                            self.start_job(self.cur, None);
                        }
                    }
                }
            }
            (View::Topic, 720) => self.unit = self.unit.saturating_sub(1),
            (View::Topic, 721) => self.unit += 1,
            (View::Topic, k @ 700..=719) => self.open_lesson(self.unit, (k - 700) as usize),
            (View::Loading, 750) => {
                let lesson = self.job.as_ref().and_then(|j| j.2).or_else(|| self.topics[self.cur].2.as_ref().and_then(|t| t.next()));
                let lesson = if self.topics[self.cur].2.is_some() { lesson } else { None };
                self.start_job(self.cur, lesson);
            }
            (View::Lesson, k @ 740..=743) => self.answer((k - 740) as usize),
            (View::Lesson, 730) => self.advance(),
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn wants_text(&mut self) -> Option<String> {
        if let Some(c) = &mut self.child {
            return c.wants_text();
        }
        std::mem::take(&mut self.want_text).then(|| "Que assunto você quer aprender?".to_string())
    }

    fn text_input(&mut self, text: &str) -> Redraw {
        if let Some(c) = &mut self.child {
            return c.text_input(text);
        }
        let theme = text.trim();
        if theme.chars().count() < 2 {
            return Redraw::None;
        }
        let mut id = slug(theme);
        while root().join(&id).exists() {
            id.push('2');
        }
        let _ = std::fs::create_dir_all(root().join(&id));
        let _ = std::fs::write(root().join(&id).join("tema.txt"), theme);
        self.topics = topics();
        self.cur = self.topics.iter().position(|t| t.0 == id).unwrap_or(0);
        self.view = View::Loading;
        self.start_job(self.cur, None);
        Redraw::Full
    }

    fn tick(&mut self) -> Redraw {
        if let Some(c) = &mut self.child {
            return c.tick();
        }
        if self.last_poll.elapsed().as_millis() < 700 {
            return Redraw::None;
        }
        self.last_poll = std::time::Instant::now();
        let Some((rx, topic, lesson)) = &self.job else { return Redraw::None };
        let Ok(res) = rx.try_recv() else { return Redraw::None };
        let (topic, lesson) = (*topic, *lesson);
        self.job = None;
        self.topics = topics();
        match res {
            Err(e) => self.error = Some(format!("{e}.")),
            Ok(()) => match lesson {
                Some((u, l)) => {
                    self.cur = topic;
                    self.open_lesson(u, l);
                }
                None => {
                    self.cur = topic;
                    self.unit = 0;
                    self.view = if self.topics[topic].2.is_some() { View::Topic } else { View::Hub };
                }
            },
        }
        Redraw::Full
    }

    fn keep_awake(&self) -> bool {
        self.child.as_ref().is_some_and(|c| c.keep_awake()) || self.job.is_some()
    }

    fn hold(&mut self, id: u32) -> Option<Redraw> {
        self.child.as_mut().and_then(|c| c.hold(id))
    }

    fn at_menu(&self) -> bool {
        self.child.is_none() && self.view == View::Hub
    }

    fn back(&mut self) -> bool {
        if let Some(c) = &mut self.child {
            if !c.back() {
                self.child = None;
                self.topics = topics();
            }
            return true;
        }
        if self.cel.take().is_some() {
            return true;
        }
        match self.view {
            View::Hub => false,
            View::Topic => {
                self.view = View::Hub;
                true
            }
            View::Lesson | View::Loading => {
                self.run = None;
                self.view = if self.topics.get(self.cur).is_some_and(|t| t.2.is_some()) { View::Topic } else { View::Hub };
                true
            }
        }
    }

    fn context(&self) -> String {
        if let Some(c) = &self.child {
            return c.context();
        }
        match (&self.run, self.view) {
            (Some(r), View::Lesson) => {
                let mut s = format!("Estudando \"{}\" no Kindle, lição \"{}\".", r.topic.title, r.topic.units[r.unit].1[r.lesson]);
                if let Step::Card(k) = r.step {
                    s.push_str(&format!(" Explicação na tela: {}", r.data.cards[k].1));
                }
                if let Step::Ex(_) = r.step {
                    let e = &r.data.exs[r.queue[0]];
                    s.push_str(&format!(" Exercício: {} Opções: {}.", e.q, e.opts.join(" / ")));
                }
                s
            }
            _ => "Na seção Aprender do Kindle (estudo no estilo Duolingo).".into(),
        }
    }

    fn summary(&self) -> String {
        self.child.as_ref().map(|c| c.summary()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_e_licao() {
        assert_eq!(slug("Inglês para devs!"), "ingles-para-devs");
        let dir = std::env::temp_dir().join(format!("estudo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("l.json");
        std::fs::write(&p, r#"{"cartoes": [{"titulo": "A", "texto": "B"}], "exercicios": [{"pergunta": "Q", "opcoes": ["x", "y"], "certa": 1, "explicacao": "porque"}, {"pergunta": "R", "opcoes": ["x"], "certa": 3}]}"#).unwrap();
        let l = load_lesson(&p).unwrap();
        assert_eq!(l.exs.len(), 1);
        assert_eq!(l.exs[0].ok, 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
