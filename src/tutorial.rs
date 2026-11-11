//! "Como jogar": tutorial curto e ilustrado de cada jogo. Abre sozinho na primeira vez e fica
//! no botão "Como jogar" do menu de cada jogo.
//!
//! Cada página tem um título, uma ilustração montada em texto e uma explicação. Na ilustração,
//! cada caractere é uma casa (`board`); `shade` (opcional, mesmo formato) pinta o fundo:
//!
//! - `.` casa vazia · `#` casa preta · `g` cinza · `~` água · ` ` sem casa
//! - `x` X · `c` coroa · `o` peça preta · `w` peça branca · `*` mina/acerto · `f` bandeira
//! - `@` você · `b` caixa · `t` alvo · `%` caixa no alvo · `p` poção · `s` espada · `$` ouro · `>` escada
//! - `+` ponto · `-` `|` linhas (Pontos e caixas) · algarismos e letras maiúsculas aparecem como texto
//! - no `shade`: `A`–`E` tons de reino, `K` letra certa, `P` letra em outro lugar, `N` fora,
//!   `M` monstro (fundo preto), `L`/`D` casas claras e escuras do tabuleiro, `R` destaque

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const MARGIN: i32 = 72;
const W: i32 = 1072;

pub struct Page {
    pub title: &'static str,
    pub text: &'static str,
    pub board: &'static str,
    pub shade: &'static str,
}

const fn p(title: &'static str, text: &'static str, board: &'static str, shade: &'static str) -> Page {
    Page { title, text, board, shade }
}

/// Páginas de cada jogo, na ordem da tela Jogar.
pub fn pages(game: usize) -> &'static [Page] {
    match game {
        0 => &SUDOKU,
        1 => &XADREZ,
        2 => &COROAS,
        3 => &LIG4,
        4 => &CAIXAS,
        5 => &NAVAL,
        6 => &MINAS,
        7 => &NONOGRAMA,
        8 => &UNO,
        9 => &PALAVRA,
        10 => &SOKOBAN,
        11 => &PACIENCIA,
        12 => &MASMORRA,
        13 => &CRIPTA,
        14 => &HISTORIAS,
        15 => &DAMAS,
        _ => &[],
    }
}

const FASES: Page = p(
    "Fases",
    "A dificuldade sobe sozinha a cada vitória. De 5 em 5 fases, a partir da 10, vem uma difícil; de 10 em 10, um desafio. Vencer ao menos uma por dia mantém a sequência. Treino livre não conta.",
    "FASES",
    "",
);

static SUDOKU: [Page; 4] = [
    p("O objetivo", "Complete a grade 9×9 com números de 1 a 9: cada linha, cada coluna e cada quadrado 3×3 tem todos eles, sem repetir.", "53..7....\n6..195...\n.98....6.", "RRR......\nRRR......\nRRR......"),
    p("Jogando", "Toque numa casa e depois num número. Errou? O erro é contado e dá para desfazer. Uma dica preenche uma casa certa.", "534678912\n672195348\n198342567", "........R"),
    p("Anotações", "Ligue Anotar para escrever candidatos pequenos numa casa. Quando você põe um número, as anotações iguais na linha, coluna e quadrado somem.", "1.3\n.5.\n7.9", ""),
    FASES,
];

static XADREZ: [Page; 4] = [
    p("Peças", "R rei, D dama, T torre, B bispo, C cavalo, P peão. Toque numa peça sua para ver as casas possíveis e depois no destino.", "TCBDRBCT\nPPPPPPPP", "LDLDLDLD\nDLDLDLDL"),
    p("Treinadores", "São 30, de 100 a 3000 de rating, com o Stockfish no Kindle. Vença um para liberar o próximo; sua pontuação estimada sobe e desce com as partidas.", "...R....\n........\n....D...", "LDLDLDLD\nDLDLDLDL\nLDLDLDLD"),
    p("Revisão", "Ao terminar, a revisão mostra cada lance seu classificado, o lance melhor quando houve um, e os momentos que decidiram a partida.", "..T.....\n.....R..\n..R.....", "LDLDLDLD\nDLRLDLDL\nLDLDLDLD"),
    p("Duas pessoas", "No jogo local, ligue Girar para a tela virar de cabeça para baixo a cada vez, para quem está do outro lado da mesa.", "PPPPPPPP\n........\nPPPPPPPP", "KKKKKKKK\nDLDLDLDL\nLDLDLDLD"),
];

static COROAS: [Page; 4] = [
    p("As regras", "Ponha uma coroa por linha, uma por coluna e uma por reino (cada área de cor). Coroas não podem se encostar, nem na diagonal.", ".c...\n...c.\nc....\n..c..\n....c", "AABBB\nACCBB\nECCFF\nEECFF\nEEEFF"),
    p("Marcar X", "Toque numa casa para marcar X onde a coroa não pode ficar. Passe o dedo por várias casas para marcar todas de uma vez.", "xxxxx\nx....\nx....\n.....\n.....", "AABBB\nACCBB\nECCFF\nEECFF\nEEEFF"),
    p("Pôr a coroa", "Segure o dedo numa casa para pôr a coroa (segure de novo para tirar). Coroa em conflito fica sublinhada.", "c....\n.....\n.....\n.....\n.....", "AABBB\nACCBB\nECCFF\nEECFF\nEEEFF"),
    FASES,
];

