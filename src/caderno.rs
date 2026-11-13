//! Caderno à mão: cada página é um PNG em tons de cinza, salvo ao levantar o dedo.

use crate::fb::{Framebuffer, Rect};
use crate::input::Touch;
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const CANVAS: Rect = Rect::new(96, 368, 880, 800);
const WIDTH: usize = CANVAS.w as usize;
const HEIGHT: usize = CANVAS.h as usize;

#[derive(Clone)]
struct Page { pixels: Vec<u8> }
impl Page {
    fn blank() -> Self { Self { pixels: vec![PAPER; WIDTH * HEIGHT] } }
    fn dot(&mut self, x: i32, y: i32, radius: i32, color: u8) {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius { continue; }
                let (x, y) = (x + dx, y + dy);
                if x >= 0 && y >= 0 && x < WIDTH as i32 && y < HEIGHT as i32 {
                    self.pixels[y as usize * WIDTH + x as usize] = color;
                }
            }
        }
    }
    /// Pontos intermediários evitam buracos quando o sensor manda posições espaçadas.
    fn segment(&mut self, from: (i32, i32), to: (i32, i32), radius: i32, color: u8) {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let steps = dx.abs().max(dy.abs());
        if steps == 0 { self.dot(from.0, from.1, radius, color); return; }
        for i in 0..=steps {
            self.dot(from.0 + dx * i / steps, from.1 + dy * i / steps, radius, color);
        }
    }
}

/// Último ponto dentro da folha quando o dedo a atravessa.
fn edge_point(from: (i32, i32), x: i32, y: i32) -> (i32, i32) {
    let (tx, ty) = (x as f64 - CANVAS.x as f64, y as f64 - CANVAS.y as f64);
    let (sx, sy) = (from.0 as f64, from.1 as f64);
    let (dx, dy) = (tx - sx, ty - sy);
    let mut t = 1.0f64;
    if tx < 0.0 { t = t.min(-sx / dx); }
    if tx > (CANVAS.w - 1) as f64 { t = t.min(((CANVAS.w - 1) as f64 - sx) / dx); }
    if ty < 0.0 { t = t.min(-sy / dy); }
    if ty > (CANVAS.h - 1) as f64 { t = t.min(((CANVAS.h - 1) as f64 - sy) / dy); }
    ((sx + dx * t).round().clamp(0.0, (CANVAS.w - 1) as f64) as i32, (sy + dy * t).round().clamp(0.0, (CANVAS.h - 1) as f64) as i32)
}

fn page_path(dir: &Path, id: u32) -> PathBuf { dir.join(format!("pagina-{id:04}.png")) }
fn page_ids(dir: &Path) -> Vec<u32> {
    let mut ids: Vec<u32> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| {
        let name = e.file_name();
        let name = name.to_str()?;
        let id = name.strip_prefix("pagina-")?.strip_suffix(".png")?;
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) { return None; }
        let id: u32 = id.parse().ok()?;
        if name != format!("pagina-{id:04}.png") { return None; }
        Some(id)
    }).collect();
    ids.sort_unstable(); ids.dedup(); ids
}

fn load_page(path: &Path) -> Result<Page, String> {
    if !path.exists() { return Ok(Page::blank()); }
    let dimensions = image::image_dimensions(path).map_err(|_| "Não consegui abrir esta página.".to_string())?;
    if dimensions != (WIDTH as u32, HEIGHT as u32) { return Err("O tamanho desta página não é compatível.".into()); }
    let img = image::open(path).map_err(|_| "Não consegui abrir esta página.".to_string())?.to_luma8();
    if img.dimensions() != dimensions { return Err("O tamanho desta página não é compatível.".into()); }
    Ok(Page { pixels: img.into_raw() })
}

