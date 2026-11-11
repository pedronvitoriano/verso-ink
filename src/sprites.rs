//! Sprites dos jogos de masmorra (assets/sprites → /mnt/us/verso-ink/sprites): carregados na
//! primeira vez, reduzidos para o tamanho da casa e guardados na memória. O fundo branco do
//! sprite não apaga a casa: só os tons mais escuros que o papel são desenhados.

use crate::fb::{Framebuffer, Rect};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<(String, i32), Option<Vec<u8>>>> = Default::default();
}

fn path(name: &str) -> std::path::PathBuf {
    crate::home_dir().join("sprites").join(format!("{name}.png"))
}

#[allow(dead_code)]
pub fn exists(name: &str) -> bool {
    path(name).exists()
}

/// Desenha o sprite `name` centrado em `r` (quadrado do lado menor). Devolve false se não houver.
pub fn draw(fb: &mut Framebuffer, name: &str, r: Rect) -> bool {
    let side = r.w.min(r.h);
    let px = CACHE.with(|c| {
        c.borrow_mut()
            .entry((name.to_string(), side))
            .or_insert_with(|| {
                let img = image::open(path(name)).ok()?.to_luma8();
                Some(image::imageops::resize(&img, side as u32, side as u32, image::imageops::FilterType::Triangle).into_raw())
            })
            .clone()
    });
    let Some(px) = px else { return false };
    let (x0, y0) = (r.x + (r.w - side) / 2, r.y + (r.h - side) / 2);
    for y in 0..side {
        for x in 0..side {
            let v = px[(y * side + x) as usize];
            if v < 0xF0 {
                // Opaco nos tons do desenho; a borda clara funde com o fundo.
                fb.blend(x0 + x, y0 + y, v, if v < 0xD0 { 255 } else { 120 });
            }
        }
    }
    true
}