static LIG4: [Page; 3] = [
    p("O objetivo", "Faça quatro peças em linha: deitadas, em pé ou na diagonal. As peças caem até o fundo da coluna.", ".......\n.......\n...o...\n..ow...\n.owwo..\nowwow..", ""),
    p("Jogando", "Toque em qualquer lugar da coluna para soltar a sua peça. A setinha em cima mostra a última jogada.", ".......\n...o...\n...w...", ""),
    FASES,
];

static CAIXAS: [Page; 3] = [
    p("O objetivo", "Na sua vez, ligue dois pontos vizinhos. Quem fecha o quarto lado de uma caixa marca a caixa e joga de novo. Ganha quem tiver mais caixas.", "+-+-+\n|1| |\n+-+ +\n|   |\n+-+-+", ""),
    p("Cuidado", "Fazer o terceiro lado de uma caixa entrega ela de presente para o outro. No fim, as cadeias decidem a partida.", "+-+ +\n| |  \n+-+-+", ""),
    FASES,
];

static NAVAL: [Page; 3] = [
    p("Sua frota", "Posicione os 8 navios tocando na casa da proa de cada um. Girar muda a direção; Sortear arruma a frota por você.", "#####.....\n..........\n####..###.\n..........", "~~~~~~~~~~\n~~~~~~~~~~\n~~~~~~~~~~\n~~~~~~~~~~"),
    p("Atirando", "Toque numa casa do mar inimigo. Acertou: atire de novo. Água: a vez passa. Afunde toda a frota para vencer.", "..x.......\n..*****...\n.x........", "~~~~~~~~~~\n~~~~~~~~~~\n~~~~~~~~~~"),
    FASES,
];

static MINAS: [Page; 3] = [
    p("Os números", "Cada número diz quantas minas encostam naquela casa, contando as diagonais. Abra todas as casas sem mina.", "1f1..\n111..\n.....\n11...\ngg1..", ""),
    p("Toque e segure", "Toque abre; segure põe ou tira a bandeira (o botão embaixo inverte). O primeiro toque nunca é mina.", "gggg\ngfgg\ngggg", ""),
    FASES,
];

static NONOGRAMA: [Page; 3] = [
    p("As dicas", "Os números de cada linha e coluna dizem os blocos pintados, em ordem. \"3 1\" é um bloco de 3, um espaço ou mais, e um bloco de 1.", "   1   1\n   12311\n 31###.#\n  3.###.\n111#.#.#", "nnnnnnnn\nnnnnnnnn\nnnn.....\nnnn.....\nnnn....."),
    p("Pintando", "Toque: pinta, marca X e limpa. Arraste para fazer o mesmo em várias casas. Toda grade sai pela lógica, sem chute.", "##x..\nx#...", ""),
    FASES,
];

static UNO: [Page; 3] = [
    p("Jogando", "Jogue uma carta da mesma cor ou do mesmo número. As que servem ficam levantadas. Sem jogada, toque no monte para comprar.", "7 7 2\n. . .", "K . P"),
    p("Especiais", "+2 e +4 acumulam no próximo. Bloqueio pula a vez, Inverte troca o sentido e Troca a cor deixa você escolher. Cada cor tem padrão e símbolo próprios.", "2 4", "K P"),
    p("UNO!", "Ficou com uma carta? Toque em UNO! antes de jogar a penúltima. Quem esquece compra 2.", "U N O", "KKK"),
];

static PALAVRA: [Page; 3] = [
    p("O objetivo", "Descubra a palavra de 5 letras em 6 tentativas. A do dia é a mesma o dia todo; o Treino sorteia outra.", "PRATO\n.....", "NPNKN"),
    p("As cores", "Bloco preto: letra certa no lugar certo. Cinza: a letra está na palavra, em outro lugar. Contorno: a letra não está. Acentos não contam.", "CARTA\nCERTO", "KNKKN\nKNKKN"),
    p("Digitando", "Use o teclado na tela e toque em OK. Palavras fora da lista não valem tentativa.", "", ""),
];

static SOKOBAN: [Page; 3] = [
    p("O objetivo", "Empurre todas as caixas até os alvos. Você só empurra, não puxa, e uma caixa de cada vez.", "#######\n#.@b.t#\n#.....#\n#######", ""),
    p("Andando", "Use as setas embaixo. Prendeu uma caixa no canto? Desfazer volta um passo; Recomeçar volta a fase inteira.", "#####\n#%..#\n#.@.#\n#####", ""),
    FASES,
];

