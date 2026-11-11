//! Palavra do dia: adivinhe a palavra de 5 letras em 6 tentativas. A do dia é a mesma o dia todo;
//! "Treino" sorteia outra. Acentos não contam na comparação. No e-ink: letra certa no lugar = bloco
//! preto; letra na palavra mas em outro lugar = cinza; fora = contorno claro.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

/// Lista própria de palavras comuns de 5 letras (sem depender de lista de terceiros).
const WORDS: &[&str] = &[
    "amigo", "abrir", "acaso", "achar", "afeto", "agora", "aguia", "ainda", "alegre", "algum", "alma", "altar", "amado", "amora", "andar", "anjos",
    "antes", "apoio", "arroz", "aroma", "astro", "atlas", "atual", "aviao", "baile", "banho", "barco", "beijo", "bolsa", "brisa", "bruxa", "cacau",
    "caixa", "calma", "campo", "canto", "carta", "casal", "causa", "certo", "chave", "chuva", "cinco", "circo", "claro", "clima", "coisa", "conto",
    "copos", "corda", "corpo", "couro", "crise", "culpa", "curso", "dança", "dente", "desde", "doce", "drama", "duplo", "escola", "etapa", "falar",
    "falta", "farol", "fazer", "febre", "feliz", "festa", "fibra", "filho", "final", "firme", "flor", "folha", "fonte", "forca", "forma", "forno",
    "forte", "fraco", "frase", "fruta", "fundo", "garfo", "gato", "geral", "gesto", "girar", "globo", "gosto", "graça", "grama", "grato", "gripe",
    "grito", "grupo", "guiar", "haver", "heroi", "hotel", "humor", "ideia", "igual", "ilhas", "irmao", "janta", "jeito", "jogar", "jovem", "juizo",
    "junto", "justo", "lapis", "largo", "leite", "lenda", "lento", "letra", "leve", "limao", "limpo", "linha", "livro", "lousa", "lugar", "luzes",
    "macio", "magia", "maior", "manha", "manso", "marco", "massa", "medir", "meiga", "melao", "menor", "mente", "mesmo", "metal", "metro", "mexer",
    "milho", "mirar", "moeda", "molde", "monte", "moral", "morno", "mundo", "museu", "musgo", "nadar", "naipe", "navio", "negro", "neve", "noite",
    "norte", "nossa", "novel", "nuvem", "obter", "olhar", "ombro", "ordem", "orgao", "ouvir", "padre", "palco", "papel", "parar", "parte", "passo",
    "pasta", "patio", "pedra", "peixe", "pente", "perto", "piano", "pilha", "pinho", "pista", "plano", "plena", "poder", "poema", "ponte", "porta",
    "prato", "praia", "prazo", "preto", "primo", "prova", "pular", "punho", "quase", "queda", "quilo", "radio", "raiva", "ramal", "rapaz", "regra",
    "reino", "remar", "renda", "ritmo", "rocha", "roupa", "rumor", "saber", "saida", "salto", "samba", "santo", "selva", "senha", "serra", "sinal",
    "sobre", "sonho", "sorte", "suave", "subir", "sujar", "susto", "tarde", "tecla", "tempo", "terra", "texto", "tigre", "times", "tinta", "titio",
    "toada", "trigo", "trono", "turma", "uniao", "untar", "usina", "vagar", "valor", "vapor", "vazio", "velho", "vento", "verao", "verde", "verso",
    "vidro", "vigor", "vinho", "viola", "virar", "visao", "vista", "viver", "votar", "vulto", "zebra", "zerar",
];

fn fold(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' => 'a',
        'é' | 'ê' => 'e',
        'í' => 'i',
        'ó' | 'ô' | 'õ' => 'o',
        'ú' | 'ü' => 'u',
        'ç' => 'c',
        _ => c,
    }
}

/// Palavras válidas da lista (5 letras depois de tirar acentos).
fn pool() -> Vec<String> {
    WORDS.iter().map(|w| w.chars().map(fold).collect::<String>()).filter(|w| w.chars().count() == 5).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Right,
    Present,
    Absent,
}

