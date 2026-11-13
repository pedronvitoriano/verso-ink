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
    let cal = calendar_rect();
    ui::card(fb, cal, (cal.h / 2).min(40), PAPER, false);
    let cw = f.measure(Face::Strong, 32.0, "Calendário");
    f.draw(fb, Face::Strong, 32.0, cal.x + (cal.w - cw) / 2, cal.y + 54, INK, "Calendário");
    let add = add_rect();
    ui::card(fb, add, (add.h / 2).min(40), PAPER, false);
    let aw = f.measure(Face::Strong, 32.0, "+ Adicionar");
    f.draw(fb, Face::Strong, 32.0, add.x + (add.w - aw) / 2, add.y + 54, INK, "+ Adicionar");
    if streak > 0 {
        let label = if streak == 1 { "1 dia completo".to_string() } else { format!("{streak} dias seguidos") };
        let tw = f.measure(Face::Strong, 30.0, &label);
        let r = Rect::new(W - MARGIN - tw - 56, 236, tw + 56, 72);
        fb.round_rect(r, 36, INK);
        f.draw(fb, Face::Strong, 30.0, r.x + 28, r.y + 47, PAPER, &label);
    }
    let mut hits = Vec::new();
    if items.is_empty() {
        let lines = [
            "Seu dia ainda está vazio.",
            "Escreva prioridades, tarefas, lembretes e",
            "hábitos pelo PC (tools/dia.sh) ou peça ao",
            "Verso, e eles aparecem aqui.",
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
            let n = day::habit_streak(it, st.offset_min);
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
    // Hábitos do Aprender: marcados sozinhos quando você estuda (chama na frente, sem toque).
    let learn = crate::aprender::habit_topics();
    if y < BOTTOM - 120 {
        section(fb, f, y, "APRENDER");
        y += 50;
        let mut x = MARGIN;
        for (name, srcs) in &learn {
            let (n, on) = crate::aprender::habit_streak(srcs);
            let label = if n > 0 { format!("{name} · {n}") } else { name.clone() };
            let tw = f.measure(Face::Strong, 30.0, &label).min(W - 2 * MARGIN - 90);
            let wdt = tw + 90;
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
            fb.round_rect(Rect::new(r.x + 14, r.y + 12, 48, 48), 24, PAPER);
            crate::aprender::flame(fb, Rect::new(r.x + 16, r.y + 14, 44, 44), on);
            f.draw(fb, Face::Strong, 30.0, r.x + 72, r.y + 47, if on { PAPER } else { INK }, &fit(f, Face::Strong, 30.0, &label, tw));
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
    ui::card(fb, r, (r.h / 2).min(40), INK, false);
    let label = match focus_today {
        0 => "Começar um foco de 25 min".to_string(),
        n => format!("Foco de 25 min · {n} hoje"),
    };
    let tw = f.measure(Face::Strong, 40.0, &label);
    f.draw(fb, Face::Strong, 40.0, r.x + (r.w - tw) / 2, r.y + 70, PAPER, &label);
    hits
}

pub fn calendar_rect() -> Rect {
    Rect::new(W - MARGIN - 270 - 20 - 230, 76, 230, 84)
}

pub const CAL_DAYS: i64 = 30;

/// Uma linha do calendário: nome, detalhe e, para cada dia, Some(feito) se valia ou None.
struct CalRow {
    label: String,
    detail: String,
    cells: Vec<Option<bool>>,
}

/// Calendário dos últimos 30 dias: dia completo, leitura e cada hábito.
pub fn draw_calendar(fb: &mut Framebuffer, f: &Fonts, st: &Status) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 120.0, MARGIN - 6, 300, INK, "Calendário");
    let today = crate::data::today(st.offset_min);
    let days: Vec<i64> = (0..CAL_DAYS).map(|k| today - (CAL_DAYS - 1) + k).collect();
    let marks: Vec<HashSet<String>> = days.iter().map(|&d| day::checked(d)).collect();
    let all = day::load_all();
    let mut rows = Vec::new();
    if all.iter().any(|i| matches!(i.kind, Kind::Priority | Kind::Task)) {
        let cells = days
            .iter()
            .zip(&marks)
            .map(|(&d, m)| all.iter().any(|i| matches!(i.kind, Kind::Priority | Kind::Task) && i.on(d)).then(|| day::day_complete(&all, d, m)))
            .collect();
        rows.push(CalRow { label: "Dia completo".into(), detail: "prioridades e tarefas".into(), cells });
    }
    let reading = day::reading_by_day(st.offset_min);
    let goal = day::reading_goal_min() as i64;
    rows.push(CalRow {
        label: "Leitura".into(),
        detail: format!("meta de {goal} min"),
        cells: days.iter().map(|d| Some(reading.get(d).copied().unwrap_or(0) >= goal * 60)).collect(),
    });
    // Hábitos do Aprender, marcados pelo XP de cada dia.
    for (name, srcs) in crate::aprender::habit_topics().into_iter().take(4) {
        rows.push(CalRow { detail: if srcs.is_empty() { "qualquer estudo".into() } else { "estudo".into() }, label: name, cells: days.iter().map(|&d| Some(crate::aprender::habit_on(&srcs, d))).collect() });
    }
    for it in all.iter().filter(|i| i.kind == Kind::Habit).take(7) {
        let key = it.key();
        let cells = days.iter().zip(&marks).map(|(&d, m)| it.on(d).then(|| m.contains(&key))).collect();
        let detail = match it.days_label() {
            l if l.is_empty() => "todo dia".to_string(),
            l => l,
        };
        rows.push(CalRow { label: it.text.clone(), detail, cells });
    }
    let cell = (W - 2 * MARGIN) / CAL_DAYS as i32;
    let sq = cell - 6;
    let mut y = 380;
    for row in &rows {
        let due = row.cells.iter().filter(|c| c.is_some()).count();
        let done = row.cells.iter().filter(|c| **c == Some(true)).count();
        let count = format!("{done} de {due}");
        let cw = f.measure(Face::Strong, 30.0, &count);
        f.draw(fb, Face::Strong, 30.0, W - MARGIN - cw, y + 36, INK, &count);
        let label = fit(f, Face::Strong, 34.0, &row.label, W - 2 * MARGIN - cw - 40);
        let end = f.draw(fb, Face::Strong, 34.0, MARGIN, y + 36, INK, &label);
        let room = W - MARGIN - cw - 24 - end - 16;
        if room > 80 {
            f.draw(fb, Face::Body, 26.0, end + 16, y + 36, MUTED, &fit(f, Face::Body, 26.0, &row.detail, room));
        }
        let top = y + 56;
        for (k, c) in row.cells.iter().enumerate() {
            let r = Rect::new(MARGIN + k as i32 * cell, top, sq, sq);
            match c {
                Some(true) => fb.round_rect(r, 6, INK),
                Some(false) => fb.round_outline(r, 6, 3, 0x88, PAPER),
                None => fb.fill(Rect::new(r.x + sq / 2 - 2, r.y + sq / 2 - 2, 4, 4), 0xAA),
            }
        }
        // Hoje: traço embaixo.
        let last = MARGIN + (CAL_DAYS as i32 - 1) * cell;
        fb.fill(Rect::new(last, top + sq + 6, sq, 4), INK);
        y += 56 + sq + 44;
        if y > 1180 {
            break;
        }
    }
    let axis = y - 20;
    f.draw(fb, Face::Body, 26.0, MARGIN, axis + 10, MUTED, "há 30 dias");
    let tw = f.measure(Face::Body, 26.0, "hoje");
    f.draw(fb, Face::Body, 26.0, W - MARGIN - tw, axis + 10, MUTED, "hoje");
    // Legenda.
    let ly = 1340;
    let mut x = MARGIN;
    for (k, l) in ["feito", "valia e faltou", "não valia"].iter().enumerate() {
        let r = Rect::new(x, ly - 24, 26, 26);
        match k {
            0 => fb.round_rect(r, 6, INK),
            1 => fb.round_outline(r, 6, 3, 0x88, PAPER),
            _ => fb.fill(Rect::new(r.x + 11, r.y + 11, 4, 4), 0xAA),
        }
        let end = f.draw(fb, Face::Body, 28.0, x + 38, ly, MUTED, l);
        x = end + 40;
    }
}

pub fn add_rect() -> Rect {
    Rect::new(W - MARGIN - 270, 76, 270, 84)
}

pub const KINDS: [(&str, char); 4] = [("Tarefa", '-'), ("Prioridade", '!'), ("Lembrete", '@'), ("Hábito", '*')];

pub fn chip_rect(k: usize) -> Rect {
    let w = (W - 2 * MARGIN - 3 * 16) / 4;
    Rect::new(MARGIN + k as i32 * (w + 16), 300, w, 80)
}
pub fn field_rect() -> Rect {
    Rect::new(MARGIN, 420, W - 2 * MARGIN, 180)
}

pub fn draw_compose(fb: &mut Framebuffer, f: &Fonts, kind: usize, text: &str, hint: &str) {
    fb.fill(fb.screen(), PAPER);
    ui::draw_back(fb, f, false);
    f.draw(fb, Face::Display, 64.0, MARGIN, 250, INK, "Adicionar ao dia");
    for (k, (label, _)) in KINDS.iter().enumerate() {
        let r = chip_rect(k);
        let on = k == kind;
        if on {
            fb.round_rect(r, 40, INK);
        } else {
            fb.round_outline(r, 40, 4, INK, PAPER);
        }
        let w = f.measure(Face::Strong, 30.0, label);
        f.draw(fb, Face::Strong, 30.0, r.x + (r.w - w) / 2, r.y + 51, if on { PAPER } else { INK }, label);
    }
    draw_field(fb, f, text, hint);
}

pub fn draw_field(fb: &mut Framebuffer, f: &Fonts, text: &str, hint: &str) {
    let r = field_rect();
    fb.fill(Rect::new(0, r.y - 4, W, 470), PAPER);
    fb.round_outline(r, 24, 4, INK, PAPER);
    // Mostra o final do texto se não couber; o cursor é um traço vertical.
    let max_w = r.w - 60;
    let mut shown: String = text.to_string();
    while f.measure(Face::Body, 48.0, &shown) > max_w && !shown.is_empty() {
        shown.remove(0);
    }
    let end = f.draw(fb, Face::Body, 48.0, r.x + 28, r.y + 108, INK, &shown);
    fb.fill(Rect::new(end + 4, r.y + 62, 4, 60), INK);
    f.draw(fb, Face::Body, 32.0, MARGIN, r.y + r.h + 60, crate::ui::MUTED, hint);
}

const REMINDER_LINE_SIZE: f32 = 72.0;
const REMINDER_LINES_PER_PAGE: usize = 5;

pub fn reminder_ok_rect() -> Rect {
    Rect::new(MARGIN, 1200, W - 2 * MARGIN, 96)
}
pub fn reminder_prev_rect() -> Rect { Rect::new(MARGIN, 1040, 260, 80) }
pub fn reminder_next_rect() -> Rect { Rect::new(W - MARGIN - 260, 1040, 260, 80) }

/// Quebra também palavras longas, preservando todos os caracteres imprimíveis.
fn reminder_lines(f: &Fonts, text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if f.measure(Face::Display, REMINDER_LINE_SIZE, &candidate) <= W - 2 * MARGIN {
            line = candidate;
            continue;
        }
        if !line.is_empty() { lines.push(std::mem::take(&mut line)); }
        for ch in word.chars() {
            let mut candidate = line.clone();
            candidate.push(ch);
            if f.measure(Face::Display, REMINDER_LINE_SIZE, &candidate) > W - 2 * MARGIN && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            line.push(ch);
        }
    }
    if !line.is_empty() { lines.push(line); }
    if lines.is_empty() { lines.push(String::new()); }
    lines
}

