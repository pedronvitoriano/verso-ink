//! Folhear: PDF e quadrinhos (CBZ, CBR) dentro do Verso Ink, página por página como imagem.
//! Antes, esses arquivos saíam para o KOReader (segundos de troca de programa). A página é
//! desenhada numa thread à parte, que já prepara a próxima enquanto a pessoa lê esta.
//! Livro da loja da Amazon (com DRM) continua no leitor da Amazon: ninguém mais abre.

use crate::fb::{Framebuffer, Rect};
use crate::ui::{INK, MUTED, PAPER};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

pub const W: i32 = 1072;
/// Altura da página na tela; embaixo fica a linha com página e progresso.
pub const AREA_H: i32 = 1390;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tipo {
    Pdf,
    Zip,
    Rar,
}

fn tipo(path: &Path) -> Option<Tipo> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "pdf" => Some(Tipo::Pdf),
        "cbz" => Some(Tipo::Zip),
        "cbr" => Some(Tipo::Rar),
        _ => None,
    }
}

/// PDF e quadrinhos abrem aqui (o resto do que não tem DRM abre no leitor de texto).
pub fn supported(path: &Path) -> bool {
    tipo(path).is_some()
}

/// Página pronta para a tela: tons de cinza, já no tamanho de exibição.
pub struct Pagina {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

struct Pedido {
    page: usize,
    cortar: bool,
}

type Resposta = (usize, bool, Result<Pagina, String>);

pub struct Folheador {
    pub path: PathBuf,
    pub title: String,
    pub n: usize,
    pub page: usize,
    pub menu: bool,
    /// PDF: corta as margens brancas (a letra fica maior na tela de 6").
    pub cortar: bool,
    pub pdf: bool,
    pub turns: u32,
    tx: Sender<Pedido>,
    rx: Receiver<Resposta>,
    prontas: Vec<(usize, bool, Pagina)>,
    pedidas: Vec<(usize, bool)>,
}

// ---------- Ordem das páginas nos quadrinhos ----------

/// Ordem natural: "pagina-2" antes de "pagina-10".
pub fn ordem_natural(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let mut na = String::new();
                while let Some(c) = x.peek().copied().filter(char::is_ascii_digit) {
                    na.push(c);
                    x.next();
                }
                let mut nb = String::new();
                while let Some(d) = y.peek().copied().filter(char::is_ascii_digit) {
                    nb.push(d);
                    y.next();
                }
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let o = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

fn e_imagem(nome: &str) -> bool {
    let l = nome.to_ascii_lowercase();
    // Miniaturas do macOS (__MACOSX/._x.jpg) não são páginas.
    !l.contains("__macosx/") && !l.rsplit('/').next().unwrap_or("").starts_with("._") && [".jpg", ".jpeg", ".png", ".gif", ".webp"].iter().any(|e| l.ends_with(e))
}

// ---------- Desenho das páginas (na thread) ----------

/// Cabe a imagem em `w`×`h` mantendo a proporção.
fn caber(img: image::GrayImage, w: i32, h: i32) -> Pagina {
    let (iw, ih) = (img.width() as f32, img.height() as f32);
    let s = (w as f32 / iw).min(h as f32 / ih);
    let (tw, th) = (((iw * s).round() as u32).max(1), ((ih * s).round() as u32).max(1));
    let img = if (tw, th) == (img.width(), img.height()) { img } else { image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle) };
    Pagina { w: tw as i32, h: th as i32, px: img.into_raw() }
}

fn decodificar(bytes: &[u8]) -> Result<Pagina, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("imagem ilegível: {e}"))?;
    Ok(caber(img.to_luma8(), W, AREA_H))
}

/// Caixa do conteúdo (o que não é quase branco) numa página em cinza, com folga.
pub fn caixa_do_conteudo(px: &[u8], w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let escuro = |v: u8| v < 200;
    let linha = |y: usize| px[y * w..(y + 1) * w].iter().any(|&v| escuro(v));
    let coluna = |x: usize, y0: usize, y1: usize| (y0..y1).any(|y| escuro(px[y * w + x]));
    let y0 = (0..h).find(|&y| linha(y))?;
    let y1 = (0..h).rev().find(|&y| linha(y))? + 1;
    let x0 = (0..w).find(|&x| coluna(x, y0, y1))?;
    let x1 = (0..w).rev().find(|&x| coluna(x, y0, y1))? + 1;
    let (fx, fy) = ((w / 50).max(2), (h / 60).max(2));
    Some((x0.saturating_sub(fx), y0.saturating_sub(fy), (x1 + fx).min(w), (y1 + fy).min(h)))
}

