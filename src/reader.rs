//! Leitor próprio (EPUB e TXT): abre na hora, sem trocar de programa. Texto em Noto Serif
//! (a do KOReader, se estiver no Kindle; senão Inter), justificado, com tamanho ajustável,
//! sumário e posição salva por livro. PDF e quadrinhos continuam no KOReader.

use crate::fb::{Framebuffer, Rect};
use crate::ui::{INK, MUTED, PAPER};
use fontdue::{Font, FontSettings, Metrics};
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const W: i32 = 1072;
const SIDE: i32 = 70;
const TOP: i32 = 100;
const BOTTOM: i32 = 1370;
pub const SIZES: [f32; 7] = [28.0, 32.0, 36.0, 40.0, 44.0, 50.0, 56.0];

// ---------- Fontes com cache de letras ----------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Style {
    Regular,
    Italic,
    Bold,
}

pub struct RFonts {
    reg: Font,
    ital: Font,
    bold: Font,
    cache: RefCell<HashMap<(Style, u32, char), (Metrics, Vec<u8>)>>,
}

fn font_file(name: &str, fallback: &'static [u8]) -> Font {
    let p = Path::new("/mnt/us/koreader/fonts/noto").join(name);
    std::fs::read(p)
        .ok()
        .and_then(|b| Font::from_bytes(b, FontSettings::default()).ok())
        .unwrap_or_else(|| Font::from_bytes(fallback, FontSettings::default()).expect("fonte embutida"))
}

impl RFonts {
    pub fn load() -> Self {
        let inter = include_bytes!("../assets/fonts/Inter-Regular.ttf");
        let inter_b = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
        RFonts {
            reg: font_file("NotoSerif-Regular.ttf", inter),
            ital: font_file("NotoSerif-Italic.ttf", inter),
            bold: font_file("NotoSerif-Bold.ttf", inter_b),
            cache: RefCell::new(HashMap::new()),
        }
    }

    fn font(&self, s: Style) -> &Font {
        match s {
            Style::Regular => &self.reg,
            Style::Italic => &self.ital,
            Style::Bold => &self.bold,
        }
    }

    pub fn measure(&self, s: Style, size: f32, text: &str) -> f32 {
        let f = self.font(s);
        let mut w = 0.0;
        let mut prev = None;
        for c in text.chars() {
            if let Some(p) = prev {
                w += f.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            w += f.metrics(c, size).advance_width;
            prev = Some(c);
        }
        w
    }

    /// Desenha com a linha de base em `baseline`, usando o cache de letras rasterizadas.
    pub fn draw(&self, fb: &mut Framebuffer, s: Style, size: f32, x: f32, baseline: i32, ink: u8, text: &str) -> f32 {
        let f = self.font(s);
        let key_size = (size * 4.0) as u32;
        let mut pen = x;
        let mut prev = None;
        let mut cache = self.cache.borrow_mut();
        if cache.len() > 4000 {
            cache.clear();
        }
        for c in text.chars() {
            if let Some(p) = prev {
                pen += f.horizontal_kern(p, c, size).unwrap_or(0.0);
            }
            let (m, bitmap) = cache.entry((s, key_size, c)).or_insert_with(|| f.rasterize(c, size));
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
        pen
    }
}

// ---------- Livro ----------

#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Para(Vec<Run>),
    Head(Vec<Run>),
    Rule,
}

pub struct Book {
    pub path: PathBuf,
    pub title: String,
    /// Arquivos do capítulo (EPUB) na ordem de leitura, ou um só (TXT).
    spine: Vec<String>,
    /// Títulos do sumário por capítulo (quando o livro tiver).
    pub titles: Vec<Option<String>>,
    kind: Kind,
    /// MOBI/AZW3: o livro inteiro em blocos e os capítulos como faixas desses blocos.
    blocks: Vec<Block>,
    ranges: Vec<(usize, usize)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Epub,
    Txt,
    Mobi,
}

pub fn supported(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("epub") | Some("txt") | Some("azw3") | Some("mobi"))
}

// ---------- MOBI / AZW3 (sem DRM) ----------

fn be16(b: &[u8], i: usize) -> Option<usize> {
    Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?) as usize)
}
fn be32(b: &[u8], i: usize) -> Option<usize> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?) as usize)
}

/// Descompressão PalmDOC (LZ77 simples do formato MOBI).
pub fn palmdoc(src: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(src.len() * 2);
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        i += 1;
        match c {
            0x01..=0x08 => {
                let n = c as usize;
                out.extend_from_slice(&src[i..(i + n).min(src.len())]);
                i += n;
            }
            0x00 | 0x09..=0x7F => out.push(c),
            0x80..=0xBF => {
                let Some(&c2) = src.get(i) else { break };
                i += 1;
                let pair = ((c as usize) << 8 | c2 as usize) & 0x3FFF;
                let dist = pair >> 3;
                let len = (pair & 7) + 3;
                if dist == 0 || dist > out.len() {
                    continue;
                }
                let start = out.len() - dist;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => {
                out.push(b' ');
                out.push(c ^ 0x80);
            }
        }
    }
    out
}