static PACIENCIA: [Page; 3] = [
    p("O objetivo", "Monte as quatro fundações de A até K, cada uma de um naipe.", "A 2 3 4", "R R R R"),
    p("As colunas", "Nas colunas, as cartas descem alternando preto e cinza: um 7 preto vai sobre um 8 cinza. Coluna vazia só recebe K.", "8 . .\n7 . .\n6 . .", "P . .\n. . .\nP . ."),
    p("Jogando", "Toque numa carta e depois no destino. Dois toques na mesma carta mandam ela para a fundação.", "", ""),
];

static MASMORRA: [Page; 3] = [
    p("O mundo", "Você é o aventureiro de capuz. Ande com as setas: o mundo só se move quando você anda. Desça cinco andares; o dragão guarda o último.", "#######\n#.@..>#\n#..R..#\n#######", ""),
    p("Lutando", "Ande para cima de um monstro para atacar. Pegue poções, espadas, escudos e ouro pelo caminho; a escada leva ao próximo andar.", "#####\n#@R.#\n#.p$#\n#####", ""),
    p("Uma vida", "Morreu, recomeça do primeiro andar: é um roguelike. Cada aventura tem um mapa novo.", "", ""),
];

static CRIPTA: [Page; 4] = [
    p("Descobrir", "Toque numa pedra clara, vizinha das casas abertas, para descobri-la. Casa vazia recupera 1 de vida.", "gggggg\ngg..gg\ng.@..g\ngg..gg", ""),
    p("Monstros", "Monstros não andam: bloqueiam a passagem. Toque num para ver quanto de vida a luta custa; toque de novo para lutar.", "g.....\n..2...\n.@.p..", "......\n..M...\n......"),
    p("Ficar forte", "Vencer dá experiência; subir de nível enche a vida e aumenta a força. Espadas aumentam a força, poções curam quando você beber.", "s..p..$", ""),
    p("Descer", "Ache a escada e toque nela para descer. Os andares ficam mais perigosos; seus pontos são os andares e o ouro.", ".....>", ""),
];

static DAMAS: [Page; 4] = [
    p("O objetivo", "Capture todas as peças do outro ou deixe-o sem lances. As peças andam só nas casas escuras, na diagonal.", ".o.o.o.o\no.o.o.o.\n........\n.w.w.w.w\nw.w.w.w.", "LDLDLDLD\nDLDLDLDL\nLDLDLDLD\nLDLDLDLD\nDLDLDLDL"),
    p("Capturar", "Pule por cima de uma peça vizinha para uma casa livre. O peão captura para a frente e para trás, e pode emendar capturas em sequência.", "...\n.o.\n..w", "LDL\nDLD\nLDL"),
    p("É obrigatório", "Se puder capturar, tem de capturar, e pela maioria: o lance que leva mais peças. As peças que podem jogar ficam marcadas.", ".o.o\no.w.", "LDLD\nDLDL"),
    p("A dama", "O peão que chega à última linha vira dama (com coroa). Ela anda e captura em diagonais longas, várias casas de uma vez.", "c...\n.o..\n....", "DLDL\nLDLD\nDLDL"),
];

static HISTORIAS: [Page; 2] = [
    p("Você escolhe", "Leia o trecho e toque numa das escolhas embaixo. Algumas só aparecem se você pegou ou descobriu algo antes. Há mais de um final.", "", ""),
    p("Novas histórias", "Escreva as suas em data/historias, em texto simples (o formato está no começo do arquivo de exemplo), ou peça ao Claude. O ponto onde você parou fica salvo.", "", ""),
];

// ---------- Visto / não visto ----------

fn seen_path(game: usize) -> std::path::PathBuf {
    let d = crate::home_dir().join("data/tutoriais");
    let _ = std::fs::create_dir_all(&d);
    d.join(format!("{game}.visto"))
}

pub fn seen(game: usize) -> bool {
    pages(game).is_empty() || seen_path(game).exists()
}

pub fn mark_seen(game: usize) {
    let _ = std::fs::write(seen_path(game), "1");
}

// ---------- Desenho ----------

pub fn help_rect() -> Rect {
    Rect::new(W - MARGIN - 260, 76, 260, 84)
}

pub fn draw_help_button(fb: &mut Framebuffer, f: &Fonts) {
    let r = help_rect();
    fb.fill(r, PAPER);
    fb.round_outline(r, r.h / 2, 4, INK, PAPER);
    let l = "Como jogar";
    let w = f.measure(Face::Strong, 32.0, l);
    f.draw(fb, Face::Strong, 32.0, r.x + (r.w - w) / 2, r.y + 54, INK, l);
}

