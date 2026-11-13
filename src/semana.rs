//! Fechamento da semana: o Kindle manda os números dos últimos 7 dias (aprendizado, leitura,
//! foco, violão), a ponte junta o trabalho dos agentes e o Claude escreve o resumo com 3
//! perguntas de toque. As respostas viram o plano da próxima semana e uma nota no Brain pessoal.

use crate::chat::Json;
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::sync::mpsc::{channel, Receiver};

#[derive(Default, Clone)]
pub struct Week {
    #[allow(dead_code)] // guardado no arquivo; ainda sem tela que o mostre
    pub id: String,
    pub title: String,
    pub summary: Vec<String>,
    pub numbers: Vec<(String, String)>,
    pub questions: Vec<(String, String, Vec<String>)>,
    pub plan: Vec<String>,
}

fn parse_week(j: &Json) -> Option<Week> {
    let s = |k: &str| j.get(k).and_then(Json::str).unwrap_or("").to_string();
    let strs = |k: &str| j.get(k).map(|a| a.arr().iter().filter_map(|x| x.str().map(str::to_string)).collect()).unwrap_or_default();
    Some(Week {
        id: s("semana"),
        title: s("titulo"),
        summary: strs("resumo"),
        numbers: j.get("numeros")?.arr().iter().filter_map(|n| Some((n.get("rotulo")?.str()?.to_string(), n.get("valor")?.str()?.to_string()))).collect(),
        questions: j
            .get("perguntas")?
            .arr()
            .iter()
            .filter_map(|q| Some((q.get("id")?.str()?.to_string(), q.get("texto")?.str()?.to_string(), q.get("opcoes")?.arr().iter().filter_map(|o| o.str().map(str::to_string)).collect())))
            .collect(),
        plan: strs("plano"),
    })
}

/// Números do Kindle nos últimos 7 dias, em JSON para a ponte.
pub fn kindle_stats() -> String {
    let t = crate::aprender::today();
    let off = crate::offset_min();
    let xp = crate::aprender::xp_days(t - 6, t);
    let st = crate::aprender::status();
    let days: Vec<String> = (0..7)
        .map(|k| {
            let d = t - 6 + k as i64;
            let (read, focus) = crate::data::day_stats(off, d);
            format!("{{\"dia\": {}, \"xp\": {}, \"leitura_min\": {read}, \"pomodoros\": {focus}}}", k as i64 - 6, xp[k])
        })
        .collect();
    let src: Vec<String> = crate::aprender::xp_by_source(t - 6, t).iter().map(|(s, x)| format!("{}: {x}", crate::chat::json_str(s))).collect();
    format!(
        "{{\"dias\": [{}], \"xp_por_fonte\": {{{}}}, \"ofensiva\": {}, \"recorde\": {}, \"violao_dias\": {}, \"partitura_licoes_vistas\": {}, \"livro\": {}}}",
        days.join(","),
        src.join(","),
        st.streak,
        st.best,
        crate::violao::practice_days(t - 6, t),
        crate::partitura::done().len(),
        crate::chat::json_str(&crate::data::last_book().map(|b| b.title).unwrap_or_default())
    )
}

#[derive(Clone, Copy, PartialEq)]
enum View {
    Intro,
    Loading,
    Summary,
    Question(usize),
    Plan,
}

enum Job {
    Get,
    Make,
    Answer,
}

pub struct Semana {
    view: View,
    week: Option<Week>,
    answers: Vec<String>,
    job: Option<(Job, Receiver<Result<String, String>>)>,
    error: Option<String>,
    last_poll: std::time::Instant,
}

impl Semana {
    pub fn new() -> Self {
        let mut s = Semana { view: View::Loading, week: None, answers: Vec::new(), job: None, error: None, last_poll: std::time::Instant::now() };
        if crate::chat::configured() {
            s.spawn(Job::Get);
        } else {
            s.view = View::Intro;
            s.error = Some("A ponte do Verso não está configurada.".into());
        }
        s
    }

    #[cfg(test)]
    pub fn with(week: Week, view: usize) -> Self {
        let v = match view {
            0 => View::Intro,
            1 => View::Summary,
            2 => View::Question(0),
            _ => View::Plan,
        };
        Semana { view: v, week: Some(week), answers: Vec::new(), job: None, error: None, last_poll: std::time::Instant::now() }
    }

    fn spawn(&mut self, job: Job) {
        let (tx, rx) = channel();
        let body = match job {
            Job::Get => None,
            Job::Make => Some(format!("{{\"kindle\": {}}}", kindle_stats())),
            Job::Answer => {
                let qs = self.week.as_ref().map(|w| w.questions.clone()).unwrap_or_default();
                let pairs: Vec<String> = qs.iter().zip(&self.answers).map(|((id, _, _), a)| format!("{}: {}", crate::chat::json_str(id), crate::chat::json_str(a))).collect();
                Some(format!("{{\"respostas\": {{{}}}}}", pairs.join(",")))
            }
        };
        let path = match job {
            Job::Get => "/semana",
            Job::Make => "/semana/gerar",
            Job::Answer => "/semana/respostas",
        };
        std::thread::spawn(move || {
            let r = match &body {
                Some(b) => crate::chat::call_raw("POST", path, Some(b), 330),
                None => crate::chat::call_raw("GET", path, None, 30),
            };
            let _ = tx.send(r);
        });
        self.job = Some((job, rx));
        self.error = None;
        self.view = View::Loading;
    }