/// Tamanho dos dados extras no fim de um registro de texto (marcados nos bits de `flags`).
fn trailing_size(rec: &[u8], flags: usize) -> usize {
    let mut end = rec.len();
    for bit in (1..16).rev() {
        if flags & (1 << bit) == 0 {
            continue;
        }
        // Tamanho em largura variável, lido de trás para frente.
        let (mut size, mut shift) = (0usize, 0);
        let mut k = end;
        while k > 0 && shift < 28 {
            k -= 1;
            let b = rec[k] as usize;
            size |= (b & 0x7F) << shift;
            shift += 7;
            if b & 0x80 != 0 {
                break;
            }
        }
        end = end.saturating_sub(size);
    }
    if flags & 1 != 0 && end > 0 {
        end = end.saturating_sub((rec[end - 1] & 3) as usize + 1);
    }
    rec.len() - end
}

/// Lê o texto (HTML) de um MOBI/AZW3 sem DRM, com compressão nenhuma ou PalmDOC.
pub fn mobi_text(data: &[u8]) -> Result<(String, String), String> {
    let n = be16(data, 76).ok_or("arquivo curto")?;
    let offs: Vec<usize> = (0..n).filter_map(|i| be32(data, 78 + i * 8)).collect();
    let rec = |i: usize| -> &[u8] {
        let a = offs.get(i).copied().unwrap_or(data.len()).min(data.len());
        let b = offs.get(i + 1).copied().unwrap_or(data.len()).min(data.len());
        &data[a..b.max(a)]
    };
    let r0 = rec(0);
    let comp = be16(r0, 0).ok_or("cabeçalho")?;
    let text_recs = be16(r0, 8).ok_or("cabeçalho")?;
    if be16(r0, 12).unwrap_or(0) != 0 {
        return Err("livro com DRM".into());
    }
    if comp != 1 && comp != 2 {
        return Err("compressão não suportada (HUFF)".into());
    }
    let hlen = be32(r0, 20).unwrap_or(0);
    let utf8 = be32(r0, 28).unwrap_or(0) == 65001;
    let flags = if hlen >= 0xE4 { be16(r0, 0xF2).unwrap_or(0) } else { 0 };
    let title = match (be32(r0, 0x54), be32(r0, 0x58)) {
        (Some(o), Some(l)) if o + l <= r0.len() => String::from_utf8_lossy(&r0[o..o + l]).to_string(),
        _ => String::new(),
    };
    let mut raw = Vec::new();
    for i in 1..=text_recs {
        let r = rec(i);
        let body = &r[..r.len() - trailing_size(r, flags).min(r.len())];
        if comp == 2 {
            raw.extend(palmdoc(body));
        } else {
            raw.extend_from_slice(body);
        }
    }
    let html = if utf8 { String::from_utf8_lossy(&raw).to_string() } else { raw.iter().map(|&b| b as char).collect() };
    Ok((title, html))
}

/// Parágrafo curto do tipo "CAPÍTULO IV", "Parte 2", "Livro I" (título não marcado como tal).
fn looks_like_heading(runs: &[Run]) -> Option<String> {
    let t: String = runs.iter().map(|r| r.text.as_str()).collect::<String>();
    let t = t.trim();
    if t.split_whitespace().count() > 8 {
        return None;
    }
    let low = crate::apps::fold(t);
    let mut w = low.split_whitespace();
    let first = w.next()?;
    let second = w.next()?;
    let is_num = second.chars().all(|c| c.is_ascii_digit()) || second.chars().all(|c| "ivxlc".contains(c));
    (["capitulo", "parte", "livro", "chapter", "part"].contains(&first) && is_num).then(|| t.to_string())
}

/// Divide os blocos em capítulos: a cada título (depois de algum texto).
fn split_chapters(blocks: &[Block]) -> (Vec<(usize, usize)>, Vec<Option<String>>) {
    let mut ranges = Vec::new();
    let mut titles = Vec::new();
    let mut start = 0;
    let mut cur_title: Option<String> = None;
    let mut words = 0usize;
    for (i, b) in blocks.iter().enumerate() {
        match b {
            Block::Head(r) => {
                if words > 60 && i > start {
                    ranges.push((start, i));
                    titles.push(cur_title.take());
                    start = i;
                    words = 0;
                }
                if cur_title.is_none() {
                    cur_title = Some(r.iter().map(|x| x.text.as_str()).collect::<String>());
                }
            }
            Block::Para(r) => {
                if let Some(h) = looks_like_heading(r) {
                    if words > 60 && i > start {
                        ranges.push((start, i));
                        titles.push(cur_title.take());
                        start = i;
                        words = 0;
                    }
                    if cur_title.is_none() {
                        cur_title = Some(h);
                    }
                } else {
                    words += r.iter().map(|x| x.text.split_whitespace().count()).sum::<usize>();
                }
            }
            Block::Rule => {}
        }
    }
    if start < blocks.len() || ranges.is_empty() {
        ranges.push((start, blocks.len()));
        titles.push(cur_title);
    }
    (ranges, titles)
}