fn cinza(p: &hayro::vello_cpu::Pixmap) -> Vec<u8> {
    p.data_as_u8_slice().chunks_exact(4).map(|c| ((c[0] as u32 * 77 + c[1] as u32 * 150 + c[2] as u32 * 29) >> 8) as u8).collect()
}

fn pdf_trabalhar(path: PathBuf, rx: Receiver<Pedido>, tx: Sender<Resposta>, n_tx: Sender<Result<usize, String>>) {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::vello_cpu::color::palette::css::WHITE;
    use hayro::{render, PixmapSettings, RenderCache, RenderSettings};
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            let _ = n_tx.send(Err(e.to_string()));
            return;
        }
    };
    let pdf = match Pdf::new(data) {
        Ok(p) => p,
        Err(e) => {
            let _ = n_tx.send(Err(format!("PDF ilegível ({e:?})")));
            return;
        }
    };
    let pages = pdf.pages();
    if pages.is_empty() {
        let _ = n_tx.send(Err("PDF sem páginas".into()));
        return;
    }
    let _ = n_tx.send(Ok(pages.len()));
    let cache = RenderCache::new();
    let is = InterpreterSettings::default();
    let rs = RenderSettings::default();
    let desenhar = |i: usize, s: f32| {
        let px = render(&pages[i], &cache, &is, &rs, &PixmapSettings { x_scale: s, y_scale: s, bg_color: WHITE });
        let (w, h) = (px.width() as usize, px.height() as usize);
        (cinza(&px), w, h)
    };
    // Só o pedaço (x, y, w, h), em pixels da página na escala s: desenhar a página inteira
    // ampliada e recortar custava ~1 s por página no Kindle.
    let regiao = |i: usize, s: f32, x: f64, y: f64, w: u16, h: u16| {
        use hayro::hayro_interpret::util::TransformExt;
        use hayro::kurbo::Affine;
        use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
        let mut ctx = RenderContext::new(w, h);
        let t = Affine::translate((-x, -y)) * Affine::scale(s as f64) * pages[i].initial_transform(true).to_kurbo();
        hayro::render_into(&pages[i], &cache, &is, &rs, &mut ctx, t);
        ctx.flush();
        let mut px = Pixmap::new(w, h);
        ctx.render_with(&mut px, &mut Resources::default(), RasterizerSettings { target_init: TargetInit::Clear(WHITE), ..Default::default() });
        cinza(&px)
    };
    for p in rx {
        let (pw, ph) = pages[p.page].render_dimensions();
        let (pw, ph) = (pw.max(1.0), ph.max(1.0));
        let fit = (W as f32 / pw).min(AREA_H as f32 / ph);
        let mut out = None;
        if p.cortar {
            // Rascunho pequeno para achar a caixa do texto; depois a página na escala que faz
            // essa caixa ocupar a tela, recortada.
            let s0 = fit * 0.3;
            let (g, w, h) = desenhar(p.page, s0);
            if let Some((x0, y0, x1, y1)) = caixa_do_conteudo(&g, w, h) {
                let (bw, bh) = ((x1 - x0) as f32 / s0, (y1 - y0) as f32 / s0);
                // Só vale a pena se a página crescer de verdade na tela.
                let s = (W as f32 / bw).min(AREA_H as f32 / bh).min(fit * 3.0);
                if s > fit * 1.06 {
                    let k = s / s0;
                    let (rw, rh) = (((bw * s).round() as u16).clamp(1, W as u16), ((bh * s).round() as u16).clamp(1, AREA_H as u16));
                    let g = regiao(p.page, s, (x0 as f32 * k) as f64, (y0 as f32 * k) as f64, rw, rh);
                    out = Some(Pagina { w: rw as i32, h: rh as i32, px: g });
                }
            }
        }
        let pagina = out.unwrap_or_else(|| {
            let (g, w, h) = desenhar(p.page, fit);
            Pagina { w: w as i32, h: h as i32, px: g }
        });
        if tx.send((p.page, p.cortar, Ok(pagina))).is_err() {
            break;
        }
    }
}

