//! Diário: uma pergunta por dia e respostas guardadas em texto simples.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const PER_PAGE: usize = 5;
const ANSWER_LINES: usize = 10;

fn question(day: i64) -> &'static str {
    let qs: Vec<_> = include_str!("../assets/diario-perguntas.txt").lines().filter(|s| !s.trim().is_empty()).collect();
    qs[day.rem_euclid(qs.len() as i64) as usize]
}

fn date(day: i64) -> String {
    let z = day + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{d:02}/{m:02}/{y}")
}

fn dir() -> PathBuf { crate::home_dir().join("data/diario") }
fn path(root: &Path, day: i64) -> PathBuf { root.join(format!("{day}.txt")) }
fn read(root: &Path, day: i64) -> std::io::Result<Option<String>> {
    let file = path(root, day);
    match std::fs::read_to_string(&file) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match std::fs::symlink_metadata(&file) {
            Err(m) if m.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(m) => Err(m),
            Ok(_) => Err(e),
        },
        Err(e) => Err(e),
    }
}
fn save(root: &Path, day: i64, text: &str) -> std::io::Result<()> {
    // Não substitua um arquivo existente que não pôde ser lido como texto.
    read(root, day)?;
    std::fs::create_dir_all(root)?;
    let tmp = root.join(format!(".{day}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
        file.write_all(text.trim().as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&tmp, path(root, day))
    })();
    if result.is_err() { let _ = std::fs::remove_file(tmp); }
    result
}
fn saved_days(root: &Path) -> Vec<i64> {
    let mut days: Vec<i64> = std::fs::read_dir(root).into_iter().flatten().flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| e.path().file_stem()?.to_str()?.parse().ok()).collect();
    days.sort_unstable_by(|a, b| b.cmp(a));
    days
}
fn wrap(f: &Fonts, face: Face, size: f32, text: &str, width: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if f.measure(face, size, &next) <= width { line = next; continue; }
            if !line.is_empty() { lines.push(std::mem::take(&mut line)); }
            // Uma palavra sem espaços também deve continuar na página seguinte.
            for ch in word.chars() {
                let mut next = line.clone();
                next.push(ch);
                if !line.is_empty() && f.measure(face, size, &next) > width {
                    lines.push(std::mem::take(&mut line));
                }
                line.push(ch);
            }
        }
        lines.push(line);
    }
    lines
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View { Today, History, Entry(i64) }

pub struct Diario { view: View, page: usize, answer_page: usize, days: Vec<i64>, want: bool, edit_day: Option<i64>, draft: Option<(i64, String)>, draft_read_error: bool, read_error: bool, loaded_day: i64, answer: String, lines: Vec<String> }
impl Diario {
    pub fn new() -> Self {
        let day = Self::today();
        let mut diary = Diario { view: View::Today, page: 0, answer_page: 0, days: saved_days(&dir()), want: false, edit_day: None, draft: None, draft_read_error: false, read_error: false, loaded_day: day, answer: String::new(), lines: Vec::new() };
        diary.load(day);
        diary
    }
    fn load(&mut self, day: i64) { self.load_at(&dir(), day); }
    fn load_at(&mut self, root: &Path, day: i64) {
        self.loaded_day = day;
        match read(root, day) {
            Ok(answer) => {
                self.answer = answer.unwrap_or_default(); self.read_error = false;
                if self.draft.as_ref().is_some_and(|(draft_day, _)| *draft_day == day) { self.draft_read_error = false; }
            }
            Err(_) => { self.answer.clear(); self.read_error = true; }
        }
        self.lines = wrap(&Fonts::new(), Face::Body, 32.0, &self.answer, W - 2 * MARGIN);
        self.answer_page = 0;
    }
    fn pages(&self) -> usize { self.lines.len().max(1).div_ceil(ANSWER_LINES) }
    fn pages_button(i: usize) -> Rect { Rect::new(MARGIN + i as i32 * 696, 1136, 232, 76) }
    fn save_draft(&mut self, day: i64, text: String) { self.save_draft_at(&dir(), day, text); }
    fn save_draft_at(&mut self, root: &Path, day: i64, text: String) {
        if read(root, day).is_err() {
            self.draft = Some((day, text));
            self.draft_read_error = true;
            if self.loaded_day == day { self.read_error = true; }
            return;
        }
        self.draft_read_error = false;
        if save(root, day, &text).is_ok() {
            self.draft = None;
            self.days = saved_days(root);
            self.load_at(root, Self::today());
        } else { self.draft = Some((day, text)); }
    }
    fn today() -> i64 { crate::data::today(crate::offset_min()) }
    fn row(i: usize) -> Rect { Rect::new(MARGIN, 390 + i as i32 * 165, W - 2 * MARGIN, 145) }
    fn bottom(i: usize) -> Rect { Rect::new(MARGIN + i as i32 * 472, 1288, 456, 80) }
}

