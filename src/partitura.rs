//! Curso de partitura do zero ao avançado, para quem já lê cifra: 6 módulos, 64 lições curtas
//! com a pauta desenhada e uma pergunta no fim de cada uma, e quatro treinos (notas, ritmo,
//! armaduras e intervalos). Todo o texto é original; as melodias de exemplo (Brilha, brilha
//! estrelinha, cantiga tradicional, e Ode à Alegria, de Beethoven) são de domínio público.
//!
//! Posições na pauta em "passos" a partir da 1ª linha: 0 é a 1ª linha, 1 o 1º espaço, … 8 a 5ª
//! linha; negativos abaixo. Na clave de sol o passo 0 é Mi; na de fá, Sol. O violão é escrito uma
//! oitava acima do som, como em toda partitura de violão.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const NAMES: [&str; 7] = ["Mi", "Fá", "Sol", "Lá", "Si", "Dó", "Ré"];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Clef {
    Sol,
    Fa,
}

/// Nome da nota no passo, na clave de sol.
#[cfg(test)]
pub fn note_name(step: i32) -> &'static str {
    name_in(step, Clef::Sol)
}

pub fn name_in(step: i32, clef: Clef) -> &'static str {
    let off = if clef == Clef::Fa { 2 } else { 0 };
    NAMES[(step + off).rem_euclid(7) as usize]
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fig {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}

