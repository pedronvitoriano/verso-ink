//! Curso de partitura para quem já lê cifra: lições curtas com a pauta desenhada e um treino de
//! leitura (aparece uma nota, toque no nome). Tudo original; a melodia de exemplo (Ode à
//! Alegria, Beethoven) é de domínio público.
//!
//! Posições na pauta em "passos" a partir da 1ª linha (Mi): 0 Mi, 1 Fá, 2 Sol, … 8 Fá (5ª
//! linha); negativos abaixo (−2 é o Dó com linha suplementar). O violão é escrito uma oitava
//! acima do som, como em toda partitura de violão.

use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const NAMES: [&str; 7] = ["Mi", "Fá", "Sol", "Lá", "Si", "Dó", "Ré"];

pub fn note_name(step: i32) -> &'static str {
    NAMES[step.rem_euclid(7) as usize]
}

#[derive(Clone, Copy, PartialEq)]
pub enum Fig {
    Whole,
    Half,
    Quarter,
    Eighth,
}

/// Pauta com a clave de sol. Devolve a posição y da 1ª linha (a de baixo) e o espaço entre linhas.
pub fn draw_staff(fb: &mut Framebuffer, r: Rect, gap: i32) -> (i32, i32) {
    let bottom = r.y + r.h / 2 + 2 * gap;
    for k in 0..5 {
        fb.fill(Rect::new(r.x, bottom - k * gap, r.w, 2), INK);
    }
    // Clave: o desenho do Codex, ou um "G" grande se faltar.
    // A clave vai da linha suplementar de baixo até acima da 5ª linha (≈ 7 espaços de altura).
    let ch = gap * 15 / 2;
    if !crate::sprites::draw(fb, "clave-sol", Rect::new(r.x - ch / 5, bottom - 6 * gap, ch, ch)) {
        fb.round_outline(Rect::new(r.x + 10, bottom - 3 * gap, gap * 2, gap * 3), gap, 4, INK, PAPER);
    }
    (bottom, gap)
}

/// Uma nota na pauta: cabeça (cheia ou vazada), haste, bandeirola e linhas suplementares.
pub fn draw_note(fb: &mut Framebuffer, x: i32, bottom: i32, gap: i32, step: i32, fig: Fig) {
    let y = bottom - step * gap / 2;
    // Linhas suplementares (abaixo de Ré e acima de Sol do alto).
    let mut s = -2;
    while s >= step {
        fb.fill(Rect::new(x - gap, bottom - s * gap / 2, gap * 2 + gap / 3, 2), INK);
        s -= 2;
    }
    let mut s = 10;
    while s <= step {
        fb.fill(Rect::new(x - gap, bottom - s * gap / 2, gap * 2 + gap / 3, 2), INK);
        s += 2;
    }
    let (hw, hh) = (gap * 13 / 10, gap);
    let head = Rect::new(x - hw / 2 + gap / 6, y - hh / 2, hw, hh);
    match fig {
        Fig::Whole | Fig::Half => fb.round_outline(head, hh / 2, (gap / 5).max(3), INK, PAPER),
        _ => fb.round_rect(head, hh / 2, INK),
    }
    if fig != Fig::Whole {
        // Haste para cima nas notas baixas, para baixo nas altas.
        let up = step < 4;
        let sx = if up { head.x + head.w - 3 } else { head.x };
        let (y0, y1) = if up { (y - gap * 7 / 2, y) } else { (y, y + gap * 7 / 2) };
        fb.fill(Rect::new(sx, y0, 3, y1 - y0), INK);
        if fig == Fig::Eighth {
            let fy = if up { y0 } else { y1 };
            let dir = if up { 1.0 } else { -1.0 };
            ui::stroke(fb, (sx as f32 + 1.0, fy as f32), (sx as f32 + gap as f32 * 0.9, fy as f32 + dir * gap as f32 * 1.4), 4.0, INK);
        }
    }
}

/// Ilustração de cada lição.
pub enum Demo {
    /// Pauta vazia com as linhas numeradas.
    Staff,
    /// Notas com o nome embaixo (passo, figura, legenda opcional).
    Notes(&'static [(i32, Fig, &'static str)]),
    /// As figuras e quanto valem.
    Durations,
}

pub struct Lesson {
    pub title: &'static str,
    pub text: &'static [&'static str],
    pub demo: Demo,
}

use Fig::*;

