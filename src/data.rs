//! Dados do dia a dia, em arquivos de texto simples em /mnt/us/verso-ink/data
//! (fáceis de ler e editar pelo PC ou pelo Claude).

use crate::home_dir;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn dir() -> PathBuf {
    let d = home_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn epoch() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Número do dia local (dias desde 1970) para um instante em segundos.
pub fn day_of(secs: i64, offset_min: i64) -> i64 {
    (secs + offset_min * 60).div_euclid(86_400)
}

pub fn today(offset_min: i64) -> i64 {
    day_of(epoch(), offset_min)
}

fn append(name: &str, line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir().join(name)) {
        let _ = writeln!(f, "{line}");
    }
}

fn lines(name: &str) -> Vec<String> {
    std::fs::read_to_string(dir().join(name)).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default()
}

// ---------- Foco ----------

pub fn add_focus() {
    append("focus.log", &epoch().to_string());
}

pub fn focus_today(offset_min: i64) -> u32 {
    let t = today(offset_min);
    lines("focus.log").iter().filter_map(|l| l.trim().parse::<i64>().ok()).filter(|&s| day_of(s, offset_min) == t).count() as u32
}

// ---------- Leitura ----------

pub struct LastBook {
    pub title: String,
    /// Fração lida (0..1), do arquivo de progresso do KOReader, quando existir.
    pub progress: Option<f32>,
}

pub fn set_last_book(path: &Path, title: &str) {
    let _ = std::fs::write(dir().join("last_book"), format!("{}\n{}\n", path.display(), title));
}

/// Progresso que o KOReader guarda ao lado do livro: `<livro>.sdr/metadata.<ext>.lua`.
fn koreader_progress(path: &Path) -> Option<f32> {
    let ext = path.extension()?.to_str()?;
    let stem = path.with_extension("");
    let sdr = PathBuf::from(format!("{}.sdr", stem.display()));
    let meta = std::fs::read_to_string(sdr.join(format!("metadata.{ext}.lua"))).ok()?;
    let i = meta.find("[\"percent_finished\"]")?;
    let rest = &meta[i..];
    let num: String = rest[rest.find('=')? + 1..].trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    num.parse().ok()
}

pub fn last_book() -> Option<LastBook> {
    let l = lines("last_book");
    let path = PathBuf::from(l.first()?);
    let title = l.get(1).cloned().unwrap_or_default();
    Some(LastBook { progress: koreader_progress(&path), title })
}

/// Minutos lidos no KOReader hoje (o `session.sh` registra início e duração de cada leitura).
pub fn reading_minutes_today(offset_min: i64) -> u32 {
    let t = today(offset_min);
    let secs: i64 = lines("reading.log")
        .iter()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.parse::<i64>().ok()?, it.next()?.parse::<i64>().ok()?))
        })
        .filter(|(start, _)| day_of(*start, offset_min) == t)
        .map(|(_, dur)| dur.max(0))
        .sum();
    (secs / 60) as u32
}

// ---------- Resumo da tela inicial ----------

pub struct Summary {
    pub line: Option<String>,
}

pub fn summary(offset_min: i64) -> Summary {
    let mut parts = Vec::new();
    let items = crate::day::load_items();
    let now = crate::clock::now(offset_min);
    let now_min = now.hour * 60 + now.minute;
    if let Some(r) = crate::day::next_reminder(&items, offset_min, now_min) {
        parts.push(format!("{} {}", r.time_label(), r.text));
    }
    let streak = crate::day::todo_streak(&items, offset_min);
    if streak > 1 {
        parts.push(format!("{streak} dias completos"));
    }
    let rs = crate::day::reading_streak(offset_min);
    if rs > 1 {
        parts.push(format!("{rs} dias lendo"));
    }
    let read = reading_minutes_today(offset_min);
    if read > 0 {
        parts.push(format!("{read} min de leitura hoje"));
    }
    let focus = focus_today(offset_min);
    if focus > 0 {
        parts.push(format!("{focus} {} hoje", if focus == 1 { "foco" } else { "focos" }));
    }
    Summary { line: (!parts.is_empty()).then(|| parts.join(" · ")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dia_local_vira_na_meia_noite_de_manaus() {
        // 2026-11-11 03:59 UTC = 23:59 do dia 10 em Manaus (UTC-4).
        let s = 1_794_369_540; // 2026-11-11T03:59:00Z
        assert_eq!(day_of(s, -240), day_of(s, 0) - 1);
        assert_eq!(day_of(s + 60, -240), day_of(s, 0));
    }
}
