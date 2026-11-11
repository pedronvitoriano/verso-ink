//! Telas "Hoje" (prioridades, tarefas, hábitos, lembretes) e "Lembrete".

use crate::apps::fit;
use crate::day::{self, Item, Kind};
use crate::fb::{Framebuffer, Rect};
use crate::text::{Face, Fonts};
use crate::ui::{self, Status, INK, MUTED, PAPER};
use std::collections::HashSet;

const MARGIN: i32 = 72;
const W: i32 = 1072;
const BOTTOM: i32 = 1270;

pub fn pomodoro_rect() -> Rect {
    Rect::new(MARGIN, 1300, W - 2 * MARGIN, 110)
}

fn checkbox(fb: &mut Framebuffer, x: i32, y: i32, s: i32, on: bool) {
    let r = Rect::new(x, y, s, s);
    if on {
        fb.round_rect(r, s / 5, INK);
        let (x, y, s) = (x as f32, y as f32, s as f32);
        ui::stroke(fb, (x + s * 0.24, y + s * 0.52), (x + s * 0.43, y + s * 0.72), s * 0.12, PAPER);
        ui::stroke(fb, (x + s * 0.43, y + s * 0.72), (x + s * 0.78, y + s * 0.3), s * 0.12, PAPER);
    } else {
        fb.round_outline(r, s / 5, 4, INK, PAPER);
    }
}

fn section(fb: &mut Framebuffer, f: &Fonts, y: i32, label: &str) {
    f.draw(fb, Face::Strong, 28.0, MARGIN, y + 34, MUTED, label);
}

