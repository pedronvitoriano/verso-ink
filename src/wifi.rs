//! Redes Wi-Fi pelo wifid do Kindle (lipc): lista, conecta em conhecidas, cria perfil com senha.

use crate::apps::fit;
use crate::fb::{Framebuffer, Rect};
use crate::sys::{log, sh};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MARGIN: i32 = 72;
const W: i32 = 1072;

#[derive(Clone, Debug, PartialEq)]
pub struct Net {
    pub essid: String,
    pub secured: bool,
    pub known: bool,
    pub signal: i32,
}

/// Lê a saída do `lipc-hash-prop` (blocos "Hash Index", linhas `chave = "valor"` ou `(n)`).
pub fn parse_hashes(text: &str) -> Vec<Vec<(String, String)>> {
    let mut out: Vec<Vec<(String, String)>> = Vec::new();
    for line in text.lines() {
        if line.starts_with("Hash Index") {
            out.push(Vec::new());
            continue;
        }
        if let (Some(cur), Some((k, v))) = (out.last_mut(), line.split_once('=')) {
            let v = v.trim().trim_matches('"').trim_start_matches('(').trim_end_matches(')').to_string();
            cur.push((k.trim().to_string(), v));
        }
    }
    out
}

fn hash_prop(prop: &str) -> String {
    sh("lipc-hash-prop", &["-n", "com.lab126.wifid", prop]).unwrap_or_default()
}

/// Redes da última varredura, sem repetir nome, mais fortes primeiro.
pub fn networks() -> Vec<Net> {
    let mut v: Vec<Net> = Vec::new();
    for h in parse_hashes(&hash_prop("scanList")) {
        let get = |k: &str| h.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone()).unwrap_or_default();
        let essid = get("essid");
        if essid.is_empty() || get("supported") == "no" || get("enterprise") == "yes" {
            continue;
        }
        let n = Net { secured: get("secured") == "yes", known: get("known") == "yes", signal: get("signal").parse().unwrap_or(0), essid };
        match v.iter_mut().find(|x| x.essid == n.essid) {
            Some(x) if x.signal < n.signal => *x = n,
            Some(_) => {}
            None => v.push(n),
        }
    }
    v.sort_by_key(|n| (std::cmp::Reverse(n.signal), n.essid.clone()));
    v
}

/// Rede atual, pela página de diagnóstico do wifid ("2.3 AP: Nome (bssid)").
pub fn current() -> Option<String> {
    let diag = sh("lipc-get-prop", &["com.lab126.wifid", "711"])?;
    let line = diag.lines().find(|l| l.trim_start().starts_with("2.3 AP:"))?;
    let name = line.split_once("AP:")?.1.trim();
    let name = name.rsplit_once(" (").map(|(a, _)| a).unwrap_or(name).trim();
    (!name.is_empty() && name != "None").then(|| name.to_string())
}

pub fn connected() -> bool {
    sh("lipc-get-prop", &["com.lab126.wifid", "cmState"]).is_some_and(|s| s == "CONNECTED")
}

pub fn enabled() -> bool {
    sh("lipc-get-prop", &["com.lab126.cmd", "wirelessEnable"]).is_some_and(|s| s == "1")
}

pub fn scan() {
    let _ = sh("lipc-set-prop", &["com.lab126.wifid", "scan", ""]);
}

fn quote(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn create_profile(essid: &str, psk: Option<&str>) -> bool {
    let hash = match psk {
        Some(p) => format!("{{ essid = \"{}\", secured = \"yes\", psk = \"{}\", store_nw_user_pref = (0) }}", quote(essid), quote(p)),
        None => format!("{{ essid = \"{}\", secured = \"no\", store_nw_user_pref = (0) }}", quote(essid)),
    };
    let Ok(mut child) = Command::new("lipc-hash-prop").args(["com.lab126.wifid", "createProfile"]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(hash.as_bytes());
    }
    child.wait().is_ok_and(|s| s.success())
}

/// Conecta e espera até `timeout`. Com senha, cria o perfil antes; se falhar, esquece a rede nova
/// e volta para a anterior. Devolve a rede em que ficou conectado.
pub fn connect(essid: &str, psk: Option<&str>, secured: bool, timeout: Duration) -> Result<String, String> {
    let previous = current();
    if psk.is_some() || !secured {
        if !create_profile(essid, psk) {
            return Err("Não consegui salvar a rede.".into());
        }
    }
    log(&format!("wifi: conectando em {essid}"));
    let _ = sh("lipc-set-prop", &["com.lab126.wifid", "cmConnect", essid]);
    let t0 = Instant::now();
    while t0.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(1000));
        if connected() && current().as_deref() == Some(essid) {
            log(&format!("wifi: conectado em {essid} em {} s", t0.elapsed().as_secs()));
            return Ok(essid.to_string());
        }
    }
    log(&format!("wifi: não conectou em {essid}"));
    if psk.is_some() {
        let _ = sh("lipc-set-prop", &["com.lab126.wifid", "deleteProfile", essid]);
    }
    if let Some(prev) = previous.filter(|p| p != essid) {
        let _ = sh("lipc-set-prop", &["com.lab126.wifid", "cmConnect", &prev]);
        return Err(format!("Não conectou. Senha errada? Voltei para {prev}."));
    }
    Err(format!("Não conectou em {essid}. Confira a senha."))
}

// ---------- Telas ----------

