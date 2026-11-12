//! Framebuffer do Kindle PW3 (EPDC i.MX6 "Wario"), 8 bits por pixel (Y8).
//!
//! Desenhamos num buffer em memória e enviamos à tela só a região que mudou,
//! escolhendo a forma de onda do e-ink conforme o caso:
//! - `Wave::Fast` (DU): só preto e branco, ~120 ms, para resposta ao toque;
//! - `Wave::Quality` (GC16): 16 cinzas, para conteúdo;
//! - `flash = true`: atualização cheia (pisca), limpa fantasmas ao trocar de tela.

use crate::sys::ioctl;
use std::fs::{File, OpenOptions};
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;

const FBIOGET_VSCREENINFO: u32 = 0x4600;
const FBIOGET_FSCREENINFO: u32 = 0x4602;
const MXCFB_SEND_UPDATE: u32 = 0x4048_462E;
const MXCFB_WAIT_FOR_UPDATE_COMPLETE: u32 = 0xC008_462F;

const WAVEFORM_MODE_DU: u32 = 1;
const WAVEFORM_MODE_GC16: u32 = 2;
const UPDATE_MODE_PARTIAL: u32 = 0;
const UPDATE_MODE_FULL: u32 = 1;
const TEMP_USE_AMBIENT: i32 = 0x1000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wave {
    Fast,
    Quality,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
}

#[repr(C)]
#[derive(Default)]
struct MxcfbRect {
    top: u32,
    left: u32,
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Default)]
struct MxcfbAltBufferData {
    phys_addr: u32,
    width: u32,
    height: u32,
    alt_update_region: MxcfbRect,
}

#[repr(C)]
#[derive(Default)]
struct MxcfbUpdateData {
    update_region: MxcfbRect,
    waveform_mode: u32,
    update_mode: u32,
    update_marker: u32,
    hist_bw_waveform_mode: u32,
    hist_gray_waveform_mode: u32,
    temp: i32,
    flags: u32,
    alt_buffer_data: MxcfbAltBufferData,
}

pub struct Framebuffer {
    file: File,
    pub width: i32,
    pub height: i32,
    stride: usize,
    yoffset: u32,
    /// Buffer de trabalho: um byte por pixel, 0 = preto, 255 = branco.
    pub buf: Vec<u8>,
    marker: u32,
    /// Modo noturno: inverte os tons ao enviar para a tela (o desenho continua normal).
    pub invert: bool,
    /// Tela de cabeça para baixo (xadrez local: vira para quem joga).
    pub rot180: bool,
    /// Atualizações completas pedidas no modo escuro (só uma a cada algumas pisca de verdade).
    night_full: u32,
}

