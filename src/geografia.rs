//! Geografia offline: pequenas lições de capitais e continentes.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

struct Question {
    prompt: &'static str,
    options: [&'static str; 4],
    correct: usize,
    explanation: &'static str,
}

struct Lesson {
    title: &'static str,
    intro: &'static str,
    questions: &'static [Question],
}

const AMERICA: &[Question] = &[
    Question { prompt: "Qual é a capital da Argentina?", options: ["Santiago", "Buenos Aires", "Lima", "Montevidéu"], correct: 1, explanation: "Buenos Aires é a capital argentina, às margens do Rio da Prata." },
    Question { prompt: "Qual é a capital do Canadá?", options: ["Toronto", "Vancouver", "Ottawa", "Montreal"], correct: 2, explanation: "Ottawa é a capital do Canadá; Toronto é sua cidade mais populosa." },
    Question { prompt: "Qual é a capital do Peru?", options: ["Lima", "Quito", "La Paz", "Bogotá"], correct: 0, explanation: "Lima fica na costa do Pacífico e é a capital peruana." },
    Question { prompt: "Qual é a capital do Chile?", options: ["Assunção", "Santiago", "Lima", "Caracas"], correct: 1, explanation: "Santiago fica no vale central do Chile, perto da Cordilheira dos Andes." },
    Question { prompt: "Qual é a capital da Colômbia?", options: ["Bogotá", "Quito", "Caracas", "Panamá"], correct: 0, explanation: "Bogotá é a capital da Colômbia e fica nos Andes." },
    Question { prompt: "Em qual região fica o México?", options: ["América do Sul", "Europa", "América do Norte", "África"], correct: 2, explanation: "O México fica na América do Norte, ao sul dos Estados Unidos." },
];
const EUROPA: &[Question] = &[
    Question { prompt: "Qual é a capital de Portugal?", options: ["Porto", "Lisboa", "Madri", "Coimbra"], correct: 1, explanation: "Lisboa é a capital portuguesa e fica junto ao estuário do Tejo." },
    Question { prompt: "Qual é a capital da Espanha?", options: ["Barcelona", "Lisboa", "Madri", "Sevilha"], correct: 2, explanation: "Madri é a capital da Espanha, no interior da Península Ibérica." },
    Question { prompt: "Qual é a capital da Itália?", options: ["Milão", "Veneza", "Roma", "Atenas"], correct: 2, explanation: "Roma é a capital da Itália, atravessada pelo rio Tibre." },
    Question { prompt: "Qual é a capital da Alemanha?", options: ["Berlim", "Munique", "Viena", "Frankfurt"], correct: 0, explanation: "Berlim é a capital e a maior cidade da Alemanha." },
    Question { prompt: "Qual é a capital da Noruega?", options: ["Estocolmo", "Helsinque", "Copenhague", "Oslo"], correct: 3, explanation: "Oslo é a capital da Noruega, no norte da Europa." },
    Question { prompt: "Em qual continente fica a Grécia?", options: ["África", "Europa", "América do Sul", "Oceania"], correct: 1, explanation: "A Grécia fica no sudeste da Europa; sua capital é Atenas." },
];
const AFRICA_ASIA: &[Question] = &[
    Question { prompt: "Qual é a capital do Egito?", options: ["Cairo", "Nairóbi", "Rabat", "Túnis"], correct: 0, explanation: "Cairo fica no norte do Egito, perto do delta do Nilo." },
    Question { prompt: "Qual é a capital de Angola?", options: ["Maputo", "Luanda", "Lusaca", "Nairóbi"], correct: 1, explanation: "Luanda é a capital de Angola e fica na costa atlântica." },
    Question { prompt: "Qual país tem uma bandeira branca com um círculo vermelho?", options: ["China", "Vietnã", "Japão", "Índia"], correct: 2, explanation: "A bandeira do Japão traz um círculo vermelho; sua capital é Tóquio." },
    Question { prompt: "Qual é a capital da Índia?", options: ["Mumbai", "Nova Délhi", "Daca", "Islamabad"], correct: 1, explanation: "Nova Délhi é a capital da Índia; Mumbai é outro grande centro do país." },
    Question { prompt: "Em qual continente fica o Quênia?", options: ["Europa", "Oceania", "Ásia", "África"], correct: 3, explanation: "O Quênia fica no leste da África; sua capital é Nairóbi." },
    Question { prompt: "Em qual continente fica o Nepal?", options: ["Ásia", "África", "Europa", "América do Norte"], correct: 0, explanation: "O Nepal fica no sul da Ásia, na região do Himalaia." },
];
const MUNDO: &[Question] = &[
    Question { prompt: "Qual é a capital da Austrália?", options: ["Sydney", "Melbourne", "Canberra", "Auckland"], correct: 2, explanation: "Canberra é a capital australiana e fica no interior, entre Sydney e Melbourne." },
    Question { prompt: "Qual é a capital da Nova Zelândia?", options: ["Auckland", "Wellington", "Canberra", "Suva"], correct: 1, explanation: "Wellington fica no extremo sul da Ilha Norte da Nova Zelândia." },
    Question { prompt: "Qual destes países fica na Oceania?", options: ["Fiji", "Chile", "Egito", "Nepal"], correct: 0, explanation: "Fiji é um país insular da Oceania, no oceano Pacífico." },
    Question { prompt: "Em qual região fica o Uruguai?", options: ["América do Norte", "Europa", "América do Sul", "Ásia"], correct: 2, explanation: "O Uruguai fica na América do Sul, entre Brasil e Argentina." },
    Question { prompt: "Qual é a capital do Marrocos?", options: ["Casablanca", "Rabat", "Argel", "Cairo"], correct: 1, explanation: "Rabat é a capital do Marrocos; Casablanca é sua maior cidade." },
    Question { prompt: "Qual é a capital da China?", options: ["Xangai", "Seul", "Pequim", "Tóquio"], correct: 2, explanation: "Pequim é a capital chinesa, no norte do país." },
];
const LESSONS: &[Lesson] = &[
    Lesson { title: "Américas", intro: "Nas Américas, as capitais nem sempre são as cidades que aparecem mais nas notícias. Vamos situar países e conhecer suas sedes de governo.", questions: AMERICA },
    Lesson { title: "Europa", intro: "Da Península Ibérica ao norte europeu, cada capital ajuda a localizar seu país no mapa.", questions: EUROPA },
    Lesson { title: "África e Ásia", intro: "África e Ásia reúnem países e paisagens muito diferentes. Comece por algumas capitais e pela localização de cada país.", questions: AFRICA_ASIA },
    Lesson { title: "Oceania e mundo", intro: "A Oceania inclui a Austrália, a Nova Zelândia e muitos países insulares do Pacífico. Vamos rever também outras regiões.", questions: MUNDO },
];

#[derive(Clone, Copy, PartialEq)]
enum View { Menu, Intro, Quiz, Done }

pub struct Geografia {
    view: View,
    lesson: usize,
    queue: Vec<usize>,
    tried: Vec<bool>,
    answer: Option<usize>,
    first_correct: usize,
    xp: u32,
    completed: Vec<usize>,
    celebration: Option<crate::aprender::Cel>,
}

fn progress_path() -> std::path::PathBuf { crate::home_dir().join("data/geografia.txt") }
fn read_completed(text: &str) -> Vec<usize> {
    let mut v: Vec<usize> = text.lines().filter_map(|s| s.parse::<usize>().ok()).filter(|&i| i < LESSONS.len()).collect();
    v.sort_unstable(); v.dedup(); v
}

impl Geografia {
    pub fn new() -> Self {
        let completed = read_completed(&std::fs::read_to_string(progress_path()).unwrap_or_default());
        Self { view: View::Menu, lesson: 0, queue: vec![], tried: vec![], answer: None, first_correct: 0, xp: 0, completed, celebration: None }
    }