fn zip_trabalhar(path: PathBuf, rx: Receiver<Pedido>, tx: Sender<Resposta>, n_tx: Sender<Result<usize, String>>) {
    let aberto = std::fs::File::open(&path).map_err(|e| e.to_string()).and_then(|f| zip::ZipArchive::new(f).map_err(|_| "não é um CBZ válido".to_string()));
    let mut z = match aberto {
        Ok(z) => z,
        Err(e) => {
            let _ = n_tx.send(Err(e));
            return;
        }
    };
    let mut nomes: Vec<String> = (0..z.len()).filter_map(|i| z.by_index(i).ok().and_then(|e| e.name().ok().map(|n| n.to_string()))).filter(|n| e_imagem(n)).collect();
    nomes.sort_by(|a, b| ordem_natural(a, b));
    if nomes.is_empty() {
        let _ = n_tx.send(Err("quadrinho sem imagens".into()));
        return;
    }
    let _ = n_tx.send(Ok(nomes.len()));
    for p in rx {
        let r = z.by_name(&nomes[p.page]).map_err(|e| e.to_string()).and_then(|mut e| {
            let mut b = Vec::new();
            e.read_to_end(&mut b).map_err(|e| e.to_string())?;
            Ok(b)
        });
        if tx.send((p.page, p.cortar, r.and_then(|b| decodificar(&b)))).is_err() {
            break;
        }
    }
}

fn rar_trabalhar(path: PathBuf, rx: Receiver<Pedido>, tx: Sender<Resposta>, n_tx: Sender<Result<usize, String>>) {
    let abrir = || std::fs::File::open(&path).map_err(|e| e.to_string()).and_then(|f| unrar_rs::RarArchive::open(f).map_err(|e| format!("não é um CBR válido ({e})")));
    let mut a = match abrir() {
        Ok(a) => a,
        Err(e) => {
            let _ = n_tx.send(Err(e));
            return;
        }
    };
    let mut nomes: Vec<(usize, String)> = a.entries().enumerate().map(|(i, m)| (i, m.name.to_string())).filter(|(_, n)| e_imagem(n)).collect();
    nomes.sort_by(|a, b| ordem_natural(&a.1, &b.1));
    if nomes.is_empty() {
        let _ = n_tx.send(Err("quadrinho sem imagens".into()));
        return;
    }
    let _ = n_tx.send(Ok(nomes.len()));
    for p in rx {
        let mut b = Vec::new();
        let r = a.by_index(nomes[p.page].0).map_err(|e| e.to_string()).and_then(|m| m.copy_to(&mut b).map_err(|e| e.to_string()));
        if tx.send((p.page, p.cortar, r.and_then(|_| decodificar(&b)))).is_err() {
            break;
        }
    }
}

// ---------- Estado ----------

/// Página em que o KOReader parou (`["last_page"] = N` no metadata.pdf.lua), contada de 0.
pub fn koreader_pagina(path: &Path) -> Option<usize> {
    let meta = std::fs::read_to_string(crate::apps::sidecar(path)).ok()?;
    let i = meta.find("[\"last_page\"]")?;
    let rest = &meta[i..];
    let num: String = rest[rest.find("=")? + 1..].trim_start().chars().take_while(char::is_ascii_digit).collect();
    num.parse::<usize>().ok().map(|n| n.saturating_sub(1))
}

fn pos_file(path: &Path) -> PathBuf {
    crate::reader::pos_file(path)
}