fn zip_text(z: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let i = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let j = tag[i..].find('"')? + i;
    Some(tag[i..j].to_string())
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Junta o diretório do OPF com um href relativo (resolve "..").
fn join(dir: &str, href: &str) -> String {
    let href = percent_decode(href.split('#').next().unwrap_or(""));
    let mut parts: Vec<&str> = if dir.is_empty() { vec![] } else { dir.split('/').collect() };
    for p in href.split('/') {
        match p {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            x => parts.push(x),
        }
    }
    parts.join("/")
}

impl Book {
    pub fn open(path: &Path) -> Result<Book, String> {
        let title_from_file = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Livro").to_string();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("txt")) {
            return Ok(Book { path: path.into(), title: title_from_file, spine: vec![String::new()], titles: vec![None], kind: Kind::Txt, blocks: vec![], ranges: vec![] });
        }
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).unwrap_or_default();
        if ext == "azw3" || ext == "mobi" {
            let data = std::fs::read(path).map_err(|e| e.to_string())?;
            let (title, html) = mobi_text(&data)?;
            // Quebras de página do MOBI viram títulos de seção para dividir capítulos.
            let html = html.replace("<mbp:pagebreak/>", "<hr/>").replace("<mbp:pagebreak />", "<hr/>");
            let blocks = parse_xhtml(&html);
            let (ranges, titles) = split_chapters(&blocks);
            let title = if title.trim().is_empty() { title_from_file } else { decode_entities(title.trim()) };
            return Ok(Book { path: path.into(), title, spine: vec![String::new(); ranges.len()], titles, kind: Kind::Mobi, blocks, ranges });
        }
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut z = zip::ZipArchive::new(file).map_err(|_| "não é um EPUB válido".to_string())?;
        let container = zip_text(&mut z, "META-INF/container.xml").ok_or("EPUB sem container.xml")?;
        let opf_path = container.find("full-path=\"").and_then(|i| attr(&container[i - 1..], "full-path")).ok_or("EPUB sem OPF")?;
        let opf = zip_text(&mut z, &opf_path).ok_or("OPF ilegível")?;
        let dir = opf_path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
        let title = opf
            .find("<dc:title")
            .and_then(|i| {
                let s = &opf[i..];
                let a = s.find('>')? + 1;
                let b = s.find("</dc:title")?;
                Some(decode_entities(s[a..b].trim()))
            })
            .filter(|t| !t.is_empty())
            .unwrap_or(title_from_file);
        // Manifesto: id -> (href, tipo, propriedades).
        let mut items: HashMap<String, (String, String, String)> = HashMap::new();
        for tag in opf.split("<item ").skip(1) {
            let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
            if let (Some(id), Some(href)) = (attr(tag, "id"), attr(tag, "href")) {
                items.insert(id, (href, attr(tag, "media-type").unwrap_or_default(), attr(tag, "properties").unwrap_or_default()));
            }
        }
        let mut spine = Vec::new();
        for tag in opf.split("<itemref ").skip(1) {
            let tag = &tag[..tag.find('>').unwrap_or(tag.len())];
            if let Some((href, _, _)) = attr(tag, "idref").and_then(|id| items.get(&id)) {
                spine.push(join(&dir, href));
            }
        }
        if spine.is_empty() {
            return Err("EPUB sem capítulos".into());
        }
        // Sumário: nav.xhtml (EPUB 3) ou toc.ncx (EPUB 2) → título por arquivo.
        let mut toc: HashMap<String, String> = HashMap::new();
        let nav = items.values().find(|(_, _, p)| p.contains("nav")).map(|(h, _, _)| join(&dir, h));
        let ncx = items.values().find(|(_, t, _)| t.contains("ncx")).map(|(h, _, _)| join(&dir, h));
        if let Some(nav) = nav.and_then(|n| zip_text(&mut z, &n).map(|t| (n, t))) {
            let ndir = nav.0.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            for a in nav.1.split("<a ").skip(1) {
                if let (Some(href), Some(end)) = (attr(a, "href"), a.find("</a>")) {
                    let label = strip_tags(&a[a.find('>').map_or(0, |i| i + 1)..end]);
                    toc.entry(join(&ndir, &href)).or_insert(label);
                }
            }
        } else if let Some(n) = ncx.and_then(|n| zip_text(&mut z, &n).map(|t| (n, t))) {
            let ndir = n.0.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            for np in n.1.split("<navPoint").skip(1) {
                let label = np.find("<text>").and_then(|i| np[i + 6..].find("</text>").map(|j| decode_entities(np[i + 6..i + 6 + j].trim())));
                let src = np.find("<content ").and_then(|i| attr(&np[i..], "src"));
                if let (Some(l), Some(s)) = (label, src) {
                    toc.entry(join(&ndir, &s)).or_insert(l);
                }
            }
        }
        let titles = spine.iter().map(|s| toc.get(s).cloned().filter(|t| !t.is_empty())).collect();
        Ok(Book { path: path.into(), title, spine, titles, kind: Kind::Epub, blocks: vec![], ranges: vec![] })
    }

    pub fn chapters(&self) -> usize {
        self.spine.len()
    }

    /// Lê e converte um capítulo em blocos de texto.
    pub fn chapter(&self, k: usize) -> Vec<Block> {
        match self.kind {
            Kind::Txt => {
                let bytes = std::fs::read(&self.path).unwrap_or_default();
                let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
                text.split("\n\n")
                    .flat_map(|p| if p.contains("\r\n\r\n") { p.split("\r\n\r\n").map(str::to_string).collect::<Vec<_>>() } else { vec![p.to_string()] })
                    .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
                    .filter(|p| !p.is_empty())
                    .map(|p| Block::Para(vec![Run { text: p, style: Style::Regular }]))
                    .collect()
            }
            Kind::Mobi => self.ranges.get(k).map(|&(a, b)| self.blocks[a..b].to_vec()).unwrap_or_default(),
            Kind::Epub => {
                let Some(name) = self.spine.get(k) else { return vec![] };
                let Ok(file) = std::fs::File::open(&self.path) else { return vec![] };
                let Ok(mut z) = zip::ZipArchive::new(file) else { return vec![] };
                zip_text(&mut z, name).map(|x| parse_xhtml(&x)).unwrap_or_default()
            }
        }
    }

    pub fn chapter_title(&self, k: usize, blocks: &[Block]) -> String {
        if let Some(Some(t)) = self.titles.get(k) {
            return t.clone();
        }
        blocks
            .iter()
            .find_map(|b| if let Block::Head(r) = b { Some(r.iter().map(|x| x.text.as_str()).collect::<String>()) } else { None })
            .unwrap_or_else(|| format!("Parte {}", k + 1))
    }
}