pub const LESSONS: [Lesson; 10] = [
    Lesson {
        title: "A pauta e a clave",
        text: &[
            "Partitura é cifra com mais detalhe: além de qual nota, ela diz quando e por quanto tempo.",
            "As notas moram em 5 linhas e 4 espaços, contados de baixo para cima. Quanto mais alto na pauta, mais aguda a nota.",
            "O desenho do começo é a clave de sol. Ela enrola na 2ª linha e diz: aqui é o Sol. Partitura de violão usa sempre essa clave.",
        ],
        demo: Demo::Staff,
    },
    Lesson {
        title: "Notas nas linhas",
        text: &["De baixo para cima, as linhas são Mi, Sol, Si, Ré e Fá.", "Uma frase ajuda a guardar: \"Mi Sol, Si Ré Fá\". Repita em voz alta três vezes."],
        demo: Demo::Notes(&[(0, Whole, ""), (2, Whole, ""), (4, Whole, ""), (6, Whole, ""), (8, Whole, "")]),
    },
    Lesson {
        title: "Notas nos espaços",
        text: &["Nos espaços, de baixo para cima: Fá, Lá, Dó e Mi.", "Juntando linhas e espaços, a pauta é só a escala subindo: Mi, Fá, Sol, Lá, Si, Dó, Ré, Mi, Fá."],
        demo: Demo::Notes(&[(1, Whole, ""), (3, Whole, ""), (5, Whole, ""), (7, Whole, "")]),
    },
    Lesson {
        title: "Acima e abaixo da pauta",
        text: &[
            "Quando a nota passa da pauta, ganha uma linhazinha só para ela: a linha suplementar.",
            "Logo abaixo da 1ª linha fica o Ré; com uma linha suplementar, o Dó. Acima da 5ª linha, Sol; com linha, Lá.",
        ],
        demo: Demo::Notes(&[(-2, Whole, ""), (-1, Whole, ""), (9, Whole, ""), (10, Whole, "")]),
    },
    Lesson {
        title: "Quanto dura cada nota",
        text: &[
            "O formato da nota diz quanto tempo ela dura. Conte em voz alta \"1, 2, 3, 4\" num ritmo constante, como um relógio.",
            "Semibreve (vazada, sem haste): 4 tempos. Mínima (vazada, com haste): 2. Semínima (cheia, com haste): 1. Colcheia (com bandeirola): meio tempo.",
        ],
        demo: Demo::Durations,
    },
    Lesson {
        title: "Compasso",
        text: &[
            "A partitura é dividida em compassos por barras verticais. O 4/4 no começo quer dizer: 4 tempos em cada compasso, e a semínima vale 1.",
            "A batida que você já faz na cifra cabe nisso: \"1 e 2 e 3 e 4 e\". O tempo 1 é o mais forte.",
        ],
        demo: Demo::Notes(&[(2, Quarter, "1"), (2, Quarter, "2"), (4, Quarter, "3"), (4, Quarter, "4")]),
    },
    Lesson {
        title: "Sustenido e bemol",
        text: &[
            "O # (sustenido) sobe a nota meio tom: uma casa para frente no violão. O b (bemol) desce meio tom: uma casa para trás.",
            "É o mesmo # e b das cifras: F#m é o Fá sustenido menor. Quando aparecem logo depois da clave, valem para a música inteira.",
        ],
        demo: Demo::Notes(&[(1, Whole, "Fá"), (1, Whole, "Fá#"), (4, Whole, "Si"), (4, Whole, "Sib")]),
    },
    Lesson {
        title: "Do papel para o violão",
        text: &[
            "As cordas soltas, da mais grossa para a mais fina: Mi, Lá, Ré, Sol, Si e Mi. Na pauta do violão elas ficam assim:",
            "A 6ª corda fica bem abaixo da pauta (três linhas suplementares); a 1ª, no último espaço. Cada casa do braço é meio tom.",
        ],
        demo: Demo::Notes(&[(-7, Whole, "6ª"), (-4, Whole, "5ª"), (-1, Whole, "4ª"), (2, Whole, "3ª"), (4, Whole, "2ª"), (7, Whole, "1ª")]),
    },
    Lesson {
        title: "Tablatura e partitura",
        text: &[
            "Tablatura mostra onde pôr o dedo (corda e casa), mas não o ritmo. Partitura mostra a nota e o tempo, mas não o dedo.",
            "Por isso muitas músicas vêm com as duas juntas: lendo partitura você entende o ritmo da tablatura.",
        ],
        demo: Demo::Notes(&[(0, Quarter, "4ª·2"), (1, Quarter, "4ª·3"), (2, Quarter, "3ª·0"), (-1, Quarter, "4ª·0")]),
    },
    Lesson {
        title: "Sua primeira melodia",
        text: &[
            "Ode à Alegria (Beethoven, domínio público). Embaixo de cada nota: a corda e a casa. Toque devagar, uma semínima por tempo.",
            "Mi Mi Fá Sol | Sol Fá Mi Ré | Dó Dó Ré Mi | Mi Ré Ré. Quando sair sem olhar o braço, você está lendo partitura.",
        ],
        demo: Demo::Notes(&[(0, Quarter, "4ª·2"), (0, Quarter, "4ª·2"), (1, Quarter, "4ª·3"), (2, Quarter, "3ª·0"), (2, Quarter, "3ª·0"), (1, Quarter, "4ª·3"), (0, Quarter, "4ª·2"), (-1, Quarter, "4ª·0")]),
    },
];

