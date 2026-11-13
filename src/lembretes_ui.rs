//! Lembretes avulsos em arquivo próprio; os recorrentes do plano do dia não são alterados.

use crate::day::{self, Item, Kind};
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const PER_PAGE: usize = 6;

fn parse_time(value: &str) -> Option<u32> {
    let (hour, minute) = value.trim().split_once(':')?;
    if hour.is_empty() || hour.len() > 2 || minute.len() != 2 || !hour.bytes().all(|b| b.is_ascii_digit()) || !minute.bytes().all(|b| b.is_ascii_digit()) { return None; }
    let (hour, minute): (u32, u32) = (hour.parse().ok()?, minute.parse().ok()?);
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Avulso {
    pub epoch: i64,
    pub texto: String,
}

fn parse_once(source: &str) -> Vec<(usize, Avulso)> {
    source.lines().enumerate().filter_map(|(line, text)| {
        let (epoch, texto) = text.split_once('\t')?;
        if texto.trim().is_empty() || texto.chars().any(char::is_control) { return None; }
        let epoch = epoch.parse::<i64>().ok()?;
        if epoch < 0 { return None; }
        Some((line, Avulso { epoch, texto: texto.to_string() }))
    }).collect()
}

/// A próxima ocorrência local da hora escolhida, incluindo o minuto atual.
fn occurrence(now: i64, offset_min: i64, time: u32) -> i64 {
    let offset = offset_min * 60;
    let day = (now + offset).div_euclid(86_400);
    let candidate = day * 86_400 - offset + i64::from(time) * 60;
    if candidate + 60 <= now { candidate + 86_400 } else { candidate }
}

fn append_reminder(source: &str, epoch: i64, text: &str) -> Result<String, &'static str> {
    let text = text.trim();
    if epoch < 0 || text.is_empty() || text.chars().any(char::is_control) { return Err("Escreva um lembrete em uma linha."); }
    if parse_once(source).iter().any(|(_, i)| i.epoch == epoch && i.texto.eq_ignore_ascii_case(text)) {
        return Err("Esse lembrete já está na lista.");
    }
    let mut out = source.to_string();
    if !out.is_empty() && !out.ends_with('\n') { out.push('\n'); }
    out.push_str(&format!("{epoch}\t{text}\n"));
    Ok(out)
}

fn path() -> PathBuf { crate::home_dir().join("data/lembretes_avulsos.txt") }

fn read_path(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        value => value,
    }
}
fn read() -> std::io::Result<String> { read_path(&path()) }

/// Usa um temporário exclusivo no mesmo diretório; falhas não apagam a lista anterior.
fn save_to(path: &Path, value: &str, expected: Option<&str>) -> std::io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    let (tmp, mut f) = loop {
        let suffix = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp-{}-{suffix}", std::process::id()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        f.write_all(value.as_bytes())?;
        f.sync_all()?;
        if let Some(previous) = expected {
            if read_path(path)? != previous { return Err(std::io::Error::other("A lista foi alterada; tente novamente.")); }
        }
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(tmp); }
    result
}
fn save(previous: &str, value: &str) -> std::io::Result<()> { save_to(&path(), value, Some(previous)) }

fn list(source: &str) -> Vec<Avulso> {
    let mut items: Vec<_> = parse_once(source).into_iter().map(|(_, item)| item).collect();
    items.sort_by_key(|item| item.epoch);
    items
}

/// A leitura falha explicitamente se a lista existir mas não puder ser lida.
pub fn tentar_pendentes() -> std::io::Result<Vec<Avulso>> { read().map(|source| list(&source)) }

/// Lembretes ainda não confirmados, na ordem em que devem aparecer.
pub fn pendentes() -> Vec<Avulso> { tentar_pendentes().unwrap_or_default() }

pub fn vencido(now: i64) -> Option<Avulso> {
    pendentes().into_iter().find(|item| item.epoch <= now)
}

fn remove_once(source: &str, item: &Avulso) -> Option<String> {
    let (line, _) = parse_once(source).into_iter().find(|(_, value)| value == item)?;
    Some(source.split_inclusive('\n').enumerate().filter_map(|(i, text)| (i != line).then_some(text)).collect())
}