    /// Cenas de exemplo independentes dos arquivos de progresso do usuário.
    #[cfg(test)]
    fn cena_print(kind: u8) -> Self {
        let mut g = Self { view: View::Menu, lesson: 0, queue: vec![], tried: vec![], answer: None, first_correct: 0, xp: 0, completed: vec![1], celebration: None };
        if kind > 0 { g.start(0); g.view = View::Quiz; }
        if kind == 2 { g.answer(0); }
        if kind == 3 { g.view = View::Done; g.queue.clear(); g.first_correct = 6; g.xp = 15; }
        if kind == 4 { g.view = View::Intro; }
        g
    }
    #[cfg(test)]
    pub fn print_menu() -> Self { Self::cena_print(0) }
    #[cfg(test)]
    pub fn print_questao() -> Self { Self::cena_print(1) }
    #[cfg(test)]
    pub fn print_feedback() -> Self { Self::cena_print(2) }
    #[cfg(test)]
    pub fn print_fim() -> Self { Self::cena_print(3) }
    #[cfg(test)]
    pub fn print_intro() -> Self { Self::cena_print(4) }

    /// Exercita uma lição inteira no perfil temporário do teste de telas.
    #[cfg(test)]
    pub fn verificar_registro() {
        let mut g = Self::new();
        g.start(0);
        g.advance();
        while g.view == View::Quiz {
            let correct = LESSONS[g.lesson].questions[g.queue[0]].correct;
            g.answer(correct);
            g.advance();
        }
        assert!(g.completed.contains(&0));
        assert!(Self::new().completed.contains(&0));
        assert!(crate::aprender::topic_streak(&["geografia"]).1);
    }