// ---------- XHTML → blocos ----------

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(out.split_whitespace().collect::<Vec<_>>().join(" ").trim())
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(end) = tail.find(';').filter(|&e| e < 12) else {
            out.push('&');
            rest = &tail[1..];
            continue;
        };
        let name = &tail[1..end];
        let ch: Option<String> = if let Some(n) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
            u32::from_str_radix(n, 16).ok().and_then(char::from_u32).map(String::from)
        } else if let Some(n) = name.strip_prefix('#') {
            n.parse().ok().and_then(char::from_u32).map(String::from)
        } else {
            Some(
                match name {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    "nbsp" => "\u{a0}",
                    "mdash" => "—",
                    "ndash" => "–",
                    "hellip" => "…",
                    "lsquo" => "‘",
                    "rsquo" => "’",
                    "ldquo" => "“",
                    "rdquo" => "”",
                    "laquo" => "«",
                    "raquo" => "»",
                    "aacute" => "á",
                    "eacute" => "é",
                    "iacute" => "í",
                    "oacute" => "ó",
                    "uacute" => "ú",
                    "atilde" => "ã",
                    "otilde" => "õ",
                    "ccedil" => "ç",
                    "acirc" => "â",
                    "ecirc" => "ê",
                    "ocirc" => "ô",
                    "agrave" => "à",
                    "Aacute" => "Á",
                    "Eacute" => "É",
                    "Ccedil" => "Ç",
                    "shy" => "",
                    _ => return_unknown(),
                }
                .to_string(),
            )
        };
        match ch {
            Some(c) if !c.is_empty() || name == "shy" => out.push_str(&c),
            _ => out.push_str(&tail[..=end]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

fn return_unknown() -> &'static str {
    ""
}

/// Converte o XHTML de um capítulo em parágrafos e títulos, com itálico e negrito.
pub fn parse_xhtml(src: &str) -> Vec<Block> {
    let body = src.find("<body").map(|i| &src[i..]).unwrap_or(src);
    let mut blocks = Vec::new();
    let mut runs: Vec<Run> = Vec::new();
    let (mut ital, mut bold, mut skip, mut head) = (0i32, 0i32, 0i32, 0i32);
    let mut text = String::new();
    let flush_text = |text: &mut String, runs: &mut Vec<Run>, ital: i32, bold: i32| {
        if text.is_empty() {
            return;
        }
        let style = if bold > 0 { Style::Bold } else if ital > 0 { Style::Italic } else { Style::Regular };
        let t = decode_entities(text);
        match runs.last_mut() {
            Some(r) if r.style == style => r.text.push_str(&t),
            _ => runs.push(Run { text: t, style }),
        }
        text.clear();
    };
    let flush_block = |runs: &mut Vec<Run>, blocks: &mut Vec<Block>, head: bool| {
        // Junta espaços e tira os das pontas.
        let mut clean: Vec<Run> = Vec::new();
        for r in runs.drain(..) {
            let t: String = r.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let lead = r.text.starts_with(char::is_whitespace) && !clean.is_empty();
            let trail = r.text.ends_with(char::is_whitespace);
            if t.is_empty() {
                if lead || trail {
                    if let Some(l) = clean.last_mut() {
                        if !l.text.ends_with(' ') {
                            l.text.push(' ');
                        }
                    }
                }
                continue;
            }
            let mut t = if lead { format!(" {t}") } else { t };
            if trail {
                t.push(' ');
            }
            clean.push(Run { text: t, style: r.style });
        }
        if let Some(f) = clean.first_mut() {
            f.text = f.text.trim_start().to_string();
        }
        if let Some(l) = clean.last_mut() {
            l.text = l.text.trim_end().to_string();
        }
        clean.retain(|r| !r.text.is_empty());
        if !clean.is_empty() {
            blocks.push(if head { Block::Head(clean) } else { Block::Para(clean) });
        }
    };
    let mut rest = body;
    while !rest.is_empty() {
        if let Some(i) = rest.find('<') {
            if skip == 0 {
                text.push_str(&rest[..i]);
            }
            let tail = &rest[i..];
            let end = tail.find('>').map(|e| e + 1).unwrap_or(tail.len());
            let tag = &tail[1..end.saturating_sub(1)];
            rest = &tail[end..];
            let closing = tag.starts_with('/');
            let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase();
            let selfclose = tag.ends_with('/');
            match name.as_str() {
                "script" | "style" | "head" | "title" => {
                    if !selfclose {
                        skip += if closing { -1 } else { 1 };
                        skip = skip.max(0);
                    }
                }
                "i" | "em" | "cite" => {
                    flush_text(&mut text, &mut runs, ital, bold);
                    ital += if closing { -1 } else { 1 };
                    ital = ital.max(0);
                }
                "b" | "strong" => {
                    flush_text(&mut text, &mut runs, ital, bold);
                    bold += if closing { -1 } else { 1 };
                    bold = bold.max(0);
                }
                "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                    flush_text(&mut text, &mut runs, ital, bold);
                    flush_block(&mut runs, &mut blocks, head > 0);
                    head += if closing { -1 } else { 1 };
                    head = head.max(0);
                }
                "p" | "div" | "li" | "blockquote" | "tr" | "section" | "dd" | "dt" | "pre" | "table" | "ul" | "ol" | "figure" => {
                    flush_text(&mut text, &mut runs, ital, bold);
                    flush_block(&mut runs, &mut blocks, head > 0);
                }
                "br" => {
                    flush_text(&mut text, &mut runs, ital, bold);
                    flush_block(&mut runs, &mut blocks, head > 0);
                }
                "hr" => {
                    flush_text(&mut text, &mut runs, ital, bold);
                    flush_block(&mut runs, &mut blocks, head > 0);
                    blocks.push(Block::Rule);
                }
                _ => {}
            }
        } else {
            if skip == 0 {
                text.push_str(rest);
            }
            break;
        }
    }
    flush_text(&mut text, &mut runs, ital, bold);
    flush_block(&mut runs, &mut blocks, head > 0);
    blocks
}