pub fn toggle_rect() -> Rect {
    Rect::new(W - MARGIN - 180, 240, 180, 96)
}
pub fn rescan_rect() -> Rect {
    Rect::new(W - MARGIN - 250, 76, 250, 84)
}
pub fn row_rect(i: usize) -> Rect {
    Rect::new(MARGIN - 16, 520 + i as i32 * 116, W - 2 * MARGIN + 32, 112)
}
pub const ROWS: usize = 6;

/// Páginas da lista de redes (quando há mais que cabe numa tela).
pub fn nav_rect(k: usize) -> Rect {
    Rect::new(if k == 0 { MARGIN } else { W - MARGIN - 330 }, 1250, 330, 90)
}

fn bars(fb: &mut Framebuffer, x: i32, y: i32, signal: i32) {
    for k in 0..5 {
        let h = 12 + k * 9;
        let r = Rect::new(x + k * 14, y + 48 - h, 10, h);
        if k < signal {
            fb.fill(r, INK);
        } else {
            fb.round_outline(r, 2, 2, 0x99, PAPER);
        }
    }
}

pub fn draw(fb: &mut Framebuffer, f: &Fonts, on: bool, cur: Option<&str>, nets: &[Net], note: &str, page: usize) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    let rr = rescan_rect();
    fb.round_outline(rr, rr.h / 2, 4, INK, PAPER);
    let tw = f.measure(Face::Strong, 32.0, "Procurar");
    f.draw(fb, Face::Strong, 32.0, rr.x + (rr.w - tw) / 2, rr.y + 54, INK, "Procurar");
    f.draw(fb, Face::Display, 130.0, MARGIN - 6, 330, INK, "Wi-Fi");
    let sw = toggle_rect();
    if on {
        fb.round_rect(sw, 48, INK);
        fb.round_rect(Rect::new(sw.x + sw.w - 84, sw.y + 12, 72, 72), 36, PAPER);
    } else {
        fb.round_outline(sw, 48, 5, INK, PAPER);
        fb.round_rect(Rect::new(sw.x + 18, sw.y + 18, 60, 60), 30, INK);
    }
    let status = match (on, cur) {
        (false, _) => "Desligado".to_string(),
        (true, Some(c)) => format!("Conectado a {c}"),
        (true, None) => "Sem conexão".to_string(),
    };
    f.draw(fb, Face::Strong, 34.0, MARGIN, 410, INK, &fit(f, Face::Strong, 34.0, &status, W - 2 * MARGIN));
    f.draw(fb, Face::Body, 30.0, MARGIN, 466, MUTED, &fit(f, Face::Body, 30.0, note, W - 2 * MARGIN));
    if !on {
        return;
    }
    for (i, n) in nets.iter().skip(page * ROWS).take(ROWS).enumerate() {
        let r = row_rect(i);
        fb.fill(Rect::new(MARGIN, r.y, W - 2 * MARGIN, 3), INK);
        let here = cur == Some(n.essid.as_str());
        f.draw(fb, Face::Display, 44.0, MARGIN, r.y + 62, INK, &fit(f, Face::Display, 44.0, &n.essid, W - 2 * MARGIN - 120));
        let sub = match (here, n.known, n.secured) {
            (true, _, _) => "Conectada".to_string(),
            (_, true, _) => "Conhecida · toque para conectar".to_string(),
            (_, false, true) => "Protegida · pede senha".to_string(),
            (_, false, false) => "Aberta".to_string(),
        };
        f.draw(fb, Face::Body, 28.0, MARGIN + 2, r.y + 100, if here { INK } else { MUTED }, &sub);
        bars(fb, W - MARGIN - 70, r.y + 30, n.signal);
    }
    let shown = nets.len().saturating_sub(page * ROWS).min(ROWS);
    fb.fill(Rect::new(MARGIN, row_rect(shown).y, W - 2 * MARGIN, 3), INK);
    // Mais redes do que cabe: páginas, com quantas faltam.
    let pages = nets.len().div_ceil(ROWS);
    if page > 0 {
        ui::button(fb, f, nav_rect(0), "‹ Anteriores", false);
    }
    if pages > 1 {
        let t = format!("{} de {pages}", page + 1);
        let w = f.measure(Face::Strong, 30.0, &t);
        f.draw(fb, Face::Strong, 30.0, (W - w) / 2, nav_rect(0).y + 56, MUTED, &t);
    }
    if page + 1 < pages {
        let rest = nets.len() - (page + 1) * ROWS;
        ui::button(fb, f, nav_rect(1), &format!("Mais {rest} rede{} ›", if rest == 1 { "" } else { "s" }), true);
    }
}

/// Tela da senha: o campo e o teclado ficam nas mesmas posições da tela "Adicionar ao dia".
pub fn draw_password_header(fb: &mut Framebuffer, f: &Fonts, essid: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Strong, 32.0, MARGIN, 250, MUTED, "SENHA DA REDE");
    f.draw(fb, Face::Display, 64.0, MARGIN, 340, INK, &fit(f, Face::Display, 64.0, essid, W - 2 * MARGIN));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn le_a_lista() {
        let t = "Hash Index: 0\n\tessid = \"Casa\"\n\tsecured = \"yes\"\n\tknown = \"yes\"\n\tsignal = (5)\n\nHash Index: 1\n\tessid = \"Casa\"\n\tsecured = \"yes\"\n\tknown = \"yes\"\n\tsignal = (2)\n\nHash Index: 2\n\tessid = \"Café\"\n\tsecured = \"no\"\n\tknown = \"no\"\n\tsignal = (3)\n";
        let h = parse_hashes(t);
        assert_eq!(h.len(), 3);
        assert_eq!(h[2][0], ("essid".to_string(), "Café".to_string()));
        assert_eq!(quote("a\"b"), "a\\\"b");
    }
}
