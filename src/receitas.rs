//! Receitas para a cozinha: lista, ingredientes para marcar e preparo um passo por tela, em letra
//! grande. A tela não apaga enquanto uma receita está aberta.
//!
//! Formato (`/mnt/us/verso-ink/data/receitas/*.txt`):
//! ```text
//! # Brigadeiro
//! Rende: 25 docinhos · 30 min
//! ## Ingredientes
//! - 1 lata de leite condensado
//! ## Preparo
//! 1. Leve tudo ao fogo baixo…
//! ```

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

pub struct Recipe {
    pub title: String,
    pub info: String,
    pub ingredients: Vec<String>,
    pub steps: Vec<String>,
}

pub fn parse(src: &str) -> Recipe {
    let mut r = Recipe { title: String::new(), info: String::new(), ingredients: vec![], steps: vec![] };
    let mut section = 0;
    for line in src.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if let Some(t) = line.strip_prefix("## ") {
            section = if t.to_lowercase().starts_with("ingred") { 1 } else { 2 };
        } else if let Some(t) = line.strip_prefix("# ") {
            r.title = t.to_string();
        } else if section == 1 {
            r.ingredients.push(line.trim_start_matches(['-', '*', '•', ' ']).to_string());
        } else if section == 2 {
            let t = line.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')' || c == ' ' || c == '-');
            r.steps.push(t.to_string());
        } else if r.info.is_empty() && !r.title.is_empty() {
            r.info = line.to_string();
        }
    }
    r
}

fn dir() -> std::path::PathBuf {
    crate::home_dir().join("data/receitas")
}

/// Receitas guardadas; na primeira vez, copia os exemplos que vêm com o Verso Ink.
pub fn load_all() -> Vec<(std::path::PathBuf, Recipe)> {
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
        .filter(|(_, r)| !r.title.is_empty())
        .collect();
    v.sort_by(|a, b| a.1.title.cmp(&b.1.title));
    v
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    List,
    Overview(usize),
    Step(usize, usize),
}

pub struct Receitas {
    list: Vec<(std::path::PathBuf, Recipe)>,
    view: View,
    checked: Vec<bool>,
    page: usize,
}

const PER_PAGE: usize = 6;

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

impl Receitas {
    pub fn new() -> Self {
        Receitas { list: load_all(), view: View::List, checked: vec![], page: 0 }
    }

    fn row(i: usize) -> Rect {
        Rect::new(MARGIN - 16, 430 + i as i32 * 130, W - 2 * MARGIN + 32, 126)
    }

    fn ing_row(i: usize) -> Rect {
        Rect::new(MARGIN - 16, 430 + i as i32 * 76, W - 2 * MARGIN + 32, 72)
    }
}