/// Resultado de uma tentativa (com letras repetidas contadas certo).
pub fn score(guess: &str, answer: &str) -> [Hit; 5] {
    let g: Vec<char> = guess.chars().collect();
    let a: Vec<char> = answer.chars().collect();
    let mut out = [Hit::Absent; 5];
    let mut left: Vec<Option<char>> = a.iter().map(|&c| Some(c)).collect();
    for i in 0..5 {
        if g[i] == a[i] {
            out[i] = Hit::Right;
            left[i] = None;
        }
    }
    for i in 0..5 {
        if out[i] == Hit::Right {
            continue;
        }
        if let Some(j) = (0..5).find(|&j| left[j] == Some(g[i])) {
            out[i] = Hit::Present;
            left[j] = None;
        }
    }
    out
}

pub struct Palavra {
    menu: bool,
    daily: bool,
    answer: String,
    guesses: Vec<String>,
    typing: String,
    msg: String,
}

const KEYS: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];

impl Palavra {
    pub fn new() -> Self {
        Palavra { menu: true, daily: true, answer: String::new(), guesses: vec![], typing: String::new(), msg: String::new() }
    }

    fn start(&mut self, daily: bool) {
        let p = pool();
        let idx = if daily {
            let day = crate::data::today(-240) as usize;
            day.wrapping_mul(2654435761) % p.len()
        } else {
            (Rng::new().next() % p.len() as u64) as usize
        };
        *self = Palavra { menu: false, daily, answer: p[idx].clone(), guesses: vec![], typing: String::new(), msg: "Digite uma palavra de 5 letras.".into() };
    }

    fn done(&self) -> bool {
        self.guesses.last().is_some_and(|g| *g == self.answer) || self.guesses.len() == 6
    }

    fn tile(r: usize, c: usize) -> Rect {
        let s = 128;
        let x0 = (W - 5 * s - 4 * 12) / 2;
        Rect::new(x0 + c as i32 * (s + 12), 270 + r as i32 * (s + 10), s, s)
    }

    fn key(row: usize, k: usize) -> Rect {
        let w = 88;
        let n = KEYS[row].len() as i32;
        let x0 = (W - n * w - (n - 1) * 8) / 2 + if row == 2 { 0 } else { 0 };
        Rect::new(x0 + k as i32 * (w + 8), 1100 + row as i32 * 106, w, 96)
    }

    fn enter_rect() -> Rect {
        Rect::new(MARGIN - 50, 1100 + 2 * 106, 150, 96)
    }

    fn back_rect() -> Rect {
        Rect::new(W - MARGIN - 100, 1100 + 2 * 106, 150, 96)
    }

    /// Melhor informação conhecida de cada letra (para pintar o teclado).
    fn letter_state(&self, ch: char) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        for g in &self.guesses {
            for (i, h) in score(g, &self.answer).iter().enumerate() {
                if g.chars().nth(i) == Some(ch) {
                    best = match (best, h) {
                        (Some(Hit::Right), _) | (_, Hit::Right) => Some(Hit::Right),
                        (Some(Hit::Present), _) | (_, Hit::Present) => Some(Hit::Present),
                        _ => Some(Hit::Absent),
                    };
                }
            }
        }
        best
    }

    fn draw_tile(fb: &mut Framebuffer, f: &Fonts, r: Rect, ch: Option<char>, hit: Option<Hit>) {
        let fg = match hit {
            Some(Hit::Right) => {
                fb.round_rect(r, 14, INK);
                PAPER
            }
            Some(Hit::Present) => {
                fb.round_rect(r, 14, 0x99);
                PAPER
            }
            Some(Hit::Absent) => {
                fb.round_outline(r, 14, 3, 0xBB, 0xF0);
                MUTED
            }
            None => {
                fb.round_outline(r, 14, if ch.is_some() { 5 } else { 3 }, if ch.is_some() { INK } else { 0xBB }, PAPER);
                INK
            }
        };
        if let Some(c) = ch {
            let t = c.to_uppercase().to_string();
            let tw = f.measure(Face::Display, 70.0, &t);
            f.draw(fb, Face::Display, 70.0, r.x + (r.w - tw) / 2, r.y + r.h / 2 + 25, fg, &t);
        }
    }
}