pub fn reminder_page_count(f: &Fonts, text: &str) -> usize {
    reminder_lines(f, text).len().div_ceil(REMINDER_LINES_PER_PAGE).max(1)
}

/// Desenha uma página. `page` é limitado mesmo se o texto mudar enquanto a tela estiver aberta.
pub fn draw_reminder_page(fb: &mut Framebuffer, f: &Fonts, it: &Item, page: usize) {
    fb.fill(fb.screen(), PAPER);
    f.draw(fb, Face::Display, 40.0, MARGIN, 150, INK, "Verso Ink");
    ui::bell(fb, MARGIN as f32 + 70.0, 300.0);
    f.draw(fb, Face::Strong, 34.0, MARGIN, 560, MUTED, &format!("LEMBRETE · {}", it.time_label()));
    let lines = reminder_lines(f, &it.text);
    let pages = lines.len().div_ceil(REMINDER_LINES_PER_PAGE).max(1);
    let page = page.min(pages - 1);
    for (k, line) in lines.iter().skip(page * REMINDER_LINES_PER_PAGE).take(REMINDER_LINES_PER_PAGE).enumerate() {
        f.draw(fb, Face::Display, REMINDER_LINE_SIZE, MARGIN, 635 + k as i32 * 90, INK, line);
    }
    if pages > 1 {
        if page > 0 { ui::button(fb, f, reminder_prev_rect(), "Anterior", false); }
        if page + 1 < pages { ui::button(fb, f, reminder_next_rect(), "Próxima", false); }
        let label = format!("{} de {}", page + 1, pages);
        let width = f.measure(Face::Strong, 28.0, &label);
        f.draw(fb, Face::Strong, 28.0, (W - width) / 2, 1090, MUTED, &label);
    }
    ui::button(fb, f, reminder_ok_rect(), "Feito", true);
}

