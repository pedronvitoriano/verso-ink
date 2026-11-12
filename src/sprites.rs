//! Sprites dos jogos de masmorra (assets/sprites → /mnt/us/verso-ink/sprites): carregados na
//! primeira vez, reduzidos para o tamanho da casa e guardados na memória. Se o PNG tem
//! transparência (peças de xadrez, ladrilhos), ela vale; se não tem, o fundo branco do sprite
//! não apaga a casa: só os tons mais escuros que o papel são desenhados.

use crate::fb::{Framebuffer, Rect};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<(String, i32), Option<Vec<u8>>>> = Default::default();
}

fn path(name: &str) -> std::path::PathBuf {
    crate::home_dir().join("sprites").join(format!("{name}.png"))
}

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
                let img = image::open(path(name)).ok()?;
                let alpha = img.color().has_alpha();
                let la = image::imageops::resize(&img.to_luma_alpha8(), side as u32, side as u32, image::imageops::FilterType::Triangle);
                let mut out = Vec::with_capacity((side * side * 2) as usize);
                for p in la.pixels() {
                    let v = p[0];
                    // Sem transparência no arquivo: opaco nos tons do desenho, a borda clara funde.
                    let a = if alpha { p[1] } else if v < 0xD0 { 255 } else if v < 0xF0 { 120 } else { 0 };
                    out.extend([v, a]);
                }
                Some(out)
            })
            .clone()
    });
    let Some(px) = px else { return false };
    let (x0, y0) = (r.x + (r.w - side) / 2, r.y + (r.h - side) / 2);
    for y in 0..side {
        for x in 0..side {
            let k = 2 * (y * side + x) as usize;
            let (v, a) = (px[k], px[k + 1]);
            if a > 8 {
                fb.blend(x0 + x, y0 + y, v, a);
            }
        }
    }
    true
}