impl MiniGame for Palavra {
    fn at_menu(&self) -> bool {
        self.menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu {
            minigame::draw_menu(fb, f, "Palavra", "Adivinhe a palavra de 5 letras em 6 tentativas.", &["Palavra do dia", "Treino (palavra sorteada)"]);
            return;
        }
        let status = if self.guesses.last().is_some_and(|g| *g == self.answer) {
            format!("Acertou em {} de 6!", self.guesses.len())
        } else if self.guesses.len() == 6 {
            format!("Era {}.", self.answer.to_uppercase())
        } else {
            self.msg.clone()
        };
        minigame::header(fb, f, if self.daily { "Palavra do dia" } else { "Palavra · treino" }, &status);
        for r in 0..6 {
            for c in 0..5 {
                let (ch, hit) = if let Some(g) = self.guesses.get(r) {
                    (g.chars().nth(c), Some(score(g, &self.answer)[c]))
                } else if r == self.guesses.len() {
                    (self.typing.chars().nth(c), None)
                } else {
                    (None, None)
                };
                Self::draw_tile(fb, f, Self::tile(r, c), ch, hit);
            }
        }
        if self.done() {
            minigame::pill(fb, f, minigame::bottom_button(0, 1), "Outra partida", true);
            return;
        }
        for (row, keys) in KEYS.iter().enumerate() {
            for (k, ch) in keys.chars().enumerate() {
                let mut r = Self::key(row, k);
                if row == 2 {
                    r.x += 20;
                }
                let st = self.letter_state(ch);
                let fg = match st {
                    Some(Hit::Right) => {
                        fb.round_rect(r, 14, INK);
                        PAPER
                    }
                    Some(Hit::Present) => {
                        fb.round_rect(r, 14, 0x99);
                        PAPER
                    }
                    Some(Hit::Absent) => {
                        fb.round_outline(r, 14, 2, 0xCC, PAPER);
                        0xBB
                    }
                    None => {
                        fb.round_outline(r, 14, 3, INK, PAPER);
                        INK
                    }
                };
                let t = ch.to_uppercase().to_string();
                let tw = f.measure(Face::Strong, 40.0, &t);
                f.draw(fb, Face::Strong, 40.0, r.x + (r.w - tw) / 2, r.y + 64, fg, &t);
            }
        }
        minigame::pill(fb, f, Self::enter_rect(), "OK", true);
        minigame::pill(fb, f, Self::back_rect(), "Apagar", false);
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu {
            return (0..2).map(|k| (900 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.done() {
            return vec![(803, minigame::bottom_button(0, 1))];
        }
        let mut v = vec![(801, Self::enter_rect()), (802, Self::back_rect())];
        for (row, keys) in KEYS.iter().enumerate() {
            for (k, ch) in keys.chars().enumerate() {
                let mut r = Self::key(row, k);
                if row == 2 {
                    r.x += 20;
                }
                v.push((ch as u32, r));
            }
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 | 901 => {
                self.start(id == 900);
                return Redraw::Full;
            }
            803 => {
                self.menu = true;
                return Redraw::Full;
            }
            801 => {
                if self.typing.chars().count() < 5 {
                    self.msg = "Faltam letras.".into();
                } else {
                    self.guesses.push(std::mem::take(&mut self.typing));
                    self.msg = format!("Tentativa {} de 6.", self.guesses.len() + 1);
                    if self.done() {
                        return Redraw::Full;
                    }
                }
            }
            802 => {
                self.typing.pop();
            }
            c => {
                if let Some(ch) = char::from_u32(c) {
                    if self.typing.chars().count() < 5 {
                        self.typing.push(ch);
                    }
                }
            }
        }
        Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pontua_com_letras_repetidas() {
        use Hit::*;
        assert_eq!(score("arara", "barco"), [Present, Present, Absent, Absent, Absent]);
        assert_eq!(score("barco", "barco"), [Right; 5]);
        assert!(pool().len() > 200 && pool().iter().all(|w| w.chars().count() == 5));
    }
}
