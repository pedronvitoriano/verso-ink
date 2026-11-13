//! Texto com a família Inter (SIL OFL), embutida no binário.

use crate::fb::Framebuffer;
use fontdue::{Font, FontSettings, Metrics};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use unicode_normalization::UnicodeNormalization;

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
    /// Glifos já rasterizados (fonte, tamanho, letra): desenhar texto não refaz a curva de cada letra.
    glyphs: Mutex<HashMap<(u8, u32, char), (Metrics, Arc<Vec<u8>>)>>,
}

/// Limite do cache de glifos (cada um tem poucos KB); passou, começa de novo.
const GLYPH_CACHE: usize = 6000;

fn load(bytes: &'static [u8]) -> Font {
    Font::from_bytes(bytes, FontSettings::default()).expect("fonte embutida inválida")
}

impl Fonts {
    pub fn new() -> Self {
        Fonts {
            display: load(include_bytes!("../assets/fonts/InterDisplay-Bold.ttf")),
            strong: load(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
            body: load(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
            glyphs: Mutex::new(HashMap::new()),
        }
    }

    fn font(&self, face: Face) -> &Font {
        match face {
            Face::Display => &self.display,
            Face::Strong => &self.strong,
            Face::Body => &self.body,
        }
    }

    fn glyph(&self, face: Face, size: f32, c: char) -> (Metrics, Arc<Vec<u8>>) {
        let key = (face as u8, size.to_bits(), c);
        let mut cache = self.glyphs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(g) = cache.get(&key) {
            return g.clone();
        }
        if cache.len() >= GLYPH_CACHE {
            cache.clear();
        }
        let (m, bitmap) = self.font(face).rasterize(c, size);
        let g = (m, Arc::new(bitmap));
        cache.insert(key, g.clone());
        g
    }

    /// Largura do texto em pixels, com kerning.
    pub fn measure(&self, face: Face, size: f32, text: &str) -> i32 {
        // Acentos como caractere separado (NFD, comum em nomes de arquivo) viram um caractere só.
        let text: String = text.nfc().collect();
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
        let _p = crate::perfil::Guard::new("texto");
        let text: String = text.nfc().collect();
        let f = self.font(face);
        let mut pen = x as f32;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                pen += f.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            let (m, bitmap) = self.glyph(face, size, c);
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

#[cfg(test)]
mod cache_igual {
    use super::*;

    /// O cache de glifos desenha exatamente o mesmo que rasterizar a cada vez (como antes).
    #[test]
    fn texto_com_cache_igual_ao_antigo() {
        let f = Fonts::new();
        let draw_old = |fb: &mut Framebuffer, face: Face, size: f32, x: i32, base: i32, ink: u8, text: &str| {
            let text: String = text.nfc().collect();
            let font = f.font(face);
            let mut pen = x as f32;
            let mut prev = None;
            for c in text.chars() {
                if let Some(p) = prev {
                    pen += font.horizontal_kern(p, c, size).unwrap_or(0.0);
                }
                let (m, bitmap) = font.rasterize(c, size);
                let (gx, gy) = (pen.round() as i32 + m.xmin, base - m.height as i32 - m.ymin);
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
        };
        let cases = [(Face::Display, 130.0, "Histórias"), (Face::Body, 36.0, "Ação, coração e pão — “aspas” 1234"), (Face::Strong, 30.0, "Lances anteriores ‹ ›"), (Face::Body, 21.5, "Pequeno é ÇÃÕ ü")];
        for _ in 0..2 {
            for (face, size, t) in cases {
                let (mut a, mut b) = (Framebuffer::memory(), Framebuffer::memory());
                draw_old(&mut a, face, size, 37, 300, 0x20, t);
                f.draw(&mut b, face, size, 37, 300, 0x20, t);
                assert!(a.buf == b.buf, "{t} {size}");
            }
        }
    }
}
