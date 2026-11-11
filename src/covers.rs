//! Capas dos livros: extraídas de EPUB (OPF) e de AZW3/MOBI (registro EXTH 201),
//! reduzidas para miniatura em cinza e guardadas em cache (/mnt/us/verso-ink/cache/covers).

use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct Thumb {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

fn cache_path(book: &Path, w: u32, h: u32) -> Option<PathBuf> {
    let meta = std::fs::metadata(book).ok()?;
    let mut hs = std::collections::hash_map::DefaultHasher::new();
    (book, meta.len(), meta.modified().ok(), w, h).hash(&mut hs);
    let dir = crate::home_dir().join("cache/covers");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join(format!("{:016x}.gray", hs.finish())))
}

/// Título guardado no cache (lido de dentro do livro na primeira vez).
pub fn cached_title(book: &Path) -> Option<String> {
    let p = cache_path(book, 0, 0)?.with_extension("title");
    std::fs::read_to_string(p).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn save_title(book: &Path, title: &str) {
    if let Some(p) = cache_path(book, 0, 0) {
        let _ = std::fs::write(p.with_extension("title"), title);
    }
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

/// Miniatura já pronta no cache (rápido; usado ao desenhar a lista).
pub fn cached(book: &Path, w: u32, h: u32) -> Option<Option<Thumb>> {
    let data = std::fs::read(cache_path(book, w, h)?).ok()?;
    if data.is_empty() {
        return Some(None); // já tentamos: o livro não tem capa
    }
    (data.len() == (w * h) as usize).then(|| Some(Thumb { w, h, px: data }))
}

/// Gera a miniatura (lento: abre o livro e decodifica a imagem) e grava no cache.
pub fn generate(book: &Path, w: u32, h: u32) -> Option<Thumb> {
    let ext = book.extension()?.to_str()?.to_ascii_lowercase();
    let bytes = match ext.as_str() {
        "epub" => epub_cover(book),
        "azw3" | "mobi" | "azw" => mobi_cover(book),
        "cbz" => cbz_cover(book),
        _ => None,
    };
    let thumb = bytes.and_then(|b| to_thumb(&b, w, h));
    if let Some(p) = cache_path(book, w, h) {
        let _ = std::fs::write(p, thumb.as_ref().map(|t| t.px.clone()).unwrap_or_default());
    }
    thumb
}

/// Recorta ao centro para a proporção da caixa e reduz.
fn to_thumb(bytes: &[u8], w: u32, h: u32) -> Option<Thumb> {
    let img = image::load_from_memory(bytes).ok()?.to_luma8();
    let (iw, ih) = img.dimensions();
    if iw < 8 || ih < 8 {
        return None;
    }
    let target = w as f32 / h as f32;
    let (cw, ch) = if iw as f32 / ih as f32 > target {
        ((ih as f32 * target) as u32, ih)
    } else {
        (iw, (iw as f32 / target) as u32)
    };
    let crop = image::imageops::crop_imm(&img, (iw - cw) / 2, (ih - ch) / 2, cw, ch).to_image();
    let small = image::imageops::resize(&crop, w, h, image::imageops::FilterType::Triangle);
    Some(Thumb { w, h, px: small.into_raw() })
}

// ---------- EPUB ----------

/// Atributos de todas as tags com esse nome (parser mínimo, suficiente para container.xml e OPF).
fn tags<'a>(xml: &'a str, name: &str) -> Vec<Vec<(String, String)>> {
    let mut out = Vec::new();
    let open = format!("<{name}");
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        let end = after.find('>').unwrap_or(after.len());
        let body = &after[..end];
        if body.starts_with([' ', '\n', '\t', '\r', '/']) {
            let mut attrs = Vec::new();
            let mut s = body;
            while let Some(eq) = s.find('=') {
                let key = s[..eq].split_whitespace().last().unwrap_or("").to_string();
                let v = s[eq + 1..].trim_start();
                let Some(q) = v.chars().next().filter(|c| *c == '"' || *c == '\'') else { break };
                let Some(close) = v[1..].find(q) else { break };
                attrs.push((key, v[1..1 + close].to_string()));
                s = &v[close + 2..];
            }
            out.push(attrs);
        }
        rest = &after[end.min(after.len())..];
    }
    out
}