impl MiniGame for Receitas {
    fn context(&self) -> String {
        let show = |k: usize| self.list.get(k).map(|(_, r)| format!("Receita: {} ({})\nIngredientes:\n{}\nPreparo:\n{}", r.title, r.info, r.ingredients.join("\n"), r.steps.iter().enumerate().map(|(i, s)| format!("{}. {s}", i + 1)).collect::<Vec<_>>().join("\n")));
        match self.view {
            View::List => format!("Lista de receitas: {}", self.list.iter().map(|(_, r)| r.title.as_str()).collect::<Vec<_>>().join(", ")),
            View::Overview(k) => show(k).unwrap_or_default(),
            View::Step(k, st) => format!("{}\nNo passo {} agora.", show(k).unwrap_or_default(), st + 1),
        }
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::List => {
                f.draw(fb, Face::Display, 130.0, MARGIN - 6, 310, INK, "Receitas");
                let sub = if self.list.is_empty() { "Nenhuma receita. Mande pelo PC (tools/receita.sh) ou peça ao Claude." } else { "A tela não apaga com uma receita aberta" };
                f.draw(fb, Face::Strong, 28.0, MARGIN, 380, MUTED, sub);
                for (i, (_, r)) in self.list.iter().skip(self.page * PER_PAGE).take(PER_PAGE).enumerate() {
                    let row = Self::row(i);
                    fb.fill(Rect::new(MARGIN, row.y, W - 2 * MARGIN, 3), INK);
                    f.draw(fb, Face::Display, 50.0, MARGIN, row.y + 66, INK, &crate::apps::fit(f, Face::Display, 50.0, &r.title, W - 2 * MARGIN));
                    let info = if r.info.is_empty() { format!("{} passos", r.steps.len()) } else { r.info.clone() };
                    f.draw(fb, Face::Body, 28.0, MARGIN + 2, row.y + 106, MUTED, &crate::apps::fit(f, Face::Body, 28.0, &info, W - 2 * MARGIN));
                }
                if self.list.len() > PER_PAGE {
                    minigame::pill(fb, f, minigame::bottom_button(0, 1), "Mais receitas", false);
                }
            }
            View::Overview(k) => {
                let r = &self.list[k].1;
                f.draw(fb, Face::Display, 72.0, MARGIN, 260, INK, &crate::apps::fit(f, Face::Display, 72.0, &r.title, W - 2 * MARGIN));
                f.draw(fb, Face::Body, 30.0, MARGIN, 320, MUTED, &r.info);
                f.draw(fb, Face::Strong, 28.0, MARGIN, 400, MUTED, "INGREDIENTES · TOQUE PARA MARCAR");
                for (i, ing) in r.ingredients.iter().enumerate().take(10) {
                    let row = Self::ing_row(i);
                    let on = self.checked.get(i).copied().unwrap_or(false);
                    let b = Rect::new(MARGIN, row.y + 16, 40, 40);
                    if on {
                        fb.round_rect(b, 8, INK);
                        ui::stroke(fb, (b.x as f32 + 9.0, b.y as f32 + 21.0), (b.x as f32 + 17.0, b.y as f32 + 30.0), 5.0, PAPER);
                        ui::stroke(fb, (b.x as f32 + 17.0, b.y as f32 + 30.0), (b.x as f32 + 31.0, b.y as f32 + 11.0), 5.0, PAPER);
                    } else {
                        fb.round_outline(b, 8, 4, INK, PAPER);
                    }
                    f.draw(fb, Face::Body, 34.0, MARGIN + 62, row.y + 48, if on { MUTED } else { INK }, &crate::apps::fit(f, Face::Body, 34.0, ing, W - 2 * MARGIN - 70));
                }
                minigame::pill(fb, f, minigame::bottom_button(0, 1), &format!("Começar o preparo · {} passos", r.steps.len()), true);
            }
            View::Step(k, s) => {
                let r = &self.list[k].1;
                f.draw(fb, Face::Strong, 30.0, MARGIN, 230, MUTED, &format!("{} · PASSO {} DE {}", r.title.to_uppercase(), s + 1, r.steps.len()));
                // Barra de progresso dos passos.
                let bar = Rect::new(MARGIN, 260, W - 2 * MARGIN, 16);
                fb.round_outline(bar, 8, 2, INK, PAPER);
                let w = (bar.w * (s as i32 + 1) / r.steps.len().max(1) as i32).max(16);
                fb.round_rect(Rect::new(bar.x, bar.y, w, bar.h), 8, INK);
                let text = r.steps.get(s).cloned().unwrap_or_default();
                // Letra grande; diminui só se o passo for muito longo.
                let mut size = 60.0;
                let mut lines = wrap(f, Face::Body, size, &text, W - 2 * MARGIN);
                while lines.len() as f32 * size * 1.35 > 900.0 && size > 36.0 {
                    size -= 4.0;
                    lines = wrap(f, Face::Body, size, &text, W - 2 * MARGIN);
                }
                for (n, l) in lines.iter().enumerate() {
                    f.draw(fb, Face::Body, size, MARGIN, 400 + n as i32 * (size * 1.35) as i32, INK, l);
                }
                let last = s + 1 == r.steps.len();
                minigame::pill(fb, f, minigame::bottom_button(0, 2), "‹ Anterior", false);
                minigame::pill(fb, f, minigame::bottom_button(1, 2), if last { "Pronto!" } else { "Próximo ›" }, true);
            }
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.view {
            View::List => {
                let mut v: Vec<(u32, Rect)> = (0..self.list.len().saturating_sub(self.page * PER_PAGE).min(PER_PAGE)).map(|i| (i as u32, Self::row(i))).collect();
                if self.list.len() > PER_PAGE {
                    v.push((800, minigame::bottom_button(0, 1)));
                }
                v
            }
            View::Overview(k) => {
                let mut v: Vec<(u32, Rect)> = (0..self.list[k].1.ingredients.len().min(10)).map(|i| (100 + i as u32, Self::ing_row(i))).collect();
                v.push((801, minigame::bottom_button(0, 1)));
                v
            }
            View::Step(..) => vec![(802, minigame::bottom_button(0, 2)), (803, minigame::bottom_button(1, 2))],
        }
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match (self.view, id) {
            (View::List, 800) => self.page = (self.page + 1) % self.list.len().div_ceil(PER_PAGE),
            (View::List, i) => {
                let k = self.page * PER_PAGE + i as usize;
                self.checked = vec![false; self.list[k].1.ingredients.len()];
                self.view = View::Overview(k);
            }
            (View::Overview(_), i @ 100..=199) => {
                if let Some(c) = self.checked.get_mut((i - 100) as usize) {
                    *c = !*c;
                }
                return Redraw::Part(Rect::new(0, 400, W, 800));
            }
            (View::Overview(k), 801) => self.view = View::Step(k, 0),
            (View::Step(k, s), 802) => self.view = if s == 0 { View::Overview(k) } else { View::Step(k, s - 1) },
            (View::Step(k, s), 803) => self.view = if s + 1 >= self.list[k].1.steps.len() { View::List } else { View::Step(k, s + 1) },
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn keep_awake(&self) -> bool {
        !matches!(self.view, View::List)
    }

    /// Voltar dentro das receitas antes de sair para o Mais.
    fn back(&mut self) -> bool {
        self.view = match self.view {
            View::List => return false,
            View::Overview(_) => View::List,
            View::Step(k, _) => View::Overview(k),
        };
        true
    }
}

const EXAMPLES: [(&str, &str); 3] = [
    (
        "brigadeiro.txt",
        "# Brigadeiro\nRende: 25 docinhos · 30 min\n## Ingredientes\n- 1 lata de leite condensado\n- 1 colher (sopa) de manteiga\n- 3 colheres (sopa) de chocolate em pó\n- Chocolate granulado para enrolar\n## Preparo\n1. Coloque o leite condensado, a manteiga e o chocolate em pó numa panela.\n2. Leve ao fogo baixo, mexendo sem parar com uma colher de pau.\n3. Cozinhe até a massa desgrudar do fundo da panela, uns 10 minutos.\n4. Passe para um prato untado com manteiga e deixe esfriar.\n5. Com as mãos untadas, enrole bolinhas e passe no granulado.\n",
    ),
    (
        "pao-de-queijo.txt",
        "# Pão de queijo\nRende: 30 unidades · 50 min\n## Ingredientes\n- 500 g de polvilho azedo\n- 1 xícara de leite\n- 1/2 xícara de óleo\n- 1 colher (chá) de sal\n- 2 ovos\n- 200 g de queijo meia cura ralado\n## Preparo\n1. Ferva o leite com o óleo e o sal.\n2. Despeje sobre o polvilho e misture até escaldar toda a massa.\n3. Espere amornar e junte os ovos, um de cada vez, sovando bem.\n4. Acrescente o queijo e sove até a massa ficar lisa.\n5. Faça bolinhas e coloque numa assadeira, com espaço entre elas.\n6. Asse em forno preaquecido a 200 °C por uns 25 minutos, até dourar.\n",
    ),
    (
        "arroz-branco.txt",
        "# Arroz branco\nRende: 4 porções · 25 min\n## Ingredientes\n- 1 xícara de arroz\n- 2 xícaras de água quente\n- 1 colher (sopa) de óleo\n- 1/2 cebola picada\n- 1 dente de alho amassado\n- Sal a gosto\n## Preparo\n1. Aqueça o óleo e doure a cebola e o alho.\n2. Junte o arroz e refogue por 2 minutos, mexendo.\n3. Acrescente a água quente e o sal.\n4. Quando ferver, abaixe o fogo, tampe parcialmente e cozinhe até secar a água.\n5. Desligue, tampe e deixe descansar 5 minutos antes de servir.\n",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn le_os_exemplos() {
        for (_, body) in EXAMPLES {
            let r = parse(body);
            assert!(!r.title.is_empty() && r.ingredients.len() >= 4 && r.steps.len() >= 5, "{}", r.title);
        }
    }
}