/// Ao confirmar o alerta, remove somente a ocorrência exibida e preserva as demais linhas.
pub fn confirmar(item: &Avulso) -> std::io::Result<bool> {
    let source = read()?;
    let Some(updated) = remove_once(&source, item) else { return Ok(false) };
    save(&source, &updated)?;
    Ok(true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field { Time, Text }

pub struct Lembretes {
    page: usize,
    pending: Option<Field>,
    requested: bool,
    time: Option<u32>,
    message: String,
}

impl Lembretes {
    pub fn new() -> Self { Self { page: 0, pending: None, requested: false, time: None, message: String::new() } }

    fn recurring() -> Vec<Item> {
        day::load_all().into_iter().filter(|i| i.kind == Kind::Reminder).collect()
    }

    fn count() -> usize { pendentes().len() + Self::recurring().len() }

    fn row(k: usize) -> Rect { Rect::new(MARGIN, 290 + k as i32 * 150, W - 2 * MARGIN, 132) }
    fn nav(next: bool) -> Rect { Rect::new(if next { W - MARGIN - 220 } else { MARGIN }, 1185, 220, 70) }
    fn bottom(k: usize, n: usize) -> Rect {
        let w = (W - 2 * MARGIN - (n as i32 - 1) * 16) / n as i32;
        Rect::new(MARGIN + k as i32 * (w + 16), 1288, w, 80)
    }

    fn input(&mut self, text: &str) {
        match self.pending.take() {
            Some(Field::Time) => match parse_time(text) {
                Some(time) => { self.time = Some(time); self.message = "Agora escreva o texto do lembrete.".into(); }
                None => { self.time = None; self.message = "Use o formato 08:30, de 00:00 a 23:59.".into(); }
            },
            Some(Field::Text) => {
                let Some(time) = self.time else { return };
                let epoch = occurrence(crate::data::epoch(), crate::offset_min(), time);
                let result = read().map_err(|_| "Não foi possível abrir a lista.").and_then(|source| append_reminder(&source, epoch, text).map(|value| (source, value)));
                match result {
                    Ok((source, value)) if save(&source, &value).is_ok() => { self.time = None; self.message = "Lembrete salvo para a próxima ocorrência.".into(); }
                    Ok(_) => self.message = "Não foi possível salvar o lembrete.".into(),
                    Err(e) => self.message = e.into(),
                }
            }
            None => {}
        }
    }

    #[cfg(test)]
    pub fn screenshot_time(&mut self) { self.time = Some(8 * 60 + 30); }

    #[cfg(test)]
    pub fn print_vazio() -> Self { Self::new() }
    #[cfg(test)]
    pub fn print_hora() -> Self { let mut g = Self::new(); g.screenshot_time(); g }
}

impl MiniGame for Lembretes {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        minigame::header(fb, f, "Mais", "Lembretes");
        let stored = tentar_pendentes();
        let mut items: Vec<(String, String, String)> = stored.as_deref().unwrap_or_default().iter().map(|i| {
            let local = i.epoch.saturating_add(crate::offset_min().saturating_mul(60));
            let minute = local.rem_euclid(86_400) / 60;
            let today = crate::data::today(crate::offset_min());
            let label = if local.div_euclid(86_400) > today { "Amanhã" } else if local.div_euclid(86_400) < today { "Atrasado" } else { "Hoje" };
            (format!("{:02}:{:02}", minute / 60, minute % 60), i.texto.clone(), label.into())
        }).collect();
        items.extend(Self::recurring().iter().map(|item| {
            let days = if item.days.is_none() { "Todos os dias".into() } else { item.days_label() };
            (item.time_label(), item.text.clone(), days)
        }));
        if items.is_empty() && stored.is_ok() {
            let lines = crate::chat::wrap_words(f, Face::Body, 34.0, "Nenhum lembrete. Escolha uma hora e escreva o que quer lembrar.", W - 2 * MARGIN);
            for (i, line) in lines.iter().enumerate() { f.draw(fb, Face::Body, 34.0, MARGIN, 370 + i as i32 * 48, MUTED, line); }
        }
        for (k, (time, text, days)) in items.iter().skip(self.page * PER_PAGE).take(PER_PAGE).enumerate() {
            let r = Self::row(k);
            ui::card(fb, r, 20, PAPER, false);
            f.draw(fb, Face::Display, 42.0, r.x + 24, r.y + 65, INK, time);
            let title = crate::apps::fit(f, Face::Strong, 30.0, text, r.w - 195);
            f.draw(fb, Face::Strong, 30.0, r.x + 176, r.y + 57, INK, &title);
            f.draw(fb, Face::Body, 25.0, r.x + 176, r.y + 101, MUTED, days);
        }
        if self.page > 0 { minigame::pill(fb, f, Self::nav(false), "Anterior", false); }
        if (self.page + 1) * PER_PAGE < items.len() { minigame::pill(fb, f, Self::nav(true), "Próxima", false); }
        let hint = if stored.is_err() { "Não consegui abrir a lista de lembretes.".to_string() } else if !self.message.is_empty() { self.message.clone() } else if let Some(time) = self.time { format!("Hora escolhida: {:02}:{:02}.", time / 60, time % 60) } else { "Novos lembretes avisam uma vez.".into() };
        let hint = crate::apps::fit(f, Face::Body, 25.0, &hint, W - 2 * MARGIN);
        if self.page == 0 && items.len() <= PER_PAGE { f.draw(fb, Face::Body, 25.0, MARGIN, 1265, MUTED, &hint); }
        else if stored.is_err() || !self.message.is_empty() { f.draw(fb, Face::Body, 25.0, MARGIN, 268, MUTED, &hint); }
        let labels = if self.time.is_some() { ["Mudar hora", "Escrever lembrete"] } else { ["Escolher hora", ""] };
        if self.time.is_some() {
            for (i, label) in labels.iter().enumerate() { minigame::pill(fb, f, Self::bottom(i, 2), label, i == 1); }
        } else { minigame::pill(fb, f, Self::bottom(0, 1), labels[0], true); }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        let mut out = Vec::new();
        if self.time.is_some() {
            out.push((800, Self::bottom(0, 2)));
            out.push((801, Self::bottom(1, 2)));
        } else { out.push((800, Self::bottom(0, 1))); }
        if self.page > 0 { out.push((810, Self::nav(false))); }
        if (self.page + 1) * PER_PAGE < Self::count() { out.push((811, Self::nav(true))); }
        out
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            800 | 801 => { self.pending = Some(if id == 800 { Field::Time } else { Field::Text }); self.requested = true; }
            810 => self.page = self.page.saturating_sub(1),
            811 => self.page += 1,
            _ => return Redraw::None,
        }
        Redraw::Full
    }

    fn wants_text(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.requested) { return None; }
        Some(if self.pending == Some(Field::Time) { "Hora, por exemplo 08:30" } else { "Texto do lembrete" }.into())
    }

    fn text_input(&mut self, text: &str) -> Redraw { self.input(text); Redraw::Full }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valida_horarios() {
        assert_eq!(parse_time("8:30"), Some(510));
        assert_eq!(parse_time("23:59"), Some(1439));
        assert_eq!(parse_time("00:00"), Some(0));
        for invalid in ["24:00", "12:60", "8:3", "-1:00", "12:00:00", "１２:００", ""] { assert_eq!(parse_time(invalid), None); }
    }
    #[test]
    fn salva_avulso_e_preserva_linhas_anteriores() {
        let source = "# comentário\ntexto livre";
        let result = append_reminder(source, 510, "Consulta").unwrap();
        assert_eq!(result, format!("{source}\n510\tConsulta\n"));
        let saved = &parse_once(&result)[0].1;
        assert_eq!(saved.epoch, 510);
        assert_eq!(saved.texto, "Consulta");
        assert_eq!(remove_once(&result, saved).unwrap(), "# comentário\ntexto livre\n");
        assert!(append_reminder(&result, 510, "Consulta").is_err());
        assert!(append_reminder(&result, 600, "duas\nlinhas").is_err());
    }

    #[test]
    fn avisos_simultaneos_sao_removidos_um_por_vez() {
        let source = "# manter\n100\tÁgua\n100\tRemédio\n100\tÁgua\n200\tOutro\ntexto livre\n";
        let first = list(source);
        assert_eq!(first.len(), 4);
        let updated = remove_once(source, &first[0]).unwrap();
        let remaining = list(&updated);
        assert_eq!(remaining.len(), 3);
        assert_eq!(remaining[0].texto, "Remédio");
        assert_eq!(remaining[1].texto, "Água");
        assert!(updated.contains("# manter\n") && updated.contains("texto livre\n"));
        assert!(remove_once(&updated, &Avulso { epoch: 999, texto: "Ausente".into() }).is_none());
    }

    #[test]
    fn horario_invalido_nao_reutiliza_hora_anterior() {
        let mut g = Lembretes::print_hora();
        assert!(Lembretes::print_vazio().time.is_none());
        g.pending = Some(Field::Time);
        g.input("24:00");
        assert!(g.time.is_none());
        assert!(g.message.contains("08:30"));
    }

    #[test]
    fn erro_de_gravacao_nao_apaga_lista_existente() {
        let dir = std::env::temp_dir().join(format!("verso-lembretes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lista.txt");
        save_to(&path, "100\tManter\n", None).unwrap();
        assert_eq!(read_path(&path).unwrap(), "100\tManter\n");
        let invalid = path.join("lista");
        assert!(save_to(&invalid, "200\tNovo\n", None).is_err());
        assert!(save_to(&path, "200\tNovo\n", Some("versão antiga")).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "100\tManter\n");
        assert!(read_path(&path.join("outra")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn proxima_ocorrencia_respeita_o_horario_local() {
        let midnight = 20_000 * 86_400 + 4 * 3600;
        assert_eq!(occurrence(midnight + 8 * 3600, -240, 510), midnight + 8 * 3600 + 30 * 60);
        assert_eq!(occurrence(midnight + 9 * 3600, -240, 510), midnight + 86_400 + 8 * 3600 + 30 * 60);
        assert_eq!(occurrence(midnight + 8 * 3600 + 30 * 60 + 20, -240, 510), midnight + 8 * 3600 + 30 * 60);
    }
}