    fn start(&mut self, lesson: usize) {
        if lesson >= LESSONS.len() { return; }
        self.lesson = lesson;
        self.queue = (0..LESSONS[lesson].questions.len()).collect();
        self.tried = vec![false; self.queue.len()];
        self.answer = None; self.first_correct = 0; self.xp = 0; self.view = View::Intro;
    }

    fn answer(&mut self, answer: usize) {
        if self.answer.is_some() || self.view != View::Quiz || answer >= 4 || self.queue.is_empty() { return; }
        let i = self.queue[0];
        if !self.tried[i] && answer == LESSONS[self.lesson].questions[i].correct { self.first_correct += 1; }
        self.tried[i] = true;
        self.answer = Some(answer);
    }

    fn advance(&mut self) {
        if self.view == View::Intro { self.view = View::Quiz; return; }
        if self.view != View::Quiz { return; }
        let Some(answer) = self.answer.take() else { return };
        let i = self.queue.remove(0);
        if answer != LESSONS[self.lesson].questions[i].correct { self.queue.push(i); }
        if self.queue.is_empty() {
            self.xp = lesson_xp(self.first_correct, LESSONS[self.lesson].questions.len());
            if !self.completed.contains(&self.lesson) {
                self.completed.push(self.lesson); self.completed.sort_unstable();
                let path = progress_path();
                if let Some(dir) = path.parent() { let _ = std::fs::create_dir_all(dir); }
                let _ = std::fs::write(path, self.completed.iter().map(|i| format!("{i}\n")).collect::<String>());
            }
            self.celebration = crate::aprender::add(self.xp, "geografia");
            self.view = View::Done;
        }
    }

    fn row(k: usize) -> Rect { Rect::new(MARGIN, 550 + k as i32 * 148, W - 2 * MARGIN, 126) }
    fn option(k: usize) -> Rect { Rect::new(MARGIN, 550 + k as i32 * 108, W - 2 * MARGIN, 96) }
    fn wide() -> Rect { Rect::new(MARGIN, 1288, W - 2 * MARGIN, 80) }

    fn map(&self, fb: &mut Framebuffer, f: &Fonts) {
        f.draw(fb, Face::Strong, 25.0, MARGIN, 790, MUTED, "ESQUEMA DOS CONTINENTES");
        let areas = [
            ("Américas", Rect::new(108, 840, 210, 116), self.lesson == 0),
            ("Europa", Rect::new(370, 840, 190, 70), self.lesson == 1),
            ("África", Rect::new(370, 934, 190, 108), self.lesson == 2),
            ("Ásia", Rect::new(598, 840, 276, 116), self.lesson == 2),
            ("Oceania", Rect::new(700, 980, 190, 70), self.lesson == 3),
        ];
        for (label, r, active) in areas {
            ui::card(fb, r, 20, if active { INK } else { PAPER }, false);
            let size = 27.0;
            let width = f.measure(Face::Strong, size, label);
            f.draw(fb, Face::Strong, size, r.x + (r.w - width) / 2, r.y + r.h / 2 + 9, if active { PAPER } else { INK }, label);
        }
    }