impl MiniGame for Diario {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        let (title, sub) = match self.view {
            View::Today => ("Diário", "Uma pergunta para guardar o dia".to_string()),
            View::History => ("Respostas", "Suas anotações por data".to_string()),
            View::Entry(day) => ("Diário", date(day)),
        };
        f.draw(fb, Face::Display, 100.0, MARGIN, 300, INK, title);
        f.draw(fb, Face::Strong, 30.0, MARGIN, 350, MUTED, &sub);
        match self.view {
            View::History => {
                if self.days.is_empty() {
                    f.draw(fb, Face::Body, 34.0, MARGIN, 460, MUTED, "Suas respostas aparecerão aqui.");
                }
                for (i, &day) in self.days.iter().skip(self.page * PER_PAGE).take(PER_PAGE).enumerate() {
                    let r = Self::row(i);
                    ui::card(fb, r, 28, PAPER, false);
                    f.draw(fb, Face::Display, 36.0, r.x + 28, r.y + 56, INK, &date(day));
                    f.draw(fb, Face::Body, 26.0, r.x + 28, r.y + 106, MUTED, &crate::apps::fit(f, Face::Body, 26.0, question(day), r.w - 56));
                }
                if self.page > 0 { minigame::pill(fb, f, Self::bottom(0), "Anteriores", false); }
                if (self.page + 1) * PER_PAGE < self.days.len() { minigame::pill(fb, f, Self::bottom(1), "Próximas", false); }
            }
            View::Today | View::Entry(_) => {
                let day = match self.view { View::Entry(d) => d, _ => Self::today() };
                let r = Rect::new(MARGIN, 390, W - 2 * MARGIN, 230);
                ui::card(fb, r, 30, PAPER, false);
                for (i, line) in wrap(f, Face::Display, 38.0, question(day), r.w - 64).iter().take(3).enumerate() {
                    f.draw(fb, Face::Display, 38.0, r.x + 32, r.y + 68 + i as i32 * 52, INK, line);
                }
                if self.read_error {
                    f.draw(fb, Face::Body, 32.0, MARGIN, 700, INK, "Não foi possível ler esta resposta.");
                    f.draw(fb, Face::Body, 28.0, MARGIN, 750, MUTED, "A resposta original foi preservada.");
                } else if self.answer.is_empty() {
                    f.draw(fb, Face::Body, 32.0, MARGIN, 700, MUTED, "Ainda não há resposta para este dia.");
                } else {
                    for (i, line) in self.lines.iter().skip(self.answer_page * ANSWER_LINES).take(ANSWER_LINES).enumerate() {
                        f.draw(fb, Face::Body, 32.0, MARGIN, 700 + i as i32 * 44, INK, line);
                    }
                }
                if self.pages() > 1 {
                    if self.answer_page > 0 { minigame::pill(fb, f, Self::pages_button(0), "Anterior", false); }
                    if self.answer_page + 1 < self.pages() { minigame::pill(fb, f, Self::pages_button(1), "Próxima", false); }
                    let label = format!("{} de {}", self.answer_page + 1, self.pages());
                    let width = f.measure(Face::Strong, 28.0, &label);
                    f.draw(fb, Face::Strong, 28.0, (W - width) / 2, 1185, MUTED, &label);
                }
                if self.draft.is_some() {
                    let message = if self.draft_read_error { "Não foi possível ler a resposta original." } else { "Não foi possível salvar. Tente novamente." };
                    f.draw(fb, Face::Strong, 26.0, MARGIN, 1245, INK, message);
                }
                if self.read_error { minigame::pill(fb, f, Self::bottom(0), "Tentar ler", false); }
                if self.view == View::Today {
                    if !self.read_error { minigame::pill(fb, f, Self::bottom(0), if self.draft.is_some() { "Tentar salvar" } else if self.answer.is_empty() { "Responder" } else { "Substituir resposta" }, true); }
                    minigame::pill(fb, f, Self::bottom(1), "Respostas antigas", false);
                }
            }
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.view {
            View::Today | View::Entry(_) => {
                let mut v = Vec::new();
                if self.view == View::Today {
                    v.push((2, Self::bottom(1)));
                    if !self.read_error { v.push((1, Self::bottom(0))); }
                }
                if self.read_error { v.push((7, Self::bottom(0))); }
                if self.answer_page > 0 { v.push((5, Self::pages_button(0))); }
                if self.answer_page + 1 < self.pages() { v.push((6, Self::pages_button(1))); }
                v
            }
            View::History => {
                let mut v: Vec<_> = self.days.iter().skip(self.page * PER_PAGE).take(PER_PAGE).enumerate().map(|(i, _)| (100 + i as u32, Self::row(i))).collect();
                if self.page > 0 { v.push((3, Self::bottom(0))); }
                if (self.page + 1) * PER_PAGE < self.days.len() { v.push((4, Self::bottom(1))); }
                v
            }
        }
    }
    fn tap(&mut self, id: u32, _: i32, _: i32) -> Redraw {
        match (self.view, id) {
            (View::Today | View::Entry(_), 7) if self.read_error => { self.load(self.loaded_day); }
            (View::Today, 1) if !self.read_error => {
                if let Some((day, text)) = self.draft.take() { self.save_draft(day, text); }
                else { self.edit_day = Some(Self::today()); self.want = true; }
            }
            (View::Today, 2) => { self.days = saved_days(&dir()); self.page = 0; self.view = View::History; }
            (View::History, 3) if self.page > 0 => self.page -= 1,
            (View::History, 4) if (self.page + 1) * PER_PAGE < self.days.len() => self.page += 1,
            (View::History, 100..=104) => if let Some(&d) = self.days.get(self.page * PER_PAGE + (id - 100) as usize) { self.load(d); self.view = View::Entry(d); },
            (View::Today | View::Entry(_), 5) if self.answer_page > 0 => self.answer_page -= 1,
            (View::Today | View::Entry(_), 6) if self.answer_page + 1 < self.pages() => self.answer_page += 1,
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn wants_text(&mut self) -> Option<String> {
        if self.want { self.want = false; Some(question(self.edit_day.unwrap_or_else(Self::today)).to_string()) } else { None }
    }
    fn text_input(&mut self, text: &str) -> Redraw {
        if !text.trim().is_empty() { self.save_draft(self.edit_day.unwrap_or_else(Self::today), text.to_string()); }
        self.edit_day = None;
        Redraw::Full
    }
    fn tick(&mut self) -> Redraw {
        if self.view == View::Today && self.loaded_day != Self::today() { self.load(Self::today()); return Redraw::Full; }
        Redraw::None
    }
    fn back(&mut self) -> bool {
        match self.view {
            View::Entry(_) => { self.view = View::History; true }
            View::History => { self.load(Self::today()); self.view = View::Today; true }
            View::Today => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pergunta_e_data_sao_estaveis() {
        assert_eq!(include_str!("../assets/diario-perguntas.txt").lines().count(), 120);
        assert_eq!(question(20768), question(20768));
        assert_eq!(date(20768), "11/11/2026");
    }
    #[test]
    fn textos_longos_sao_quebrados_sem_perder_palavras() {
        let f = Fonts::new();
        let long_word = "a".repeat(200);
        let text = format!("{}\n{}", "palavra ".repeat(250), long_word);
        let lines = wrap(&f, Face::Body, 32.0, &text, W - 2 * MARGIN);
        assert!(lines.len() > ANSWER_LINES);
        assert!(lines.iter().all(|line| f.measure(Face::Body, 32.0, line) <= W - 2 * MARGIN));
        assert_eq!(lines.iter().map(|line| line.replace(' ', "")).collect::<String>(), text.replace([' ', '\n'], ""));
        let pages = lines.len().div_ceil(ANSWER_LINES);
        assert_eq!((0..pages).map(|page| lines.iter().skip(page * ANSWER_LINES).take(ANSWER_LINES).count()).sum::<usize>(), lines.len());
        for q in include_str!("../assets/diario-perguntas.txt").lines().filter(|s| !s.trim().is_empty()) {
            assert!(wrap(&f, Face::Display, 38.0, q, W - 2 * MARGIN - 64).len() <= 3, "pergunta longa: {q}");
            assert!(crate::chat::wrap_words(&f, Face::Display, 32.0, q, W - 2 * MARGIN).len() <= 2, "pergunta longa no teclado: {q}");
        }
    }
    #[test]
    fn erro_de_leitura_preserva_original_e_rascunho() {
        let root = std::env::temp_dir().join(format!("verso-diario-invalido-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert!(read(&root, 10).unwrap().is_none());
        std::fs::create_dir_all(&root).unwrap();
        let original = [0xff, 0xfe, 0x42];
        std::fs::write(path(&root, 10), original).unwrap();
        assert!(read(&root, 10).is_err());
        assert!(save(&root, 10, "nova resposta").is_err());
        assert_eq!(std::fs::read(path(&root, 10)).unwrap(), original);
        let mut d = Diario { view: View::Today, page: 0, answer_page: 0, days: vec![], want: false, edit_day: Some(10), draft: None, draft_read_error: false, read_error: false, loaded_day: 10, answer: String::new(), lines: vec![] };
        d.load_at(&root, 10);
        assert!(d.read_error);
        assert!(!d.targets().iter().any(|(id, _)| *id == 1));
        assert!(d.targets().iter().any(|(id, _)| *id == 7));
        d.save_draft_at(&root, 10, "rascunho".into());
        assert_eq!(d.draft.as_ref().map(|(_, text)| text.as_str()), Some("rascunho"));
        assert!(d.draft_read_error);
        assert_eq!(std::fs::read(path(&root, 10)).unwrap(), original);
        std::fs::write(path(&root, 10), "resposta recuperada").unwrap();
        d.load_at(&root, 10);
        assert!(!d.read_error);
        assert_eq!(d.answer, "resposta recuperada");
        assert!(d.draft.is_some());
        let draft = d.draft.take().unwrap().1;
        d.save_draft_at(&root, 10, draft);
        assert_eq!(read(&root, 10).unwrap().as_deref(), Some("rascunho"));
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn respostas_sao_guardadas_e_ordenadas() {
        let root = std::env::temp_dir().join(format!("verso-diario-teste-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        save(&root, 10, "Primeira resposta").unwrap();
        save(&root, 12, "Outra resposta").unwrap();
        assert_eq!(read(&root, 10).unwrap().as_deref(), Some("Primeira resposta"));
        save(&root, 10, "Nova resposta\ncom duas linhas").unwrap();
        assert_eq!(read(&root, 10).unwrap().as_deref(), Some("Nova resposta\ncom duas linhas"));
        std::fs::write(root.join("ignorar.txt"), "outro arquivo").unwrap();
        assert_eq!(saved_days(&root), vec![12, 10]);
        let _ = std::fs::remove_dir_all(root);
    }
}