/// Desenha a ilustração da lição dentro de `r`.
pub fn draw_demo(fb: &mut Framebuffer, f: &Fonts, r: Rect, demo: &Demo) {
    let gap = 26;
    let (bottom, gap) = draw_staff(fb, Rect::new(r.x, r.y, r.w, r.h - 60), gap);
    let x0 = r.x + gap * 5;
    match demo {
        Demo::Staff => {
            for k in 0..5 {
                f.draw(fb, Face::Strong, 22.0, r.x + r.w - 26, bottom - k * gap + 8, MUTED, &format!("{}", k + 1));
            }
        }
        Demo::Notes(notes) => {
            let step_x = ((r.w - gap * 6) / notes.len().max(1) as i32).min(gap * 5);
            for (k, (s, fig, label)) in notes.iter().enumerate() {
                let x = x0 + gap + k as i32 * step_x;
                draw_note(fb, x, bottom, gap, *s, *fig);
                // Acidente antes da nota (sustenido ou bemol), quando a legenda pede.
                let acc = if label.ends_with('#') { "#" } else if label.ends_with('b') && label.len() > 2 { "b" } else { "" };
                if !acc.is_empty() {
                    f.draw(fb, Face::Display, gap as f32 * 1.4, x - gap * 2, bottom - s * gap / 2 + gap / 2, INK, acc);
                }
                let lab = if label.is_empty() { note_name(*s) } else { label };
                let w = f.measure(Face::Strong, 24.0, lab);
                f.draw(fb, Face::Strong, 24.0, x - w / 2 + gap / 4, r.y + r.h - 8, INK, lab);
            }
        }
        Demo::Durations => {
            let items = [(Whole, "4"), (Half, "2"), (Quarter, "1"), (Eighth, "½")];
            let step_x = (r.w - gap * 5) / 4;
            for (k, (fig, v)) in items.iter().enumerate() {
                let x = x0 + gap + k as i32 * step_x;
                draw_note(fb, x, bottom, gap, 3, *fig);
                let lab = format!("{v} tempo{}", if *v == "1" || *v == "½" { "" } else { "s" });
                let w = f.measure(Face::Strong, 24.0, &lab);
                f.draw(fb, Face::Strong, 24.0, x - w / 2 + gap / 4, r.y + r.h - 8, INK, &lab);
            }
        }
    }
}

// ---------- Treino de leitura ----------

/// Níveis: (nome, passos possíveis).
pub const LEVELS: [(&str, &[i32]); 4] = [
    ("Linhas", &[0, 2, 4, 6, 8]),
    ("Espaços", &[1, 3, 5, 7]),
    ("Pauta toda", &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
    ("Com suplementares", &[-2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]),
];

/// Os 7 nomes nos botões, na ordem da escala.
pub const ANSWERS: [&str; 7] = ["Dó", "Ré", "Mi", "Fá", "Sol", "Lá", "Si"];

fn path() -> std::path::PathBuf {
    let d = crate::home_dir().join("data/violao");
    let _ = std::fs::create_dir_all(&d);
    d.join("leitura.txt")
}

/// Recorde de acertos seguidos por nível.
pub fn best(level: usize) -> u32 {
    std::fs::read_to_string(path()).unwrap_or_default().lines().filter_map(|l| l.split_once('|')).filter(|(lv, _)| lv.parse::<usize>().ok() == Some(level)).filter_map(|(_, n)| n.parse().ok()).max().unwrap_or(0)
}

pub fn save_best(level: usize, n: u32) {
    if n > best(level) {
        let mut s = std::fs::read_to_string(path()).unwrap_or_default();
        s.push_str(&format!("{level}|{n}\n"));
        let _ = std::fs::write(path(), s);
    }
}

pub fn done_path() -> std::path::PathBuf {
    path().with_file_name("curso.txt")
}

/// Lições já vistas.
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
        // Toda resposta possível está nos botões.
        for (_, steps) in LEVELS {
            for s in steps.iter() {
                assert!(ANSWERS.contains(&note_name(*s)));
            }
        }
    }
}