impl Folheador {
    pub fn open(path: &Path, title: &str) -> Result<Folheador, String> {
        let t = tipo(path).ok_or("formato sem leitor próprio")?;
        let (tx, rx_t) = channel::<Pedido>();
        let (tx_t, rx) = channel::<Resposta>();
        let (n_tx, n_rx) = channel();
        let p = path.to_path_buf();
        std::thread::Builder::new()
            .name("folhear".into())
            .stack_size(4 << 20)
            .spawn(move || match t {
                Tipo::Pdf => pdf_trabalhar(p, rx_t, tx_t, n_tx),
                Tipo::Zip => zip_trabalhar(p, rx_t, tx_t, n_tx),
                Tipo::Rar => rar_trabalhar(p, rx_t, tx_t, n_tx),
            })
            .map_err(|e| e.to_string())?;
        let n = n_rx.recv_timeout(Duration::from_secs(30)).map_err(|_| "o arquivo demorou demais para abrir".to_string())??;
        // Posição salva: "página 0 0 cortar progresso" (o 5º campo é o que a biblioteca lê).
        let saved = std::fs::read_to_string(pos_file(path)).unwrap_or_default();
        let v: Vec<&str> = saved.split_whitespace().collect();
        // Primeira vez aqui: continua de onde parou no KOReader (página contada a partir de 1).
        let page = v.first().and_then(|x| x.parse::<usize>().ok()).or_else(|| koreader_pagina(path)).unwrap_or(0).min(n - 1);
        let cortar = v.get(3).map(|x| *x != "0").unwrap_or(true);
        let mut f = Folheador { path: path.into(), title: title.into(), n, page, menu: false, cortar, pdf: t == Tipo::Pdf, turns: 0, tx, rx, prontas: Vec::new(), pedidas: Vec::new() };
        f.pedir(page);
        Ok(f)
    }

    fn chave(&self, page: usize) -> (usize, bool) {
        (page, self.pdf && self.cortar)
    }

    fn pedir(&mut self, page: usize) {
        let k = self.chave(page);
        if page >= self.n || self.pedidas.contains(&k) || self.prontas.iter().any(|(p, c, _)| (*p, *c) == k) {
            return;
        }
        if self.tx.send(Pedido { page: k.0, cortar: k.1 }).is_ok() {
            self.pedidas.push(k);
        }
    }

    fn guardar(&mut self, (p, c, r): Resposta) {
        self.pedidas.retain(|k| *k != (p, c));
        let pagina = r.unwrap_or_else(|e| {
            crate::sys::log(&format!("folhear: página {} de {}: {e}", p + 1, self.path.display()));
            Pagina { w: 0, h: 0, px: Vec::new() }
        });
        self.prontas.push((p, c, pagina));
        // Guarda só as vizinhas: memória do Kindle é pouca.
        let atual = self.page;
        if self.prontas.len() > 4 {
            if let Some(i) = self.prontas.iter().enumerate().max_by_key(|(_, (q, _, _))| q.abs_diff(atual)).map(|(i, _)| i) {
                self.prontas.remove(i);
            }
        }
    }

    /// Recebe o que a thread já terminou, sem esperar.
    pub fn recolher(&mut self) {
        while let Ok(r) = self.rx.try_recv() {
            self.guardar(r);
        }
    }

    /// Página atual, esperando a thread se ainda não estiver pronta.
    pub fn atual(&mut self) -> Option<&Pagina> {
        let k = self.chave(self.page);
        self.pedir(self.page);
        self.recolher();
        while !self.prontas.iter().any(|(p, c, _)| (*p, *c) == k) {
            match self.rx.recv_timeout(Duration::from_secs(30)) {
                Ok(r) => self.guardar(r),
                Err(_) => return None,
            }
        }
        self.prontas.iter().find(|(p, c, _)| (*p, *c) == k).map(|(_, _, g)| g)
    }

    /// Já pede a próxima página (e a anterior), para virar sem esperar.
    pub fn adiantar(&mut self) {
        self.pedir(self.page + 1);
        if self.page > 0 {
            self.pedir(self.page - 1);
        }
    }

    pub fn ir(&mut self, page: usize) -> bool {
        let page = page.min(self.n - 1);
        if page == self.page {
            return false;
        }
        self.page = page;
        self.turns += 1;
        self.save();
        true
    }

    pub fn next(&mut self) -> bool {
        self.page + 1 < self.n && self.ir(self.page + 1)
    }

    pub fn prev(&mut self) -> bool {
        self.page > 0 && self.ir(self.page - 1)
    }

    pub fn progress(&self) -> f32 {
        (self.page + 1) as f32 / self.n.max(1) as f32
    }