/// Desenha a tela e devolve os alvos de toque: (índice do item, retângulo).
pub fn draw_today(fb: &mut Framebuffer, f: &Fonts, items: &[Item], done: &HashSet<String>, st: &Status, focus_today: u32) -> Vec<(usize, Rect)> {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 150.0, MARGIN - 6, 330, INK, "Hoje");
    let streak = day::todo_streak(items, st.offset_min);
    if streak > 0 {
        let label = if streak == 1 { "1 dia completo".to_string() } else { format!("{streak} dias seguidos") };
        let tw = f.measure(Face::Strong, 30.0, &label);
        let r = Rect::new(W - MARGIN - tw - 56, 80, tw + 56, 72);
        fb.round_rect(r, 36, INK);
        f.draw(fb, Face::Strong, 30.0, r.x + 28, r.y + 47, PAPER, &label);
    }
    let mut hits = Vec::new();
    if items.is_empty() {
        let lines = [
            "Seu dia ainda está vazio.",
            "Escreva prioridades, tarefas, lembretes e",
            "hábitos pelo PC (tools/dia.sh) ou peça ao",
            "Claude, e eles aparecem aqui.",
        ];
        for (k, l) in lines.iter().enumerate() {
            f.draw(fb, if k == 0 { Face::Display } else { Face::Body }, if k == 0 { 52.0 } else { 38.0 }, MARGIN, 470 + k as i32 * 66, INK, l);
        }
    }
    let mut y = 380;
    let mut hidden = 0;
    let group = |k: Kind| items.iter().enumerate().filter(move |(_, i)| i.kind == k);

    // Prioridades: grandes.
    if group(Kind::Priority).next().is_some() {
        section(fb, f, y, "PRIORIDADES");
        y += 50;
        for (idx, it) in group(Kind::Priority).take(3) {
            let on = done.contains(&it.key());
            checkbox(fb, MARGIN, y + 14, 52, on);
            let t = fit(f, Face::Display, 44.0, &it.text, W - 2 * MARGIN - 80);
            f.draw(fb, Face::Display, 44.0, MARGIN + 80, y + 58, if on { MUTED } else { INK }, &t);
            hits.push((idx, Rect::new(MARGIN - 16, y, W - 2 * MARGIN + 32, 80)));
            y += 84;
        }
        y += 14;
    }
    // Tarefas.
    if group(Kind::Task).next().is_some() {
        section(fb, f, y, "TAREFAS");
        y += 46;
        for (idx, it) in group(Kind::Task) {
            if y + 70 > BOTTOM - 200 {
                hidden += 1;
                continue;
            }
            let on = done.contains(&it.key());
            checkbox(fb, MARGIN, y + 13, 42, on);
            let t = fit(f, Face::Body, 38.0, &it.text, W - 2 * MARGIN - 70);
            f.draw(fb, Face::Body, 38.0, MARGIN + 66, y + 49, if on { MUTED } else { INK }, &t);
            hits.push((idx, Rect::new(MARGIN - 16, y, W - 2 * MARGIN + 32, 68)));
            y += 70;
        }
        if hidden > 0 {
            f.draw(fb, Face::Body, 30.0, MARGIN + 66, y + 34, MUTED, &format!("e mais {hidden}"));
            y += 44;
        }
        y += 14;
    }
    // Hábitos: fichas com a sequência de cada um.
    if group(Kind::Habit).next().is_some() && y < BOTTOM - 120 {
        section(fb, f, y, "HÁBITOS");
        y += 50;
        let mut x = MARGIN;
        for (idx, it) in group(Kind::Habit) {
            let on = done.contains(&it.key());
            let n = day::habit_streak(&it.key(), st.offset_min);
            let label = if n > 0 { format!("{} · {n}", it.text) } else { it.text.clone() };
            let tw = f.measure(Face::Strong, 30.0, &label).min(W - 2 * MARGIN - 48);
            let wdt = tw + 48;
            if x + wdt > W - MARGIN {
                x = MARGIN;
                y += 84;
            }
            if y + 72 > BOTTOM {
                break;
            }
            let r = Rect::new(x, y, wdt, 72);
            if on {
                fb.round_rect(r, 36, INK);
            } else {
                fb.round_outline(r, 36, 4, INK, PAPER);
            }
            let label = fit(f, Face::Strong, 30.0, &label, tw);
            f.draw(fb, Face::Strong, 30.0, r.x + 24, r.y + 47, if on { PAPER } else { INK }, &label);
            hits.push((idx, r));
            x += wdt + 16;
        }
        y += 96;
    }
    // Lembretes do dia.
    if group(Kind::Reminder).next().is_some() && y < BOTTOM - 60 {
        section(fb, f, y, "LEMBRETES");
        y += 46;
        let mut rem: Vec<_> = group(Kind::Reminder).collect();
        rem.sort_by_key(|(_, i)| i.at);
        for (idx, it) in rem {
            if y + 56 > BOTTOM {
                break;
            }
            let on = done.contains(&it.key());
            let ink = if on { MUTED } else { INK };
            let end = f.draw(fb, Face::Display, 38.0, MARGIN, y + 44, ink, &it.time_label());
            let t = fit(f, Face::Body, 36.0, &it.text, W - MARGIN - end - 24);
            f.draw(fb, Face::Body, 36.0, end + 24, y + 44, ink, &t);
            hits.push((idx, Rect::new(MARGIN - 16, y, W - 2 * MARGIN + 32, 58)));
            y += 60;
        }
    }
    // Pomodoro sempre à mão.
    let r = pomodoro_rect();
    fb.round_rect(r, r.h / 2, INK);
    let label = match focus_today {
        0 => "Começar um foco de 25 min".to_string(),
        n => format!("Foco de 25 min · {n} hoje"),
    };
    let tw = f.measure(Face::Strong, 40.0, &label);
    f.draw(fb, Face::Strong, 40.0, r.x + (r.w - tw) / 2, r.y + 70, PAPER, &label);
    hits
}

pub fn reminder_ok_rect() -> Rect {
    Rect::new(MARGIN, 1150, W - 2 * MARGIN, 120)
}

pub fn draw_reminder(fb: &mut Framebuffer, f: &Fonts, it: &Item) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    // Sino: cúpula, base e badalo.
    let (cx, top) = (MARGIN as f32 + 70.0, 300.0);
    ui::bell(fb, cx, top);
    f.draw(fb, Face::Strong, 34.0, MARGIN, 560, MUTED, &format!("LEMBRETE · {}", it.time_label()));
    let mut y = 680;
    let mut line = String::new();
    for word in it.text.split_whitespace() {
        let cand = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if f.measure(Face::Display, 96.0, &cand) > W - 2 * MARGIN && !line.is_empty() {
            f.draw(fb, Face::Display, 96.0, MARGIN - 4, y, INK, &line);
            y += 110;
            line = word.to_string();
        } else {
            line = cand;
        }
        if y > 1000 {
            break;
        }
    }
    if !line.is_empty() && y <= 1000 {
        f.draw(fb, Face::Display, 96.0, MARGIN - 4, y, INK, &fit(f, Face::Display, 96.0, &line, W - 2 * MARGIN));
    }
    let r = reminder_ok_rect();
    fb.round_rect(r, r.h / 2, INK);
    let tw = f.measure(Face::Strong, 44.0, "Feito");
    f.draw(fb, Face::Strong, 44.0, r.x + (r.w - tw) / 2, r.y + 76, PAPER, "Feito");
}