    fn menu(&self, fb: &mut Framebuffer, f: &Fonts) {
        minigame::draw_menu(fb, f, "Geografia", "Capitais e países pelo mundo.", &[]);
        let (n, done) = crate::aprender::topic_streak(&["geografia"]);
        f.draw(fb, Face::Strong, 29.0, MARGIN, 490, MUTED, &format!("{n} dia{} de prática{}", if n == 1 { "" } else { "s" }, if done { " · hoje feito" } else { "" }));
        for (k, lesson) in LESSONS.iter().enumerate() {
            let r = Self::row(k);
            ui::card(fb, r, 32, PAPER, false);
            f.draw(fb, Face::Display, 39.0, r.x + 28, r.y + 53, INK, lesson.title);
            f.draw(fb, Face::Body, 25.0, r.x + 30, r.y + 96, MUTED, if self.completed.contains(&k) { "Lição feita · toque para refazer" } else { "6 perguntas · toque para começar" });
        }
    }
}

impl MiniGame for Geografia {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        if let Some(c) = self.celebration.filter(|_| self.view == View::Menu) { return crate::aprender::draw_celebration(fb, f, c); }
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.view == View::Menu { self.menu(fb, f); return; }
        minigame::header(fb, f, "Geografia", LESSONS[self.lesson].title);
        match self.view {
            View::Intro => {
                f.draw(fb, Face::Display, 55.0, MARGIN, 375, INK, "Vamos começar");
                for (k, line) in crate::chat::wrap_words(f, Face::Body, 36.0, LESSONS[self.lesson].intro, W - 2 * MARGIN).iter().enumerate() {
                    f.draw(fb, Face::Body, 36.0, MARGIN, 460 + k as i32 * 52, INK, line);
                }
                self.map(fb, f);
                ui::button(fb, f, Self::wide(), "Continuar", true);
            }
            View::Quiz => {
                let i = self.queue[0];
                let q = &LESSONS[self.lesson].questions[i];
                let done = q_count(self.lesson) - self.queue.len();
                f.draw(fb, Face::Strong, 27.0, MARGIN, 285, MUTED, &format!("{done} de {} aprendidas{}", q_count(self.lesson), if self.tried[i] && self.answer.is_none() { " · tente de novo" } else { "" }));
                for (k, line) in crate::chat::wrap_words(f, Face::Display, 46.0, q.prompt, W - 2 * MARGIN).iter().take(3).enumerate() {
                    f.draw(fb, Face::Display, 46.0, MARGIN, 360 + k as i32 * 58, INK, line);
                }
                for (k, option) in q.options.iter().enumerate() {
                    let r = Self::option(k);
                    let correct = self.answer.is_some() && k == q.correct;
                    ui::button(fb, f, r, option, correct);
                    if correct { crate::aprender::badge(fb, r, true, true); }
                    else if self.answer == Some(k) { crate::aprender::badge(fb, r, false, false); }
                }
                if let Some(a) = self.answer {
                    let right = a == q.correct;
                    let title = if right { "Correto!".to_string() } else { format!("Resposta certa: {}", q.options[q.correct]) };
                    crate::aprender::draw_feedback(fb, f, right, &title, q.explanation);
                }
            }
            View::Done => {
                f.draw(fb, Face::Display, 94.0, MARGIN - 4, 380, INK, "Lição feita!");
                f.draw(fb, Face::Display, 76.0, MARGIN, 510, INK, &format!("+{} XP", self.xp));
                f.draw(fb, Face::Strong, 34.0, MARGIN, 590, MUTED, &format!("{} de {} certas de primeira", self.first_correct, q_count(self.lesson)));
                let status = crate::aprender::status();
                crate::aprender::draw_panel(fb, f, Rect::new(MARGIN, 730, W - 2 * MARGIN, 180), &status);
                crate::aprender::week_row(fb, f, MARGIN, 990, W - 2 * MARGIN, &status);
                ui::button(fb, f, Self::wide(), "Continuar", true);
            }
            View::Menu => {}
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.celebration.is_some() && self.view == View::Menu { return vec![(999, crate::aprender::cel_button())]; }
        match self.view {
            View::Menu => (0..LESSONS.len()).map(|k| (900 + k as u32, Self::row(k))).collect(),
            View::Intro | View::Done => vec![(800, Self::wide())],
            View::Quiz if self.answer.is_some() => vec![(800, crate::aprender::feedback_button())],
            View::Quiz => (0..4).map(|k| (k as u32, Self::option(k))).collect(),
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        if self.celebration.is_some() && self.view == View::Menu {
            if id == 999 { self.celebration = None; return Redraw::Full; }
            return Redraw::None;
        }
        match (self.view, id) {
            (View::Menu, 900..=903) => self.start((id - 900) as usize),
            (View::Intro | View::Quiz, 800) => self.advance(),
            (View::Quiz, 0..=3) => self.answer(id as usize),
            (View::Done, 800) => self.view = View::Menu,
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn back(&mut self) -> bool {
        if self.celebration.take().is_some() && self.view == View::Menu { return true; }
        if self.view != View::Menu { self.view = View::Menu; self.celebration = None; return true; }
        false
    }
    fn at_menu(&self) -> bool { self.view == View::Menu && self.celebration.is_none() }
    fn summary(&self) -> String { if self.view == View::Menu { String::new() } else { format!("Geografia · {}", LESSONS[self.lesson].title) } }
}

fn q_count(lesson: usize) -> usize { LESSONS[lesson].questions.len() }
fn lesson_xp(first_correct: usize, total: usize) -> u32 { 10 + if first_correct == total { 5 } else { 0 } }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cenas_e_xp_representam_etapas_da_licao() {
        let menu = Geografia::print_menu();
        let intro = Geografia::print_intro();
        let question = Geografia::print_questao();
        let feedback = Geografia::print_feedback();
        let done = Geografia::print_fim();
        assert!(menu.view == View::Menu && intro.view == View::Intro && question.view == View::Quiz && question.answer.is_none());
        assert_eq!(feedback.answer, Some(0));
        assert!(done.view == View::Done && done.queue.is_empty());
        assert_eq!(lesson_xp(6, 6), 15);
        assert_eq!(lesson_xp(5, 6), 10);
    }
    #[test]
    fn perguntas_tem_quatro_opcoes_distintas_e_uma_resposta() {
        for lesson in LESSONS {
            assert_eq!(lesson.questions.len(), 6);
            for q in lesson.questions {
                assert!(q.correct < 4 && !q.prompt.is_empty() && !q.explanation.is_empty());
                for i in 0..4 { for j in i + 1..4 { assert_ne!(q.options[i], q.options[j]); } }
            }
        }
        assert_eq!(AMERICA[0].options[AMERICA[0].correct], "Buenos Aires");
        assert_eq!(MUNDO[0].options[MUNDO[0].correct], "Canberra");
        assert_eq!(AFRICA_ASIA[4].options[AFRICA_ASIA[4].correct], "África");
    }
    #[test]
    fn erro_volta_para_o_fim_e_acerto_conta_uma_vez() {
        let mut g = Geografia::new();
        g.start(0); g.advance();
        g.answer(0); g.advance();
        assert_eq!(g.queue, vec![1, 2, 3, 4, 5, 0]);
        assert_eq!(g.first_correct, 0);
        for _ in 0..5 {
            let q = g.queue[0]; g.answer(AMERICA[q].correct); g.advance();
        }
        assert_eq!(g.queue, vec![0]);
        g.answer(AMERICA[0].correct);
        assert_eq!(g.first_correct, 5);
        // Não avançamos a última resposta: isso gravaria XP real no perfil do usuário.
    }
    #[test]
    fn progresso_ignora_linhas_invalidas_e_repetidas() {
        assert_eq!(read_completed("2\n1\nx\n1\n99\n"), vec![1, 2]);
    }
}
