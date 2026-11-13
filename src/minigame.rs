//! Base comum dos minijogos: cada jogo desenha a tela, declara alvos de toque e reage a eles.
//! O app cuida de navegação, destaque de toque, atualização do e-ink e lances da CPU.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{INK, MUTED, PAPER};

pub const MARGIN: i32 = 72;
pub const W: i32 = 1072;

/// O que redesenhar depois de um toque.
pub enum Redraw {
    None,
    /// Só uma região (atualização parcial, sem piscar).
    Part(Rect),
    /// A tela inteira.
    Full,
}

pub trait MiniGame {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts);
    /// Toques contínuos para módulos que desenham diretamente com o dedo.
    fn touch(&mut self, _touch: crate::input::Touch) -> Option<Redraw> { None }
    /// Alvos de toque (id do jogo, retângulo). O botão Início é do app.
    fn targets(&self) -> Vec<(u32, Rect)>;
    fn tap(&mut self, id: u32, x: i32, y: i32) -> Redraw;
    /// Guarda o estado para continuar depois (quando fizer sentido).
    fn save(&self) {}
    /// É a vez da CPU? O app mostra "pensando" e chama `cpu_move`.
    fn cpu_turn(&self) -> bool {
        false
    }
    fn cpu_move(&mut self) {}
    /// A tela não deve apagar (ex.: receita aberta na cozinha).
    fn keep_awake(&self) -> bool {
        false
    }
    /// Segurar o dedo numa casa (id < 800) por meio segundo. `None` = o jogo não usa segurar
    /// (aí o toque comum vale ao soltar).
    fn hold(&mut self, _id: u32) -> Option<Redraw> {
        None
    }
    /// Arrastar o dedo por casas: chamado para cada casa por onde ele passa, a primeira inclusive.
    /// `None` = o jogo não usa arrastar.
    fn drag(&mut self, _id: u32) -> Option<Redraw> {
        None
    }
    /// O estado do jogo em texto, para o Claude entender a pergunta (vazio = só o nome do jogo).
    /// Depois de um toque, o jogo pode pedir um texto ao leitor: devolve o título do pedido
    /// (o app abre o teclado e entrega o resultado em `text_input`).
    fn wants_text(&mut self) -> Option<String> {
        None
    }
    /// Texto digitado a pedido do jogo.
    fn text_input(&mut self, _text: &str) -> Redraw {
        Redraw::None
    }
    /// Quanto tempo segurar para valer como "segurar" (ms).
    fn hold_ms(&self) -> u64 {
        450
    }
    /// Chamado de tempos em tempos (relógios, cronômetros). Devolve o que redesenhar.
    fn tick(&mut self) -> Redraw {
        Redraw::None
    }
    /// Botão que apaga a partida em andamento (pede um segundo toque).
    fn risky(&self, _id: u32) -> bool {
        false
    }
    fn context(&self) -> String {
        String::new()
    }
    /// Resumo curto para o cartão "vai junto" (ex.: "Mesa: 7 azul · sua mão: 8 cartas").
    fn summary(&self) -> String {
        String::new()
    }
    /// Está no menu do jogo? Lá o app mostra o botão "Como jogar".
    fn at_menu(&self) -> bool {
        false
    }
    /// Fim do arrasto (o dedo saiu da tela).
    fn drag_end(&mut self) {}
    /// Voltou ao jogo depois de sair (a partida ficou guardada na memória).
    fn resume(&mut self) {}
    /// Voltar dentro do próprio módulo; `false` = sair para a tela anterior.
    fn back(&mut self) -> bool {
        false
    }
}

// ---------- Desenho comum ----------

pub fn pill(fb: &mut Framebuffer, f: &Fonts, r: Rect, label: &str, filled: bool) {
    crate::ui::button(fb, f, r, label, filled);
}

/// Linha de botões na base da tela (até 4).
pub fn bottom_button(k: usize, n: usize) -> Rect {
    let n = n.max(1) as i32;
    let w = (W - 2 * MARGIN - (n - 1) * 16) / n;
    Rect::new(MARGIN + k as i32 * (w + 16), 1322, w, 96)
}