pub fn draw_reminder(fb: &mut Framebuffer, f: &Fonts, it: &Item) {
    draw_reminder_page(fb, f, it, 0);
}

#[cfg(test)]
mod reminder_tests {
    use super::*;
    #[test]
    fn quebra_texto_e_palavra_longa_sem_descartar_caracteres() {
        let f = Fonts::new();
        let long_word = "extraordinariamente".repeat(20);
        let text = format!("Uma mensagem longa para ser lida com calma {long_word} e depois fechar.");
        let lines = reminder_lines(&f, &text);
        assert!(reminder_page_count(&f, &text) > 1);
        assert!(lines.iter().all(|line| f.measure(Face::Display, REMINDER_LINE_SIZE, line) <= W - 2 * MARGIN));
        let expected: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        let got: String = lines.concat().chars().filter(|c| !c.is_whitespace()).collect();
        assert_eq!(got, expected);
        assert_eq!(reminder_page_count(&f, "Curto"), 1);
        assert_eq!(reminder_page_count(&f, ""), 1);
    }
}

#[cfg(test)]
mod render {
    /// Desenha Hoje e o calendário com dados de exemplo num diretório temporário (só no PC):
    /// `VERSO_INK_SHOTS=pasta cargo test calendario_de_exemplo -- --ignored`
    #[test]
    #[ignore]
    fn calendario_de_exemplo() {
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let home = std::env::temp_dir().join(format!("verso-ink-teste-{}", std::process::id()));
        std::fs::create_dir_all(home.join("data/dias")).unwrap();
        std::fs::create_dir_all(home.join("settings")).unwrap();
        std::env::set_var("VERSO_INK_HOME", &home);
        std::fs::write(home.join("data/dia.txt"), "! Revisar o capítulo 3\n- Comprar pão\n- {sab} Faxina\n@ {uteis} 08:00 Remédio\n* Ler 20 min\n* {seg,qua,sex} Academia\n* {fds} Ligar para a mãe\n").unwrap();
        let st = crate::ui::Status { battery: Some(80), charging: false, wifi: true, offset_min: 0 };
        let today = crate::data::today(0);
        for k in 1..30i64 {
            let d = today - k;
            let mut v = Vec::new();
            if k % 3 != 0 {
                v.push("* Ler 20 min");
            }
            match crate::day::weekday(d) {
                0 | 2 | 4 if k % 4 != 1 => v.push("* Academia"),
                5 | 6 => v.push("* Ligar para a mãe"),
                _ => {}
            }
            if k % 5 != 2 {
                v.extend(["! Revisar o capítulo 3", "- Comprar pão", "- Faxina"]);
            }
            std::fs::write(home.join(format!("data/dias/{d}.txt")), v.join("\n")).unwrap();
        }
        std::fs::write(home.join(format!("data/dias/{today}.txt")), "* Ler 20 min").unwrap();
        let f = crate::text::Fonts::new();
        let mut fb = crate::fb::Framebuffer::memory();
        let items = crate::day::load_items();
        let done = crate::day::checked(today);
        super::draw_today(&mut fb, &f, &items, &done, &st, 1);
        fb.save_pgm(&out.join("hoje.pgm"));
        let mut fb = crate::fb::Framebuffer::memory();
        super::draw_calendar(&mut fb, &f, &st);
        fb.save_pgm(&out.join("calendario.pgm"));
        let _ = std::fs::remove_dir_all(&home);
    }
}