fn attr<'a>(a: &'a [(String, String)], k: &str) -> Option<&'a str> {
    a.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str())
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<Vec<u8>> {
    let mut e = zip.by_name(name).ok()?;
    let mut buf = Vec::new();
    e.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
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
    String::from_utf8_lossy(&out).into_owned()
}

fn epub_cover(path: &Path) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let container = String::from_utf8(read_entry(&mut zip, "META-INF/container.xml")?).ok()?;
    let opf_path = tags(&container, "rootfile").iter().find_map(|a| attr(a, "full-path").map(str::to_string))?;
    let opf = String::from_utf8_lossy(&read_entry(&mut zip, &opf_path)?).into_owned();
    if let Some(i) = opf.find("<dc:title") {
        let rest = &opf[i..];
        if let (Some(a), Some(b)) = (rest.find('>'), rest.find("</dc:title>")) {
            if a < b {
                save_title(path, xml_unescape(rest[a + 1..b].trim()).as_str());
            }
        }
    }
    let base = opf_path.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
    let items = tags(&opf, "item");
    let is_image = |a: &Vec<(String, String)>| attr(a, "media-type").is_some_and(|m| m.starts_with("image/"));
    // 1) EPUB 3: properties="cover-image"; 2) EPUB 2: <meta name="cover" content="id">; 3) nome com "cover".
    let by_prop = items.iter().find(|a| attr(a, "properties").is_some_and(|p| p.split_whitespace().any(|x| x == "cover-image")));
    let cover_id = tags(&opf, "meta").iter().find(|a| attr(a, "name") == Some("cover")).and_then(|a| attr(a, "content").map(str::to_string));
    let by_meta = cover_id.and_then(|id| items.iter().find(|a| attr(a, "id") == Some(id.as_str()) && is_image(a)));
    let by_name = items.iter().find(|a| is_image(a) && (attr(a, "id").unwrap_or("") .to_lowercase().contains("cover") || attr(a, "href").unwrap_or("").to_lowercase().contains("cover")));
    let item = by_prop.or(by_meta).or(by_name)?;
    let href = percent_decode(attr(item, "href")?);
    read_entry(&mut zip, &format!("{base}{href}")).or_else(|| read_entry(&mut zip, &href))
}

fn cbz_cover(path: &Path) -> Option<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().and_then(|e| e.name().ok().map(|n| n.to_string())))
        .filter(|n| {
            let l = n.to_lowercase();
            l.ends_with(".jpg") || l.ends_with(".jpeg") || l.ends_with(".png")
        })
        .collect();
    names.sort();
    read_entry(&mut zip, names.first()?)
}

// ---------- AZW3 / MOBI ----------

fn be32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn mobi_cover(path: &Path) -> Option<Vec<u8>> {
    let data = std::fs::read(path).ok()?;
    let nrec = u16::from_be_bytes(data.get(76..78)?.try_into().ok()?) as usize;
    let rec = |i: usize| -> Option<&[u8]> {
        let start = be32(&data, 78 + i * 8)? as usize;
        let end = if i + 1 < nrec { be32(&data, 78 + (i + 1) * 8)? as usize } else { data.len() };
        data.get(start..end)
    };
    let r0 = rec(0)?;
    if r0.get(16..20)? != b"MOBI" {
        return None;
    }
    let mobi_len = be32(r0, 20)? as usize;
    let first_image = be32(r0, 108)? as usize;
    let has_exth = be32(r0, 128)? & 0x40 != 0;
    if !has_exth {
        return None;
    }
    let ex = 16 + mobi_len;
    if r0.get(ex..ex + 4)? != b"EXTH" {
        return None;
    }
    let count = be32(r0, ex + 8)? as usize;
    let mut o = ex + 12;
    let mut cover = None;
    for _ in 0..count {
        let (typ, len) = (be32(r0, o)?, be32(r0, o + 4)? as usize);
        if typ == 201 {
            cover = be32(r0, o + 8);
        }
        // 503 = título atualizado do livro.
        if typ == 503 && len > 8 {
            if let Some(t) = r0.get(o + 8..o + len) {
                save_title(path, String::from_utf8_lossy(t).trim());
            }
        }
        o += len.max(8);
    }
    let img = rec(first_image + cover? as usize)?;
    Some(img.to_vec())
}