// ---------- Paginação ----------

/// Palavra posicionada numa linha.
#[derive(Clone, Debug)]
pub struct Piece {
    pub x: f32,
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub baseline: i32,
    pub size: f32,
    pub pieces: Vec<Piece>,
    pub rule: bool,
}

/// Página: linhas e a posição (bloco, palavra) onde ela começa.
#[derive(Clone, Debug)]
pub struct Page {
    pub lines: Vec<Line>,
    pub start: (usize, usize),
}

pub fn paginate(fonts: &RFonts, blocks: &[Block], size: f32) -> Vec<Page> {
    let width = (W - 2 * SIDE) as f32;
    let lh = (size * 1.48).round() as i32;
    let mut pages: Vec<Page> = Vec::new();
    let mut cur = Page { lines: Vec::new(), start: (0, 0) };
    let mut y = TOP;
    let mut after_head = true;
    let new_page = |pages: &mut Vec<Page>, cur: &mut Page, y: &mut i32, start: (usize, usize)| {
        let done = std::mem::replace(cur, Page { lines: Vec::new(), start });
        if !done.lines.is_empty() {
            pages.push(done);
        }
        *y = TOP;
    };
    for (bi, block) in blocks.iter().enumerate() {
        match block {
            Block::Rule => {
                if y + lh > BOTTOM {
                    new_page(&mut pages, &mut cur, &mut y, (bi, 0));
                }
                cur.lines.push(Line { baseline: y + lh / 2, size, pieces: vec![], rule: true });
                y += lh;
                after_head = true;
            }
            Block::Para(runs) | Block::Head(runs) => {
                let head = matches!(block, Block::Head(_));
                let sz = if head { (size * 1.3).round() } else { size };
                let blh = (sz * 1.48).round() as i32;
                // Palavras com estilo; a palavra mantém o espaço que vem antes dela.
                let mut words: Vec<(String, Style)> = Vec::new();
                for r in runs {
                    let st = if head { Style::Bold } else { r.style };
                    let mut first = true;
                    for (k, w) in r.text.split(' ').enumerate() {
                        if w.is_empty() {
                            continue;
                        }
                        // Palavra colada à anterior (sem espaço entre runs): junta.
                        if first && k == 0 && !r.text.starts_with(' ') && !words.is_empty() && !head {
                            if let Some(last) = words.last_mut() {
                                if last.1 == st {
                                    last.0.push_str(w);
                                    first = false;
                                    continue;
                                }
                            }
                        }
                        words.push((w.to_string(), st));
                        first = false;
                    }
                }
                if words.is_empty() {
                    continue;
                }
                if head {
                    y += lh / 2;
                }
                let indent = if head || after_head { 0.0 } else { sz * 1.4 };
                let space = fonts.measure(Style::Regular, sz, " ");
                let mut i = 0;
                let mut first_line = true;
                while i < words.len() {
                    let avail = width - if first_line { indent } else { 0.0 };
                    let mut j = i;
                    let mut natural = 0.0;
                    let mut ws: Vec<f32> = Vec::new();
                    while j < words.len() {
                        let w = fonts.measure(words[j].1, sz, &words[j].0);
                        let add = if j == i { w } else { space + w };
                        if natural + add > avail && j > i {
                            break;
                        }
                        natural += add;
                        ws.push(w);
                        j += 1;
                    }
                    if y + blh > BOTTOM {
                        new_page(&mut pages, &mut cur, &mut y, (bi, i));
                    }
                    let last = j >= words.len();
                    let gaps = (j - i).saturating_sub(1) as f32;
                    // Justifica, menos a última linha do parágrafo e os títulos.
                    let extra = if !last && !head && gaps > 0.0 && natural > avail * 0.7 { (avail - natural) / gaps } else { 0.0 };
                    let mut x = SIDE as f32 + if first_line { indent } else { 0.0 };
                    if head {
                        x = SIDE as f32;
                    }
                    let mut pieces = Vec::with_capacity(j - i);
                    for (k, idx) in (i..j).enumerate() {
                        pieces.push(Piece { x, text: words[idx].0.clone(), style: words[idx].1 });
                        x += ws[k] + space + extra;
                    }
                    y += blh;
                    cur.lines.push(Line { baseline: y - (blh - sz as i32) / 2 - 4, size: sz, pieces, rule: false });
                    i = j;
                    first_line = false;
                }
                y += if head { lh / 2 } else { (size * 0.35) as i32 };
                after_head = head;
            }
        }
    }
    if !cur.lines.is_empty() {
        pages.push(cur);
    }
    if pages.is_empty() {
        pages.push(Page { lines: vec![], start: (0, 0) });
    }
    pages
}

