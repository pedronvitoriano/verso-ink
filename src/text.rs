//! Texto com a família Inter (SIL OFL), embutida no binário.

use crate::fb::Framebuffer;
use fontdue::{Font, FontSettings};

#[derive(Clone, Copy)]
pub enum Face {
    /// InterDisplay Bold: títulos e relógio.
    Display,
    /// Inter SemiBold: rótulos.
    Strong,
    /// Inter Regular: texto corrido.
    Body,
}

pub struct Fonts {
    display: Font,
    strong: Font,
    body: Font,
}

fn load(bytes: &'static [u8]) -> Font {
    Font::from_bytes(bytes, FontSettings::default()).expect("fonte embutida inválida")
}

impl Fonts {
    pub fn new() -> Self {
        Fonts {
            display: load(include_bytes!("../assets/fonts/InterDisplay-Bold.ttf")),
            strong: load(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
            body: load(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        }
    }

    fn font(&self, face: Face) -> &Font {
        match face {
            Face::Display => &self.display,
            Face::Strong => &self.strong,
            Face::Body => &self.body,
        }
    }

    /// Largura do texto em pixels, com kerning.
    pub fn measure(&self, face: Face, size: f32, text: &str) -> i32 {
        let f = self.font(face);
        let mut w = 0.0;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                w += f.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            w += f.metrics(c, size).advance_width;
            prev = Some(c);
        }
        w.round() as i32
    }

    /// Desenha `text` com a linha de base em `baseline`. `ink` é o tom (0 = preto).
    pub fn draw(&self, fb: &mut Framebuffer, face: Face, size: f32, x: i32, baseline: i32, ink: u8, text: &str) -> i32 {
        let f = self.font(face);
        let mut pen = x as f32;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                pen += f.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            let (m, bitmap) = f.rasterize(c, size);
            let gx = pen.round() as i32 + m.xmin;
            let gy = baseline - m.height as i32 - m.ymin;
            for row in 0..m.height {
                for col in 0..m.width {
                    let a = bitmap[row * m.width + col];
                    if a > 0 {
                        fb.blend(gx + col as i32, gy + row as i32, ink, a);
                    }
                }
            }
            pen += m.advance_width;
            prev = Some(c);
        }
        pen.round() as i32
    }
}