fn u32_at(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

impl Framebuffer {
    /// Tela em memória: desenha no `buf` sem mandar ao e-ink (cenas dos tutoriais, testes no PC).
    pub fn memory() -> Self {
        Framebuffer {
            file: File::open("/dev/null").expect("/dev/null"),
            width: 1072,
            height: 1448,
            stride: 1072,
            yoffset: 0,
            buf: vec![0xFF; 1072 * 1448],
            marker: 1,
            invert: false,
            rot180: false,
            night_full: 0,
        }
    }

    /// Copia `src` (região de outra tela) reduzida para caber em `dst`, com média de área.
    pub fn blit_scaled(&mut self, other: &Framebuffer, src: Rect, dst: Rect) {
        if src.w <= 0 || src.h <= 0 || dst.w <= 0 || dst.h <= 0 {
            return;
        }
        for dy in 0..dst.h {
            let sy0 = src.y + dy * src.h / dst.h;
            let sy1 = (src.y + (dy + 1) * src.h / dst.h).max(sy0 + 1);
            for dx in 0..dst.w {
                let sx0 = src.x + dx * src.w / dst.w;
                let sx1 = (src.x + (dx + 1) * src.w / dst.w).max(sx0 + 1);
                let (mut sum, mut n) = (0u32, 0u32);
                for y in sy0..sy1 {
                    for x in sx0..sx1 {
                        if x >= 0 && y >= 0 && x < other.width && y < other.height {
                            sum += other.buf[(y * other.width + x) as usize] as u32;
                            n += 1;
                        }
                    }
                }
                let (px, py) = (dst.x + dx, dst.y + dy);
                if n > 0 && px >= 0 && py >= 0 && px < self.width && py < self.height {
                    self.buf[(py * self.width + px) as usize] = (sum / n) as u8;
                }
            }
        }
    }

    #[cfg(test)]
    pub fn save_pgm(&self, path: &std::path::Path) {
        let mut out = format!("P5 {} {} 255\n", self.width, self.height).into_bytes();
        out.extend_from_slice(&self.buf);
        std::fs::write(path, out).expect("pgm");
    }

    pub fn open() -> std::io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open("/dev/fb0")?;
        let fd = file.as_raw_fd();
        let mut var = [0u8; 160];
        let mut fix = [0u8; 68];
        unsafe {
            if ioctl(fd, FBIOGET_VSCREENINFO, var.as_mut_ptr()) != 0
                || ioctl(fd, FBIOGET_FSCREENINFO, fix.as_mut_ptr()) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
        }
        let (width, height) = (u32_at(&var, 0) as i32, u32_at(&var, 4) as i32);
        let bpp = u32_at(&var, 24);
        if bpp != 8 {
            return Err(std::io::Error::other(format!("bpp {bpp} não suportado")));
        }
        let stride = u32_at(&fix, 44) as usize;
        Ok(Framebuffer {
            file,
            width,
            height,
            stride,
            yoffset: u32_at(&var, 20),
            buf: vec![0xFF; width as usize * height as usize],
            marker: 1,
            invert: false,
            rot180: false,
            night_full: 0,
        })
    }

    pub fn screen(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// Mistura `v` sobre o pixel com cobertura `a` (0..=255).
    #[inline]
    pub fn blend(&mut self, x: i32, y: i32, v: u8, a: u8) {
        if x >= 0 && y >= 0 && x < self.width && y < self.height {
            let i = (y * self.width + x) as usize;
            let old = self.buf[i] as u32;
            self.buf[i] = ((old * (255 - a as u32) + v as u32 * a as u32 + 127) / 255) as u8;
        }
    }

    /// Copia uma imagem em cinza (um byte por pixel) para o buffer.
    pub fn blit(&mut self, x: i32, y: i32, w: i32, h: i32, px: &[u8]) {
        for row in 0..h {
            for col in 0..w {
                if let Some(&v) = px.get((row * w + col) as usize) {
                    let (px_, py) = (x + col, y + row);
                    if px_ >= 0 && py >= 0 && px_ < self.width && py < self.height {
                        self.buf[(py * self.width + px_) as usize] = v;
                    }
                }
            }
        }
    }

    /// Inverte os tons dentro de um retângulo arredondado (destaque de toque).
    /// Aplicar duas vezes devolve exatamente o desenho original.
    pub fn invert_round(&mut self, r: Rect, radius: i32) {
        // Raio limitado a (lado - 1) / 2: com metade exata, o clamp abaixo teria mínimo > máximo.
        let rad = radius.min((r.w - 1) / 2).min((r.h - 1) / 2);
        for y in r.y.max(0)..(r.y + r.h).min(self.height) {
            for x in r.x.max(0)..(r.x + r.w).min(self.width) {
                if rad > 0 {
                    let cx = x.clamp(r.x + rad, r.x + r.w - 1 - rad);
                    let cy = y.clamp(r.y + rad, r.y + r.h - 1 - rad);
                    let (dx, dy) = (x - cx, y - cy);
                    if dx * dx + dy * dy > rad * rad {
                        continue;
                    }
                }
                let i = (y * self.width + x) as usize;
                self.buf[i] = 255 - self.buf[i];
            }
        }
    }

    pub fn fill(&mut self, r: Rect, v: u8) {
        let x0 = r.x.max(0);
        let x1 = (r.x + r.w).min(self.width);
        if x1 <= x0 {
            return;
        }
        for y in r.y.max(0)..(r.y + r.h).min(self.height) {
            let row = (y * self.width) as usize;
            self.buf[row + x0 as usize..row + x1 as usize].fill(v);
        }
    }

    /// Retângulo arredondado preenchido, com borda suavizada.
    pub fn round_rect(&mut self, r: Rect, radius: i32, v: u8) {
        let rad = radius.min(r.w / 2).min(r.h / 2) as f32;
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                let cx = (x as f32 + 0.5).clamp(r.x as f32 + rad, (r.x + r.w) as f32 - rad);
                let cy = (y as f32 + 0.5).clamp(r.y as f32 + rad, (r.y + r.h) as f32 - rad);
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt() - rad;
                let a = (0.5 - d).clamp(0.0, 1.0);
                if a > 0.0 {
                    self.blend(x, y, v, (a * 255.0) as u8);
                }
            }
        }
    }

    /// Moldura arredondada por cima de algo já desenhado (um mapa de casas): os cantos de fora
    /// viram `outside`, a borda de espessura `t` vira `v` e, embaixo, a sombra chapada de
    /// altura `sh` em `shadow`.
    pub fn round_frame(&mut self, r: Rect, radius: i32, t: i32, v: u8, outside: u8, sh: i32, shadow: u8) {
        let rad = radius.min(r.w / 2).min(r.h / 2) as f32;
        let sd = |x: i32, y: i32, top: i32| {
            let cx = (x as f32 + 0.5).clamp(r.x as f32 + rad, (r.x + r.w) as f32 - rad);
            let cy = (y as f32 + 0.5).clamp(top as f32 + rad, (top + r.h) as f32 - rad);
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            (dx * dx + dy * dy).sqrt() - rad
        };
        for y in r.y..r.y + r.h + sh {
            for x in r.x..r.x + r.w {
                let d = sd(x, y, r.y);
                if d > -0.5 {
                    // Fora da forma: sombra (se cair dentro da forma deslocada) ou papel.
                    let ds = sd(x, y, r.y + sh);
                    let base = if sh > 0 && ds < 0.5 { shadow } else { outside };
                    if y < r.y + r.h {
                        self.blend(x, y, base, 255);
                    } else if ds < 0.5 {
                        self.blend(x, y, shadow, 255);
                    }
                    let a = (0.5 - d).clamp(0.0, 1.0);
                    if a > 0.0 && y < r.y + r.h {
                        self.blend(x, y, v, (a * 255.0) as u8);
                    }
                } else if d > -(t as f32) - 0.5 {
                    let a = (d + t as f32 + 0.5).clamp(0.0, 1.0);
                    self.blend(x, y, v, (a * 255.0) as u8);
                }
            }
        }
    }

    /// Contorno arredondado com espessura `t`.
    pub fn round_outline(&mut self, r: Rect, radius: i32, t: i32, v: u8, inner: u8) {
        self.round_rect(r, radius, v);
        let inside = Rect::new(r.x + t, r.y + t, r.w - 2 * t, r.h - 2 * t);
        self.round_rect(inside, (radius - t).max(0), inner);
    }

    /// Copia a região do buffer para a memória da tela e pede a atualização.
    pub fn refresh(&mut self, r: Rect, wave: Wave, flash: bool) {
        // No modo escuro o "flash" do e-ink vira um clarão: troca de tela sem piscar e
        // só uma atualização completa a cada 6, para limpar os rastros.
        let flash = flash && (!self.invert || {
            self.night_full = self.night_full.wrapping_add(1);
            self.night_full % 6 == 0
        });
        let x0 = r.x.clamp(0, self.width);
        let y0 = r.y.clamp(0, self.height);
        let x1 = (r.x + r.w).clamp(0, self.width);
        let y1 = (r.y + r.h).clamp(0, self.height);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let len = (x1 - x0) as usize;
        let mut row = vec![0u8; len];
        // Com a tela girada, a região vai para o canto oposto e cada linha é espelhada.
        let (dx0, dy0, dx1, dy1) = if self.rot180 { (self.width - x1, self.height - y1, self.width - x0, self.height - y0) } else { (x0, y0, x1, y1) };
        for y in y0..y1 {
            let src = (y * self.width + x0) as usize;
            let dy = if self.rot180 { self.height - 1 - y } else { y };
            let off = (dy as u64 + self.yoffset as u64) * self.stride as u64 + dx0 as u64;
            let data = &self.buf[src..src + len];
            row.copy_from_slice(data);
            if self.rot180 {
                row.reverse();
            }
            if self.invert {
                for d in row.iter_mut() {
                    *d = 255 - *d;
                }
            }
            let _ = self.file.write_all_at(&row, off);
        }
        let (x0, y0, x1, y1) = (dx0, dy0, dx1, dy1);
        self.marker = self.marker.wrapping_add(1).max(1);
        let mut upd = MxcfbUpdateData {
            update_region: MxcfbRect {
                top: y0 as u32,
                left: x0 as u32,
                width: (x1 - x0) as u32,
                height: (y1 - y0) as u32,
            },
            waveform_mode: match wave {
                Wave::Fast => WAVEFORM_MODE_DU,
                Wave::Quality => WAVEFORM_MODE_GC16,
            },
            update_mode: if flash { UPDATE_MODE_FULL } else { UPDATE_MODE_PARTIAL },
            update_marker: self.marker,
            hist_bw_waveform_mode: WAVEFORM_MODE_DU,
            hist_gray_waveform_mode: WAVEFORM_MODE_GC16,
            temp: TEMP_USE_AMBIENT,
            ..Default::default()
        };
        unsafe {
            ioctl(self.file.as_raw_fd(), MXCFB_SEND_UPDATE, &mut upd as *mut MxcfbUpdateData);
        }
    }

    /// Espera a última atualização terminar (para medir ou sincronizar).
    pub fn wait(&self) {
        let mut m: [u32; 2] = [self.marker, 0];
        unsafe {
            ioctl(self.file.as_raw_fd(), MXCFB_WAIT_FOR_UPDATE_COMPLETE, m.as_mut_ptr());
        }
    }

    pub fn present(&mut self, wave: Wave, flash: bool) {
        let r = self.screen();
        self.refresh(r, wave, flash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake(w: i32, h: i32) -> Framebuffer {
        Framebuffer {
            file: File::open("/dev/null").unwrap(),
            width: w,
            height: h,
            stride: w as usize,
            yoffset: 0,
            buf: vec![0xFF; (w * h) as usize],
            marker: 1,
            invert: false,
            rot180: false,
            night_full: 0,
        }
    }

    #[test]
    fn inverter_duas_vezes_restaura_e_nao_cai_com_raio_de_pilula() {
        let mut fb = fake(300, 200);
        fb.round_rect(Rect::new(20, 20, 200, 120), 60, 0x40);
        let before = fb.buf.clone();
        for r in [Rect::new(20, 20, 200, 120), Rect::new(0, 0, 1, 1), Rect::new(10, 10, 168, 168)] {
            let rad = r.h / 2; // pílula/círculo: o caso que derrubava
            fb.invert_round(r, rad);
            fb.invert_round(r, rad);
        }
        assert_eq!(fb.buf, before);
    }
}