/// Grava em arquivo vizinho e troca só depois de terminar o PNG, sem apagar o original.
fn save_page(path: &Path, page: &Page) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| "Não consegui salvar esta página.".to_string())?;
    std::fs::create_dir_all(parent).map_err(|_| "Não consegui salvar esta página.".to_string())?;
    let img = image::GrayImage::from_raw(WIDTH as u32, HEIGHT as u32, page.pixels.clone()).ok_or_else(|| "Não consegui salvar esta página.".to_string())?;
    let temp = parent.join(format!(".pagina-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&temp).map_err(|_| "Não consegui salvar esta página.".to_string())?;
        image::DynamicImage::ImageLuma8(img).write_to(&mut file, image::ImageFormat::Png).map_err(|_| "Não consegui salvar esta página.".to_string())?;
        file.sync_all().map_err(|_| "Não consegui salvar esta página.".to_string())?;
        std::fs::rename(&temp, path).map_err(|_| "Não consegui salvar esta página.".to_string())
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

pub struct Caderno {
    dir: PathBuf,
    pages: Vec<u32>,
    index: usize,
    page: Page,
    erase: bool,
    active: bool,
    last: Option<(i32, i32)>,
    dirty: bool,
    invalid: bool,
    error: Option<String>,
}

impl Caderno {
    pub fn new() -> Self {
        let dir = crate::home_dir().join("data/caderno");
        let mut pages = page_ids(&dir);
        if pages.is_empty() { pages.push(1); }
        let (page, invalid, error) = match load_page(&page_path(&dir, pages[0])) {
            Ok(page) => (page, false, None),
            Err(error) => (Page::blank(), true, Some(error)),
        };
        Self { dir, pages, index: 0, page, erase: false, active: false, last: None, dirty: false, invalid, error }
    }

    fn flush(&mut self) -> bool {
        if !self.dirty { return true; }
        match save_page(&page_path(&self.dir, self.pages[self.index]), &self.page) {
            Ok(()) => { self.dirty = false; self.error = None; true }
            Err(e) => { self.error = Some(e); false }
        }
    }

    fn show(&mut self, index: usize) {
        if index >= self.pages.len() || !self.flush() { return; }
        self.index = index;
        match load_page(&page_path(&self.dir, self.pages[index])) {
            Ok(page) => { self.page = page; self.invalid = false; self.error = None; }
            Err(error) => { self.page = Page::blank(); self.invalid = true; self.error = Some(error); }
        }
        self.last = None; self.active = false; self.dirty = false;
    }

    fn new_page(&mut self) {
        if !self.flush() { return; }
        let current = page_path(&self.dir, self.pages[self.index]);
        if !self.invalid && !current.exists() {
            if let Err(e) = save_page(&current, &self.page) { self.error = Some(e); return; }
        }
        let Some(id) = self.pages.iter().copied().max().unwrap_or(0).checked_add(1) else { self.error = Some("Não foi possível criar outra página.".into()); return; };
        let blank = Page::blank();
        match save_page(&page_path(&self.dir, id), &blank) {
            Ok(()) => {
                self.pages.push(id); self.index = self.pages.len() - 1;
                self.page = blank; self.invalid = false; self.error = None; self.dirty = false;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn tool(k: usize) -> Rect {
        let gap = 12;
        let width = (W - 2 * MARGIN - 2 * gap) / 3;
        Rect::new(MARGIN + k as i32 * (width + gap), 250, width, 80)
    }

    fn bottom(k: usize) -> Rect {
        let width = (W - 2 * MARGIN - 32) / 3;
        Rect::new(MARGIN + k as i32 * (width + 16), 1288, width, 80)
    }

    fn draw_segment(&mut self, from: (i32, i32), to: (i32, i32)) -> Rect {
        let radius = if self.erase { 18 } else { 4 };
        self.page.segment(from, to, radius, if self.erase { PAPER } else { INK });
        self.dirty = true;
        let x1 = (from.0.min(to.0) - radius).max(0);
        let y1 = (from.1.min(to.1) - radius).max(0);
        let x2 = (from.0.max(to.0) + radius + 1).min(CANVAS.w);
        let y2 = (from.1.max(to.1) + radius + 1).min(CANVAS.h);
        Rect::new(CANVAS.x + x1, CANVAS.y + y1, x2 - x1, y2 - y1)
    }

    /// Eventos crus do dedo. `Some` consome o evento e contém a região a atualizar.
    /// A integração deve chamar antes do processamento de gestos/botões do app.
    pub fn touch(&mut self, touch: Touch) -> Option<Redraw> {
        match touch {
            Touch::Down(x, y) if CANVAS.contains(x, y) && !self.invalid => {
                self.active = true;
                let p = (x - CANVAS.x, y - CANVAS.y);
                self.last = Some(p);
                Some(Redraw::Part(self.draw_segment(p, p)))
            }
            Touch::Move(x, y) if self.active => {
                if !CANVAS.contains(x, y) {
                    return Some(match self.last.take() { Some(prev) => Redraw::Part(self.draw_segment(prev, edge_point(prev, x, y))), None => Redraw::None });
                }
                let p = (x - CANVAS.x, y - CANVAS.y);
                let prev = self.last.replace(p).unwrap_or(p);
                Some(Redraw::Part(self.draw_segment(prev, p)))
            }
            Touch::Up(x, y) if self.active => {
                if CANVAS.contains(x, y) {
                    let p = (x - CANVAS.x, y - CANVAS.y);
                    let prev = self.last.unwrap_or(p);
                    self.draw_segment(prev, p);
                } else if let Some(prev) = self.last {
                    self.draw_segment(prev, edge_point(prev, x, y));
                }
                self.last = None; self.active = false;
                if self.flush() { Some(Redraw::Part(CANVAS)) } else { Some(Redraw::Full) }
            }
            _ => None,
        }
    }
}

impl MiniGame for Caderno {
    fn touch(&mut self, touch: Touch) -> Option<Redraw> { Caderno::touch(self, touch) }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        minigame::header(fb, f, "Caderno", &format!("Página {} de {}", self.index + 1, self.pages.len()));
        for (k, name) in ["Caneta", "Borracha", "Limpar"].iter().enumerate() {
            minigame::pill(fb, f, Self::tool(k), name, (k == 0 && !self.erase) || (k == 1 && self.erase));
        }
        ui::card(fb, Rect::new(72, 344, 928, 848), 28, PAPER, false);
        if !self.invalid { fb.blit(CANVAS.x, CANVAS.y, CANVAS.w, CANVAS.h, &self.page.pixels); }
        else { f.draw(fb, Face::Strong, 30.0, CANVAS.x + 25, CANVAS.y + 80, MUTED, "Não consegui abrir esta página."); }
        if let Some(error) = &self.error {
            let msg = crate::apps::fit(f, Face::Strong, 25.0, error, W - 2 * MARGIN);
            f.draw(fb, Face::Strong, 25.0, MARGIN, 1256, INK, &msg);
        }
        if self.index > 0 { minigame::pill(fb, f, Self::bottom(0), "Anterior", false); }
        minigame::pill(fb, f, Self::bottom(1), "Nova página", true);
        if self.index + 1 < self.pages.len() { minigame::pill(fb, f, Self::bottom(2), "Próxima", false); }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        let mut v = vec![(800, Self::tool(0)), (801, Self::tool(1)), (811, Self::bottom(1))];
        if !self.invalid { v.push((802, Self::tool(2))); }
        if self.index > 0 { v.push((810, Self::bottom(0))); }
        if self.index + 1 < self.pages.len() { v.push((812, Self::bottom(2))); }
        v
    }
    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            800 => self.erase = false,
            801 => self.erase = true,
            802 if !self.invalid => { self.page = Page::blank(); self.dirty = true; self.flush(); }
            810 if self.index > 0 => self.show(self.index - 1),
            811 => self.new_page(),
            812 if self.index + 1 < self.pages.len() => self.show(self.index + 1),
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn risky(&self, id: u32) -> bool { id == 802 }
    fn save(&self) { if self.dirty && !self.invalid { let _ = save_page(&page_path(&self.dir, self.pages[self.index]), &self.page); } }
    fn back(&mut self) -> bool { !self.flush() }
    fn summary(&self) -> String { format!("Caderno · página {}", self.index + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traco_interpolado_borracha_e_limites() {
        let mut page = Page::blank();
        page.segment((2, 2), (120, 62), 2, INK);
        for x in 2..=120 {
            let y = 2 + 60 * (x - 2) / 118;
            assert_eq!(page.pixels[y as usize * WIDTH + x as usize], INK);
        }
        page.segment((50, 26), (70, 36), 8, PAPER);
        assert_eq!(page.pixels[31 * WIDTH + 60], PAPER);
        page.dot(0, 0, 18, INK);
        assert_eq!(page.pixels[0], INK);
        assert_eq!(edge_point((100, 100), CANVAS.x - 100, CANVAS.y + 200), (0, 150));
        let end = edge_point((100, 100), i32::MAX, i32::MAX);
        assert!(end.0 == CANVAS.w - 1 || end.1 == CANVAS.h - 1);
    }
    #[test]
    fn gesto_na_borda_e_falha_de_gravacao_preservam_desenho() {
        let dir = std::env::temp_dir().join(format!("verso-caderno-gesto-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut c = Caderno { dir: dir.clone(), pages: vec![1], index: 0, page: Page::blank(), erase: false, active: false, last: None, dirty: false, invalid: false, error: None };
        c.touch(Touch::Down(CANVAS.x + 100, CANVAS.y + 100));
        c.touch(Touch::Move(CANVAS.x - 100, CANVAS.y + 200));
        c.touch(Touch::Up(CANVAS.x - 100, CANVAS.y + 200));
        let saved = load_page(&page_path(&dir, 1)).unwrap();
        assert_eq!(saved.pixels[150 * WIDTH], INK);
        assert!(!c.dirty);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::write(&dir, b"arquivo que impede criar pasta").unwrap();
        c.page.dot(25, 25, 3, INK); c.dirty = true;
        assert!(!c.flush());
        assert!(c.dirty);
        assert_eq!(c.page.pixels[25 * WIDTH + 25], INK);
        assert_eq!(std::fs::read(&dir).unwrap(), b"arquivo que impede criar pasta");
        assert!(c.back());
        let _ = std::fs::remove_file(dir);
    }
    #[test]
    fn png_grava_reabre_e_preserva_arquivo_invalido() {
        let dir = std::env::temp_dir().join(format!("verso-caderno-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = page_path(&dir, 1);
        let mut page = Page::blank(); page.dot(60, 80, 4, INK);
        save_page(&path, &page).unwrap();
        assert_eq!(load_page(&path).unwrap().pixels, page.pixels);
        std::fs::write(page_path(&dir, 2), b"conteudo invalido").unwrap();
        assert!(load_page(&page_path(&dir, 2)).is_err());
        assert_eq!(std::fs::read(page_path(&dir, 2)).unwrap(), b"conteudo invalido");
        assert_eq!(page_ids(&dir), vec![1, 2]);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn cria_varias_paginas_e_reabre_o_desenho_de_cada_uma() {
        let dir = std::env::temp_dir().join(format!("verso-caderno-paginas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut c = Caderno { dir: dir.clone(), pages: vec![1], index: 0, page: Page::blank(), erase: false, active: false, last: None, dirty: false, invalid: false, error: None };
        c.touch(Touch::Down(CANVAS.x + 80, CANVAS.y + 90));
        c.touch(Touch::Up(CANVAS.x + 80, CANVAS.y + 90));
        c.new_page();
        assert_eq!(page_ids(&dir), vec![1, 2]);
        assert_eq!(c.page.pixels[90 * WIDTH + 80], PAPER);
        c.touch(Touch::Down(CANVAS.x + 120, CANVAS.y + 130));
        c.touch(Touch::Up(CANVAS.x + 120, CANVAS.y + 130));
        c.show(0);
        assert_eq!(c.page.pixels[90 * WIDTH + 80], INK);
        assert_eq!(c.page.pixels[130 * WIDTH + 120], PAPER);
        c.show(1);
        assert_eq!(c.page.pixels[130 * WIDTH + 120], INK);
        let _ = std::fs::remove_dir_all(dir);
    }
}