pub fn prev_rect() -> Rect {
    Rect::new(MARGIN, 1300, 300, 100)
}
pub fn next_rect() -> Rect {
    Rect::new(W - MARGIN - 380, 1300, 380, 100)
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

pub fn draw(fb: &mut Framebuffer, f: &Fonts, game: usize, k: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let ps = pages(game);
    let Some(page) = ps.get(k) else { return };
    let name = crate::apps::GAMES.get(game).map(|g| g.0).unwrap_or("");
    let kicker = format!("COMO JOGAR {}", name.to_uppercase());
    f.draw(fb, Face::Strong, 28.0, MARGIN, 240, MUTED, &kicker);
    f.draw(fb, Face::Display, 84.0, MARGIN - 4, 330, INK, page.title);
    let mut y = 390;
    if let Some(sc) = scene(game, k) {
        y = draw_scene(fb, f, game, &sc, y) + 40;
    } else if page.board == "FASES" {
        y = draw_fases(fb, f, y) + 50;
    } else if !page.board.is_empty() {
        y = draw_board(fb, f, page, y) + 50;
    }
    for l in wrap(f, Face::Body, 38.0, page.text, W - 2 * MARGIN) {
        f.draw(fb, Face::Body, 38.0, MARGIN, y + 40, INK, &l);
        y += 58;
    }
    // Pontinhos de página.
    let n = ps.len() as i32;
    let x0 = (W - (n * 30 - 14)) / 2;
    for i in 0..n {
        let r = Rect::new(x0 + i * 30, 1250, 16, 16);
        if i == k as i32 {
            fb.round_rect(r, 8, INK);
        } else {
            fb.round_outline(r, 8, 3, INK, PAPER);
        }
    }
    let last = k + 1 == ps.len();
    let pill = |fb: &mut Framebuffer, r: Rect, l: &str, filled: bool| {
        if filled {
            fb.round_rect(r, r.h / 2, INK);
        } else {
            fb.round_outline(r, r.h / 2, 4, INK, PAPER);
        }
        let w = f.measure(Face::Strong, 36.0, l);
        f.draw(fb, Face::Strong, 36.0, r.x + (r.w - w) / 2, r.y + r.h / 2 + 13, if filled { PAPER } else { INK }, l);
    };
    pill(fb, prev_rect(), if k == 0 { "Pular" } else { "‹ Voltar" }, false);
    pill(fb, next_rect(), if last { "Jogar" } else { "Próximo ›" }, true);
}

// ---------- Cenas: o jogo de verdade, preparado por alguns toques ----------

#[derive(Clone, Copy)]
pub enum Op {
    /// Primeira opção do menu (fase 1, contra a CPU, primeira história).
    Start,
    Tap(u32),
    Hold(u32),
    Drag(&'static [u32]),
}

#[derive(Clone, Copy)]
pub enum Gesture {
    None,
    Tap(u32),
    Hold(u32),
    Drag(u32, u32),
}

pub struct Scene {
    pub ops: &'static [Op],
    pub gesture: Gesture,
}

const fn sc(ops: &'static [Op], gesture: Gesture) -> Scene {
    Scene { ops, gesture }
}

/// Cena de cada página (quando faz sentido mostrar o jogo de verdade).
fn scene(game: usize, page: usize) -> Option<Scene> {
    use Gesture as G;
    use Op::*;
    Some(match (game, page) {
        (2, 0) => sc(&[Start, Tap(802), Tap(802), Tap(802)], G::None),
        (2, 1) => sc(&[Start, Drag(&[0, 1, 2, 3])], G::Drag(0, 3)),
        (2, 2) => sc(&[Start, Hold(12)], G::Hold(12)),
        (3, 0) => sc(&[Start, Tap(3)], G::None),
        (3, 1) => sc(&[Start], G::Tap(3)),
        (4, 0) => sc(&[Start], G::Tap(0)),
        (5, 0) => sc(&[Start, Tap(0)], G::Tap(20)),
        (5, 1) => sc(&[Start, Tap(811), Tap(812), Tap(44), Tap(45)], G::Tap(46)),
        (6, 0) => sc(&[Start, Tap(27)], G::None),
        (6, 1) => sc(&[Start, Tap(27), Hold(0)], G::Hold(0)),
        (7, 0) => sc(&[Start], G::None),
        (7, 1) => sc(&[Start, Drag(&[0, 1, 2])], G::Drag(0, 2)),
        (8, 0) => sc(&[Start], G::Tap(0)),
        (10, 0) => sc(&[Start], G::None),
        (10, 1) => sc(&[Start], G::Tap(813)),
        (11, 0) | (11, 2) => sc(&[Start], G::None),
        (12, 0) | (12, 1) => sc(&[Start], G::Tap(813)),
        (13, 0) | (13, 1) => sc(&[Start], G::None),
        (14, 0) => sc(&[Tap(900)], G::Tap(0)),
        (15, 0) => sc(&[Start], G::None),
        (15, 1) => sc(&[Start, Tap(40)], G::Tap(40)),
        _ => return None,
    })
}

/// Joga os toques preparados num jogo novo, desenha numa tela em memória e mostra só a
/// área do tabuleiro, reduzida, com o gesto (dedo e seta) por cima.
fn draw_scene(fb: &mut Framebuffer, f: &Fonts, game: usize, sc: &Scene, top: i32) -> i32 {
    let Some(mut g) = crate::minigame::make(game) else { return top };
    for op in sc.ops {
        match *op {
            Op::Start => {
                g.tap(900, 0, 0);
            }
            Op::Tap(id) => {
                g.tap(id, 0, 0);
            }
            Op::Hold(id) => {
                g.hold(id);
            }
            Op::Drag(ids) => {
                for &id in ids {
                    g.drag(id);
                }
                g.drag_end();
            }
        }
    }
    let mut off = Framebuffer::memory();
    g.draw(&mut off, f);
    let targets = g.targets();
    // Área: as casas do jogo (ids < 800) e os controles do gesto; senão, o meio da tela.
    let mut want: Vec<Rect> = targets.iter().filter(|(id, _)| *id < 800).map(|(_, r)| *r).collect();
    if want.is_empty() {
        // Jogo sem casas tocáveis (setas, cartas): mostra a área do jogo inteira.
        want.push(Rect::new(40, 200, W - 80, 1200));
    }
    let gid = match sc.gesture {
        Gesture::Tap(i) | Gesture::Hold(i) => Some(i),
        Gesture::Drag(a, _) => Some(a),
        Gesture::None => None,
    };
    if let Some(r) = gid.and_then(|i| targets.iter().find(|(id, _)| *id == i)).map(|(_, r)| *r) {
        want.push(r);
    }
    let src = if want.is_empty() {
        Rect::new(0, 250, W, 1020)
    } else {
        let x0 = want.iter().map(|r| r.x).min().unwrap() - 24;
        let y0 = want.iter().map(|r| r.y).min().unwrap() - 24;
        let x1 = want.iter().map(|r| r.x + r.w).max().unwrap() + 24;
        let y1 = want.iter().map(|r| r.y + r.h).max().unwrap() + 24;
        Rect::new(x0.max(0), y0.max(170), (x1 - x0).min(W), (y1 - y0).min(1448 - 170))
    };
    let (max_w, max_h) = (W - 2 * MARGIN, 560);
    let scale = (max_w as f32 / src.w as f32).min(max_h as f32 / src.h as f32).min(1.0);
    let (dw, dh) = ((src.w as f32 * scale) as i32, (src.h as f32 * scale) as i32);
    let dst = Rect::new((W - dw) / 2, top, dw, dh);
    fb.blit_scaled(&off, src, dst);
    fb.round_outline(Rect::new(dst.x - 4, dst.y - 4, dst.w + 8, dst.h + 8), 12, 3, 0xBB, PAPER);
    fb.blit_scaled(&off, src, dst);
    // Gesto por cima, em coordenadas da miniatura.
    let map = |r: Rect| -> (i32, i32) {
        let (cx, cy) = (r.x + r.w / 2, r.y + r.h / 2);
        (dst.x + ((cx - src.x) as f32 * scale) as i32, dst.y + ((cy - src.y) as f32 * scale) as i32)
    };
    let rect_of = |id: u32| targets.iter().find(|(i, _)| *i == id).map(|(_, r)| *r);
    match sc.gesture {
        Gesture::Tap(id) => {
            if let Some(p) = rect_of(id).map(map) {
                finger(fb, p, false);
            }
        }
        Gesture::Hold(id) => {
            if let Some(p) = rect_of(id).map(map) {
                finger(fb, p, true);
            }
        }
        Gesture::Drag(a, b) => {
            if let (Some(pa), Some(pb)) = (rect_of(a).map(map), rect_of(b).map(map)) {
                ui::stroke(fb, (pa.0 as f32, pa.1 as f32), (pb.0 as f32, pb.1 as f32), 8.0, PAPER);
                ui::stroke(fb, (pa.0 as f32, pa.1 as f32), (pb.0 as f32, pb.1 as f32), 4.0, INK);
                finger(fb, pb, false);
            }
        }
        Gesture::None => {}
    }
    dst.y + dst.h
}

/// Dedo tocando: anel com miolo; segurando: anéis duplos (ondas).
fn finger(fb: &mut Framebuffer, (x, y): (i32, i32), hold: bool) {
    if hold {
        for (rad, t) in [(46, 4), (32, 4)] {
            fb.round_outline(Rect::new(x - rad, y - rad, 2 * rad, 2 * rad), rad, t + 2, PAPER, PAPER);
        }
        for (rad, t) in [(46, 4), (32, 4)] {
            let r = Rect::new(x - rad, y - rad, 2 * rad, 2 * rad);
            outline_only(fb, r, rad, t);
        }
    } else {
        outline_only(fb, Rect::new(x - 30, y - 30, 60, 60), 30, 5);
    }
    fb.round_rect(Rect::new(x - 15, y - 15, 30, 30), 15, PAPER);
    fb.round_rect(Rect::new(x - 11, y - 11, 22, 22), 11, INK);
}

/// Anel sem pintar o miolo (o desenho de baixo continua visível).
fn outline_only(fb: &mut Framebuffer, r: Rect, rad: i32, t: i32) {
    let (cx, cy) = (r.x as f32 + r.w as f32 / 2.0, r.y as f32 + r.h as f32 / 2.0);
    let (ro, ri) = (rad as f32, (rad - t) as f32);
    for yy in r.y..r.y + r.h {
        for xx in r.x..r.x + r.w {
            let d = ((xx as f32 - cx).powi(2) + (yy as f32 - cy).powi(2)).sqrt();
            if d <= ro && d >= ri - 2.0 {
                let v = if d >= ri { INK } else { PAPER };
                fb.fill(Rect::new(xx, yy, 1, 1), v);
            }
        }
    }
}

/// Trilha de fases: 6 a 15, com a 10 (desafio) preta e a 15 (difícil) cinza.
fn draw_fases(fb: &mut Framebuffer, f: &Fonts, top: i32) -> i32 {
    let (n, s, gap) = (10, 80, 10);
    let x0 = (W - (n * s + (n - 1) * gap)) / 2;
    for i in 0..n {
        let k = 6 + i;
        let r = Rect::new(x0 + i * (s + gap), top + 20, s, s);
        let (bg, ink) = match k {
            10 => (INK, PAPER),
            15 => (0xAA, INK),
            _ => (PAPER, INK),
        };
        if bg == PAPER {
            fb.round_outline(r, s / 2, 3, INK, PAPER);
        } else {
            fb.round_rect(r, s / 2, bg);
        }
        let t = k.to_string();
        let w = f.measure(Face::Display, 34.0, &t);
        f.draw(fb, Face::Display, 34.0, r.x + (s - w) / 2, r.y + s / 2 + 12, ink, &t);
        if i + 1 < n {
            fb.fill(Rect::new(r.x + s, r.y + s / 2 - 2, gap, 4), INK);
        }
    }
    let y = top + 20 + s + 50;
    fb.round_rect(Rect::new(MARGIN, y - 24, 28, 28), 14, INK);
    let e = f.draw(fb, Face::Body, 30.0, MARGIN + 40, y, INK, "desafio");
    fb.round_rect(Rect::new(e + 40, y - 24, 28, 28), 14, 0xAA);
    f.draw(fb, Face::Body, 30.0, e + 80, y, INK, "difícil");
    y + 10
}

/// Desenha a ilustração centrada a partir de `top`; devolve onde ela termina.
fn draw_board(fb: &mut Framebuffer, f: &Fonts, page: &Page, top: i32) -> i32 {
    draw_board_in(fb, f, page, Rect::new(MARGIN, top, W - 2 * MARGIN, 440), 110)
}

/// Miniatura do jogo (a ilustração da 1ª página do tutorial) dentro de `area`, alinhada à direita.
pub fn draw_preview(fb: &mut Framebuffer, f: &Fonts, game: usize, area: Rect) {
    if let Some(p) = pages(game).iter().find(|p| !p.board.is_empty() && p.board != "FASES") {
        let rows = p.board.lines().count() as i32;
        let cols = p.board.lines().map(|l| l.chars().count()).max().unwrap_or(1) as i32;
        let s = (area.w / cols.max(1)).min(area.h / rows.max(1)).min(64);
        let (w, h) = (s * cols, s * rows);
        draw_board_in(fb, f, p, Rect::new(area.x + area.w - w, area.y + (area.h - h) / 2, w, h), s);
    }
}

fn draw_board_in(fb: &mut Framebuffer, f: &Fonts, page: &Page, area: Rect, max_cell: i32) -> i32 {
    let top = area.y;
    let rows: Vec<Vec<char>> = page.board.lines().map(|l| l.chars().collect()).collect();
    let shade: Vec<Vec<char>> = page.shade.lines().map(|l| l.chars().collect()).collect();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0) as i32;
    let n = rows.len() as i32;
    if cols == 0 {
        return top;
    }
    let s = max_cell.min(area.w / cols).min(area.h / n.max(1));
    let x0 = area.x + (area.w - s * cols) / 2;
    let lines_mode = page.board.contains('+');
    for (r, row) in rows.iter().enumerate() {
        for (c, &ch) in row.iter().enumerate() {
            let rect = Rect::new(x0 + c as i32 * s, top + r as i32 * s, s, s);
            let sh = shade.get(r).and_then(|l| l.get(c)).copied().unwrap_or('.');
            cell(fb, f, rect, ch, sh, lines_mode);
        }
    }
    // Reinos (Coroas): borda grossa entre letras diferentes, como no jogo.
    if page.shade.contains('A') {
        let at = |r: usize, c: usize| shade.get(r).and_then(|l| l.get(c)).copied();
        for r in 0..n as usize {
            for c in 0..cols as usize {
                let (x, y) = (x0 + c as i32 * s, top + r as i32 * s);
                if at(r, c + 1).is_some_and(|o| Some(o) != at(r, c)) {
                    fb.fill(Rect::new(x + s - 3, y, 6, s), INK);
                }
                if at(r + 1, c).is_some_and(|o| Some(o) != at(r, c)) {
                    fb.fill(Rect::new(x, y + s - 3, s, 6), INK);
                }
            }
        }
        let (bw, bh) = (cols * s, n * s);
        for rr in [Rect::new(x0 - 3, top - 3, bw + 6, 6), Rect::new(x0 - 3, top + bh - 3, bw + 6, 6), Rect::new(x0 - 3, top - 3, 6, bh + 6), Rect::new(x0 + bw - 3, top - 3, 6, bh + 6)] {
            fb.fill(rr, INK);
        }
    }
    top + n * s
}

fn cell(fb: &mut Framebuffer, f: &Fonts, r: Rect, ch: char, sh: char, lines_mode: bool) {
    let s = r.w;
    let inner = Rect::new(r.x + 3, r.y + 3, s - 6, s - 6);
    let (cx, cy) = (r.x as f32 + s as f32 / 2.0, r.y as f32 + s as f32 / 2.0);
    if lines_mode {
        // Pontos e caixas: pontos, linhas e caixas marcadas.
        match ch {
            '+' => fb.round_rect(Rect::new(r.x + s / 2 - 9, r.y + s / 2 - 9, 18, 18), 9, INK),
            '-' => fb.fill(Rect::new(r.x - 4, r.y + s / 2 - 4, s + 8, 8), INK),
            '|' => fb.fill(Rect::new(r.x + s / 2 - 4, r.y - 4, 8, s + 8), INK),
            '1' | '2' => {
                fb.round_rect(Rect::new(r.x - s / 2 + 14, r.y - s / 2 + 14, 2 * s - 28, 2 * s - 28), 10, if ch == '1' { INK } else { 0xAA });
            }
            _ => {}
        }
        return;
    }
    if ch == ' ' {
        return;
    }
    if sh == 'n' {
        // Só o texto, sem casa (dicas do Nonograma, rótulos).
        let t = ch.to_string();
        let size = s as f32 * 0.5;
        let w = f.measure(Face::Strong, size, &t);
        f.draw(fb, Face::Strong, size, r.x + (s - w) / 2, r.y + s / 2 + (size * 0.36) as i32, INK, &t);
        return;
    }
    // Fundo.
    let (bg, ink) = match sh {
        'A' => (0xFF, INK),
        'B' => (0xDC, INK),
        'C' => (0xBE, INK),
        'D' => (0xB4, INK),
        'E' => (0xEC, INK),
        'F' => (0xA4, INK),
        'K' | 'M' => (INK, PAPER),
        'P' => (0xAA, INK),
        'N' => (PAPER, MUTED),
        'L' => (0xF2, INK),
        'R' => (0xD0, INK),
        _ => (PAPER, INK),
    };
    match ch {
        '#' => fb.fill(inner, INK),
        'g' => fb.round_rect(inner, 8, 0xAA),
        '~' => fb.fill(inner, 0xE6),
        _ => {
            fb.fill(inner, bg);
            if sh != 'N' || ch != '.' {
                fb.round_outline(Rect::new(r.x + 2, r.y + 2, s - 4, s - 4), 6, 2, 0xBB, bg);
                fb.fill(Rect::new(r.x + 4, r.y + 4, s - 8, s - 8), bg);
            }
        }
    }
    let d = s as f32 * 0.18;
    match ch {
        'x' => {
            ui::stroke(fb, (cx - d, cy - d), (cx + d, cy + d), 3.0, MUTED);
            ui::stroke(fb, (cx + d, cy - d), (cx - d, cy + d), 3.0, MUTED);
        }
        'c' => crown(fb, inner, ink),
        'o' => fb.round_rect(Rect::new(r.x + 8, r.y + 8, s - 16, s - 16), (s - 16) / 2, INK),
        'w' => fb.round_outline(Rect::new(r.x + 8, r.y + 8, s - 16, s - 16), (s - 16) / 2, 4, INK, PAPER),
        '*' => fb.round_rect(Rect::new(r.x + s / 2 - s / 5, r.y + s / 2 - s / 5, 2 * (s / 5), 2 * (s / 5)), s / 5, INK),
        'f' => {
            let (x, y) = (r.x as f32 + s as f32 * 0.38, r.y as f32 + s as f32 * 0.22);
            ui::stroke(fb, (x, y), (x, y + s as f32 * 0.56), 4.0, INK);
            for k in 0..6 {
                let yy = y + k as f32 * s as f32 * 0.04;
                ui::stroke(fb, (x, yy), (x + s as f32 * 0.3, y + s as f32 * 0.12), 3.0, INK);
            }
        }
        '@' => {
            fb.round_rect(Rect::new(cx as i32 - s / 7, cy as i32 - s / 3, 2 * (s / 7), 2 * (s / 7)), s / 7, ink);
            fb.round_rect(Rect::new(cx as i32 - s / 4, cy as i32, s / 2, s / 3), s / 7, ink);
        }
        'b' | '%' => {
            let b = Rect::new(r.x + 10, r.y + 10, s - 20, s - 20);
            fb.round_rect(b, 8, if ch == '%' { INK } else { 0x99 });
            ui::stroke(fb, (b.x as f32 + 8.0, b.y as f32 + 8.0), ((b.x + b.w) as f32 - 8.0, (b.y + b.h) as f32 - 8.0), 4.0, PAPER);
            ui::stroke(fb, ((b.x + b.w) as f32 - 8.0, b.y as f32 + 8.0), (b.x as f32 + 8.0, (b.y + b.h) as f32 - 8.0), 4.0, PAPER);
        }
        't' => {
            let dd = s / 5;
            fb.round_outline(Rect::new(r.x + s / 2 - dd, r.y + s / 2 - dd, 2 * dd, 2 * dd), dd, 4, INK, bg);
        }
        'p' => {
            fb.round_rect(Rect::new(cx as i32 - s / 6, cy as i32 - s / 12, s / 3, s / 3), s / 6, ink);
            fb.fill(Rect::new(cx as i32 - s / 16, cy as i32 - s / 4, s / 8, s / 6), ink);
        }
        's' => {
            ui::stroke(fb, (cx - d * 1.6, cy + d * 1.6), (cx + d * 1.6, cy - d * 1.6), 6.0, ink);
            ui::stroke(fb, (cx - d * 1.4, cy + d * 0.2), (cx - d * 0.2, cy + d * 1.4), 6.0, ink);
        }
        '$' => {
            for (dx, dy) in [(-0.6f32, 0.4f32), (0.6, 0.4), (0.0, -0.5)] {
                let rr = (s / 7).max(6);
                let c = Rect::new((cx + dx * d * 1.5) as i32 - rr, (cy + dy * d * 1.5) as i32 - rr, 2 * rr, 2 * rr);
                fb.round_outline(c, rr, 4, ink, 0xDD);
            }
        }
        '>' => {
            for k in 0..3 {
                let w = s / 2 - k * s / 8;
                fb.fill(Rect::new(r.x + s / 4 + k * s / 8, r.y + s * 2 / 3 - k * s / 6, w, s / 6), if k % 2 == 0 { ink } else { 0x77 });
            }
        }
        '.' | '#' | 'g' | '~' => {}
        ch if ch.is_ascii_digit() || ch.is_alphabetic() || ch == '+' => {
            let t = ch.to_string();
            let size = s as f32 * 0.55;
            let w = f.measure(Face::Display, size, &t);
            f.draw(fb, Face::Display, size, r.x + (s - w) / 2, r.y + s / 2 + (size * 0.36) as i32, ink, &t);
        }
        _ => {}
    }
}

fn crown(fb: &mut Framebuffer, r: Rect, v: u8) {
    let (x, y, s) = (r.x as f32, r.y as f32, r.w as f32);
    let pts = [(0.15, 0.75), (0.15, 0.35), (0.33, 0.55), (0.5, 0.25), (0.67, 0.55), (0.85, 0.35), (0.85, 0.75)];
    for w in pts.windows(2) {
        ui::stroke(fb, (x + w[0].0 * s, y + w[0].1 * s), (x + w[1].0 * s, y + w[1].1 * s), s * 0.09, v);
    }
    fb.fill(Rect::new((x + 0.15 * s) as i32, (y + 0.66 * s) as i32, (0.7 * s) as i32, (0.12 * s) as i32), v);
    for k in 0..8 {
        let yy = y + (0.5 + k as f32 * 0.025) * s;
        ui::stroke(fb, (x + 0.2 * s, yy), (x + 0.8 * s, yy), s * 0.05, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn todo_jogo_tem_tutorial() {
        for g in 0..crate::apps::GAMES.len() {
            assert!(!pages(g).is_empty(), "jogo {g} sem tutorial");
            for p in pages(g) {
                assert!(!p.title.is_empty() && p.text.len() > 40);
            }
        }
    }

    /// `VERSO_INK_SHOTS=pasta cargo test tutoriais_em_imagem -- --ignored`
    #[test]
    #[ignore]
    fn tutoriais_em_imagem() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let f = Fonts::new();
        for g in 0..crate::apps::GAMES.len() {
            for k in 0..pages(g).len() {
                let mut fb = Framebuffer::memory();
                draw(&mut fb, &f, g, k);
                fb.save_pgm(&out.join(format!("tut-{g:02}-{k}.pgm")));
            }
        }
    }
}