    pub fn save(&self) {
        let _ = std::fs::write(pos_file(&self.path), format!("{} 0 0 {} {:.4}\n", self.page, self.cortar as u8, self.progress()));
    }
}

// ---------- Tela ----------

pub fn menu_top() -> Rect {
    Rect::new(0, 0, W, 190)
}
pub fn menu_bottom() -> Rect {
    Rect::new(0, 1130, W, 318)
}
/// Botões do menu: 0 biblioteca, 1 margens (PDF), 2 dez páginas para trás, 3 dez para frente.
pub fn menu_btn(k: usize) -> Rect {
    match k {
        0 => Rect::new(48, 56, 250, 84),
        1 => Rect::new(W - 48 - 290, 56, 290, 84),
        2 => Rect::new(48, 1334, 300, 88),
        _ => Rect::new(W - 48 - 300, 1334, 300, 88),
    }
}

pub fn draw(fb: &mut Framebuffer, f: &crate::text::Fonts, r: &mut Folheador) {
    use crate::text::Face;
    fb.fill(fb.screen(), PAPER);
    match r.atual() {
        Some(p) if p.w > 0 => {
            // Linha a linha (a página ocupa a tela toda: o blit ponto a ponto custava caro).
            let (x, y) = (((W - p.w) / 2).max(0), ((AREA_H - p.h) / 2).max(0));
            let w = p.w.min(fb.width - x) as usize;
            for row in 0..p.h.min(fb.height - y) {
                let d = ((y + row) * fb.width + x) as usize;
                let s = (row * p.w) as usize;
                fb.buf[d..d + w].copy_from_slice(&p.px[s..s + w]);
            }
        }
        _ => {
            f.draw(fb, Face::Display, 40.0, 72, 640, INK, "Não deu para mostrar esta página.");
            f.draw(fb, Face::Body, 28.0, 72, 700, MUTED, "Toque à direita para a próxima.");
        }
    }
    let foot = format!("{} de {}", r.page + 1, r.n);
    f.draw(fb, Face::Body, 24.0, 48, 1428, MUTED, &foot);
    let pct = format!("{}%", (r.progress() * 100.0).round() as i32);
    let w = f.measure(Face::Body, 24.0, &pct);
    f.draw(fb, Face::Body, 24.0, W - 48 - w, 1428, MUTED, &pct);
    let bar = Rect::new(48 + 160, 1420, W - 96 - 320, 4);
    fb.fill(bar, 0xDD);
    fb.fill(Rect::new(bar.x, bar.y, (bar.w as f32 * r.progress()) as i32, bar.h), MUTED);
    if r.menu {
        draw_menu(fb, f, r);
    }
}