/// Página que contém a posição (bloco, palavra).
pub fn page_of(pages: &[Page], pos: (usize, usize)) -> usize {
    pages.iter().rposition(|p| p.start <= pos).unwrap_or(0)
}

// ---------- Estado de leitura ----------

pub struct Reader {
    pub book: Book,
    pub size_k: usize,
    pub chapter: usize,
    pub blocks: Vec<Block>,
    pub pages: Vec<Page>,
    pub page: usize,
    pub menu: bool,
    pub turns: u32,
    pub chapter_title: String,
}

fn pos_file(path: &Path) -> PathBuf {
    let d = crate::home_dir().join("data/leitura");
    let _ = std::fs::create_dir_all(&d);
    let name: String = path.file_name().and_then(|n| n.to_str()).unwrap_or("livro").chars().map(|c| if c.is_alphanumeric() || c == '.' { c } else { '_' }).collect();
    d.join(format!("{name}.pos"))
}

/// Progresso (0..1) guardado pelo leitor próprio, se o livro já foi aberto nele.
pub fn progress_of(path: &Path) -> Option<f32> {
    let t = std::fs::read_to_string(pos_file(path)).ok()?;
    t.split_whitespace().nth(4)?.parse().ok()
}

impl Reader {
    pub fn open(path: &Path, fonts: &RFonts) -> Result<Reader, String> {
        let book = Book::open(path)?;
        let saved = std::fs::read_to_string(pos_file(path)).unwrap_or_default();
        let n: Vec<usize> = saved.split_whitespace().take(4).filter_map(|x| x.parse().ok()).collect();
        let (chapter, block, word, size_k) = (n.first().copied().unwrap_or(0), n.get(1).copied().unwrap_or(0), n.get(2).copied().unwrap_or(0), n.get(3).copied().unwrap_or(2));
        let mut r = Reader { book, size_k: size_k.min(SIZES.len() - 1), chapter: 0, blocks: vec![], pages: vec![], page: 0, menu: false, turns: 0, chapter_title: String::new() };
        r.load_chapter(chapter.min(r.book.chapters() - 1), fonts);
        r.page = page_of(&r.pages, (block, word));
        Ok(r)
    }

    pub fn size(&self) -> f32 {
        SIZES[self.size_k]
    }

    fn load_chapter(&mut self, k: usize, fonts: &RFonts) {
        self.chapter = k;
        self.blocks = self.book.chapter(k);
        // Capítulos vazios (capa, separadores) não travam a leitura.
        self.pages = paginate(fonts, &self.blocks, self.size());
        self.chapter_title = self.book.chapter_title(k, &self.blocks);
        self.page = 0;
    }

    /// Avança uma página (ou capítulo). Devolve false no fim do livro.
    pub fn next(&mut self, fonts: &RFonts) -> bool {
        if self.page + 1 < self.pages.len() {
            self.page += 1;
        } else if self.chapter + 1 < self.book.chapters() {
            self.load_chapter(self.chapter + 1, fonts);
        } else {
            return false;
        }
        self.turns += 1;
        self.save();
        true
    }

    pub fn prev(&mut self, fonts: &RFonts) -> bool {
        if self.page > 0 {
            self.page -= 1;
        } else if self.chapter > 0 {
            self.load_chapter(self.chapter - 1, fonts);
            self.page = self.pages.len() - 1;
        } else {
            return false;
        }
        self.turns += 1;
        self.save();
        true
    }

    pub fn go_chapter(&mut self, k: usize, fonts: &RFonts) {
        self.load_chapter(k.min(self.book.chapters() - 1), fonts);
        self.save();
    }

    /// Muda o tamanho da letra mantendo o ponto de leitura.
    pub fn set_size(&mut self, k: usize, fonts: &RFonts) {
        let pos = self.pages.get(self.page).map(|p| p.start).unwrap_or((0, 0));
        self.size_k = k.min(SIZES.len() - 1);
        self.pages = paginate(fonts, &self.blocks, self.size());
        self.page = page_of(&self.pages, pos);
        self.save();
    }

