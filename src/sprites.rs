//! Sprites dos jogos de masmorra (assets/sprites → /mnt/us/verso-ink/sprites): carregados na
//! primeira vez, reduzidos para o tamanho da casa e guardados na memória. Se o PNG tem
//! transparência (peças de xadrez, ladrilhos), ela vale; se não tem, o fundo branco do sprite
//! não apaga a casa: só os tons mais escuros que o papel são desenhados.

use crate::fb::{Framebuffer, Rect};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<(String, i32), Option<Vec<u8>>>> = Default::default();
    /// Quais sprites existem (evita ir ao disco a cada desenho).
    static EXISTS: RefCell<HashMap<String, bool>> = Default::default();
}

fn path(name: &str) -> std::path::PathBuf {
    crate::home_dir().join("sprites").join(format!("{name}.png"))
}

pub fn exists(name: &str) -> bool {
    EXISTS.with(|e| *e.borrow_mut().entry(name.to_string()).or_insert_with(|| path(name).exists()))
}

/// Desenha o sprite `name` centrado em `r` (quadrado do lado menor). Devolve false se não houver.
pub fn draw(fb: &mut Framebuffer, name: &str, r: Rect) -> bool {
        let _p = crate::perfil::Guard::new("sprite");
    let side = r.w.min(r.h);
    if side <= 0 || !exists(name) {
        return false;
    }
    // Desenha direto do cache, sem copiar os pixels a cada vez.
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let px = c
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
            });
        let Some(px) = px.as_ref() else { return false };
        let (x0, y0) = (r.x + (r.w - side) / 2, r.y + (r.h - side) / 2);
        // Recorte calculado uma vez; depois, mistura direto no buffer (sem checar borda a cada ponto).
        let (xa, xb) = ((-x0).max(0), (fb.width - x0).min(side));
        let (ya, yb) = ((-y0).max(0), (fb.height - y0).min(side));
        let w = fb.width as usize;
        for y in ya..yb {
            let src = 2 * (y * side) as usize;
            let dst = (y0 + y) as usize * w + x0 as usize;
            for x in xa..xb {
                let k = src + 2 * x as usize;
                let a = px[k + 1] as u32;
                if a == 255 {
                    fb.buf[dst + x as usize] = px[k];
                } else if a > 8 {
                    let d = &mut fb.buf[dst + x as usize];
                    *d = crate::fb::mix(*d, px[k], a);
                }
            }
        }
        true
    })
}