pub fn header(fb: &mut Framebuffer, f: &Fonts, title: &str, status: &str) {
    let tw = f.measure(Face::Strong, 34.0, title);
    f.draw(fb, Face::Strong, 34.0, W - MARGIN - tw, 128, INK, title);
    fb.fill(Rect::new(0, 170, W, 80), PAPER);
    f.draw(fb, Face::Display, 44.0, MARGIN, 226, INK, &crate::apps::fit(f, Face::Display, 44.0, status, W - 2 * MARGIN));
}

/// Escolha de modo (contra a CPU / duas pessoas) e de nível, comum aos jogos de dois lados.
pub fn menu_row(k: usize) -> Rect {
    Rect::new(MARGIN, 470 + k as i32 * 150, W - 2 * MARGIN, 124)
}

pub fn draw_menu(fb: &mut Framebuffer, f: &Fonts, title: &str, subtitle: &str, options: &[&str]) {
    fb.fill(Rect::new(0, 170, W, 1448 - 170), PAPER);
    f.draw(fb, Face::Display, 130.0, MARGIN - 6, 330, INK, title);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 410, MUTED, subtitle);
    for (k, o) in options.iter().enumerate() {
        pill(fb, f, menu_row(k), o, k == 0);
    }
}

/// A ilustração do jogo num cartão, ocupando o espaço livre do menu (de `top` até o rodapé).
pub fn menu_art(fb: &mut Framebuffer, title: &str, top: i32) {
    menu_art_ate(fb, title, top, 1390);
}

/// Ilustração com limite inferior escolhido para telas que reservam margem maior.
pub fn menu_art_ate(fb: &mut Framebuffer, title: &str, top: i32, bottom: i32) {
    let h = bottom - top;
    if h < 200 {
        return;
    }
    let name = format!("icone-{}", crate::apps::fold(title).replace(' ', ""));
    if !crate::sprites::exists(&name) {
        return;
    }
    let r = Rect::new(MARGIN, top, W - 2 * MARGIN, h);
    crate::ui::card(fb, r, 32, crate::ui::SURFACE, false);
    let d = (h - 44).min(300);
    crate::sprites::draw(fb, &name, Rect::new(r.x + (r.w - d) / 2, r.y + (h - d) / 2, d, d));
}

/// Dica curta acima dos botões do rodapé, com folga para não grudar neles.
pub fn hint(fb: &mut Framebuffer, f: &Fonts, text: &str) {
    let w = f.measure(Face::Body, 26.0, text);
    f.draw(fb, Face::Body, 26.0, (W - w) / 2, 1278, crate::ui::MUTED, text);
}

/// Grade quadrada centrada: (x0, y0, lado da casa).
pub fn grid_geom(cols: i32, rows: i32, top: i32, max_h: i32) -> (i32, i32, i32) {
    let cell = ((W - 2 * 40) / cols).min(max_h / rows);
    let x0 = (W - cell * cols) / 2;
    (x0, top, cell)
}

/// Gerador simples para os jogos (xorshift do Sudoku).
pub use crate::sudoku::Rng;

/// Cria o minijogo pelo índice da tela Jogar (None para Sudoku e Xadrez, que são telas do app).
pub fn make(i: usize) -> Option<Box<dyn MiniGame>> {
    Some(match i {
        2 => Box::new(crate::coroas::Coroas::new()),
        3 => Box::new(crate::lig4::Lig4::new()),
        4 => Box::new(crate::caixas::Caixas::new()),
        5 => Box::new(crate::naval::Naval::new()),
        6 => Box::new(crate::minas::Minas::new()),
        7 => Box::new(crate::nonograma::Nonograma::new()),
        8 => Box::new(crate::uno::Uno::new()),
        9 => Box::new(crate::palavra::Palavra::new()),
        10 => Box::new(crate::sokoban::Sokoban::new()),
        11 => Box::new(crate::paciencia::Paciencia::new()),
        12 => Box::new(crate::masmorra::Masmorra::new()),
        13 => Box::new(crate::cripta::Cripta::new()),
        14 => Box::new(crate::historias::Historias::new()),
        15 => Box::new(crate::damas::Damas::new()),
        16 => Box::new(crate::velha::Velha::new()),
        17 => Box::new(crate::reversi::Reversi::new()),
        18 => Box::new(crate::go::Go::new()),
        19 => Box::new(crate::cruzadas::Cruzadas::new()),
        _ => return None,
    })
}