const SB: Fig = Fig::Whole;
const MI: Fig = Fig::Half;
const SM: Fig = Fig::Quarter;
const CO: Fig = Fig::Eighth;
const SC: Fig = Fig::Sixteenth;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Acc {
    No,
    Sharp,
    Flat,
    Natural,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Stem {
    Auto,
    Up,
    Down,
}

/// Um elemento da partitura de exemplo.
#[derive(Clone, Copy, Debug)]
pub enum It {
    /// Nota: passo, figura, acidente, ponto, haste, legenda ("" = nome da nota, "-" = nada).
    N(i32, Fig, Acc, bool, Stem, &'static str),
    /// Pausa: figura, ponto, legenda.
    R(Fig, bool, &'static str),
    /// Notas empilhadas numa haste só.
    Chord(&'static [i32], Fig, &'static str),
    /// Duas vozes na mesma coluna: baixo (haste para baixo) e melodia (haste para cima).
    Two(i32, Fig, i32, Fig, &'static str),
    Bar,
    Double,
    End,
    RepL,
    RepR,
    /// Armadura nova no meio da música.
    Key(i8),
    /// Texto acima da próxima nota.
    Above(&'static str),
    /// Texto abaixo da pauta, sob a próxima nota (dinâmica).
    Below(&'static str),
    /// Liga a nota anterior à próxima (ligadura de valor).
    Tie,
    /// Ligadura de expressão sobre as próximas n notas.
    Slur(u8),
    /// Crescendo (true) ou diminuendo sob as próximas n notas.
    Hair(bool, u8),
    Stac,
    Accent,
    Ferm,
}

use It::*;

const fn n(s: i32, f: Fig, l: &'static str) -> It {
    N(s, f, Acc::No, false, Stem::Auto, l)
}
const fn d(s: i32, f: Fig, l: &'static str) -> It {
    N(s, f, Acc::No, true, Stem::Auto, l)
}
const fn sh(s: i32, f: Fig, l: &'static str) -> It {
    N(s, f, Acc::Sharp, false, Stem::Auto, l)
}
const fn fl(s: i32, f: Fig, l: &'static str) -> It {
    N(s, f, Acc::Flat, false, Stem::Auto, l)
}
const fn na(s: i32, f: Fig, l: &'static str) -> It {
    N(s, f, Acc::Natural, false, Stem::Auto, l)
}
const fn up(s: i32, f: Fig, l: &'static str) -> It {
    N(s, f, Acc::No, false, Stem::Up, l)
}
const fn r(f: Fig, l: &'static str) -> It {
    R(f, false, l)
}

pub struct Score {
    pub clef: Clef,
    pub key: i8,
    pub time: Option<(u8, u8)>,
    pub items: &'static [It],
}

/// Ilustração de cada lição.
pub enum Demo {
    /// Pauta vazia com as linhas numeradas.
    Staff,
    Score(Score),
    /// As figuras e quanto valem.
    Durations,
    /// Armaduras lado a lado, com o nome do tom.
    Keys(&'static [(i8, &'static str)]),
    /// Tabelinha de termos.
    Words(&'static [(&'static str, &'static str)]),
    /// Partitura e tablatura juntas: (passo, corda, casa).
    Tab(&'static [(i32, u8, u8)]),
}

const T24: Option<(u8, u8)> = Some((2, 4));
const T34: Option<(u8, u8)> = Some((3, 4));
const T44: Option<(u8, u8)> = Some((4, 4));
const T68: Option<(u8, u8)> = Some((6, 8));
const T78: Option<(u8, u8)> = Some((7, 8));

const fn sc(key: i8, time: Option<(u8, u8)>, items: &'static [It]) -> Demo {
    Demo::Score(Score { clef: Clef::Sol, key, time, items })
}
const fn fa(items: &'static [It]) -> Demo {
    Demo::Score(Score { clef: Clef::Fa, key: 0, time: None, items })
}

/// Pergunta do fim da lição.
pub struct Check {
    pub q: &'static str,
    pub opts: &'static [&'static str],
    pub ok: usize,
    pub why: &'static str,
}

pub struct Lesson {
    pub title: &'static str,
    pub text: &'static [&'static str],
    pub demo: Demo,
    pub check: Check,
}

pub struct Module {
    pub title: &'static str,
    pub sub: &'static str,
    pub lessons: &'static [Lesson],
}

const fn ck(q: &'static str, opts: &'static [&'static str], ok: usize, why: &'static str) -> Check {
    Check { q, opts, ok, why }
}

// ---------- O curso ----------

pub const MODULES: [Module; 6] = [
    Module {
        title: "Primeiros passos",
        sub: "A pauta, a clave e o nome das notas",
        lessons: &[
            Lesson {
                title: "A pauta e a clave",
                text: &[
                    "Partitura é cifra com mais detalhe: além de qual nota, ela diz quando e por quanto tempo.",
                    "As notas moram em 5 linhas e 4 espaços, contados de baixo para cima. Quanto mais alto na pauta, mais aguda a nota.",
                    "O desenho do começo é a clave de sol. Ela enrola na 2ª linha e diz: aqui é o Sol. Partitura de violão usa sempre essa clave.",
                ],
                demo: Demo::Staff,
                check: ck("Na clave de sol, em qual linha mora o Sol?", &["1ª linha", "2ª linha", "3ª linha"], 1, "A clave enrola na 2ª linha: ali é o Sol, e dele saem todas as outras."),
            },
            Lesson {
                title: "Notas nas linhas",
                text: &["De baixo para cima, as linhas são Mi, Sol, Si, Ré e Fá.", "Uma frase ajuda a guardar: \"Mi Sol, Si Ré Fá\". Repita em voz alta três vezes, apontando cada linha."],
                demo: sc(0, None, &[n(0, SB, ""), n(2, SB, ""), n(4, SB, ""), n(6, SB, ""), n(8, SB, "")]),
                check: ck("Contando de baixo para cima, a 3ª linha é…", &["Sol", "Si", "Ré"], 1, "Mi (1ª), Sol (2ª), Si (3ª), Ré (4ª) e Fá (5ª)."),
            },
            Lesson {
                title: "Notas nos espaços",
                text: &["Nos espaços, de baixo para cima: Fá, Lá, Dó e Mi.", "Juntando linhas e espaços, a pauta é só a escala subindo: Mi, Fá, Sol, Lá, Si, Dó, Ré, Mi, Fá."],
                demo: sc(0, None, &[n(1, SB, ""), n(3, SB, ""), n(5, SB, ""), n(7, SB, "")]),
                check: ck("O 1º espaço, entre a 1ª e a 2ª linha, é…", &["Fá", "Lá", "Mi"], 0, "Espaços de baixo para cima: Fá, Lá, Dó, Mi."),
            },
            Lesson {
                title: "A pauta é uma escada",
                text: &[
                    "Linha, espaço, linha, espaço: cada degrau é a nota seguinte do alfabeto musical. Dó Ré Mi Fá Sol Lá Si, e de novo Dó.",
                    "Saber uma nota basta para achar as vizinhas: um degrau acima é a próxima, um abaixo é a anterior. Comece sempre pelo Sol da clave.",
                ],
                demo: sc(0, None, &[n(-2, SM, ""), n(-1, SM, ""), n(0, SM, ""), n(1, SM, ""), n(2, SM, ""), n(3, SM, ""), n(4, SM, ""), n(5, SM, "")]),
                check: ck("Uma nota está no espaço logo acima do Sol. Qual é?", &["Fá", "Lá", "Si"], 1, "Depois do Sol vem o Lá: um degrau acima, no 2º espaço."),
            },
            Lesson {
                title: "Abaixo da pauta",
                text: &[
                    "Logo abaixo da 1ª linha fica o Ré, encostado. Mais abaixo, a nota ganha uma linha só dela, a linha suplementar: o Dó é cortado por ela.",
                    "No violão as notas descem bastante: com duas e três linhas suplementares aparecem Lá, Sol, Fá e o Mi da 6ª corda.",
                ],
                demo: sc(0, None, &[n(-1, SB, ""), n(-2, SB, ""), n(-3, SB, ""), n(-4, SB, ""), n(-5, SB, ""), n(-6, SB, ""), n(-7, SB, "")]),
                check: ck("Uma nota cortada pela primeira linha suplementar de baixo é…", &["Dó", "Ré", "Si"], 0, "Ré encosta embaixo da pauta; o Dó vem cortado pela linha suplementar."),
            },
            Lesson {
                title: "Acima da pauta",
                text: &["Acima da 5ª linha (Fá) vem o Sol, encostado. Com uma linha suplementar, o Lá; depois o Si; com duas linhas, o Dó.", "No violão essas notas agudas moram na 1ª corda, da casa 3 em diante."],
                demo: sc(0, None, &[n(8, SB, ""), n(9, SB, ""), n(10, SB, ""), n(11, SB, ""), n(12, SB, "")]),
                check: ck("O Lá agudo fica…", &["cortado pela 1ª linha suplementar de cima", "no último espaço", "na 5ª linha"], 0, "Fá na 5ª linha, Sol encostado em cima, Lá cortado pela linha suplementar."),
            },
            Lesson {
                title: "Oitava: mesmo nome, outra altura",
                text: &[
                    "Depois do Si, o nome recomeça no Dó. A distância entre duas notas com o mesmo nome é a oitava: oito degraus, contando as duas pontas.",
                    "O Mi da 1ª linha e o Mi do 4º espaço têm o mesmo nome; o segundo vibra duas vezes mais rápido e soa mais agudo.",
                ],
                demo: sc(0, None, &[n(0, SB, ""), n(7, SB, ""), n(-5, SB, ""), n(2, SB, ""), n(9, SB, "")]),
                check: ck("Quantos degraus tem uma oitava, contando as duas pontas?", &["7", "8", "12"], 1, "Mi, Fá, Sol, Lá, Si, Dó, Ré, Mi: oito nomes, daí o nome oitava."),
            },
            Lesson {
                title: "O violão soa uma oitava abaixo",
                text: &[
                    "A partitura de violão é escrita uma oitava acima do som real, para não encher a pauta de linhas suplementares. Às vezes um 8 pequeno embaixo da clave avisa isso.",
                    "Para você não muda nada: leia como está escrito e toque no lugar indicado. Quem lê para piano é que precisa lembrar.",
                ],
                demo: Demo::Words(&[("Escrito", "Mi na 1ª linha"), ("Toca", "4ª corda, casa 2"), ("Soa", "o Mi uma oitava abaixo"), ("Aviso", "um 8 sob a clave, às vezes")]),
                check: ck("Por que o violão é escrito uma oitava acima do som?", &["Para evitar muitas linhas suplementares", "Porque o violão é agudo", "Para combinar com o piano"], 0, "O violão é grave: escrito no som real, quase tudo ficaria abaixo da pauta."),
            },
        ],
    },
    Module {
        title: "Ritmo",
        sub: "Pulso, figuras, pausas e compassos",
        lessons: &[
            Lesson {
                title: "Pulso",
                text: &[
                    "Toda música tem um pulso, a batida constante que faz o pé bater sozinho. Ritmo é como as notas se encaixam nesse pulso.",
                    "Antes de tocar, bata o pé e conte \"1, 2, 3, 4\" em voz alta. Ler ritmo é contar enquanto toca, sem parar a contagem.",
                ],
                demo: sc(0, T44, &[n(2, SM, "1"), n(2, SM, "2"), n(2, SM, "3"), n(2, SM, "4"), Bar, n(2, SM, "1"), n(2, SM, "2"), n(2, SM, "3"), n(2, SM, "4"), End]),
                check: ck("O pulso deve ser…", &["constante, como um relógio", "mais lento nas partes difíceis", "livre, cada hora de um jeito"], 0, "Se o pulso muda, o ritmo se perde. Nas partes difíceis, diminua o andamento da música inteira."),
            },
            Lesson {
                title: "Semibreve, mínima e semínima",
                text: &["Semínima (cheia, com haste): 1 tempo. Mínima (vazada, com haste): 2. Semibreve (vazada, sem haste): 4.", "Cada figura vale metade da anterior. Segure a nota até o fim do valor dela, contando por dentro."],
                demo: Demo::Durations,
                check: ck("Uma mínima dura…", &["1 tempo", "2 tempos", "4 tempos"], 1, "Vazada com haste: mínima, 2 tempos."),
            },
            Lesson {
                title: "Colcheia",
                text: &[
                    "A colcheia tem uma bandeirola e vale meio tempo: duas cabem num pulso. Conte \"1 e 2 e\": o número cai com o pé, o \"e\" quando o pé sobe.",
                    "Várias colcheias juntas costumam vir unidas por uma barra grossa em vez de bandeirolas. É a mesma coisa.",
                ],
                demo: sc(0, T44, &[n(2, CO, "1"), n(2, CO, "e"), n(3, CO, "2"), n(3, CO, "e"), n(4, CO, "3"), n(4, CO, "e"), n(3, CO, "4"), n(3, CO, "e"), Bar, n(2, SB, "1"), End]),
                check: ck("Quantas colcheias cabem numa semínima?", &["1", "2", "4"], 1, "Semínima vale 1 tempo; colcheia, meio. Duas colcheias, um tempo."),
            },
            Lesson {
                title: "Semicolcheia",
                text: &["Com duas bandeirolas, a semicolcheia vale um quarto de tempo: quatro por pulso.", "Para contar, use sílabas: \"ta-ka-di-mi\" para as quatro do mesmo tempo, com o \"ta\" no pé. Comece bem devagar."],
                demo: sc(0, T24, &[n(2, SC, "ta"), n(2, SC, "ka"), n(2, SC, "di"), n(2, SC, "mi"), n(4, SC, "ta"), n(4, SC, "ka"), n(4, SC, "di"), n(4, SC, "mi"), Bar, n(2, MI, "ta"), End]),
                check: ck("Quantas semicolcheias cabem em um tempo?", &["2", "3", "4"], 2, "Duas bandeirolas: um quarto de tempo cada. Quatro fazem um pulso."),
            },
            Lesson {
                title: "Pausas: o silêncio escrito",
                text: &["Cada figura tem uma pausa com o mesmo valor. Na pausa, conte do mesmo jeito, só que sem som.", "A pausa de semibreve pendura na 4ª linha; a de mínima senta na 3ª. No violão, abafe as cordas com a palma da mão."],
                demo: sc(0, None, &[r(SB, "4"), r(MI, "2"), r(SM, "1"), r(CO, "½"), r(SC, "¼")]),
                check: ck("A pausa pendurada na 4ª linha vale…", &["4 tempos", "2 tempos", "1 tempo"], 0, "Pendurada: semibreve, 4 tempos. Sentada na 3ª linha: mínima, 2."),
            },
            Lesson {
                title: "Compasso e barra",
                text: &["A música é dividida em compassos por barras verticais. Cada compasso tem o mesmo número de tempos.", "O primeiro tempo de cada compasso é o mais forte. Achar o \"1\" é o segredo para não se perder."],
                demo: sc(0, T44, &[n(4, MI, "1"), n(2, MI, "3"), Bar, n(3, SM, "1"), n(4, SM, "2"), n(5, SM, "3"), n(3, SM, "4"), Bar, n(2, SB, "1"), End]),
                check: ck("Em 4/4, um compasso com uma mínima e duas semínimas está…", &["completo", "faltando um tempo", "sobrando um tempo"], 0, "2 + 1 + 1 = 4 tempos. Completo."),
            },
            Lesson {
                title: "Fórmula de compasso",
                text: &[
                    "Os dois números no começo dizem como contar. O de cima: quantos tempos por compasso. O de baixo: qual figura vale um tempo (4 é a semínima).",
                    "2/4 é marcha (1 2), 3/4 é valsa (1 2 3), 4/4 é a maior parte do pop e do rock.",
                ],
                demo: sc(0, T34, &[n(2, MI, "1"), n(4, SM, "3"), Bar, n(3, SM, "1"), n(2, SM, "2"), n(1, SM, "3"), Bar, d(2, MI, "1"), End]),
                check: ck("Em 3/4, quantos tempos tem cada compasso?", &["2", "3", "4"], 1, "O número de cima: 3 tempos, de semínima."),
            },
            Lesson {
                title: "Ponto de aumento",
                text: &[
                    "Um ponto depois da nota soma metade do valor dela. Mínima pontuada: 2 + 1 = 3 tempos. Semínima pontuada: 1 + ½ = 1 tempo e meio.",
                    "Semínima pontuada seguida de colcheia é um par muito comum: segure por um tempo e meio, e a colcheia cai no \"e\" do tempo seguinte.",
                ],
                demo: sc(0, T44, &[n(-2, SM, "1"), n(-2, SM, "2"), n(-1, SM, "3"), n(0, SM, "4"), Bar, d(0, SM, "1 (2)"), n(-1, CO, "e"), n(-1, MI, "3"), End]),
                check: ck("Quanto vale uma mínima pontuada?", &["2 tempos", "2 tempos e meio", "3 tempos"], 2, "Mínima vale 2; o ponto soma a metade, 1. Total: 3."),
            },
            Lesson {
                title: "Ligadura de valor",
                text: &["Uma curva unindo duas notas iguais soma os valores: toque a primeira e segure pela duração das duas, sem tocar de novo.", "É assim que uma nota atravessa a barra de compasso, coisa que o ponto não consegue."],
                demo: sc(0, T44, &[n(2, SM, "1"), n(2, SM, "2"), n(3, SM, "3"), n(4, SM, "4"), Tie, Bar, n(4, MI, "(1 2)"), n(3, MI, "3"), End]),
                check: ck("Duas semínimas iguais ligadas soam como…", &["2 notas de 1 tempo", "1 nota de 2 tempos", "1 nota de meio tempo"], 1, "A ligadura soma: toca uma vez e segura pelos dois tempos."),
            },
            Lesson {
                title: "Anacruse",
                text: &[
                    "Às vezes a música começa antes do tempo 1: notas soltas num compasso incompleto, a anacruse. É o \"e…\" antes de cantar.",
                    "Conte o compasso inteiro na cabeça e entre no tempo certo. O último compasso da música costuma completar o que faltou.",
                ],
                demo: sc(0, T34, &[n(-1, SM, "3"), Bar, n(2, MI, "1"), n(4, SM, "3"), Bar, n(3, SM, "1"), n(2, SM, "2"), n(0, SM, "3"), Bar, n(2, MI, "1"), End]),
                check: ck("Anacruse é…", &["um compasso incompleto no início", "uma pausa longa", "o último compasso"], 0, "Notas que chegam antes do primeiro tempo forte."),
            },
            Lesson {
                title: "Compasso composto: 6/8",
                text: &[
                    "Em 6/8 cabem seis colcheias, agrupadas de três em três. Sinta 2 batidas, cada uma dividida em três: força no 1 e no 4.",
                    "É o balanço de muitas cantigas de ninar e baladas. Aqui a semínima pontuada vale uma batida inteira.",
                ],
                demo: sc(0, T68, &[n(2, CO, "1"), n(3, CO, "2"), n(4, CO, "3"), n(5, CO, "4"), n(4, CO, "5"), n(3, CO, "6"), Bar, d(2, SM, "1"), d(0, SM, "4"), End]),
                check: ck("Em 6/8, quantas batidas fortes você sente por compasso?", &["2", "3", "6"], 0, "Seis colcheias em dois grupos de três: duas batidas."),
            },
            Lesson {
                title: "Síncopa",
                text: &[
                    "Síncopa é a nota que começa no tempo fraco e atravessa o forte. É o tempero do samba, do baião e do pop.",
                    "Padrão clássico: colcheia, semínima, colcheia. Conte \"1 e 2 e\" em voz alta e deixe o 2 passar por dentro da nota longa.",
                ],
                demo: sc(0, T24, &[n(2, CO, "1"), n(4, SM, "e (2)"), n(2, CO, "e"), Bar, n(3, CO, "1"), n(5, SM, "e (2)"), n(3, CO, "e"), Bar, n(2, MI, "1"), End]),
                check: ck("Na síncopa, a nota longa começa…", &["no tempo forte", "no contratempo (o \"e\")", "depois da barra"], 1, "Ela nasce no \"e\" e atravessa o tempo seguinte: daí o balanço."),
            },
            Lesson {
                title: "Quiálteras",
                text: &["Um 3 sobre um grupo diz: três notas no espaço de duas. A tercina de colcheias divide um tempo em três partes iguais.", "Fale \"tri-pli-cá\" num tempo, enquanto o pé marca o pulso. Depois alterne com duas colcheias normais."],
                demo: sc(0, T24, &[Above("3"), n(2, CO, "tri"), n(3, CO, "pli"), n(4, CO, "cá"), Above("3"), n(5, CO, "tri"), n(4, CO, "pli"), n(3, CO, "cá"), Bar, n(2, CO, "1"), n(2, CO, "e"), n(2, SM, "2"), End]),
                check: ck("Uma tercina de colcheias ocupa…", &["1 tempo", "1 tempo e meio", "2 tempos"], 0, "Três colcheias no lugar de duas: o mesmo tempo, dividido em três."),
            },
        ],
    },
    Module {
        title: "Tons e tonalidades",
        sub: "Acidentes, escalas, armaduras e acordes",
        lessons: &[
            Lesson {
                title: "Tom e semitom",
                text: &[
                    "Semitom é a menor distância: uma casa no violão. Tom são duas casas.",
                    "Entre as notas naturais quase sempre há um tom. As exceções são Mi–Fá e Si–Dó: só um semitom, sem nota no meio.",
                ],
                demo: sc(0, None, &[n(0, SB, "Mi"), n(1, SB, "Fá"), n(2, SB, "Sol"), n(4, SB, "Si"), n(5, SB, "Dó"), n(6, SB, "Ré")]),
                check: ck("Entre Mi e Fá há…", &["um semitom", "um tom", "um tom e meio"], 0, "Mi–Fá e Si–Dó são os dois semitons naturais."),
            },
            Lesson {
                title: "Sustenido, bemol e bequadro",
                text: &[
                    "Sustenido (#) sobe meio tom: uma casa para frente. Bemol (b) desce meio tom: uma casa para trás. Bequadro cancela os dois.",
                    "Fá# e Solb são a mesma casa no violão, com nomes diferentes. A partitura escolhe o nome conforme o tom da música.",
                ],
                demo: sc(0, None, &[n(1, SB, "Fá"), sh(1, SB, "Fá#"), n(4, SB, "Si"), fl(4, SB, "Sib"), na(4, SB, "Si")]),
                check: ck("Lá bemol fica onde no violão, em relação ao Lá?", &["uma casa para trás", "uma casa para frente", "duas casas para frente"], 0, "Bemol desce meio tom: uma casa na direção da cabeça do violão."),
            },
            Lesson {
                title: "O acidente vale o compasso",
                text: &["Um acidente ao lado da nota vale até a barra de compasso, para aquela nota naquela altura. No compasso seguinte, volta ao normal.", "Por isso existe o bequadro: para cancelar dentro do mesmo compasso."],
                demo: sc(0, T44, &[sh(1, SM, "Fá#"), n(2, SM, "Sol"), n(1, SM, "Fá#"), n(0, SM, "Mi"), Bar, n(1, SM, "Fá"), sh(1, SM, "Fá#"), na(1, MI, "Fá"), End]),
                check: ck("Um Fá# no tempo 1; no tempo 3 do mesmo compasso, um Fá sem sinal. Ele é…", &["Fá#", "Fá natural", "Fá bemol"], 0, "O sustenido continua valendo até a barra."),
            },
            Lesson {
                title: "A escala maior",
                text: &["A escala maior segue sempre a mesma receita de distâncias: tom, tom, semitom, tom, tom, tom, semitom.", "A partir do Dó, a receita cai certinho nas notas naturais. Por isso Dó maior não tem nenhum acidente."],
                demo: sc(0, None, &[n(-2, SM, "Dó"), n(-1, SM, "T"), n(0, SM, "T"), n(1, SM, "S"), n(2, SM, "T"), n(3, SM, "T"), n(4, SM, "T"), n(5, SM, "S")]),
                check: ck("A receita da escala maior é…", &["T T S T T T S", "T S T T S T T", "T T T S T T S"], 0, "Tom, tom, semitom, tom, tom, tom, semitom."),
            },
            Lesson {
                title: "Sol maior e a armadura",
                text: &[
                    "Aplique a receita a partir do Sol: Sol Lá Si Dó Ré Mi… e o 7º degrau precisa ser Fá#, para sobrar só meio tom até o Sol.",
                    "Em vez de escrever # em todo Fá, a partitura põe um # na linha do Fá logo depois da clave: a armadura de clave.",
                ],
                demo: sc(1, None, &[n(2, SM, "Sol"), n(3, SM, ""), n(4, SM, ""), n(5, SM, ""), n(6, SM, ""), n(7, SM, ""), n(8, SM, "Fá#"), n(9, SM, "Sol")]),
                check: ck("Quantos sustenidos tem Sol maior?", &["nenhum", "1, o Fá#", "2, Fá# e Dó#"], 1, "Só o Fá# é preciso para a receita fechar."),
            },
            Lesson {
                title: "Armadura de clave",
                text: &["Os acidentes logo depois da clave valem para a música inteira, em qualquer altura. Eles dizem o tom.", "Antes da primeira nota, olhe a armadura e diga em voz alta quais notas mudam. Depois, esqueça os sinais e só toque."],
                demo: sc(2, T44, &[n(-1, SM, "Ré"), n(1, SM, "Fá#"), n(3, SM, "Lá"), n(5, SM, "Dó#"), Bar, n(6, MI, "Ré"), n(1, MI, "Fá#"), End]),
                check: ck("Armadura com Fá# e Dó#. Um Dó no meio da música é…", &["Dó", "Dó#", "Dó bemol"], 1, "A armadura vale para todas as notas da música, em todas as alturas."),
            },
            Lesson {
                title: "Tons com sustenidos",
                text: &["Os sustenidos entram sempre nesta ordem: Fá, Dó, Sol, Ré, Lá, Mi, Si.", "Truque: suba meio tom a partir do último sustenido e você tem o tom. Último Dó#: Ré maior. Último Sol#: Lá maior."],
                demo: Demo::Keys(&[(1, "Sol"), (2, "Ré"), (3, "Lá"), (4, "Mi")]),
                check: ck("O último sustenido da armadura é Sol#. O tom é…", &["Sol maior", "Lá maior", "Mi maior"], 1, "Sol# mais meio tom é Lá."),
            },
            Lesson {
                title: "Tons com bemóis",
                text: &["Os bemóis entram na ordem contrária: Si, Mi, Lá, Ré, Sol, Dó, Fá.", "Truque: o penúltimo bemol é o nome do tom. Sib e Mib: Sib maior. Com um bemol só, decore: Fá maior."],
                demo: Demo::Keys(&[(-1, "Fá"), (-2, "Sib"), (-3, "Mib"), (-4, "Láb")]),
                check: ck("Armadura com Sib, Mib e Láb. O tom é…", &["Láb maior", "Mib maior", "Sib maior"], 1, "O penúltimo bemol é Mib: Mib maior."),
            },
            Lesson {
                title: "O tom menor relativo",
                text: &[
                    "Toda armadura serve a dois tons: um maior e um menor, três semitons abaixo. Dó maior e Lá menor não têm acidentes.",
                    "Para saber qual dos dois, olhe o último acorde e a última nota da música: quase sempre são o tom.",
                ],
                demo: Demo::Keys(&[(0, "Dó · Lám"), (1, "Sol · Mim"), (2, "Ré · Sim"), (-1, "Fá · Rém")]),
                check: ck("O relativo menor de Sol maior é…", &["Mi menor", "Ré menor", "Si menor"], 0, "Três semitons abaixo de Sol: Fá#, Fá, Mi. Mi menor, com o mesmo Fá#."),
            },
            Lesson {
                title: "Escalas menores",
                text: &[
                    "A menor natural usa a armadura do relativo maior: Lá Si Dó Ré Mi Fá Sol Lá. Abaixo, a versão harmônica.",
                    "A harmônica sobe o 7º grau (Sol#) e ganha um sabor quase árabe. A melódica sobe o 6º e o 7º na subida e volta ao natural na descida.",
                ],
                demo: sc(0, None, &[n(3, SM, "Lá"), n(4, SM, ""), n(5, SM, ""), n(6, SM, ""), n(7, SM, ""), n(8, SM, ""), sh(9, SM, "Sol#"), n(10, SM, "Lá")]),
                check: ck("Na escala de Lá menor harmônica, o Sol vira…", &["Sol#", "Sol bemol", "continua Sol"], 0, "A harmônica sobe o 7º grau meio tom: Sol#."),
            },
            Lesson {
                title: "Intervalos",
                text: &["Intervalo é a distância entre duas notas. Conte as notas do caminho incluindo as duas pontas: Dó até Mi é uma 3ª (Dó, Ré, Mi).", "Na pauta é visual: de linha a linha vizinha, 3ª; de linha ao espaço vizinho, 2ª; de linha a linha pulando uma, 5ª."],
                demo: sc(0, None, &[Chord(&[-2, -1], SB, "2ª"), Chord(&[-2, 0], SB, "3ª"), Chord(&[-2, 1], SB, "4ª"), Chord(&[-2, 2], SB, "5ª"), Chord(&[-2, 5], SB, "8ª")]),
                check: ck("De Sol a Ré, subindo, é…", &["4ª", "5ª", "6ª"], 1, "Sol, Lá, Si, Dó, Ré: cinco notas, uma 5ª."),
            },
            Lesson {
                title: "Maior, menor e justo",
                text: &[
                    "O número diz quantas notas; a qualidade diz quantos semitons. 3ª maior tem 4 semitons (Dó–Mi); 3ª menor, 3 (Lá–Dó).",
                    "4ª, 5ª e 8ª são justas. A 5ª justa (7 semitons) é a base do power chord do rock.",
                ],
                demo: Demo::Words(&[("2ª maior", "2 casas"), ("3ª menor", "3 casas"), ("3ª maior", "4 casas"), ("4ª justa", "5 casas"), ("5ª justa", "7 casas"), ("8ª justa", "12 casas")]),
                check: ck("Uma 3ª maior tem…", &["3 semitons", "4 semitons", "5 semitons"], 1, "Quatro casas: Dó–Mi, a 3ª que deixa o acorde maior."),
            },
            Lesson {
                title: "Acordes na pauta",
                text: &[
                    "Acorde é nota empilhada de 3ª em 3ª. Na pauta parece um boneco de neve: linha, linha, linha (ou espaço, espaço, espaço).",
                    "Dó–Mi–Sol é o C da cifra; Lá–Dó–Mi é o Am. Agora você sabe de onde vem cada letra que você já toca.",
                ],
                demo: sc(0, None, &[Chord(&[-2, 0, 2], SB, "C"), Chord(&[3, 5, 7], SB, "Am"), Chord(&[1, 3, 5], SB, "F"), Chord(&[2, 4, 6], SB, "G")]),
                check: ck("Ré–Fá–Lá empilhados formam…", &["D", "Dm", "F"], 1, "Ré–Fá é 3ª menor (3 casas): acorde menor, Dm."),
            },
            Lesson {
                title: "Campo harmônico",
                text: &[
                    "Empilhe um acorde sobre cada nota da escala de Dó maior: saem C, Dm, Em, F, G, Am e B°. São os acordes que moram no tom de Dó.",
                    "Por isso tantas músicas em C usam G, Am e F: todos são do mesmo campo. O mesmo vale para qualquer tom.",
                ],
                demo: sc(0, None, &[Chord(&[-2, 0, 2], SB, "C"), Chord(&[-1, 1, 3], SB, "Dm"), Chord(&[0, 2, 4], SB, "Em"), Chord(&[1, 3, 5], SB, "F"), Chord(&[2, 4, 6], SB, "G"), Chord(&[3, 5, 7], SB, "Am"), Chord(&[4, 6, 8], SB, "B°")]),
                check: ck("No campo harmônico de Dó maior, o acorde de Ré é…", &["D", "Dm", "D7"], 1, "Ré–Fá–Lá, só com notas naturais: Dm."),
            },
        ],
    },
    Module {
        title: "Expressão e mapa",
        sub: "Dinâmica, articulação, andamento e repetições",
        lessons: &[
            Lesson {
                title: "Dinâmica",
                text: &["As letras sob a pauta dizem o volume. Vêm do italiano: p (piano) é fraco, f (forte) é forte.", "Do mais fraco ao mais forte: pp, p, mp, mf, f, ff. O m é \"meio\"."],
                demo: Demo::Words(&[("pp", "pianíssimo, bem fraco"), ("p", "piano, fraco"), ("mp", "meio fraco"), ("mf", "meio forte"), ("f", "forte"), ("ff", "fortíssimo")]),
                check: ck("Qual é mais forte?", &["mp", "mf", "p"], 1, "mf é meio forte; mp, meio fraco."),
            },
            Lesson {
                title: "Crescendo e diminuendo",
                text: &["Um sinal que abre como < pede crescendo: aumentar o volume aos poucos. Fechando como > é diminuendo.", "Também aparecem por escrito, cresc. e dim. Faça a mudança aos poucos, ao longo de todas as notas do sinal."],
                demo: sc(0, T44, &[Below("p"), Hair(true, 4), n(0, SM, "-"), n(1, SM, "-"), n(2, SM, "-"), n(3, SM, "-"), Bar, Below("f"), Hair(false, 4), n(4, SM, "-"), n(3, SM, "-"), n(2, SM, "-"), n(1, SM, "-"), End]),
                check: ck("O sinal que abre como < pede…", &["mais forte aos poucos", "mais fraco aos poucos", "acelerar"], 0, "Abre = cresce. Fecha = diminui."),
            },
            Lesson {
                title: "Articulação",
                text: &[
                    "Ponto junto da cabeça é staccato: nota curta e solta. O sinal > é acento: aquela nota mais forte.",
                    "Uma curva sobre notas diferentes é ligadura de expressão (legato): toque sem separar. Não confunda com a de valor, que une notas iguais.",
                ],
                demo: sc(0, T44, &[Stac, n(2, SM, "curta"), Stac, n(2, SM, "curta"), Accent, n(4, SM, "forte"), n(2, SM, "-"), Bar, Slur(4), n(0, SM, "ligado"), n(1, SM, "-"), n(2, SM, "-"), n(3, SM, "-"), End]),
                check: ck("Um ponto junto da nota quer dizer…", &["nota curta e solta", "nota mais longa", "repetir a nota"], 0, "Staccato: solte a nota logo, deixando um respiro até a próxima."),
            },
            Lesson {
                title: "Andamento",
                text: &[
                    "A palavra no alto diz a velocidade. Largo e Adagio são lentos; Andante é passo de caminhada; Moderato, médio; Allegro, rápido; Presto, muito rápido.",
                    "Às vezes vem em número: semínima = 80 quer dizer 80 batidas por minuto. Um metrônomo no celular ajuda a sentir.",
                ],
                demo: Demo::Words(&[("Largo", "bem lento, uns 50"), ("Adagio", "lento, uns 70"), ("Andante", "caminhando, uns 90"), ("Moderato", "médio, uns 110"), ("Allegro", "rápido, uns 130"), ("Presto", "muito rápido, 170+")]),
                check: ck("Andante é…", &["muito rápido", "passo de caminhada", "bem lento"], 1, "Vem de \"andar\": velocidade de caminhada."),
            },
            Lesson {
                title: "Fermata e respiração",
                text: &["A fermata, um arco com um ponto, manda segurar a nota além do valor, o quanto soar bonito.", "rit. (ritardando) pede para desacelerar aos poucos; a tempo manda voltar à velocidade de antes."],
                demo: sc(0, T44, &[n(2, SM, "-"), n(3, SM, "-"), n(4, SM, "-"), n(5, SM, "-"), Bar, Above("rit."), n(4, SM, "-"), n(3, SM, "-"), Ferm, n(2, MI, "segura"), End]),
                check: ck("A fermata pede para…", &["segurar a nota mais tempo", "tocar mais rápido", "não tocar a nota"], 0, "Ela suspende o pulso naquela nota; depois, o ritmo segue."),
            },
            Lesson {
                title: "Ritornello",
                text: &["Barras com dois pontos são o ritornello. Ao chegar nos pontos virados para trás, volte até os pontos virados para frente (ou ao início).", "Toque o trecho duas vezes e siga em frente."],
                demo: sc(0, T44, &[RepL, n(2, SM, "-"), n(4, SM, "-"), n(3, SM, "-"), n(5, SM, "-"), Bar, n(4, MI, "-"), n(2, MI, "-"), RepR, n(2, SB, "segue")]),
                check: ck("Chegando ao ritornello do fim pela primeira vez, você…", &["volta ao sinal de início", "para a música", "pula para o fim"], 0, "Na primeira vez volta; na segunda, segue em frente."),
            },
            Lesson {
                title: "Casa 1 e casa 2",
                text: &["Quando a repetição termina diferente, aparecem colchetes numerados. Na 1ª vez, toque a casa 1 e volte. Na 2ª, pule a casa 1 e toque a casa 2.", "É o jeito de economizar papel em refrões que terminam de outro jeito."],
                demo: sc(0, T44, &[RepL, n(2, SM, "-"), n(3, SM, "-"), n(4, MI, "-"), Bar, Above("1."), n(3, SM, "-"), n(1, SM, "-"), n(3, MI, "1ª vez"), RepR, Above("2."), n(4, SM, "-"), n(3, SM, "-"), n(2, MI, "2ª vez"), End]),
                check: ck("Na segunda vez, a casa 1 é…", &["tocada", "pulada", "tocada duas vezes"], 1, "Na segunda passada, pule direto da repetição para a casa 2."),
            },
            Lesson {
                title: "D.C., D.S., Fine e Coda",
                text: &[
                    "D.C. (da capo) manda voltar ao começo. D.S. (dal segno) manda voltar ao segno, um S cortado. Fine marca onde a música acaba nessa volta.",
                    "Coda, um círculo com uma cruz, é um final separado: ao ver \"al Coda\", pule para lá.",
                ],
                demo: Demo::Words(&[("D.C.", "volta ao começo"), ("D.S.", "volta ao segno"), ("Fine", "fim, na volta"), ("al Coda", "pula para a Coda"), ("D.C. al Fine", "começo até o Fine")]),
                check: ck("D.C. al Fine quer dizer…", &["volte ao começo e pare no Fine", "pule para o fim", "repita o último compasso"], 0, "Da capo: do começo. Al Fine: até onde está escrito Fine."),
            },
        ],
    },
    Module {
        title: "Violão na partitura",
        sub: "Notas no braço, dedos, posições e técnicas",
        lessons: &[
            Lesson {
                title: "Cordas soltas",
                text: &["As cordas soltas, da mais grossa para a mais fina: Mi, Lá, Ré, Sol, Si e Mi. Na pauta do violão elas ficam assim:", "A 6ª corda fica bem abaixo da pauta (três linhas suplementares); a 1ª, no último espaço. Cada casa do braço é meio tom."],
                demo: sc(0, None, &[n(-7, SB, "6ª"), n(-4, SB, "5ª"), n(-1, SB, "4ª"), n(2, SB, "3ª"), n(4, SB, "2ª"), n(7, SB, "1ª")]),
                check: ck("A 3ª corda solta é…", &["Sol", "Ré", "Si"], 0, "Mi Lá Ré Sol Si Mi, da 6ª para a 1ª: a 3ª é o Sol, na 2ª linha."),
            },
            Lesson {
                title: "6ª e 5ª cordas",
                text: &["Na 6ª corda: Mi solta, Fá na casa 1, Sol na casa 3. Na 5ª: Lá solta, Si na casa 2, Dó na casa 3.", "Todas ficam abaixo da pauta. Conte as linhas suplementares a partir do Dó, que tem uma só."],
                demo: sc(0, None, &[n(-7, SM, "6ª·0"), n(-6, SM, "6ª·1"), n(-5, SM, "6ª·3"), n(-4, SM, "5ª·0"), n(-3, SM, "5ª·2"), n(-2, SM, "5ª·3")]),
                check: ck("O Dó com uma linha suplementar é tocado…", &["na 5ª corda, casa 3", "na 6ª corda, casa 3", "na 4ª corda, casa 2"], 0, "5ª corda: Lá solta, Si na 2, Dó na 3."),
            },
            Lesson {
                title: "4ª e 3ª cordas",
                text: &["Na 4ª: Ré solta, Mi na casa 2, Fá na casa 3. Na 3ª: Sol solta, Lá na casa 2.", "Atenção: o Mi da 1ª linha é a 4ª corda na casa 2, não a 1ª corda solta. A 1ª solta é o Mi do 4º espaço."],
                demo: sc(0, None, &[n(-1, SM, "4ª·0"), n(0, SM, "4ª·2"), n(1, SM, "4ª·3"), n(2, SM, "3ª·0"), n(3, SM, "3ª·2")]),
                check: ck("O Sol da 2ª linha é tocado…", &["na 3ª corda solta", "na 2ª corda, casa 1", "na 1ª corda, casa 3"], 0, "A 3ª corda solta é exatamente o Sol da 2ª linha."),
            },
            Lesson {
                title: "2ª e 1ª cordas",
                text: &["Na 2ª: Si solta, Dó na casa 1, Ré na casa 3. Na 1ª: Mi solta, Fá na casa 1, Sol na casa 3.", "Com estas três lições você conhece a primeira posição inteira: tudo entre a corda solta e a casa 3."],
                demo: sc(0, None, &[n(4, SM, "2ª·0"), n(5, SM, "2ª·1"), n(6, SM, "2ª·3"), n(7, SM, "1ª·0"), n(8, SM, "1ª·1"), n(9, SM, "1ª·3")]),
                check: ck("O Fá da 5ª linha é tocado…", &["na 1ª corda, casa 1", "na 2ª corda, casa 1", "na 1ª corda, casa 3"], 0, "1ª corda: Mi solta, Fá na casa 1, Sol na 3."),
            },
            Lesson {
                title: "Os dedos na partitura",
                text: &["Números junto das notas são a mão esquerda: 1 indicador, 2 médio, 3 anelar, 4 mínimo e 0 corda solta.", "Letras são a mão direita: p polegar, i indicador, m médio, a anelar. Número dentro de um círculo indica a corda."],
                demo: Demo::Words(&[("1 2 3 4", "dedos da esquerda"), ("0", "corda solta"), ("p", "polegar (direita)"), ("i  m  a", "indicador, médio, anelar"), ("número no círculo", "a corda")]),
                check: ck("A letra m indica…", &["o médio da mão direita", "o mínimo da esquerda", "tocar meio forte"], 0, "p, i, m, a são os dedos da mão direita."),
            },
            Lesson {
                title: "Posições",
                text: &[
                    "Posição é onde fica o dedo 1. Na primeira posição, dedo 1 na casa 1; na quinta, dedo 1 na casa 5, e cada dedo cuida de uma casa.",
                    "A partitura indica com algarismo romano: V é a quinta posição. C V pede pestana inteira na casa 5; ½C, meia pestana.",
                ],
                demo: Demo::Words(&[("I", "dedo 1 na casa 1"), ("V", "dedo 1 na casa 5"), ("VII", "dedo 1 na casa 7"), ("C V", "pestana na casa 5"), ("½C III", "meia pestana na casa 3")]),
                check: ck("VII acima da pauta indica…", &["a 7ª posição", "a 7ª corda", "7 tempos"], 0, "Romano é posição: dedo 1 na casa 7."),
            },
            Lesson {
                title: "Mesma nota, vários lugares",
                text: &["No violão a mesma nota existe em vários lugares. O Mi da 1ª linha está na 4ª corda casa 2, na 5ª corda casa 7 e na 6ª corda casa 12.", "A partitura dá a altura; você escolhe onde tocar, a não ser que ela indique a corda ou a posição."],
                demo: sc(0, None, &[n(0, SB, "4ª·2"), n(0, SB, "5ª·7"), n(0, SB, "6ª·12")]),
                check: ck("O Mi da 1ª linha na 5ª corda fica na casa…", &["5", "7", "12"], 1, "Lá solta + 7 casas = Mi."),
            },
            Lesson {
                title: "Tablatura e partitura",
                text: &["Tablatura mostra onde pôr o dedo (corda e casa), mas não o ritmo. Partitura mostra a nota e o tempo, mas não o dedo.", "Por isso muita música vem com as duas juntas: lendo partitura você entende o ritmo da tablatura."],
                demo: Demo::Tab(&[(0, 4, 2), (1, 4, 3), (2, 3, 0), (3, 3, 2), (4, 2, 0), (5, 2, 1)]),
                check: ck("O que a tablatura sozinha não mostra bem?", &["o ritmo", "a corda", "a casa"], 0, "Ela diz onde tocar, não quando nem por quanto tempo."),
            },
            Lesson {
                title: "Duas vozes",
                text: &[
                    "Violão solo costuma ter duas vozes na mesma pauta: o baixo com haste para baixo, tocado com o polegar, e a melodia com haste para cima.",
                    "Toque o baixo e a nota de cima juntos quando estiverem alinhados, e deixe o baixo soando pelo valor dele.",
                ],
                demo: sc(0, T44, &[Two(-4, MI, 5, SM, "p+m"), up(4, SM, "i"), Two(-1, MI, 3, SM, "p+m"), up(5, SM, "i"), Bar, Two(-7, SB, 4, MI, "p+m"), up(2, MI, "i"), End]),
                check: ck("Na escrita a duas vozes, as hastes para baixo são…", &["o baixo, com o polegar", "a melodia", "notas abafadas"], 0, "Haste para baixo: baixo e polegar. Para cima: melodia e dedos."),
            },
            Lesson {
                title: "Ligados e técnicas",
                text: &[
                    "Hammer-on (H): toque a primeira nota e martele a segunda com o dedo, sem tocar a corda de novo. Pull-off (P): puxe o dedo para soar a nota de baixo.",
                    "sl. é slide: deslize o dedo pela corda. Harm. é harmônico: encoste o dedo sobre o traste, sem apertar, e solte ao tocar.",
                ],
                demo: sc(0, None, &[Slur(2), Above("H"), n(-1, SM, "4ª·0"), n(0, SM, "4ª·2"), Slur(2), Above("P"), n(0, SM, "4ª·2"), n(-1, SM, "4ª·0"), Slur(2), Above("sl."), n(3, SM, "3ª·2"), n(4, SM, "3ª·4")]),
                check: ck("No hammer-on você…", &["martela a 2ª nota sem tocar a corda de novo", "toca as duas com a mão direita", "abafa a corda"], 0, "Só a primeira nota é tocada; a segunda nasce da martelada."),
            },
            Lesson {
                title: "Brilha, brilha, parte 1",
                text: &[
                    "Sua primeira melodia: cantiga tradicional, de domínio público. Embaixo de cada nota, corda e casa. Uma semínima por tempo; a mínima segura dois.",
                    "Dó Dó Sol Sol | Lá Lá Sol. Toque devagar, contando \"1 2 3 4\" em voz alta.",
                ],
                demo: sc(0, T44, &[n(-2, SM, "5ª·3"), n(-2, SM, "5ª·3"), n(2, SM, "3ª·0"), n(2, SM, "3ª·0"), Bar, n(3, SM, "3ª·2"), n(3, SM, "3ª·2"), n(2, MI, "3ª·0"), End]),
                check: ck("Nesta melodia, o Sol da 2ª linha é tocado…", &["na 3ª corda solta", "na 1ª corda, casa 3", "na 4ª corda, casa 3"], 0, "Na primeira posição, o Sol da 2ª linha é a 3ª corda solta."),
            },
            Lesson {
                title: "Brilha, brilha, parte 2",
                text: &[
                    "Fá Fá Mi Mi | Ré Ré Dó. A melodia desce degrau por degrau até voltar ao Dó do começo.",
                    "Fá e Mi na 4ª corda (casas 3 e 2), Ré na 4ª solta, Dó na 5ª casa 3. Junte as duas partes: quando sair sem olhar o braço, você está lendo.",
                ],
                demo: sc(0, T44, &[n(1, SM, "4ª·3"), n(1, SM, "4ª·3"), n(0, SM, "4ª·2"), n(0, SM, "4ª·2"), Bar, n(-1, SM, "4ª·0"), n(-1, SM, "4ª·0"), n(-2, MI, "5ª·3"), End]),
                check: ck("Quantos tempos dura o último Dó?", &["1", "2", "4"], 1, "É uma mínima, vazada com haste: 2 tempos."),
            },
            Lesson {
                title: "Ode à Alegria, parte 1",
                text: &["Beethoven, domínio público. Mi Mi Fá Sol | Sol Fá Mi Ré. Uma semínima por tempo.", "Fique na primeira posição: Mi e Fá na 4ª corda (casas 2 e 3), Sol na 3ª solta, Ré na 4ª solta."],
                demo: sc(0, T44, &[n(0, SM, "4ª·2"), n(0, SM, "4ª·2"), n(1, SM, "4ª·3"), n(2, SM, "3ª·0"), Bar, n(2, SM, "3ª·0"), n(1, SM, "4ª·3"), n(0, SM, "4ª·2"), n(-1, SM, "4ª·0"), End]),
                check: ck("A melodia começa com qual nota?", &["Mi", "Sol", "Ré"], 0, "Primeira nota na 1ª linha: Mi, 4ª corda casa 2."),
            },
            Lesson {
                title: "Ode à Alegria, parte 2",
                text: &[
                    "Dó Dó Ré Mi | Mi Ré Ré. Repare na semínima pontuada e na colcheia: segure o Mi por um tempo e meio, e o Ré entra no \"e\".",
                    "O Dó é a 5ª corda na casa 3. Toque as duas partes seguidas: quatro compassos de música de verdade, lidos por você.",
                ],
                demo: sc(0, T44, &[n(-2, SM, "5ª·3"), n(-2, SM, "5ª·3"), n(-1, SM, "4ª·0"), n(0, SM, "4ª·2"), Bar, d(0, SM, "4ª·2"), n(-1, CO, "4ª·0"), n(-1, MI, "4ª·0"), End]),
                check: ck("Quanto dura o Mi pontuado?", &["1 tempo", "1 tempo e meio", "2 tempos"], 1, "Semínima (1) mais o ponto (½)."),
            },
        ],
    },
    Module {
        title: "Avançado",
        sub: "Clave de fá, ritmos difíceis, modulação e leitura à primeira vista",
        lessons: &[
            Lesson {
                title: "Clave de fá",
                text: &[
                    "Baixo, violoncelo e a mão esquerda do piano leem na clave de fá. Os dois pontos cercam a 4ª linha: ali é o Fá.",
                    "Nela as linhas, de baixo para cima, são Sol, Si, Ré, Fá e Lá. Os espaços: Lá, Dó, Mi e Sol.",
                ],
                demo: fa(&[n(0, SB, ""), n(2, SB, ""), n(4, SB, ""), n(6, SB, ""), n(8, SB, "")]),
                check: ck("Na clave de fá, a 4ª linha é…", &["Fá", "Ré", "Sol"], 0, "A clave nasce na 4ª linha e os dois pontos a abraçam: Fá."),
            },
            Lesson {
                title: "Notas na clave de fá",
                text: &[
                    "Truque: na clave de fá, a nota fica dois degraus acima do nome que teria na clave de sol. 1ª linha: Mi na de sol, Sol na de fá.",
                    "O Dó central fica numa linha suplementar acima da pauta de fá, e numa abaixo da de sol: é a ponte entre as duas.",
                ],
                demo: fa(&[n(1, SB, ""), n(3, SB, ""), n(5, SB, ""), n(7, SB, ""), n(10, SB, "Dó central")]),
                check: ck("O 2º espaço da clave de fá é…", &["Dó", "Lá", "Mi"], 0, "Espaços na clave de fá: Lá, Dó, Mi, Sol."),
            },
            Lesson {
                title: "Ritmos pontuados rápidos",
                text: &["Colcheia pontuada + semicolcheia divide o tempo em três quartos e um quarto: \"ta . . mi\". É o ritmo saltitante das marchas e do xote.", "Não confunda com a tercina: aqui a nota curta é bem curta e fica colada na próxima."],
                demo: sc(0, T24, &[d(2, CO, "ta"), n(3, SC, "mi"), d(4, CO, "ta"), n(5, SC, "mi"), Bar, n(4, SM, "1"), n(2, SM, "2"), End]),
                check: ck("Colcheia pontuada vale…", &["meio tempo", "três quartos de tempo", "um tempo"], 1, "Colcheia (½) mais metade dela (¼): ¾."),
            },
            Lesson {
                title: "Compassos ímpares",
                text: &["5/4 e 7/8 não se dividem por igual: 5/4 costuma ser 3+2, e 7/8, 2+2+3 (ou outra ordem).", "Conte em grupos e acentue o começo de cada um. Muita música dos Bálcãs e do rock progressivo vive aqui."],
                demo: sc(0, T78, &[Accent, n(2, CO, "1"), n(2, CO, "2"), Accent, n(4, CO, "1"), n(4, CO, "2"), Accent, n(3, CO, "1"), n(3, CO, "2"), n(3, CO, "3"), End]),
                check: ck("Quantas colcheias tem um compasso de 7/8?", &["6", "7", "8"], 1, "O número de cima: 7 colcheias."),
            },
            Lesson {
                title: "Modulação",
                text: &[
                    "Quando a música muda de tom no meio, aparece uma armadura nova depois de uma barra dupla.",
                    "Acidentes soltos que se repetem (sempre o mesmo #) costumam anunciar uma modulação, ou o 7º grau de um tom menor.",
                ],
                demo: sc(0, T44, &[n(-2, SM, "Dó"), n(0, SM, ""), n(2, SM, ""), n(5, SM, ""), Double, Key(1), n(2, SM, "Sol"), n(4, SM, ""), n(6, SM, ""), n(8, SM, "Fá#"), End]),
                check: ck("Uma armadura nova no meio da música indica…", &["mudança de tom", "fim da música", "repetição"], 0, "A partir dali, vale a armadura nova."),
            },
            Lesson {
                title: "Transpor lendo",
                text: &[
                    "Transpor é mudar o tom de tudo pela mesma distância. Subir um tom: cada nota sobe um degrau e a armadura muda (Dó maior vira Ré maior, com 2 sustenidos).",
                    "No violão o capotraste faz isso por você: os mesmos desenhos, som mais agudo. Capo na casa 2 sobe um tom.",
                ],
                demo: sc(0, None, &[n(-2, SM, "Dó"), n(0, SM, "Mi"), n(2, SM, "Sol"), Double, Key(2), n(-1, SM, "Ré"), n(1, SM, "Fá#"), n(3, SM, "Lá")]),
                check: ck("Capotraste na casa 2 sobe a música…", &["meio tom", "um tom", "uma 3ª"], 1, "Duas casas, dois semitons: um tom."),
            },
            Lesson {
                title: "Leitura à primeira vista",
                text: &["Antes de tocar: armadura (o que muda), compasso (como contar), andamento, e procure saltos e ritmos difíceis.", "Depois toque sem parar, mesmo errando: o pulso vale mais que a nota certa. Olhe sempre um pouco à frente."],
                demo: Demo::Words(&[("1", "armadura: o que muda"), ("2", "compasso: como contar"), ("3", "andamento: comece devagar"), ("4", "ache os trechos difíceis"), ("5", "toque sem parar"), ("6", "olhos um compasso à frente")]),
                check: ck("Errou uma nota lendo à primeira vista. O melhor é…", &["seguir no pulso", "voltar e corrigir", "parar e recomeçar"], 0, "Quem para perde o pulso; quem segue, chega ao fim."),
            },
            Lesson {
                title: "Como continuar",
                text: &[
                    "Leitura se ganha com volume: 10 minutos por dia de coisa fácil valem mais que uma hora por semana de coisa difícil.",
                    "Material livre e excelente: os estudos de Carcassi, Carulli, Sor e Aguado, todos em domínio público. Comece pelos mais simples e use os treinos daqui todo dia.",
                ],
                demo: Demo::Words(&[("Todo dia", "treino de notas, 2 min"), ("Todo dia", "treino de ritmo, 2 min"), ("Todo dia", "uma peça fácil, 6 min"), ("Toda semana", "uma peça nova"), ("Todo mês", "um nível acima")]),
                check: ck("O que faz a leitura melhorar mais rápido?", &["um pouco todo dia, com coisa fácil", "horas de vez em quando", "só peças difíceis"], 0, "Volume e regularidade: muitas leituras fáceis treinam o olho."),
            },
        ],
    },
];

/// Total de lições do curso.
pub fn total() -> usize {
    MODULES.iter().map(|m| m.lessons.len()).sum()
}

/// Índice global da primeira lição do módulo.
pub fn first(m: usize) -> usize {
    MODULES[..m].iter().map(|m| m.lessons.len()).sum()
}

/// Módulo e posição dentro dele de uma lição (índice global).
pub fn locate(g: usize) -> (usize, usize) {
    let mut g = g;
    for (m, md) in MODULES.iter().enumerate() {
        if g < md.lessons.len() {
            return (m, g);
        }
        g -= md.lessons.len();
    }
    (MODULES.len() - 1, MODULES[MODULES.len() - 1].lessons.len() - 1)
}

pub fn lesson(g: usize) -> &'static Lesson {
    let (m, k) = locate(g);
    &MODULES[m].lessons[k]
}

// ---------- Desenho ----------

fn disc(fb: &mut Framebuffer, cx: i32, cy: i32, r: i32) {
    fb.round_rect(Rect::new(cx - r, cy - r, 2 * r, 2 * r), r, INK);
}

fn line(fb: &mut Framebuffer, a: (f32, f32), b: (f32, f32), t: f32) {
    ui::stroke(fb, a, b, t, INK);
}

/// Curva (parábola) de a até b, com a barriga `bulge` px (positivo para baixo).
fn arc(fb: &mut Framebuffer, a: (i32, i32), b: (i32, i32), bulge: i32, t: f32) {
    let steps = 14;
    let mut prev = (a.0 as f32, a.1 as f32);
    for i in 1..=steps {
        let u = i as f32 / steps as f32;
        let x = a.0 as f32 + (b.0 - a.0) as f32 * u;
        let y = a.1 as f32 + (b.1 - a.1) as f32 * u + bulge as f32 * 4.0 * u * (1.0 - u);
        line(fb, prev, (x, y), t);
        prev = (x, y);
    }
}

fn y_of(bottom: i32, gap: i32, step: i32) -> i32 {
    bottom - step * gap / 2
}

fn staff_lines(fb: &mut Framebuffer, x: i32, w: i32, bottom: i32, gap: i32) {
    for k in 0..5 {
        fb.fill(Rect::new(x, bottom - k * gap, w, 2), INK);
    }
}

/// Clave de sol (desenho do Codex) ou de fá (desenhada aqui).
fn draw_clef(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, clef: Clef) {
    match clef {
        Clef::Sol => {
            let ch = gap * 15 / 2;
            if !crate::sprites::draw(fb, "clave-sol", Rect::new(x - ch / 5, bottom - 6 * gap, ch, ch)) {
                fb.round_outline(Rect::new(x + 10, bottom - 3 * gap, gap * 2, gap * 3), gap, 4, INK, PAPER);
            }
        }
        Clef::Fa => {
            let g = gap as f32;
            let y4 = (bottom - 3 * gap) as f32;
            let x0 = x as f32 + g * 0.5;
            // Duas curvas de Bézier: do ponto na 4ª linha, sobe, contorna e desce afinando.
            let segs = [[(0.2, -0.05), (0.15, -1.25), (2.05, -1.45), (2.05, 0.1)], [(2.05, 0.1), (2.05, 1.35), (1.1, 2.3), (0.05, 2.95)]];
            let mut prev = (x0 + 0.2 * g, y4 - 0.05 * g);
            let total = 2 * 16;
            for (si, c) in segs.iter().enumerate() {
                for i in 1..=16 {
                    let t = i as f32 / 16.0;
                    let m = 1.0 - t;
                    let bx = m * m * m * c[0].0 + 3.0 * m * m * t * c[1].0 + 3.0 * m * t * t * c[2].0 + t * t * t * c[3].0;
                    let by = m * m * m * c[0].1 + 3.0 * m * m * t * c[1].1 + 3.0 * m * t * t * c[2].1 + t * t * t * c[3].1;
                    let p = (x0 + bx * g, y4 + by * g);
                    let k = (si * 16 + i) as f32 / total as f32;
                    // Mais grossa na curva de cima, fina na ponta.
                    let w = g * (0.16 + 0.2 * (1.0 - (k - 0.45).abs() * 2.0).max(0.0)) * (1.0 - 0.6 * k.powf(3.0));
                    line(fb, prev, p, w.max(3.0));
                    prev = p;
                }
            }
            disc(fb, (x0 + g * 0.32) as i32, y4 as i32, (g * 0.34) as i32);
            disc(fb, (x0 + g * 2.55) as i32, (y4 - g * 0.5) as i32, (g * 0.15).max(3.0) as i32);
            disc(fb, (x0 + g * 2.55) as i32, (y4 + g * 0.5) as i32, (g * 0.15).max(3.0) as i32);
        }
    }
}

/// Pauta com a clave de sol. Devolve a posição y da 1ª linha (a de baixo) e o espaço entre linhas.
pub fn draw_staff(fb: &mut Framebuffer, r: Rect, gap: i32) -> (i32, i32) {
    draw_staff_clef(fb, r, gap, Clef::Sol)
}

pub fn draw_staff_clef(fb: &mut Framebuffer, r: Rect, gap: i32, clef: Clef) -> (i32, i32) {
    let bottom = r.y + r.h / 2 + 2 * gap;
    staff_lines(fb, r.x, r.w, bottom, gap);
    draw_clef(fb, r.x, bottom, gap, clef);
    (bottom, gap)
}

/// Sustenido, bemol ou bequadro centrado em (cx, cy).
fn draw_acc(fb: &mut Framebuffer, cx: i32, cy: i32, gap: i32, acc: Acc) {
    let g = gap as f32;
    let (x, y) = (cx as f32, cy as f32);
    let thin = (g * 0.09).max(2.0);
    let thick = (g * 0.22).max(4.0);
    match acc {
        Acc::No => {}
        Acc::Sharp => {
            line(fb, (x - g * 0.2, y - g * 1.1), (x - g * 0.2, y + g * 1.2), thin);
            line(fb, (x + g * 0.2, y - g * 1.2), (x + g * 0.2, y + g * 1.1), thin);
            line(fb, (x - g * 0.45, y - g * 0.2), (x + g * 0.45, y - g * 0.45), thick);
            line(fb, (x - g * 0.45, y + g * 0.45), (x + g * 0.45, y + g * 0.2), thick);
        }
        Acc::Flat => {
            line(fb, (x - g * 0.3, y - g * 1.6), (x - g * 0.3, y + g * 0.5), thin.max(3.0));
            let pts = [(-0.3, 0.0), (0.05, -0.32), (0.38, -0.22), (0.35, 0.08), (0.0, 0.35), (-0.3, 0.5)];
            for w in pts.windows(2) {
                line(fb, (x + w[0].0 * g, y + w[0].1 * g), (x + w[1].0 * g, y + w[1].1 * g), thin.max(3.0) + 1.0);
            }
        }
        Acc::Natural => {
            line(fb, (x - g * 0.22, y - g * 1.1), (x - g * 0.22, y + g * 0.45), thin);
            line(fb, (x + g * 0.22, y - g * 0.45), (x + g * 0.22, y + g * 1.1), thin);
            line(fb, (x - g * 0.22, y - g * 0.2), (x + g * 0.22, y - g * 0.4), thick);
            line(fb, (x - g * 0.22, y + g * 0.4), (x + g * 0.22, y + g * 0.2), thick);
        }
    }
}

const SHARPS: [i32; 7] = [8, 5, 9, 6, 3, 7, 4];
const FLATS: [i32; 7] = [4, 7, 3, 6, 2, 5, 1];

/// Armadura a partir de x; devolve a largura usada.
fn draw_key(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, clef: Clef, key: i8) -> i32 {
    if key == 0 {
        return 0;
    }
    let shift = if clef == Clef::Fa { -2 } else { 0 };
    let n = key.unsigned_abs() as usize;
    for i in 0..n.min(7) {
        let (step, acc) = if key > 0 { (SHARPS[i], Acc::Sharp) } else { (FLATS[i], Acc::Flat) };
        draw_acc(fb, x + gap / 2 + i as i32 * gap * 9 / 10, y_of(bottom, gap, step + shift), gap, acc);
    }
    n as i32 * gap * 9 / 10 + gap / 2
}

fn key_width(gap: i32, key: i8) -> i32 {
    if key == 0 { 0 } else { key.unsigned_abs() as i32 * gap * 9 / 10 + gap / 2 }
}

fn draw_time(fb: &mut Framebuffer, f: &Fonts, x: i32, bottom: i32, gap: i32, (a, b): (u8, u8)) -> i32 {
    let size = gap as f32 * 2.1;
    let (ta, tb) = (a.to_string(), b.to_string());
    let w = f.measure(Face::Display, size, &ta).max(f.measure(Face::Display, size, &tb));
    for (t, base) in [(&ta, bottom - 2 * gap - gap / 8), (&tb, bottom - gap / 8)] {
        let tw = f.measure(Face::Display, size, t);
        f.draw(fb, Face::Display, size, x + (w - tw) / 2, base, INK, t);
    }
    w + gap
}

fn ledgers(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, step: i32) {
    let mut s = -2;
    while s >= step {
        fb.fill(Rect::new(x - gap, y_of(bottom, gap, s), gap * 2 + gap / 3, 2), INK);
        s -= 2;
    }
    let mut s = 10;
    while s <= step {
        fb.fill(Rect::new(x - gap, y_of(bottom, gap, s), gap * 2 + gap / 3, 2), INK);
        s += 2;
    }
}

fn head_rect(x: i32, y: i32, gap: i32) -> Rect {
    let (hw, hh) = (gap * 13 / 10, gap);
    Rect::new(x - hw / 2 + gap / 6, y - hh / 2, hw, hh)
}

fn draw_head(fb: &mut Framebuffer, head: Rect, gap: i32, fig: Fig) {
    match fig {
        Fig::Whole | Fig::Half => fb.round_outline(head, head.h / 2, (gap / 5).max(3), INK, PAPER),
        _ => fb.round_rect(head, head.h / 2, INK),
    }
}

/// Haste e bandeirolas a partir de (sx, y): para cima ou para baixo.
fn draw_stem(fb: &mut Framebuffer, sx: i32, y_from: i32, y_tip: i32, gap: i32, fig: Fig) {
    let (y0, y1) = if y_tip < y_from { (y_tip, y_from) } else { (y_from, y_tip) };
    fb.fill(Rect::new(sx, y0, 3, y1 - y0), INK);
    let up = y_tip < y_from;
    let flags = match fig {
        Fig::Eighth => 1,
        Fig::Sixteenth => 2,
        _ => 0,
    };
    let dir = if up { 1.0 } else { -1.0 };
    for k in 0..flags {
        let fy = y_tip as f32 + dir * k as f32 * gap as f32 * 0.75;
        line(fb, (sx as f32 + 1.0, fy), (sx as f32 + gap as f32 * 0.9, fy + dir * gap as f32 * 1.3), 4.0);
    }
}

/// Uma nota na pauta: cabeça (cheia ou vazada), haste, bandeirola e linhas suplementares.
pub fn draw_note(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, step: i32, fig: Fig) {
    note(fb, x, bottom, gap, step, fig, false, Stem::Auto);
}

/// Nota completa; devolve a cabeça e se a haste subiu.
fn note(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, step: i32, fig: Fig, dot: bool, stem: Stem) -> (Rect, bool) {
    ledgers(fb, x, bottom, gap, step);
    let y = y_of(bottom, gap, step);
    let head = head_rect(x, y, gap);
    draw_head(fb, head, gap, fig);
    let up = match stem {
        Stem::Up => true,
        Stem::Down => false,
        Stem::Auto => step < 4,
    };
    if fig != Fig::Whole {
        let len = gap * 7 / 2;
        if up {
            draw_stem(fb, head.x + head.w - 3, y, y - len, gap, fig);
        } else {
            draw_stem(fb, head.x, y, y + len, gap, fig);
        }
    }
    if dot {
        let dy = if step.rem_euclid(2) == 0 { y - gap / 2 } else { y };
        disc(fb, head.x + head.w + gap / 3, dy, (gap / 7).max(3));
    }
    (head, up)
}

/// Pausa no meio da pauta.
pub fn draw_rest(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, fig: Fig, dot: bool) {
    let g = gap as f32;
    let (xf, b) = (x as f32, bottom as f32);
    match fig {
        Fig::Whole => fb.fill(Rect::new(x - gap * 6 / 10, bottom - 3 * gap, gap * 12 / 10, gap / 2), INK),
        Fig::Half => fb.fill(Rect::new(x - gap * 6 / 10, bottom - 2 * gap - gap / 2, gap * 12 / 10, gap / 2), INK),
        Fig::Quarter => {
            let c = b - 2.0 * g;
            line(fb, (xf - 0.25 * g, c - 1.45 * g), (xf + 0.3 * g, c - 0.75 * g), 4.5);
            line(fb, (xf + 0.3 * g, c - 0.75 * g), (xf - 0.2 * g, c - 0.1 * g), g * 0.34);
            line(fb, (xf - 0.2 * g, c - 0.1 * g), (xf + 0.3 * g, c + 0.55 * g), 4.5);
            line(fb, (xf + 0.3 * g, c + 0.55 * g), (xf - 0.3 * g, c + 0.6 * g), 4.0);
            line(fb, (xf - 0.3 * g, c + 0.6 * g), (xf - 0.05 * g, c + 1.2 * g), 4.0);
        }
        Fig::Eighth | Fig::Sixteenth => {
            let top = b - 2.6 * g;
            disc(fb, (xf - 0.3 * g) as i32, (top + 0.1 * g) as i32, (g * 0.2) as i32);
            line(fb, (xf - 0.3 * g, top + 0.3 * g), (xf + 0.45 * g, top), 3.0);
            let foot = if fig == Fig::Eighth { b - 0.9 * g } else { b + 0.1 * g };
            line(fb, (xf + 0.45 * g, top), (xf - 0.1 * g, foot), 3.0);
            if fig == Fig::Sixteenth {
                let t2 = top + g;
                disc(fb, (xf - 0.5 * g) as i32, (t2 + 0.1 * g) as i32, (g * 0.2) as i32);
                line(fb, (xf - 0.5 * g, t2 + 0.3 * g), (xf + 0.25 * g, t2), 3.0);
            }
        }
    }
    if dot {
        disc(fb, x + gap, bottom - 2 * gap - gap / 2, (gap / 7).max(3));
    }
}

fn bar_line(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, w: i32) {
    fb.fill(Rect::new(x, bottom - 4 * gap, w, 4 * gap + 2), INK);
}

fn rep_dots(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32) {
    disc(fb, x, y_of(bottom, gap, 3), (gap / 6).max(3));
    disc(fb, x, y_of(bottom, gap, 5), (gap / 6).max(3));
}

fn is_note(it: &It) -> bool {
    matches!(it, N(..) | R(..) | Chord(..) | Two(..))
}

fn item_steps(it: &It) -> Vec<i32> {
    match it {
        N(s, ..) => vec![*s],
        Chord(ss, ..) => ss.to_vec(),
        Two(a, _, b, _, _) => vec![*a, *b],
        _ => vec![],
    }
}

/// Posições x dos elementos da partitura e o x final (onde a pauta termina).
fn layout(f_gap: i32, x_start: i32, x_end: i32, key: i8, time: bool, items: &[It]) -> (Vec<i32>, i32) {
    let gap = f_gap;
    let mut x = x_start + gap * 9 / 2 + key_width(gap, key) + if time { gap * 2 + gap / 2 } else { gap / 2 };
    let notes = items.iter().filter(|i| is_note(i)).count().max(1) as i32;
    let fixed: i32 = items
        .iter()
        .map(|i| match i {
            Bar | Double | End => gap,
            RepL | RepR => gap * 2,
            Key(k) => key_width(gap, *k) + gap / 2,
            _ => 0,
        })
        .sum();
    let u = ((x_end - x - fixed) / notes).clamp(gap * 2, gap * 5);
    let mut xs = Vec::with_capacity(items.len());
    for it in items {
        match it {
            _ if is_note(it) => {
                xs.push(x + u / 2);
                x += u;
            }
            Bar | Double | End => {
                xs.push(x + gap / 3);
                x += gap;
            }
            RepL => {
                xs.push(x + gap / 3);
                x += gap * 2;
            }
            RepR => {
                xs.push(x + gap * 3 / 2);
                x += gap * 2;
            }
            Key(k) => {
                xs.push(x);
                x += key_width(gap, *k) + gap / 2;
            }
            _ => xs.push(x + u / 2),
        }
    }
    let end = if matches!(items.last(), Some(Bar | Double | End | RepR)) { *xs.last().unwrap() + gap / 2 + 4 } else { x_end };
    (xs, end)
}

/// Partitura de exemplo dentro de `r`: tamanho e posição se ajustam às notas.
fn beats(fig: Fig, dot: bool) -> f32 {
    let b = match fig {
        Fig::Whole => 4.0,
        Fig::Half => 2.0,
        Fig::Quarter => 1.0,
        Fig::Eighth => 0.5,
        Fig::Sixteenth => 0.25,
    };
    if dot { b * 1.5 } else { b }
}

/// Grupos de colcheias e semicolcheias unidas por barra, como na partitura impressa: por tempo
/// (de 2 em 2 colcheias no 2/4 e 4/4), de 3 em 3 no 6/8, pelos acentos no 7/8, e a tercina inteira.
fn beam_groups(items: &[It], time: Option<(u8, u8)>) -> Vec<Vec<usize>> {
    let unit = match time {
        Some((6, 8)) => 1.5,
        Some((_, 8)) => 100.0, // compassos ímpares: os acentos dizem onde começa cada grupo
        _ => 1.0,
    };
    let (mut groups, mut cur): (Vec<Vec<usize>>, Vec<usize>) = (Vec::new(), Vec::new());
    let (mut pos, mut cur_beat, mut trip, mut accent) = (0.0f32, -1i32, 0u8, false);
    let flush = |cur: &mut Vec<usize>, groups: &mut Vec<Vec<usize>>| {
        if cur.len() >= 2 {
            groups.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for (i, it) in items.iter().enumerate() {
        match it {
            Bar | Double | End | RepL | RepR => {
                flush(&mut cur, &mut groups);
                pos = 0.0;
            }
            Above(t) if *t == "3" => {
                flush(&mut cur, &mut groups);
                trip = 3;
            }
            Accent => accent = true,
            N(_, fig, _, dot, _, _) => {
                let mut dur = beats(*fig, *dot);
                if trip > 0 {
                    dur = 1.0 / 3.0;
                }
                if matches!(fig, Fig::Eighth | Fig::Sixteenth) {
                    let beat = ((pos + 1e-3) / unit).floor() as i32;
                    if trip == 0 && (beat != cur_beat || accent) {
                        flush(&mut cur, &mut groups);
                    }
                    cur_beat = beat;
                    cur.push(i);
                } else {
                    flush(&mut cur, &mut groups);
                }
                accent = false;
                pos += dur;
                if trip > 0 {
                    trip -= 1;
                    if trip == 0 {
                        flush(&mut cur, &mut groups);
                    }
                }
            }
            R(fig, dot, _) => {
                flush(&mut cur, &mut groups);
                pos += beats(*fig, *dot);
            }
            Chord(_, fig, _) | Two(_, fig, _, _, _) => {
                flush(&mut cur, &mut groups);
                pos += beats(*fig, false);
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut groups);
    groups
}

pub fn draw_score(fb: &mut Framebuffer, f: &Fonts, r: Rect, s: &Score) {
    draw_items(fb, f, r, s.clef, s.key, s.time, s.items, true);
}

fn draw_items(fb: &mut Framebuffer, f: &Fonts, r: Rect, clef: Clef, key: i8, time: Option<(u8, u8)>, items: &[It], labels: bool) -> Vec<i32> {
    let steps: Vec<i32> = items.iter().flat_map(item_steps).collect();
    let lo = steps.iter().copied().min().unwrap_or(0).min(0);
    let hi = steps.iter().copied().max().unwrap_or(8).max(8);
    let has_above = items.iter().any(|i| matches!(i, Above(_) | Ferm | Accent));
    let has_below = items.iter().any(|i| matches!(i, Below(_) | Hair(..)));
    let top_ext = hi + if hi >= 4 { 3 } else { 6 } + if has_above { 4 } else { 0 };
    let bot_ext = lo - if lo < 4 { 3 } else { 6 } - if has_below { 5 } else { 0 };
    let label_h = if labels { 44 } else { 0 };
    let area = Rect::new(r.x, r.y + 6, r.w, r.h - 6 - label_h);
    let units = top_ext - bot_ext;
    let gap = ((area.h * 2 / units).min(26) / 2 * 2).max(12);
    let bottom = area.y + (area.h - units * gap / 2) / 2 + top_ext * gap / 2;
    let (xs, end) = layout(gap, r.x, r.x + r.w - 8, key, time.is_some(), items);
    staff_lines(fb, r.x, end - r.x, bottom, gap);
    draw_clef(fb, r.x, bottom, gap, clef);
    let mut x = r.x + gap * 9 / 2;
    x += draw_key(fb, x, bottom, gap, clef, key);
    if let Some(t) = time {
        draw_time(fb, f, x + gap / 2, bottom, gap, t);
    }
    let below_y = bottom.max(y_of(bottom, gap, lo) + gap) + gap * 3 / 2;
    let many = items.iter().filter(|i| is_note(i)).count() > 9;
    let lsize = if many { 21.0 } else { 24.0 };
    // Estado dos modificadores que valem para a próxima nota.
    let mut above: Option<&str> = None;
    let mut below: Option<&str> = None;
    let (mut stac, mut accent, mut ferm, mut tie) = (false, false, false, false);
    let mut last: Option<(i32, i32, bool)> = None;
    let mut slur: Option<(i32, i32, bool, u8)> = None;
    let mut slur_pending = 0u8;
    // Slide: a "ligadura" vira uma linha reta entre as cabeças.
    let mut slide = false;
    let mut hair: Option<(bool, i32, u8)> = None;
    let mut hair_pending: Option<(bool, u8)> = None;
    // Barras: quem está em grupo, e a direção de haste comum do grupo (pela altura média).
    let groups = beam_groups(items, time);
    let mut in_group: Vec<Option<(usize, bool)>> = vec![None; items.len()];
    for (gi, g) in groups.iter().enumerate() {
        let ss: Vec<i32> = g.iter().filter_map(|&i| if let N(s, ..) = items[i] { Some(s) } else { None }).collect();
        let up = ss.iter().sum::<i32>() < 4 * ss.len() as i32;
        for &i in g {
            in_group[i] = Some((gi, up));
        }
    }
    let mut heads: Vec<Option<(Rect, bool, Fig)>> = vec![None; items.len()];
    for (i, it) in items.iter().enumerate() {
        let x = xs[i];
        match it {
            Above(t) => above = Some(t),
            Below(t) => below = Some(t),
            Stac => stac = true,
            Accent => accent = true,
            Ferm => ferm = true,
            Tie => tie = true,
            Slur(k) => slur_pending = *k,
            Hair(c, k) => hair_pending = Some((*c, *k)),
            Bar => bar_line(fb, x, bottom, gap, 2),
            Double => {
                bar_line(fb, x - 4, bottom, gap, 2);
                bar_line(fb, x + 4, bottom, gap, 2);
            }
            End => {
                bar_line(fb, x - 6, bottom, gap, 2);
                bar_line(fb, x, bottom, gap, gap / 4 + 2);
            }
            RepL => {
                bar_line(fb, x - 6, bottom, gap, gap / 4 + 2);
                bar_line(fb, x + 6, bottom, gap, 2);
                rep_dots(fb, x + 6 + gap / 2, bottom, gap);
            }
            RepR => {
                rep_dots(fb, x - gap / 2, bottom, gap);
                bar_line(fb, x, bottom, gap, 2);
                bar_line(fb, x + 6, bottom, gap, gap / 4 + 2);
            }
            Key(k) => {
                draw_key(fb, x, bottom, gap, clef, *k);
            }
            _ => {
                // Elemento com som (ou pausa): desenha e aplica os modificadores.
                let (head, up, top_y, label_default): (Rect, bool, i32, String) = match it {
                    N(s, fig, acc, dot, stem, _) => {
                        // Em grupo com barra: sem bandeirola, haste na direção do grupo.
                        let (h, up) = match in_group[i] {
                            Some((_, gup)) => note(fb, x, bottom, gap, *s, Fig::Quarter, *dot, if gup { Stem::Up } else { Stem::Down }),
                            None => note(fb, x, bottom, gap, *s, *fig, *dot, *stem),
                        };
                        heads[i] = Some((h, up, *fig));
                        if *acc != Acc::No {
                            draw_acc(fb, h.x - gap, h.y + h.h / 2, gap, *acc);
                        }
                        let top = if up && *fig != Fig::Whole { h.y + h.h / 2 - gap * 7 / 2 } else { h.y };
                        (h, up, top, name_in(*s, clef).to_string())
                    }
                    R(fig, dot, _) => {
                        draw_rest(fb, x, bottom, gap, *fig, *dot);
                        (Rect::new(x - gap / 2, bottom - 3 * gap, gap, gap), true, bottom - 4 * gap, String::new())
                    }
                    Chord(ss, fig, _) => {
                        let mut sorted = ss.to_vec();
                        sorted.sort();
                        let mut prev: Option<i32> = None;
                        let mut first = Rect::new(0, 0, 0, 0);
                        let mut last_h = first;
                        for s in &sorted {
                            ledgers(fb, x, bottom, gap, *s);
                            let mut h = head_rect(x, y_of(bottom, gap, *s), gap);
                            if prev.is_some_and(|p| s - p == 1) {
                                h.x += h.w - 2;
                            }
                            draw_head(fb, h, gap, *fig);
                            if prev.is_none() {
                                first = h;
                            }
                            last_h = h;
                            prev = Some(*s);
                        }
                        let up = (sorted[0] + sorted[sorted.len() - 1]) < 8;
                        if *fig != Fig::Whole {
                            if up {
                                draw_stem(fb, first.x + first.w - 3, first.y + first.h / 2, last_h.y + last_h.h / 2 - gap * 7 / 2, gap, *fig);
                            } else {
                                draw_stem(fb, first.x, last_h.y + last_h.h / 2, first.y + first.h / 2 + gap * 7 / 2, gap, *fig);
                            }
                        }
                        (first, up, last_h.y - gap * 3, String::new())
                    }
                    Two(a, fa_, b, fb_, _) => {
                        let (h, _) = note(fb, x, bottom, gap, *a, *fa_, false, Stem::Down);
                        let (t, _) = note(fb, x, bottom, gap, *b, *fb_, false, Stem::Up);
                        let _ = h;
                        (t, true, t.y + t.h / 2 - gap * 7 / 2, String::new())
                    }
                    _ => unreachable!(),
                };
                let cy = head.y + head.h / 2;
                let cx = head.x + head.w / 2;
                let top_line = bottom - 4 * gap;
                let above_y = top_y.min(top_line) - gap / 2;
                if let Some(t) = above.take() {
                    let size = gap as f32 * 1.1;
                    if t == "1." || t == "2." {
                        // Casa 1 e casa 2: colchete sobre o trecho, até o ritornello (ou o fim).
                        let stop = items[i..].iter().position(|x| matches!(x, RepR | End)).map(|k| xs[i + k]).unwrap_or(r.x + r.w);
                        let (bx, by) = (cx - gap, (bottom - 4 * gap).min(above_y) - gap);
                        fb.fill(Rect::new(bx, by, (stop - gap / 2 - bx).max(gap), 3), INK);
                        fb.fill(Rect::new(bx, by, 3, gap), INK);
                        if t == "1." {
                            fb.fill(Rect::new(stop - gap / 2, by, 3, gap), INK);
                        }
                        f.draw(fb, Face::Strong, size, bx + 10, by + (size * 0.95) as i32, INK, t);
                    } else {
                        let w = f.measure(Face::Strong, size, t);
                        f.draw(fb, Face::Strong, size, cx - w / 2, above_y, INK, t);
                    }
                    if t == "sl." {
                        slide = true;
                    }
                }
                if accent {
                    let (ax, ay) = (cx as f32, (above_y - gap / 2) as f32);
                    let g = gap as f32;
                    line(fb, (ax - 0.5 * g, ay - 0.35 * g), (ax + 0.5 * g, ay), 3.0);
                    line(fb, (ax + 0.5 * g, ay), (ax - 0.5 * g, ay + 0.35 * g), 3.0);
                    accent = false;
                }
                if ferm {
                    let ay = above_y - gap / 4;
                    arc(fb, (cx - gap, ay), (cx + gap, ay), -gap, 3.5);
                    disc(fb, cx, ay - gap / 4, (gap / 6).max(3));
                    ferm = false;
                }
                if stac {
                    let sy = if up { head.y + head.h + gap / 2 } else { head.y - gap / 2 };
                    disc(fb, cx, sy, (gap / 5).max(4));
                    stac = false;
                }
                if let Some(t) = below.take() {
                    let size = gap as f32 * 1.5;
                    let w = f.measure(Face::Display, size, t);
                    f.draw(fb, Face::Display, size, x - gap / 2 - w, below_y + gap, INK, t);
                }
                if tie {
                    if let Some((px, py, pup)) = last {
                        let off = if pup { gap * 2 / 3 } else { -gap * 2 / 3 };
                        let b = if pup { gap / 2 } else { -gap / 2 };
                        arc(fb, (px + gap / 2, py + off), (cx - gap / 2, cy + off), b, 3.0);
                    }
                    tie = false;
                }
                if slur_pending > 0 {
                    slur = Some((cx, cy, up, slur_pending));
                    slur_pending = 0;
                }
                if let Some((sx, sy, sup, left)) = slur {
                    if left <= 1 {
                        let off = if sup { gap * 3 / 4 } else { -gap * 3 / 4 };
                        let b = if sup { gap * 2 / 3 } else { -gap * 2 / 3 };
                        if slide {
                            line(fb, ((sx + gap * 2 / 3) as f32, sy as f32), ((cx - gap * 2 / 3) as f32, cy as f32), 3.0);
                            slide = false;
                        } else if cx != sx {
                            arc(fb, (sx, sy + off), (cx, cy + off), b, 3.0);
                        }
                        slur = None;
                    } else {
                        slur = Some((sx, sy, sup, left - 1));
                    }
                }
                if let Some((c, k)) = hair_pending.take() {
                    hair = Some((c, x - gap / 3, k));
                }
                if let Some((c, sx, left)) = hair {
                    if left <= 1 {
                        let (ex, h) = (x + gap, gap / 2);
                        let my = below_y + gap / 3;
                        let (wide_at_end, xa, xb) = (c, sx as f32, ex as f32);
                        let (y_tip, y_w) = (my as f32, h as f32);
                        if wide_at_end {
                            line(fb, (xa, y_tip), (xb, y_tip - y_w), 3.0);
                            line(fb, (xa, y_tip), (xb, y_tip + y_w), 3.0);
                        } else {
                            line(fb, (xa, y_tip - y_w), (xb, y_tip), 3.0);
                            line(fb, (xa, y_tip + y_w), (xb, y_tip), 3.0);
                        }
                        hair = None;
                    } else {
                        hair = Some((c, sx, left - 1));
                    }
                }
                last = Some((cx, cy, up));
                let _ = &heads;
                if labels {
                    let label = match it {
                        N(.., l) | R(_, _, l) | Chord(_, _, l) | Two(.., l) => *l,
                        _ => "",
                    };
                    let lab = if label.is_empty() { label_default.as_str() } else if label == "-" { "" } else { label };
                    if !lab.is_empty() {
                        let mut size = lsize;
                        while f.measure(Face::Strong, size, lab) > gap * 5 && size > 16.0 {
                            size -= 1.0;
                        }
                        let w = f.measure(Face::Strong, size, lab);
                        f.draw(fb, Face::Strong, size, x - w / 2 + gap / 6, r.y + r.h - 8, INK, lab);
                    }
                }
            }
        }
    }
    // As barras: uma linha grossa ligando as pontas das hastes; semicolcheias ganham a segunda.
    for g in &groups {
        let pts: Vec<(i32, i32, bool, Fig)> = g
            .iter()
            .filter_map(|&i| {
                let (h, up, fig) = heads[i]?;
                let sx = if up { h.x + h.w - 3 } else { h.x };
                Some((sx, h.y + h.h / 2, up, fig))
            })
            .collect();
        if pts.len() < 2 {
            continue;
        }
        let up = pts[0].2;
        let len = gap * 7 / 2;
        let tip = |p: &(i32, i32, bool, Fig)| if up { p.1 - len } else { p.1 + len };
        let (x0, x1) = (pts[0].0, pts[pts.len() - 1].0);
        let (y0, y1) = (tip(&pts[0]), tip(&pts[pts.len() - 1]));
        // Inclinação suave (no máximo meio espaço) e nenhuma haste mais curta que o normal.
        let slope = ((y1 - y0) as f32 / (x1 - x0).max(1) as f32).clamp(-(gap as f32) / 2.0 / (x1 - x0).max(1) as f32 * 2.0, gap as f32 / 2.0 / (x1 - x0).max(1) as f32 * 2.0);
        let mut base = y0 as f32;
        for p in &pts {
            let ly = base + slope * (p.0 - x0) as f32;
            let need = tip(p) as f32;
            if up && ly > need {
                base -= ly - need;
            } else if !up && ly < need {
                base += need - ly;
            }
        }
        let line_y = |x: i32| (base + slope * (x - x0) as f32).round() as i32;
        let t = (gap / 2).max(5);
        // Hastes até a barra.
        for p in &pts {
            let ly = line_y(p.0);
            let (a, b) = if up { (ly, p.1) } else { (p.1, ly) };
            fb.fill(Rect::new(p.0, a.min(b), 3, (b - a).abs()), INK);
        }
        let bar = |fb: &mut Framebuffer, xa: i32, xb: i32, off: i32| {
            for x in xa..=xb + 2 {
                let y = line_y(x) + if up { off } else { -off - t };
                fb.fill(Rect::new(x, if up { y } else { y }, 1, t), INK);
            }
        };
        bar(fb, x0, x1, 0);
        // Segunda barra: entre semicolcheias vizinhas; sozinha, um toco em direção à vizinha.
        let off = t + t * 2 / 3;
        for (k, p) in pts.iter().enumerate() {
            if p.3 != Fig::Sixteenth {
                continue;
            }
            let next16 = pts.get(k + 1).is_some_and(|q| q.3 == Fig::Sixteenth);
            let prev16 = k > 0 && pts[k - 1].3 == Fig::Sixteenth;
            if next16 {
                bar(fb, p.0, pts[k + 1].0, off);
            } else if !prev16 {
                let (xa, xb) = if k > 0 { (p.0 - gap, p.0) } else { (p.0, p.0 + gap) };
                bar(fb, xa, xb, off);
            }
        }
    }
    xs
}

/// Desenha a ilustração da lição dentro de `r`.
pub fn draw_demo(fb: &mut Framebuffer, f: &Fonts, r: Rect, demo: &Demo) {
    match demo {
        Demo::Staff => {
            let (bottom, gap) = draw_staff(fb, Rect::new(r.x, r.y, r.w, r.h - 60), 26);
            for k in 0..5 {
                f.draw(fb, Face::Strong, 22.0, r.x + r.w - 26, bottom - k * gap + 8, MUTED, &format!("{}", k + 1));
            }
        }
        Demo::Score(s) => draw_score(fb, f, r, s),
        Demo::Durations => {
            let (bottom, gap) = draw_staff(fb, Rect::new(r.x, r.y, r.w, r.h - 60), 26);
            let x0 = r.x + gap * 5;
            let items = [(Fig::Whole, "4 tempos"), (Fig::Half, "2 tempos"), (Fig::Quarter, "1 tempo"), (Fig::Eighth, "½ tempo")];
            let step_x = (r.w - gap * 5) / 4;
            for (k, (fig, lab)) in items.iter().enumerate() {
                let x = x0 + gap + k as i32 * step_x;
                draw_note(fb, x, bottom, gap, 3, *fig);
                let w = f.measure(Face::Strong, 24.0, lab);
                f.draw(fb, Face::Strong, 24.0, x - w / 2 + gap / 4, r.y + r.h - 8, INK, lab);
            }
        }
        Demo::Keys(keys) => {
            let n = keys.len().max(1) as i32;
            let cw = r.w / n;
            for (k, (key, name)) in keys.iter().enumerate() {
                let cx = r.x + k as i32 * cw;
                let gap = 22;
                let bottom = r.y + (r.h - 60) / 2 + 2 * gap;
                staff_lines(fb, cx + 6, cw - 22, bottom, gap);
                draw_clef(fb, cx + 6, bottom, gap, Clef::Sol);
                draw_key(fb, cx + 6 + gap * 9 / 2, bottom, gap, Clef::Sol, *key);
                let mut size = 26.0;
                while f.measure(Face::Strong, size, name) > cw - 16 && size > 16.0 {
                    size -= 1.0;
                }
                let w = f.measure(Face::Strong, size, name);
                f.draw(fb, Face::Strong, size, cx + (cw - 16 - w) / 2 + 6, r.y + r.h - 8, INK, name);
            }
        }
        Demo::Words(rows) => {
            let n = rows.len().max(1) as i32;
            let rh = ((r.h - 20) / n).min(60);
            let y0 = r.y + (r.h - rh * n) / 2;
            let tw = rows.iter().map(|(t, _)| f.measure(Face::Display, 32.0, t)).max().unwrap_or(0).min(r.w / 2);
            for (k, (t, m)) in rows.iter().enumerate() {
                let base = y0 + k as i32 * rh + rh * 2 / 3;
                f.draw(fb, Face::Display, 32.0, r.x + 24, base, INK, t);
                f.draw(fb, Face::Body, 28.0, r.x + 24 + tw + 36, base, INK, m);
                if k + 1 < rows.len() {
                    fb.fill(Rect::new(r.x + 24, y0 + (k as i32 + 1) * rh - 1, r.w - 48, 1), 0xB0);
                }
            }
        }
        Demo::Tab(notes) => {
            // Partitura em cima, tablatura embaixo, alinhadas nota a nota.
            let items: Vec<It> = notes.iter().map(|(s, _, _)| up(*s, SM, "-")).collect();
            let score_r = Rect::new(r.x, r.y, r.w, r.h * 55 / 100);
            let xs = draw_items(fb, f, score_r, Clef::Sol, 0, None, &items, false);
            let lg = 17;
            let ty = r.y + r.h * 62 / 100;
            let end = xs.last().copied().unwrap_or(r.x + r.w) + 50;
            for k in 0..6 {
                fb.fill(Rect::new(r.x, ty + k * lg, end - r.x, 2), INK);
            }
            for (k, ch) in ["T", "A", "B"].iter().enumerate() {
                f.draw(fb, Face::Display, 24.0, r.x + 16, ty + 22 + k as i32 * 28, INK, ch);
            }
            for (i, (_, string, fret)) in notes.iter().enumerate() {
                let x = xs[i] + 4;
                let y = ty + (*string as i32 - 1) * lg;
                let t = fret.to_string();
                let w = f.measure(Face::Strong, 24.0, &t);
                fb.fill(Rect::new(x - w / 2 - 4, y - 11, w + 8, 22), ui::SURFACE);
                f.draw(fb, Face::Strong, 24.0, x - w / 2, y + 9, INK, &t);
            }
        }
    }
}

// ---------- Treinos ----------

/// Os quatro treinos.
pub const MODES: [&str; 4] = ["Notas", "Ritmo", "Armaduras", "Intervalos"];

/// Níveis do treino de notas: (nome, clave, passos possíveis).
pub const LEVELS: [(&str, Clef, &[i32]); 6] = [
    ("Linhas", Clef::Sol, &[0, 2, 4, 6, 8]),
    ("Espaços", Clef::Sol, &[1, 3, 5, 7]),
    ("Pauta", Clef::Sol, &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
    ("Suplem.", Clef::Sol, &[-2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
    ("Violão", Clef::Sol, &[-7, -6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
    ("Clave fá", Clef::Fa, &[-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
];

/// Os 7 nomes nos botões, na ordem da escala.
pub const ANSWERS: [&str; 7] = ["Dó", "Ré", "Mi", "Fá", "Sol", "Lá", "Si"];
const RHYTHM_ANS: [&str; 7] = ["4", "3", "2", "1½", "1", "½", "¼"];
const KEY_ANS: [&str; 7] = ["Dó", "Sol", "Ré", "Lá", "Fá", "Sib", "Mib"];
const KEY_SIG: [i8; 7] = [0, 1, 2, 3, -1, -2, -3];
const INTERVAL_ANS: [&str; 7] = ["2ª", "3ª", "4ª", "5ª", "6ª", "7ª", "8ª"];

/// Figuras do treino de ritmo: (pausa?, figura, ponto, resposta).
const RHYTHM: [(bool, Fig, bool, usize); 12] = [
    (false, Fig::Whole, false, 0),
    (false, Fig::Half, true, 1),
    (false, Fig::Half, false, 2),
    (false, Fig::Quarter, true, 3),
    (false, Fig::Quarter, false, 4),
    (false, Fig::Eighth, false, 5),
    (false, Fig::Sixteenth, false, 6),
    (true, Fig::Whole, false, 0),
    (true, Fig::Half, false, 2),
    (true, Fig::Quarter, false, 4),
    (true, Fig::Eighth, false, 5),
    (true, Fig::Sixteenth, false, 6),
];

pub fn answers(mode: usize) -> &'static [&'static str; 7] {
    match mode {
        1 => &RHYTHM_ANS,
        2 => &KEY_ANS,
        3 => &INTERVAL_ANS,
        _ => &ANSWERS,
    }
}

pub fn prompt(mode: usize) -> &'static str {
    match mode {
        1 => "QUANTOS TEMPOS VALE? (SEMÍNIMA = 1)",
        2 => "QUE TOM MAIOR É ESTE?",
        3 => "QUE INTERVALO É ESTE?",
        _ => "QUE NOTA É ESTA?",
    }
}

pub fn tip(mode: usize) -> &'static str {
    match mode {
        1 => "Cheia ou vazada, haste e bandeirolas; o ponto soma metade.",
        2 => "Sustenidos: último + meio tom. Bemóis: o penúltimo.",
        3 => "Conte as notas do caminho, incluindo as duas pontas.",
        _ => "Olhe a linha ou o espaço e suba a partir do Sol.",
    }
}

/// Uma pergunta do treino.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Q {
    pub mode: usize,
    pub level: usize,
    pub a: i32,
    pub b: i32,
}

impl Q {
    /// Sorteia uma pergunta diferente da anterior.
    pub fn new(mode: usize, level: usize, rng: &mut crate::minigame::Rng, prev: Option<Q>) -> Q {
        let mut pick = |n: usize| (rng.next() % n as u64) as i32;
        for _ in 0..20 {
            let q = match mode {
                1 => Q { mode, level, a: pick(RHYTHM.len()), b: 0 },
                2 => Q { mode, level, a: pick(KEY_SIG.len()), b: 0 },
                3 => {
                    let a = pick(7) - 2;
                    Q { mode, level, a, b: a + 1 + pick(7) }
                }
                _ => {
                    let steps = LEVELS[level.min(LEVELS.len() - 1)].2;
                    Q { mode, level, a: steps[pick(steps.len()) as usize], b: 0 }
                }
            };
            if Some(q) != prev {
                return q;
            }
        }
        prev.unwrap_or(Q { mode, level, a: 0, b: 0 })
    }

    /// Índice da resposta certa nos botões.
    pub fn correct(&self) -> usize {
        match self.mode {
            1 => RHYTHM[self.a as usize].3,
            2 => self.a as usize,
            3 => (self.b - self.a - 1) as usize,
            _ => {
                let clef = LEVELS[self.level.min(LEVELS.len() - 1)].1;
                ANSWERS.iter().position(|x| *x == name_in(self.a, clef)).unwrap_or(0)
            }
        }
    }

    pub fn answer(&self) -> &'static str {
        answers(self.mode)[self.correct()]
    }

    /// Chave do recorde (o treino de notas mantém os números antigos dos níveis).
    pub fn record_key(&self) -> usize {
        if self.mode == 0 { self.level } else { 10 * self.mode }
    }

    pub fn draw(&self, fb: &mut Framebuffer, r: Rect) {
        let clef = if self.mode == 0 { LEVELS[self.level.min(LEVELS.len() - 1)].1 } else { Clef::Sol };
        let (bottom, gap) = draw_staff_clef(fb, Rect::new(r.x + 30, r.y + 40, r.w - 60, r.h - 80), 40, clef);
        let cx = r.x + r.w / 2 + 40;
        match self.mode {
            1 => {
                let (rest, fig, dot, _) = RHYTHM[self.a as usize];
                if rest {
                    draw_rest(fb, cx, bottom, gap, fig, dot);
                } else {
                    note(fb, cx, bottom, gap, 3, fig, dot, Stem::Auto);
                }
            }
            2 => {
                // A armadura logo depois da clave, como na partitura de verdade.
                let key = KEY_SIG[self.a as usize];
                draw_key(fb, r.x + 30 + gap * 9 / 2, bottom, gap, Clef::Sol, key);
            }
            3 => {
                note(fb, cx - 110, bottom, gap, self.a, Fig::Whole, false, Stem::Auto);
                note(fb, cx + 110, bottom, gap, self.b, Fig::Whole, false, Stem::Auto);
            }
            _ => draw_note(fb, cx, bottom, gap, self.a, Fig::Whole),
        }
    }
}

fn path() -> std::path::PathBuf {
    let d = crate::home_dir().join("data/violao");
    let _ = std::fs::create_dir_all(&d);
    d.join("leitura.txt")
}

/// Recorde de acertos seguidos por treino/nível.
pub fn best(key: usize) -> u32 {
    std::fs::read_to_string(path()).unwrap_or_default().lines().filter_map(|l| l.split_once('|')).filter(|(lv, _)| lv.parse::<usize>().ok() == Some(key)).filter_map(|(_, n)| n.parse().ok()).max().unwrap_or(0)
}

pub fn save_best(key: usize, n: u32) {
    if n > best(key) {
        let mut s = std::fs::read_to_string(path()).unwrap_or_default();
        s.push_str(&format!("{key}|{n}\n"));
        let _ = std::fs::write(path(), s);
    }
}

/// Progresso do curso novo (o antigo, de 10 lições, ficava em curso.txt).
pub fn done_path() -> std::path::PathBuf {
    path().with_file_name("curso2.txt")
}

/// Lições já vistas (índices globais).
pub fn done() -> Vec<usize> {
    std::fs::read_to_string(done_path()).unwrap_or_default().lines().filter_map(|l| l.parse().ok()).collect()
}

pub fn mark_done(k: usize) {
    let mut d = done();
    if !d.contains(&k) {
        d.push(k);
        let _ = std::fs::write(done_path(), d.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("\n"));
    }
}

/// Lições vistas de um módulo.
pub fn done_in(m: usize) -> usize {
    let (a, b) = (first(m), first(m) + MODULES[m].lessons.len());
    done().iter().filter(|g| (a..b).contains(*g)).count()
}

/// Próxima lição a estudar: a primeira não vista.
pub fn next_lesson() -> usize {
    let d = done();
    (0..total()).find(|g| !d.contains(g)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nomes_na_pauta() {
        assert_eq!(note_name(0), "Mi");
        assert_eq!(note_name(2), "Sol");
        assert_eq!(note_name(8), "Fá");
        assert_eq!(note_name(-2), "Dó");
        assert_eq!(note_name(-7), "Mi");
        assert_eq!(note_name(10), "Lá");
        // Clave de fá: linhas Sol Si Ré Fá Lá; Dó central com linha suplementar em cima.
        let fa: Vec<&str> = [0, 2, 4, 6, 8].iter().map(|s| name_in(*s, Clef::Fa)).collect();
        assert_eq!(fa, ["Sol", "Si", "Ré", "Fá", "Lá"]);
        assert_eq!(name_in(10, Clef::Fa), "Dó");
        // Toda resposta possível está nos botões.
        for (_, clef, steps) in LEVELS {
            for s in steps.iter() {
                assert!(ANSWERS.contains(&name_in(*s, clef)));
            }
        }
    }

    #[test]
    fn curso_consistente() {
        assert!(total() >= 60, "{}", total());
        for (m, md) in MODULES.iter().enumerate() {
            assert!(md.lessons.len() <= 14, "módulo {m} grande demais para a lista");
            for l in md.lessons {
                assert!(l.check.ok < l.check.opts.len(), "{}", l.title);
                assert!(l.check.opts.len() <= 4, "{}", l.title);
                assert!(l.text.len() <= 3, "{}", l.title);
            }
        }
        assert_eq!(locate(first(2) + 3), (2, 3));
        assert_eq!(locate(total() - 1).0, MODULES.len() - 1);
    }

    #[test]
    fn armaduras_e_treinos() {
        // A ordem dos sustenidos (Fá Dó Sol Ré Lá Mi Si) e dos bemóis (Si Mi Lá Ré Sol Dó Fá).
        let s: Vec<&str> = SHARPS.iter().map(|x| note_name(*x)).collect();
        assert_eq!(s, ["Fá", "Dó", "Sol", "Ré", "Lá", "Mi", "Si"]);
        let b: Vec<&str> = FLATS.iter().map(|x| note_name(*x)).collect();
        assert_eq!(b, ["Si", "Mi", "Lá", "Ré", "Sol", "Dó", "Fá"]);
        let mut rng = crate::minigame::Rng::new();
        let mut prev = None;
        for mode in 0..4 {
            for level in 0..LEVELS.len() {
                for _ in 0..200 {
                    let q = Q::new(mode, level, &mut rng, prev);
                    assert_ne!(Some(q), prev);
                    assert!(q.correct() < 7);
                    if mode == 3 {
                        assert!((1..=7).contains(&(q.b - q.a)) && q.b <= 11);
                    }
                    prev = Some(q);
                }
            }
        }
        // Intervalo: Dó a Sol é 5ª.
        assert_eq!(Q { mode: 3, level: 0, a: -2, b: 2 }.answer(), "5ª");
        assert_eq!(Q { mode: 1, level: 0, a: 3, b: 0 }.answer(), "1½");
        assert_eq!(Q { mode: 2, level: 0, a: 5, b: 0 }.answer(), "Sib");
    }
}
