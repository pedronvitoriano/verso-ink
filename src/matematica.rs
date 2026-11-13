//! Treino de matemática mental: dez contas por rodada, com tempo e XP do Aprender.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::time::{Duration, Instant};

const QUESTIONS: usize = 10;
const LEVELS: [&str; 4] = ["Tabuada", "Somas de dois dígitos", "Porcentagem", "Frações"];

#[derive(Clone)]
struct Question {
    text: String,
    answer: i32,
    options: [i32; 4],
}

impl Question {
    fn generate(level: usize, rng: &mut Rng) -> Self {
        let roll = |r: &mut Rng, n: u64| (r.next() % n) as i32;
        let (text, answer) = match level {
            0 => {
                let a = 2 + roll(rng, 11);
                let b = 2 + roll(rng, 11);
                (format!("{a} × {b}"), a * b)
            }
            1 => {
                let a = 10 + roll(rng, 90);
                let b = 10 + roll(rng, 90);
                (format!("{a} + {b}"), a + b)
            }
            2 => {
                let percentages = [10, 20, 25, 50, 75];
                let percent = percentages[roll(rng, percentages.len() as u64) as usize];
                let number = (1 + roll(rng, 20)) * 20;
                (format!("{percent}% de {number}"), number * percent / 100)
            }
            _ => {
                let den = 2 + roll(rng, 5);
                let num = 1 + roll(rng, (den - 1) as u64);
                let number = den * (2 + roll(rng, 20));
                (format!("{num}/{den} de {number}"), num * number / den)
            }
        };
        let mut alternatives = vec![answer];
        let mut distance = 1;
        while alternatives.len() < 4 {
            if answer >= distance { alternatives.push(answer - distance); }
            if alternatives.len() < 4 { alternatives.push(answer + distance); }
            distance += 1;
        }
        rng.shuffle(&mut alternatives);
        Self { text, answer, options: [alternatives[0], alternatives[1], alternatives[2], alternatives[3]] }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum View { Menu, Exercise, Result, Celebration }

pub struct Matematica {
    view: View,
    level: usize,
    question: Question,
    answered: usize,
    correct: usize,
    selected: Option<usize>,
    start: Instant,
    elapsed: Duration,
    shown_second: u64,
    rng: Rng,
    celebration: Option<crate::aprender::Cel>,
}

impl Matematica {
    pub fn new() -> Self {
        let mut rng = Rng::new();
        let question = Question::generate(0, &mut rng);
        Self { view: View::Menu, level: 0, question, answered: 0, correct: 0, selected: None, start: Instant::now(), elapsed: Duration::ZERO, shown_second: 0, rng, celebration: None }
    }
    /// Quadros fixos, sem consultar ou gravar o progresso do usuário.
    #[cfg(test)]
    fn cena_print(kind: u8) -> Self {
        let question = Question { text: "25% de 80".into(), answer: 20, options: [10, 15, 20, 25] };
        Self {
            view: if kind == 0 { View::Menu } else if kind == 3 { View::Result } else { View::Exercise },
            level: 2, question, answered: if kind == 2 { 4 } else if kind == 3 { QUESTIONS } else { 3 },
            correct: if kind == 3 { 8 } else { 3 }, selected: if kind == 2 { Some(1) } else { None },
            start: Instant::now(), elapsed: Duration::from_secs(93), shown_second: 0,
            rng: Rng::seeded(42), celebration: None,
        }
    }
    #[cfg(test)]
    pub fn print_menu() -> Self { Self::cena_print(0) }
    #[cfg(test)]
    pub fn print_questao() -> Self { Self::cena_print(1) }
    #[cfg(test)]
    pub fn print_feedback() -> Self { Self::cena_print(2) }
    #[cfg(test)]
    pub fn print_fim() -> Self { Self::cena_print(3) }

    fn start(&mut self, level: usize) {
        self.level = level;
        self.question = Question::generate(level, &mut self.rng);
        self.answered = 0;
        self.correct = 0;
        self.selected = None;
        self.start = Instant::now();
        self.elapsed = Duration::ZERO;
        self.shown_second = 0;
        self.celebration = None;
        self.view = View::Exercise;
    }
    fn time_text(d: Duration) -> String {
        let s = d.as_secs();
        format!("{}:{:02}", s / 60, s % 60)
    }
    fn option(k: usize) -> Rect {
        let w = (W - 2 * MARGIN - 20) / 2;
        Rect::new(MARGIN + (k % 2) as i32 * (w + 20), 700 + (k / 2) as i32 * 140, w, 120)
    }
    fn bottom(k: usize) -> Rect { Rect::new(MARGIN + k as i32 * 472, 1288, 456, 80) }
    fn choose(&mut self, k: usize) {
        if self.view != View::Exercise || self.selected.is_some() || k >= 4 { return; }
        self.selected = Some(k);
        self.answered += 1;
        if self.question.options[k] == self.question.answer { self.correct += 1; }
        if self.answered == QUESTIONS { self.elapsed = self.start.elapsed(); }
    }
    fn advance(&mut self) {
        if self.view != View::Exercise || self.selected.is_none() { return; }
        if self.answered == QUESTIONS {
            let xp = if self.correct == QUESTIONS { 15 } else { 10 };
            self.celebration = crate::aprender::add(xp, "matematica");
            self.view = View::Result;
        } else {
            self.question = Question::generate(self.level, &mut self.rng);
            self.selected = None;
        }
    }
    fn label(&self) -> String {
        let elapsed = if self.answered == QUESTIONS { self.elapsed } else { self.start.elapsed() };
        let current = self.answered + usize::from(self.selected.is_none());
        format!("Conta {} de {} · {}", current.min(QUESTIONS), QUESTIONS, Self::time_text(elapsed))
    }
}

impl MiniGame for Matematica {
    fn at_menu(&self) -> bool { self.view == View::Menu }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Menu => {
                minigame::draw_menu(fb, f, "Matemática", "Dez contas, no seu tempo.", &LEVELS);
                let (streak, done) = crate::aprender::topic_streak(&["matematica"]);
                f.draw(fb, Face::Strong, 30.0, MARGIN, 1170, INK, &format!("Ofensiva: {streak} {}{}", if streak == 1 { "dia" } else { "dias" }, if done { " · Hoje feito" } else { "" }));
            }
            View::Exercise => {
                minigame::header(fb, f, "Matemática", &self.label());
                f.draw(fb, Face::Strong, 32.0, MARGIN, 350, MUTED, LEVELS[self.level]);
                let r = Rect::new(MARGIN, 390, W - 2 * MARGIN, 245);
                ui::card(fb, r, 32, PAPER, false);
                let mut size = 82.0;
                while f.measure(Face::Display, size, &self.question.text) > r.w - 80 && size > 44.0 { size -= 2.0; }
                let width = f.measure(Face::Display, size, &self.question.text);
                f.draw(fb, Face::Display, size, (W - width) / 2, r.y + r.h / 2 + (size * 0.35) as i32, INK, &self.question.text);
                for k in 0..4 {
                    let chosen = self.selected == Some(k);
                    let r = Self::option(k);
                    minigame::pill(fb, f, r, &self.question.options[k].to_string(), chosen);
                    if self.selected.is_some() && (chosen || self.question.options[k] == self.question.answer) {
                        crate::aprender::badge(fb, r, self.question.options[k] == self.question.answer, chosen);
                    }
                }
                if let Some(k) = self.selected {
                    let ok = self.question.options[k] == self.question.answer;
                    crate::aprender::draw_feedback(fb, f, ok, if ok { "Muito bem!" } else { "Quase!" }, &format!("{} = {}", self.question.text, self.question.answer));
                } else {
                    minigame::hint(fb, f, "Escolha o resultado sem usar calculadora.");
                }
            }
            View::Result => {
                minigame::header(fb, f, "Matemática", "Rodada concluída");
                let r = Rect::new(MARGIN, 320, W - 2 * MARGIN, 560);
                ui::card(fb, r, 32, PAPER, false);
                let title = format!("{} de {}", self.correct, QUESTIONS);
                let width = f.measure(Face::Display, 100.0, &title);
                f.draw(fb, Face::Display, 100.0, (W - width) / 2, 500, INK, &title);
                let details = ["contas certas".to_string(), format!("Tempo: {}", Self::time_text(self.elapsed)), format!("+{} XP", if self.correct == QUESTIONS { 15 } else { 10 })];
                for (k, line) in details.iter().enumerate() {
                    let width = f.measure(Face::Strong, 38.0, line);
                    f.draw(fb, Face::Strong, 38.0, (W - width) / 2, 575 + k as i32 * 78, MUTED, line);
                }
                let (streak, _) = crate::aprender::topic_streak(&["matematica"]);
                let line = format!("Ofensiva: {streak} {}", if streak == 1 { "dia" } else { "dias" });
                let width = f.measure(Face::Strong, 32.0, &line);
                f.draw(fb, Face::Strong, 32.0, (W - width) / 2, 1000, INK, &line);
                minigame::pill(fb, f, Self::bottom(0), "Menu", false);
                minigame::pill(fb, f, Self::bottom(1), "Outra rodada", true);
            }
            View::Celebration => if let Some(c) = self.celebration { crate::aprender::draw_celebration(fb, f, c); },
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.view {
            View::Menu => (0..4).map(|k| (900 + k as u32, minigame::menu_row(k))).collect(),
            View::Exercise if self.selected.is_none() => (0..4).map(|k| (k as u32, Self::option(k))).collect(),
            View::Exercise => vec![(800, crate::aprender::feedback_button())],
            View::Result => vec![(801, Self::bottom(0)), (802, Self::bottom(1))],
            View::Celebration => vec![(803, crate::aprender::cel_button())],
        }
    }
    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match (self.view, id) {
            (View::Menu, 900..=903) => self.start((id - 900) as usize),
            (View::Exercise, 0..=3) if self.selected.is_none() => self.choose(id as usize),
            (View::Exercise, 800) if self.selected.is_some() => self.advance(),
            (View::Result, 801) => { if self.celebration.is_some() { self.view = View::Celebration; } else { self.view = View::Menu; } }
            (View::Result, 802) => self.start(self.level),
            (View::Celebration, 803) => { self.celebration = None; self.view = View::Menu; }
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn tick(&mut self) -> Redraw {
        if self.view != View::Exercise || self.answered == QUESTIONS { return Redraw::None; }
        let second = self.start.elapsed().as_secs();
        if second == self.shown_second { return Redraw::None; }
        self.shown_second = second;
        Redraw::Part(Rect::new(MARGIN, 170, W - 2 * MARGIN, 80))
    }
    fn keep_awake(&self) -> bool { self.view == View::Exercise }
    fn back(&mut self) -> bool {
        match self.view {
            View::Menu => false,
            View::Celebration => { self.view = View::Menu; self.celebration = None; true }
            _ => { self.view = View::Menu; true }
        }
    }
    fn summary(&self) -> String { if self.view == View::Exercise { self.label() } else { String::new() } }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cenas_de_print_sao_deterministicas() {
        let menu = Matematica::print_menu();
        let question = Matematica::print_questao();
        let feedback = Matematica::print_feedback();
        let done = Matematica::print_fim();
        assert!(menu.view == View::Menu && question.view == View::Exercise && question.selected.is_none());
        assert_eq!(feedback.selected, Some(1));
        assert!(done.view == View::Result && done.elapsed == Duration::from_secs(93));
    }
    #[test]
    fn cronometro_atualiza_e_congela_na_ultima_resposta() {
        let mut g = Matematica::new();
        g.start(0);
        g.start = Instant::now() - Duration::from_secs(125);
        assert!(matches!(g.tick(), Redraw::Part(_)));
        g.answered = QUESTIONS - 1;
        g.choose(0);
        assert!(g.elapsed.as_secs() >= 125);
        let fixed = g.elapsed;
        g.start = Instant::now() - Duration::from_secs(500);
        assert!(matches!(g.tick(), Redraw::None));
        assert!(g.label().ends_with(&Matematica::time_text(fixed)));
    }
    #[test]
    fn aprendizado_persiste_xp_e_ofensiva_em_perfil_isolado() {
        const CHILD: &str = "VERSO_TESTE_MATEMATICA_FILHO";
        if std::env::var_os(CHILD).is_some() {
            let mut game = Matematica::new();
            game.start(0);
            for _ in 0..QUESTIONS {
                let right = game.question.options.iter().position(|&v| v == game.question.answer).unwrap();
                game.choose(right); game.advance();
            }
            assert!(game.view == View::Result);
            let path = crate::home_dir().join("data/aprender.txt");
            let log = std::fs::read_to_string(path).unwrap();
            assert_eq!(log.lines().filter(|l| l.ends_with("|15|matematica")).count(), 1);
            assert!(crate::aprender::topic_streak(&["matematica"]).1);
            assert!(crate::aprender::status().today_done);
            game.advance();
            let log = std::fs::read_to_string(crate::home_dir().join("data/aprender.txt")).unwrap();
            assert_eq!(log.lines().filter(|l| l.ends_with("|15|matematica")).count(), 1);
            game.start(1);
            for i in 0..QUESTIONS {
                let chosen = game.question.options.iter().position(|&v| if i == 0 { v != game.question.answer } else { v == game.question.answer }).unwrap();
                game.choose(chosen); game.advance();
            }
            let log = std::fs::read_to_string(crate::home_dir().join("data/aprender.txt")).unwrap();
            assert_eq!(log.lines().filter(|l| l.ends_with("|10|matematica")).count(), 1);
            assert_eq!(crate::aprender::status().xp_today, 25);
            return;
        }
        let home = std::env::temp_dir().join(format!("verso-matematica-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact").arg("matematica::tests::aprendizado_persiste_xp_e_ofensiva_em_perfil_isolado")
            .arg("--test-threads=1").env(CHILD, "1").env("VERSO_INK_HOME", &home).status().unwrap();
        let _ = std::fs::remove_dir_all(home);
        assert!(status.success());
    }
    #[test]
    fn geracao_de_contas_e_alternativas_corretas() {
        let mut rng = Rng::seeded(62);
        for level in 0..4 {
            for _ in 0..200 {
                let q = Question::generate(level, &mut rng);
                assert_eq!(q.options.iter().filter(|&&n| n == q.answer).count(), 1);
                let mut options = q.options.to_vec();
                options.sort(); options.dedup();
                assert_eq!(options.len(), 4);
                assert!(options.iter().all(|&n| n >= 0));
                let words: Vec<&str> = q.text.split_whitespace().collect();
                let expected = match level {
                    0 => words[0].parse::<i32>().unwrap() * words[2].parse::<i32>().unwrap(),
                    1 => words[0].parse::<i32>().unwrap() + words[2].parse::<i32>().unwrap(),
                    2 => words[0].trim_end_matches('%').parse::<i32>().unwrap() * words[2].parse::<i32>().unwrap() / 100,
                    _ => { let (n, d) = words[0].split_once('/').unwrap(); n.parse::<i32>().unwrap() * words[2].parse::<i32>().unwrap() / d.parse::<i32>().unwrap() }
                };
                assert_eq!(q.answer, expected);
            }
        }
    }
    #[test]
    fn contabiliza_so_uma_resposta_e_tempo() {
        let mut game = Matematica::new();
        game.start(0);
        assert!(game.label().starts_with("Conta 1 de 10"));
        let right = game.question.options.iter().position(|&n| n == game.question.answer).unwrap();
        game.choose(right); game.choose(right);
        assert!(game.label().starts_with("Conta 1 de 10"));
        assert_eq!((game.answered, game.correct), (1, 1));
        assert_eq!(Matematica::time_text(Duration::from_secs(125)), "2:05");
        game.advance();
        assert!(game.label().starts_with("Conta 2 de 10"));
        let wrong = game.question.options.iter().position(|&n| n != game.question.answer).unwrap();
        game.choose(wrong);
        assert_eq!((game.answered, game.correct), (2, 1));
    }
}