fn draw_menu(fb: &mut Framebuffer, f: &crate::text::Fonts, r: &Folheador) {
    use crate::text::Face;
    use crate::ui;
    let (t, b) = (menu_top(), menu_bottom());
    fb.fill(t, PAPER);
    fb.fill(Rect::new(0, t.h, W, 8), ui::SHADOW);
    fb.fill(Rect::new(0, t.h - 3, W, 3), INK);
    fb.fill(b, PAPER);
    fb.fill(Rect::new(0, b.y, W, 3), INK);
    ui::button(fb, f, menu_btn(0), "‹ Biblioteca", false);
    let x1 = if r.pdf {
        ui::button(fb, f, menu_btn(1), if r.cortar { "Página inteira" } else { "Cortar margens" }, false);
        menu_btn(1).x - 24
    } else {
        W - 48
    };
    let x0 = menu_btn(0).x + menu_btn(0).w + 24;
    let title = crate::apps::fit(f, Face::Display, 34.0, &r.title, x1 - x0);
    let tw = f.measure(Face::Display, 34.0, &title);
    f.draw(fb, Face::Display, 34.0, x0 + (x1 - x0 - tw) / 2, 112, INK, &title);
    let info = format!("Página {} de {}", r.page + 1, r.n);
    f.draw(fb, Face::Display, 36.0, 48, b.y + 64, INK, &info);
    let how = if r.pdf { if r.cortar { "PDF · margens cortadas para a letra ficar maior" } else { "PDF · página inteira" } } else { "Quadrinho" };
    f.draw(fb, Face::Body, 26.0, 48, b.y + 106, MUTED, how);
    let pct = format!("{}%", (r.progress() * 100.0).round() as i32);
    let pw = f.measure(Face::Strong, 30.0, &pct);
    let bar = Rect::new(48, b.y + 140, W - 96 - pw - 24, 18);
    fb.round_rect(bar, 9, 0xD0);
    let fill = (bar.w as f32 * r.progress()) as i32;
    if fill > 0 {
        fb.round_rect(Rect::new(bar.x, bar.y, fill.max(18), bar.h), 9, INK);
    }
    f.draw(fb, Face::Strong, 30.0, W - 48 - pw, bar.y + 18, INK, &pct);
    // Perto das pontas, o botão diz para onde vai de verdade.
    if r.page > 0 {
        ui::button(fb, f, menu_btn(2), if r.page < 10 { "‹ Início" } else { "‹ 10 páginas" }, false);
    }
    if r.page + 1 < r.n {
        ui::button(fb, f, menu_btn(3), if r.n - 1 - r.page <= 10 { "Fim ›" } else { "10 páginas ›" }, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordem_natural_das_paginas() {
        let mut v = vec!["p10.jpg", "p2.jpg", "P1.jpg", "p02b.jpg", "capa.jpg"];
        v.sort_by(|a, b| ordem_natural(a, b));
        assert_eq!(v, vec!["capa.jpg", "P1.jpg", "p2.jpg", "p02b.jpg", "p10.jpg"]);
        assert!(e_imagem("x/01.JPG") && e_imagem("a.webp"));
        assert!(!e_imagem("__MACOSX/x/._01.jpg") && !e_imagem("x/._01.jpg") && !e_imagem("info.txt"));
    }

    #[test]
    fn caixa_acha_o_texto_com_folga() {
        let (w, h) = (100usize, 200usize);
        let mut px = vec![255u8; w * h];
        for y in 50..150 {
            for x in 20..80 {
                px[y * w + x] = 0;
            }
        }
        let (x0, y0, x1, y1) = caixa_do_conteudo(&px, w, h).unwrap();
        assert!(x0 < 20 && x0 >= 16 && y0 < 50 && y0 >= 46, "{x0} {y0}");
        assert!(x1 > 80 && x1 <= 84 && y1 > 150 && y1 <= 154, "{x1} {y1}");
        assert!(caixa_do_conteudo(&vec![255u8; w * h], w, h).is_none());
    }
}

#[cfg(test)]
mod abrir {
    use super::*;

    /// PDF mínimo de uma página 200×300 pt com um retângulo preto no meio (margens largas).
    fn pdf_minimo() -> Vec<u8> {
        let conteudo = "0 g 60 100 80 100 re f";
        let objs = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Contents 4 0 R >>".to_string(),
            format!("<< /Length {} >>\nstream\n{conteudo}\nendstream", conteudo.len()),
        ];
        let mut out = b"%PDF-1.4\n".to_vec();
        let mut offs = Vec::new();
        for (i, o) in objs.iter().enumerate() {
            offs.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
        for o in offs {
            out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
        out
    }

    fn tmp(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("verso-folhear-{}-{nome}", std::process::id()));
        let _ = std::fs::create_dir_all(&d);
        d.join(nome)
    }

    fn escuros(p: &Pagina) -> f32 {
        p.px.iter().filter(|&&v| v < 128).count() as f32 / p.px.len().max(1) as f32
    }

    #[test]
    fn pdf_abre_e_corta_as_margens() {
        let path = tmp("teste.pdf");
        std::fs::write(&path, pdf_minimo()).unwrap();
        let mut f = Folheador::open(&path, "Teste").unwrap();
        assert_eq!(f.n, 1);
        assert!(f.pdf);
        f.cortar = false;
        let inteira = f.atual().map(|p| (p.w, p.h, escuros(p))).unwrap();
        // Página inteira: 200×300 cabe em 927×1390; o retângulo é 1/7,5 da área.
        assert!((926..=927).contains(&inteira.0) && inteira.1 == 1390, "{inteira:?}");
        assert!((inteira.2 - 0.133).abs() < 0.02, "{inteira:?}");
        f.cortar = true;
        let cortada = f.atual().map(|p| (p.w, p.h, escuros(p))).unwrap();
        // Cortada: o retângulo (com folga) ocupa quase toda a página mostrada.
        assert!(cortada.2 > 0.7, "{cortada:?}");
        assert!(cortada.0 >= 1000 || cortada.1 >= 1350, "{cortada:?}");
    }

    #[test]
    fn cbz_em_ordem_natural_e_cabendo_na_tela() {
        let path = tmp("teste.cbz");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        // Larguras diferentes para conferir a ordem: p1 = 100, p2 = 200, p10 = 300 de largura.
        for (nome, w) in [("p10.png", 300u32), ("p2.png", 200), ("p1.png", 100), ("leia.txt", 0)] {
            z.start_file(nome, zip::write::SimpleFileOptions::default()).unwrap();
            if w == 0 {
                std::io::Write::write_all(&mut z, b"nada").unwrap();
                continue;
            }
            let img = image::GrayImage::from_pixel(w, 600, image::Luma([0]));
            let mut png = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
            std::io::Write::write_all(&mut z, &png).unwrap();
        }
        z.finish().unwrap();
        let mut f = Folheador::open(&path, "Gibi").unwrap();
        assert_eq!(f.n, 3);
        assert!(!f.pdf);
        let mut larguras = Vec::new();
        for i in 0..3 {
            f.page = i;
            let p = f.atual().unwrap();
            assert_eq!(p.h, AREA_H);
            larguras.push(p.w);
        }
        assert!(larguras[0] < larguras[1] && larguras[1] < larguras[2], "{larguras:?}");
    }

    #[test]
    fn cbr_abre_as_tres_paginas() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/revista.cbr");
        let mut f = Folheador::open(&path, "Revista").unwrap();
        assert_eq!(f.n, 3);
        for i in 0..3 {
            f.page = i;
            let p = f.atual().unwrap();
            assert!(p.w > 0 && p.h > 0 && p.px.len() == (p.w * p.h) as usize);
        }
    }

    #[test]
    fn continua_da_pagina_do_koreader() {
        let path = tmp("lido.pdf");
        let sdr = path.with_extension("sdr");
        std::fs::create_dir_all(&sdr).unwrap();
        std::fs::write(sdr.join("metadata.pdf.lua"), "return {
    [\"doc_pages\"] = 361,
    [\"last_page\"] = 42,
}
").unwrap();
        assert_eq!(koreader_pagina(&path), Some(41));
        assert_eq!(koreader_pagina(&tmp("nunca-lido.pdf")), None);
    }

    #[test]
    fn arquivo_quebrado_da_erro_sem_travar() {
        let path = tmp("quebrado.pdf");
        std::fs::write(&path, b"isto nao e um pdf").unwrap();
        assert!(Folheador::open(&path, "x").is_err());
        let path = tmp("quebrado.cbz");
        std::fs::write(&path, b"isto nao e um zip").unwrap();
        assert!(Folheador::open(&path, "x").is_err());
    }
}

/// Captura da tela com um arquivo de verdade: `VERSO_FOLHEAR=arquivo cargo test --release captura -- --ignored`.
#[cfg(test)]
mod captura {
    #[test]
    #[ignore]
    fn captura_de_tela() {
        let Some(arq) = std::env::var_os("VERSO_FOLHEAR") else { return };
        let path = std::path::PathBuf::from(arq);
        let fonts = crate::text::Fonts::new();
        let mut fb = crate::fb::Framebuffer::memory();
        let mut f = super::Folheador::open(&path, "Livro de teste com um título bem comprido").unwrap();
        f.page = std::env::var("VERSO_PAGINA").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
        for (i, (cortar, menu)) in [(true, false), (false, false), (true, true)].into_iter().enumerate() {
            f.cortar = cortar;
            f.menu = menu;
            let t = std::time::Instant::now();
            super::draw(&mut fb, &fonts, &mut f);
            println!("desenho {i}: {:?}", t.elapsed());
            fb.save_pgm(&std::path::Path::new("/tmp").join(format!("folhear-{i}.pgm")));
        }
    }
}