    /// Fração lida do livro (aproximada por capítulos e páginas).
    pub fn progress(&self) -> f32 {
        let n = self.book.chapters().max(1) as f32;
        (self.chapter as f32 + (self.page as f32 + 1.0) / self.pages.len().max(1) as f32) / n
    }

    pub fn save(&self) {
        let pos = self.pages.get(self.page).map(|p| p.start).unwrap_or((0, 0));
        let _ = std::fs::write(pos_file(&self.book.path), format!("{} {} {} {} {:.4}\n", self.chapter, pos.0, pos.1, self.size_k, self.progress()));
    }

    /// Texto da página atual (para o Claude entender a pergunta).
    pub fn page_text(&self) -> String {
        self.pages
            .get(self.page)
            .map(|p| p.lines.iter().map(|l| l.pieces.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n"))
            .unwrap_or_default()
    }
}

// ---------- Desenho ----------

pub fn draw(fb: &mut Framebuffer, f: &crate::text::Fonts, rf: &RFonts, r: &Reader) {
    use crate::text::Face;
    fb.fill(fb.screen(), PAPER);
    // Cabeçalho discreto: título do capítulo.
    let head = crate::apps::fit(f, Face::Body, 24.0, &r.chapter_title, W - 2 * SIDE);
    f.draw(fb, Face::Body, 24.0, SIDE, 56, MUTED, &head);
    if let Some(p) = r.pages.get(r.page) {
        for l in &p.lines {
            if l.rule {
                fb.fill(Rect::new(W / 2 - 60, l.baseline, 120, 2), MUTED);
                continue;
            }
            for pc in &l.pieces {
                rf.draw(fb, pc.style, l.size, pc.x, l.baseline, INK, &pc.text);
            }
        }
    }
    // Rodapé: página do capítulo e porcentagem do livro.
    let foot = format!("{} de {}", r.page + 1, r.pages.len());
    f.draw(fb, Face::Body, 24.0, SIDE, 1420, MUTED, &foot);
    let pct = format!("{}%", (r.progress() * 100.0).round() as i32);
    let w = f.measure(Face::Body, 24.0, &pct);
    f.draw(fb, Face::Body, 24.0, W - SIDE - w, 1420, MUTED, &pct);
    let bar = Rect::new(SIDE + 120, 1412, W - 2 * SIDE - 240, 4);
    fb.fill(bar, 0xDD);
    fb.fill(Rect::new(bar.x, bar.y, (bar.w as f32 * r.progress()) as i32, bar.h), MUTED);
    if r.menu {
        draw_menu(fb, f, r);
    }
}

pub fn menu_top() -> Rect {
    Rect::new(0, 0, W, 300)
}
pub fn menu_bottom() -> Rect {
    Rect::new(0, 1180, W, 268)
}
/// Botões do menu: 0 biblioteca, 1 A-, 2 A+, 3 sumário, 4 Claude, 5 capítulo anterior, 6 próximo.
pub fn menu_btn(k: usize) -> Rect {
    match k {
        0 => Rect::new(40, 60, 260, 84),
        1 => Rect::new(W - 40 - 2 * 110 - 16, 60, 110, 84),
        2 => Rect::new(W - 40 - 110, 60, 110, 84),
        3 => Rect::new(40, 180, 300, 84),
        4 => Rect::new(W - 40 - 400, 180, 400, 84),
        5 => Rect::new(40, 1330, 300, 88),
        _ => Rect::new(W - 40 - 300, 1330, 300, 88),
    }
}

fn draw_menu(fb: &mut Framebuffer, f: &crate::text::Fonts, r: &Reader) {
    use crate::text::Face;
    let pill = |fb: &mut Framebuffer, rect: Rect, label: &str, filled: bool| {
        if filled {
            fb.round_rect(rect, rect.h / 2, INK);
        } else {
            fb.round_outline(rect, rect.h / 2, 4, INK, PAPER);
        }
        let w = f.measure(Face::Strong, 32.0, label);
        f.draw(fb, Face::Strong, 32.0, rect.x + (rect.w - w) / 2, rect.y + rect.h / 2 + 12, if filled { PAPER } else { INK }, label);
    };
    for area in [menu_top(), menu_bottom()] {
        fb.fill(area, PAPER);
    }
    fb.fill(Rect::new(0, 296, W, 4), INK);
    fb.fill(Rect::new(0, 1180, W, 4), INK);
    pill(fb, menu_btn(0), "‹ Biblioteca", false);
    pill(fb, menu_btn(1), "A−", false);
    pill(fb, menu_btn(2), "A+", false);
    pill(fb, menu_btn(3), "Sumário", false);
    pill(fb, menu_btn(4), "Perguntar ao Claude", true);
    let title = crate::apps::fit(f, Face::Display, 40.0, &r.book.title, W - 2 * 40 - 290);
    let tw = f.measure(Face::Display, 40.0, &title);
    let _ = tw;
    f.draw(fb, Face::Display, 40.0, 330, 116, INK, &title);
    let info = format!("Capítulo {} de {} · letra {}", r.chapter + 1, r.book.chapters(), r.size_k + 1);
    f.draw(fb, Face::Body, 28.0, 40, 1240, INK, &crate::apps::fit(f, Face::Body, 28.0, &r.chapter_title, W - 80));
    f.draw(fb, Face::Body, 26.0, 40, 1286, MUTED, &info);
    if r.chapter > 0 {
        pill(fb, menu_btn(5), "‹ Capítulo", false);
    }
    if r.chapter + 1 < r.book.chapters() {
        pill(fb, menu_btn(6), "Capítulo ›", false);
    }
}

/// Sumário: uma linha por capítulo, paginado.
pub const TOC_ROWS: usize = 9;
pub fn toc_row(i: usize) -> Rect {
    Rect::new(SIDE - 16, 300 + i as i32 * 110, W - 2 * SIDE + 32, 104)
}

pub fn draw_toc(fb: &mut Framebuffer, f: &crate::text::Fonts, r: &Reader, page: usize) {
    use crate::text::Face;
    fb.fill(fb.screen(), PAPER);
    crate::ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 72.0, SIDE - 4, 240, INK, "Sumário");
    for i in 0..TOC_ROWS {
        let k = page * TOC_ROWS + i;
        if k >= r.book.chapters() {
            break;
        }
        let row = toc_row(i);
        fb.fill(Rect::new(SIDE, row.y, W - 2 * SIDE, 2), 0xBB);
        let t = r.book.titles.get(k).cloned().flatten().unwrap_or_else(|| format!("Parte {}", k + 1));
        let cur = k == r.chapter;
        if cur {
            fb.round_rect(Rect::new(SIDE - 16, row.y + 20, 8, 64), 4, INK);
        }
        f.draw(fb, if cur { Face::Strong } else { Face::Body }, 34.0, SIDE + 4, row.y + 66, INK, &crate::apps::fit(f, Face::Body, 34.0, &t, W - 2 * SIDE - 10));
    }
    let pages = r.book.chapters().div_ceil(TOC_ROWS);
    if pages > 1 {
        let t = format!("{} de {} · toque nas laterais de baixo para passar", page + 1, pages);
        let w = f.measure(Face::Body, 26.0, &t);
        f.draw(fb, Face::Body, 26.0, (W - w) / 2, 1360, MUTED, &t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xhtml_vira_paragrafos_com_estilo() {
        let b = parse_xhtml("<html><head><title>x</title><style>p{}</style></head><body><h1>Capítulo 1</h1><p>Era uma <em>vez</em> um&nbsp;livro &amp; tal.</p><p>Outro<br/>parágrafo</p><hr/></body></html>");
        assert_eq!(b[0], Block::Head(vec![Run { text: "Capítulo 1".into(), style: Style::Regular }]));
        match &b[1] {
            Block::Para(r) => {
                assert_eq!(r.len(), 3);
                assert_eq!(r[1].style, Style::Italic);
                assert!(r[2].text.contains("livro & tal."));
            }
            _ => panic!(),
        }
        assert!(matches!(b.last(), Some(Block::Rule)));
    }

    #[test]
    fn entidades() {
        assert_eq!(decode_entities("a&#233;&#xE7;&ccedil;&amp;"), "aéçç&");
    }

    #[test]
    fn palmdoc_e_dados_extras() {
        // literal "ab", par (distância 2, tamanho 3) e "espaço + letra".
        let pair: u16 = 0x8000 | (2 << 3) | (3 - 3);
        let mut src = vec![b'a', b'b'];
        src.extend_from_slice(&pair.to_be_bytes());
        src.push(b'x' ^ 0x80 | 0x80);
        assert_eq!(palmdoc(&src), b"ababa x");
        // um campo extra de 3 bytes (bit 1) com o tamanho no fim.
        let rec = [b'h', b'i', 0, 0, 0x83];
        assert_eq!(trailing_size(&rec, 2), 3);
    }

    #[test]
    fn caminho_relativo() {
        assert_eq!(join("OEBPS/text", "../images/a%20b.png#x"), "OEBPS/images/a b.png");
    }
}

#[cfg(test)]
mod render {
    use super::*;
    /// `VERSO_INK_EPUB=livro.epub VERSO_INK_SHOTS=pasta cargo test leitor_em_imagem -- --ignored`
    #[test]
    #[ignore]
    fn leitor_em_imagem() {
        let out = PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let path = PathBuf::from(std::env::var("VERSO_INK_EPUB").expect("VERSO_INK_EPUB"));
        let f = crate::text::Fonts::new();
        let rf = RFonts::load();
        let book = Book::open(&path).expect("abre");
        eprintln!("{}: {} capítulos", book.title, book.chapters());
        let mut r = Reader { book, size_k: 2, chapter: 0, blocks: vec![], pages: vec![], page: 0, menu: false, turns: 0, chapter_title: String::new() };
        r.load_chapter(1.min(r.book.chapters() - 1), &rf);
        let mut fb = Framebuffer::memory();
        draw(&mut fb, &f, &rf, &r);
        fb.save_pgm(&out.join("leitor-pagina.pgm"));
        r.menu = true;
        let mut fb = Framebuffer::memory();
        draw(&mut fb, &f, &rf, &r);
        fb.save_pgm(&out.join("leitor-menu.pgm"));
        let mut fb = Framebuffer::memory();
        draw_toc(&mut fb, &f, &r, 0);
        fb.save_pgm(&out.join("leitor-sumario.pgm"));
    }
}
