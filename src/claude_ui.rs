//! Bloco Claude: as sessões do Claude Code do PC, ao vivo (via tools/claude-hook.sh).

use crate::apps::fit;
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::time::{SystemTime, UNIX_EPOCH};

const MARGIN: i32 = 72;
const W: i32 = 1072;

pub struct Session {
    pub when: i64,
    pub project: String,
    pub state: String,
    pub detail: String,
}

fn dir() -> std::path::PathBuf {
    crate::home_dir().join("data/claude")
}

/// Sessões com atividade nas últimas 12 horas, mais recentes primeiro.
pub fn load() -> Vec<Session> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let mut v: Vec<Session> = std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let s = std::fs::read_to_string(e.path()).ok()?;
            let mut it = s.trim().splitn(4, '|');
            let when: i64 = it.next()?.parse().ok()?;
            Some(Session { when, project: it.next()?.to_string(), state: it.next()?.to_string(), detail: it.next().unwrap_or("").to_string() })
        })
        .filter(|s| now - s.when < 12 * 3600 && s.state != "encerrada")
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.when));
    v
}

/// Momento da última mudança na pasta (para atualizar a tela sozinha).
pub fn stamp() -> u64 {
    std::fs::read_dir(dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .filter_map(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .max()
        .unwrap_or(0)
}

fn ago(secs: i64) -> String {
    match secs {
        s if s < 60 => "agora".into(),
        s if s < 3600 => format!("há {} min", s / 60),
        s => format!("há {} h", s / 3600),
    }
}

pub fn draw(fb: &mut Framebuffer, f: &Fonts, sessions: &[Session]) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 116.0, MARGIN - 6, 330, INK, "Agentes no PC");
    f.draw(fb, Face::Strong, 30.0, MARGIN, 400, MUTED, "Ao vivo · Claude Code");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    if sessions.is_empty() {
        let lines = [
            "Nenhuma sessão nas últimas horas.",
            "Quando o Claude Code trabalhar no PC, cada",
            "sessão aparece aqui: o que está fazendo e",
            "quando é a sua vez.",
        ];
        for (k, l) in lines.iter().enumerate() {
            f.draw(fb, if k == 0 { Face::Display } else { Face::Body }, if k == 0 { 48.0 } else { 38.0 }, MARGIN, 520 + k as i32 * 64, INK, l);
        }
        return;
    }
    for (i, s) in sessions.iter().take(5).enumerate() {
        let y = 450 + i as i32 * 190;
        fb.fill(Rect::new(MARGIN, y, W - 2 * MARGIN, 3), INK);
        // Quem espera por você aparece em destaque.
        let waiting = s.state == "sua vez" || s.state == "precisa de você";
        let tag = s.state.to_uppercase();
        let tw = f.measure(Face::Strong, 26.0, &tag);
        let tr = Rect::new(W - MARGIN - tw - 40, y + 30, tw + 40, 56);
        if waiting {
            fb.round_rect(tr, 28, INK);
        } else {
            fb.round_outline(tr, 28, 3, INK, PAPER);
        }
        f.draw(fb, Face::Strong, 26.0, tr.x + 20, tr.y + 38, if waiting { PAPER } else { INK }, &tag);
        f.draw(fb, Face::Display, 56.0, MARGIN, y + 80, INK, &fit(f, Face::Display, 56.0, &s.project, tr.x - MARGIN - 24));
        let detail = if s.detail.is_empty() { ago(now - s.when) } else { format!("{} · {}", s.detail, ago(now - s.when)) };
        f.draw(fb, Face::Body, 32.0, MARGIN, y + 140, MUTED, &fit(f, Face::Body, 32.0, &detail, W - 2 * MARGIN));
    }
}