    fn wide() -> Rect {
        Rect::new(MARGIN, 1290, W - 2 * MARGIN, 104)
    }

    fn option_rect(k: usize) -> Rect {
        Rect::new(MARGIN, 640 + k as i32 * 128, W - 2 * MARGIN, 108)
    }

    fn lines(fb: &mut Framebuffer, f: &Fonts, face: Face, size: f32, y: i32, text: &str, max: usize, color: u8) -> i32 {
        let lh = (size * 1.32) as i32;
        let ls = crate::chat::wrap_words(f, face, size, text, W - 2 * MARGIN);
        for (k, l) in ls.iter().take(max).enumerate() {
            f.draw(fb, face, size, MARGIN, y + k as i32 * lh, color, l);
        }
        y + ls.len().min(max) as i32 * lh
    }
}

impl MiniGame for Semana {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Loading => {
                f.draw(fb, Face::Display, 90.0, MARGIN - 4, 380, INK, "Semana");
                let msg = match self.job.as_ref().map(|j| &j.0) {
                    Some(Job::Make) => "O Verso está lendo a sua semana: aprendizado, leitura, foco, violão e o trabalho dos agentes. Leva cerca de um minuto.",
                    Some(Job::Answer) => "Montando o plano da próxima semana com as suas respostas e guardando a nota no seu Brain.",
                    _ => "Buscando a semana…",
                };
                Self::lines(fb, f, Face::Body, 34.0, 470, msg, 5, MUTED);
            }
            View::Intro => {
                f.draw(fb, Face::Display, 110.0, MARGIN - 4, 330, INK, "Semana");
                let y = Self::lines(fb, f, Face::Strong, 32.0, 400, "Feche a semana em 3 minutos: o que foi bem, o que escapou e o plano da próxima.", 3, MUTED);
                let st = crate::aprender::status();
                crate::aprender::week_row(fb, f, MARGIN, y + 60, W - 2 * MARGIN, &st);
                let steps = ["O Kindle junta os números dos últimos 7 dias.", "O Verso escreve o resumo e faz 3 perguntas.", "Você responde com toques; o plano vai para o seu Brain."];
                let mut yy = y + 260;
                for (k, t) in steps.iter().enumerate() {
                    fb.round_rect(Rect::new(MARGIN, yy - 30, 44, 44), 22, INK);
                    let n = (k + 1).to_string();
                    let w = f.measure(Face::Strong, 26.0, &n);
                    f.draw(fb, Face::Strong, 26.0, MARGIN + 22 - w / 2, yy + 1, PAPER, &n);
                    f.draw(fb, Face::Body, 30.0, MARGIN + 70, yy, INK, &crate::apps::fit(f, Face::Body, 30.0, t, W - 2 * MARGIN - 70));
                    yy += 70;
                }
                if let Some(e) = &self.error {
                    Self::lines(fb, f, Face::Strong, 28.0, 1220, e, 1, INK);
                }
                ui::button(fb, f, Self::wide(), "Montar a semana", true);
            }
            View::Summary => {
                let Some(w) = &self.week else { return };
                f.draw(fb, Face::Strong, 26.0, MARGIN, 210, MUTED, "SUA SEMANA · ÚLTIMOS 7 DIAS");
                let mut y = Self::lines(fb, f, Face::Display, 52.0, 280, &w.title, 2, INK) + 20;
                // Números em cartões, 3 por linha.
                let cw = (W - 2 * MARGIN - 2 * 16) / 3;
                for (k, (label, value)) in w.numbers.iter().enumerate() {
                    let r = Rect::new(MARGIN + (k % 3) as i32 * (cw + 16), y + (k / 3) as i32 * 136, cw, 120);
                    ui::card(fb, r, 24, if k == 0 { INK } else { PAPER }, false);
                    let fg = if k == 0 { PAPER } else { INK };
                    f.draw(fb, Face::Display, 54.0, r.x + 22, r.y + 66, fg, &crate::apps::fit(f, Face::Display, 54.0, value, r.w - 44));
                    f.draw(fb, Face::Body, 23.0, r.x + 22, r.y + 102, if k == 0 { 0xCC } else { MUTED }, &crate::apps::fit(f, Face::Body, 23.0, label, r.w - 44));
                }
                y += w.numbers.len().div_ceil(3) as i32 * 136 + 40;
                for p in &w.summary {
                    if y > 1180 {
                        break;
                    }
                    y = Self::lines(fb, f, Face::Body, 30.0, y, p, ((1230 - y) / 40).max(1) as usize, INK) + 18;
                }
                let label = if w.plan.is_empty() { format!("Responder {} perguntas ›", w.questions.len()) } else { "Ver o plano ›".to_string() };
                ui::button(fb, f, Self::wide(), &label, true);
            }
            View::Question(k) => {
                let Some(w) = &self.week else { return };
                let Some((_, q, opts)) = w.questions.get(k) else { return };
                f.draw(fb, Face::Strong, 26.0, MARGIN, 210, MUTED, &format!("PERGUNTA {} DE {}", k + 1, w.questions.len()));
                let bar = Rect::new(MARGIN, 236, W - 2 * MARGIN, 14);
                fb.round_rect(bar, 7, 0xC8);
                fb.round_rect(Rect::new(bar.x, bar.y, bar.w * (k as i32 + 1) / w.questions.len().max(1) as i32, bar.h), 7, INK);
                Self::lines(fb, f, Face::Display, 50.0, 340, q, 5, INK);
                for (i, o) in opts.iter().enumerate() {
                    ui::button(fb, f, Self::option_rect(i), o, false);
                }
            }
            View::Plan => {
                let Some(w) = &self.week else { return };
                f.draw(fb, Face::Display, 84.0, MARGIN - 4, 320, INK, "Próxima semana");
                let mut y = 400;
                for (k, p) in w.plan.iter().enumerate() {
                    let ls = crate::chat::wrap_words(f, Face::Body, 32.0, p, W - 2 * MARGIN - 130);
                    let h = (ls.len() as i32 * 44 + 56).max(120);
                    let r = Rect::new(MARGIN, y, W - 2 * MARGIN, h);
                    ui::card(fb, r, 28, PAPER, false);
                    fb.round_rect(Rect::new(r.x + 24, r.y + (h - 64) / 2, 64, 64), 32, INK);
                    let n = (k + 1).to_string();
                    let nw = f.measure(Face::Display, 36.0, &n);
                    f.draw(fb, Face::Display, 36.0, r.x + 56 - nw / 2, r.y + h / 2 + 13, PAPER, &n);
                    for (i, l) in ls.iter().enumerate() {
                        f.draw(fb, Face::Body, 32.0, r.x + 110, r.y + 28 + 32 + i as i32 * 44, INK, l);
                    }
                    y += h + 22;
                }
                Self::lines(fb, f, Face::Strong, 28.0, y + 40, "A nota da semana vai para o seu Brain, em Pessoal/Semanas.", 2, MUTED);
                ui::button(fb, f, Self::wide(), "Feito", true);
            }
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.view {
            View::Intro | View::Summary | View::Plan => vec![(1, Self::wide())],
            View::Question(k) => {
                let n = self.week.as_ref().and_then(|w| w.questions.get(k)).map(|q| q.2.len()).unwrap_or(0);
                (0..n).map(|i| (10 + i as u32, Self::option_rect(i))).collect()
            }
            View::Loading => vec![],
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match (self.view, id) {
            (View::Intro, 1) => self.spawn(Job::Make),
            (View::Summary, 1) => {
                let done = self.week.as_ref().is_some_and(|w| !w.plan.is_empty());
                self.answers.clear();
                self.view = if done { View::Plan } else { View::Question(0) };
            }
            (View::Question(k), i @ 10..=13) => {
                let Some(w) = &self.week else { return Redraw::None };
                let Some(o) = w.questions.get(k).and_then(|q| q.2.get((i - 10) as usize)) else { return Redraw::None };
                self.answers.push(o.clone());
                if k + 1 < w.questions.len() {
                    self.view = View::Question(k + 1);
                } else {
                    self.spawn(Job::Answer);
                }
            }
            (View::Plan, 1) => self.view = View::Summary,
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn tick(&mut self) -> Redraw {
        if self.last_poll.elapsed().as_millis() < 700 {
            return Redraw::None;
        }
        self.last_poll = std::time::Instant::now();
        let Some((job, rx)) = &self.job else { return Redraw::None };
        let Ok(res) = rx.try_recv() else { return Redraw::None };
        let was_answer = matches!(job, Job::Answer);
        self.job = None;
        match res.ok().and_then(|raw| crate::chat::parse(&raw)).and_then(|j| parse_week(&j)) {
            Some(w) => {
                let done = !w.plan.is_empty();
                self.week = Some(w);
                self.view = if was_answer && done { View::Plan } else { View::Summary };
            }
            None => {
                self.view = View::Intro;
                if was_answer {
                    self.error = Some("Não consegui montar o plano agora. Tente de novo.".into());
                }
            }
        }
        Redraw::Full
    }

    fn keep_awake(&self) -> bool {
        self.job.is_some()
    }

    fn back(&mut self) -> bool {
        match self.view {
            View::Question(_) | View::Plan => {
                self.view = View::Summary;
                true
            }
            _ => false,
        }
    }

    fn context(&self) -> String {
        match &self.week {
            Some(w) => format!("Fechamento da semana no Kindle: {}. {}", w.title, w.summary.join(" ")),
            None => "Fechamento da semana no Kindle.".into(),
        }
    }
}
