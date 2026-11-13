//! O dia: prioridades, tarefas, lembretes e hábitos, com sequências (streaks).
//!
//! A lista vem de `data/dia.txt`, escrito pelo PC (tools/dia.sh) ou pelo Claude:
//!
//! ```text
//! ! Terminar o relatório        ← prioridade (até 3)
//! - Comprar pão                 ← tarefa
//! @ 14:30 Reunião com a equipe  ← lembrete com hora
//! * Ler 20 min                  ← hábito (vale todo dia)
//! * {seg,qua,sex} Academia      ← só nesses dias da semana (vale para qualquer tipo)
//! @ {uteis} 08:00 Remédio       ← atalhos: uteis (seg a sex), fds (sáb e dom)
//! # linhas com # são comentários
//! ```
//!
//! O que foi marcado em cada dia fica em `data/dias/<dia>.txt` (uma chave por linha).

use crate::data::today;
use crate::home_dir;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Priority,
    Task,
    Reminder,
    Habit,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub kind: Kind,
    pub text: String,
    /// Minutos desde a meia-noite (só lembretes).
    pub at: Option<u32>,
    /// Dias da semana em que vale (segunda = 0); `None` = todo dia.
    pub days: Option<[bool; 7]>,
}

/// Dia da semana de um dia (dias desde 1970-01-01, que foi quinta): segunda = 0.
pub fn weekday(day: i64) -> usize {
    (day + 3).rem_euclid(7) as usize
}

const WEEK: [&str; 7] = ["seg", "ter", "qua", "qui", "sex", "sab", "dom"];

/// "{seg,qua}" → dias marcados. Aceita acento (sáb), nomes longos (segunda) e atalhos.
fn parse_days(spec: &str) -> Option<[bool; 7]> {
    let mut m = [false; 7];
    for w in spec.split([',', ' ']).filter(|w| !w.is_empty()) {
        let w = crate::apps::fold(w);
        match w.as_str() {
            "uteis" | "util" => m[..5].iter_mut().for_each(|d| *d = true),
            "fds" => {
                m[5] = true;
                m[6] = true;
            }
            _ => m[WEEK.iter().position(|d| w.starts_with(d))?] = true,
        }
    }
    m.iter().any(|&d| d).then_some(m)
}

impl Item {
    /// Chave estável para marcar o item num dia.
    pub fn key(&self) -> String {
        let k = match self.kind {
            Kind::Priority => "!",
            Kind::Task => "-",
            Kind::Reminder => "@",
            Kind::Habit => "*",
        };
        format!("{k} {}", self.text)
    }
    /// Vale neste dia?
    pub fn on(&self, day: i64) -> bool {
        self.days.is_none_or(|m| m[weekday(day)])
    }
    /// "seg, qua, sex" para mostrar (vazio se é todo dia).
    pub fn days_label(&self) -> String {
        match self.days {
            None => String::new(),
            Some(m) if m == [true, true, true, true, true, false, false] => "dias úteis".into(),
            Some(m) if m == [false, false, false, false, false, true, true] => "fim de semana".into(),
            Some(m) => (0..7).filter(|&k| m[k]).map(|k| ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"][k]).collect::<Vec<_>>().join(", "),
        }
    }
    pub fn time_label(&self) -> String {
        self.at.map(|m| format!("{:02}:{:02}", m / 60, m % 60)).unwrap_or_default()
    }
}

pub fn parse(src: &str) -> Vec<Item> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            let mark = l.chars().next()?;
            let mut rest = l.get(mark.len_utf8()..)?.trim();
            let mut days = None;
            if let Some(after) = rest.strip_prefix('{') {
                let (spec, tail) = after.split_once('}')?;
                days = Some(parse_days(spec)?);
                rest = tail.trim();
            }
            if rest.is_empty() {
                return None;
            }
            let item = |kind, text: &str, at| Some(Item { kind, text: text.into(), at, days });
            match mark {
                '!' => item(Kind::Priority, rest, None),
                '-' => item(Kind::Task, rest, None),
                '*' => item(Kind::Habit, rest, None),
                '@' => {
                    let (time, text) = rest.split_once(' ')?;
                    let (h, m) = time.split_once(':')?;
                    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
                    if h < 24 && m < 60 { item(Kind::Reminder, text.trim(), Some(h * 60 + m)) } else { None }
                }
                _ => None,
            }
        })
        .collect()
}

fn plan_path() -> PathBuf {
    home_dir().join("data/dia.txt")
}

fn day_path(day: i64) -> PathBuf {
    let d = home_dir().join("data/dias");
    let _ = std::fs::create_dir_all(&d);
    d.join(format!("{day}.txt"))
}

/// Tudo o que está no plano, inclusive o que só vale em outros dias da semana.
pub fn load_all() -> Vec<Item> {
    std::fs::read_to_string(plan_path()).map(|s| parse(&s)).unwrap_or_default()
}

/// O que vale hoje.
pub fn load_items() -> Vec<Item> {
    let t = today(crate::offset_min());
    load_all().into_iter().filter(|i| i.on(t)).collect()
}

pub fn checked(day: i64) -> HashSet<String> {
    std::fs::read_to_string(day_path(day)).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default()
}

pub fn toggle(day: i64, key: &str) -> bool {
    let mut set = checked(day);
    let now_on = if set.contains(key) {
        set.remove(key);
        false
    } else {
        set.insert(key.to_string());
        true
    };
    let mut v: Vec<_> = set.into_iter().collect();
    v.sort();
    let _ = std::fs::write(day_path(day), v.join("\n"));
    now_on
}

/// Marca sem desmarcar uma confirmação já registrada; erros preservam o arquivo original.
/// As demais linhas são mantidas como estavam, inclusive linhas desconhecidas.
pub fn confirmar(day: i64, key: &str) -> std::io::Result<()> {
    let path = home_dir().join("data/dias").join(format!("{day}.txt"));
    confirmar_em(&path, key)
}

fn ler_marcacoes(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        result => result,
    }
}

fn confirmar_em(path: &Path, key: &str) -> std::io::Result<()> {
    if key.trim().is_empty() || key.contains(['\r', '\n', '\0']) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Chave de lembrete inválida"));
    }
    let original = ler_marcacoes(path)?;
    if original.lines().any(|line| line == key) { return Ok(()); }
    let mut updated = original.clone();
    if !updated.is_empty() && !updated.ends_with('\n') { updated.push('\n'); }
    updated.push_str(key);
    updated.push('\n');
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let (tmp, mut file) = loop {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let tmp = path.with_extension(format!("tmp-{}-{n}", std::process::id()));
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        file.write_all(updated.as_bytes())?;
        file.sync_all()?;
        if ler_marcacoes(path)? != original { return Err(std::io::Error::other("Marcações mudaram; tente novamente")); }
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(tmp); }
    result
}

/// Dia completo: todas as prioridades e tarefas daquele dia marcadas (e havia alguma).
pub fn day_complete(items: &[Item], day: i64, done: &HashSet<String>) -> bool {
    let todo: Vec<_> = items.iter().filter(|i| matches!(i.kind, Kind::Priority | Kind::Task) && i.on(day)).collect();
    !todo.is_empty() && todo.iter().all(|i| done.contains(&i.key()))
}

/// Dias seguidos até hoje (hoje conta se já cumprido; se ainda não, a sequência vem de ontem).
fn streak(ok: impl FnMut(i64) -> bool, today: i64) -> u32 {
    streak_on(ok, |_| true, today)
}

/// Igual, mas só conta os dias em que vale (`due`): os outros não quebram nem somam.
fn streak_on(mut ok: impl FnMut(i64) -> bool, mut due: impl FnMut(i64) -> bool, today: i64) -> u32 {
    let mut d = if due(today) && !ok(today) { today - 1 } else { today };
    let mut n = 0;
    for _ in 0..3650 {
        if due(d) {
            if !ok(d) {
                break;
            }
            n += 1;
        }
        d -= 1;
    }
    n
}

pub fn todo_streak(_items: &[Item], offset_min: i64) -> u32 {
    let all = load_all();
    // Dias sem tarefa nenhuma marcada para eles não contam nem quebram.
    let due = |d: i64| all.iter().any(|i| matches!(i.kind, Kind::Priority | Kind::Task) && i.on(d));
    streak_on(|d| day_complete(&all, d, &checked(d)), due, today(offset_min))
}

pub fn habit_streak(item: &Item, offset_min: i64) -> u32 {
    let key = item.key();
    streak_on(|d| checked(d).contains(&key), |d| item.on(d), today(offset_min))
}

/// Lembretes que já passaram da hora hoje e ainda não foram vistos.
pub fn due_reminder(items: &[Item], offset_min: i64, now_min: u32) -> Option<Item> {
    let done = checked(today(offset_min));
    items
        .iter()
        .filter(|i| i.kind == Kind::Reminder && i.at.is_some_and(|m| m <= now_min) && !done.contains(&i.key()))
        .min_by_key(|i| i.at)
        .cloned()
}

pub fn next_reminder(items: &[Item], offset_min: i64, now_min: u32) -> Option<Item> {
    let done = checked(today(offset_min));
    items.iter().filter(|i| i.kind == Kind::Reminder && i.at.is_some_and(|m| m > now_min) && !done.contains(&i.key())).min_by_key(|i| i.at).cloned()
}

/// Sequência de leitura: dias seguidos com pelo menos a meta (minutos) no KOReader.
pub fn reading_goal_min() -> u32 {
    crate::sys::read_trim(home_dir().join("settings/meta_leitura_min").to_str().unwrap_or("")).and_then(|s| s.parse().ok()).unwrap_or(20)
}

/// Segundos lidos no KOReader por dia.
pub fn reading_by_day(offset_min: i64) -> std::collections::HashMap<i64, i64> {
    let mut per_day = std::collections::HashMap::<i64, i64>::new();
    if let Ok(s) = std::fs::read_to_string(home_dir().join("data/reading.log")) {
        for l in s.lines() {
            let mut it = l.split_whitespace();
            if let (Some(Ok(start)), Some(Ok(dur))) = (it.next().map(str::parse::<i64>), it.next().map(str::parse::<i64>)) {
                *per_day.entry(crate::data::day_of(start, offset_min)).or_default() += dur.max(0);
            }
        }
    }
    per_day
}

pub fn reading_streak(offset_min: i64) -> u32 {
    let goal = reading_goal_min() as i64 * 60;
    let per_day = reading_by_day(offset_min);
    streak(|d| per_day.get(&d).copied().unwrap_or(0) >= goal, today(offset_min))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmar_preserva_linhas_e_nao_desmarca_na_repeticao() {
        let dir = std::env::temp_dir().join(format!("verso-dia-confirma-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("20000.txt");
        std::fs::write(&path, "# linha livre\r\n* Hábito\ntexto sem marcador").unwrap();
        confirmar_em(&path, "@ Remédio").unwrap();
        let expected = "# linha livre\r\n* Hábito\ntexto sem marcador\n@ Remédio\n";
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        confirmar_em(&path, "@ Remédio").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        assert!(confirmar_em(&path, "@ Quebra\nde linha").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn confirmar_cria_arquivo_e_recusa_falhas_sem_apagar_dados() {
        let dir = std::env::temp_dir().join(format!("verso-dia-falha-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("dias/1.txt");
        confirmar_em(&path, "@ Consulta").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "@ Consulta\n");
        std::fs::write(&path, [0xff, 0xfe]).unwrap();
        assert!(confirmar_em(&path, "@ Outro").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0xfe]);
        assert!(confirmar_em(&path.join("arquivo"), "@ Outro").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0xfe]);
        // Nome de arquivo válido, mas temporário longo demais: falha de escrita sem dano.
        let long = dir.join("a".repeat(250));
        std::fs::write(&long, "* Guardado\n").unwrap();
        assert!(confirmar_em(&long, "@ Outro").is_err());
        assert_eq!(std::fs::read_to_string(&long).unwrap(), "* Guardado\n");
        let _ = std::fs::remove_dir_all(dir);
    }
    #[test]
    fn le_o_formato() {
        let it = parse("# comentário\n! Relatório\n- Pão\n@ 14:30 Reunião com a equipe\n@ 25:00 inválido\n* Ler 20 min\n-\n");
        assert_eq!(it.len(), 4);
        assert_eq!(it[2].at, Some(14 * 60 + 30));
        assert_eq!(it[2].text, "Reunião com a equipe");
        assert_eq!(it[3].kind, Kind::Habit);
    }
    #[test]
    fn repeticao_por_dia_da_semana() {
        let it = parse("* {seg,qua,sex} Academia\n@ {uteis} 08:00 Remédio\n- {sáb} Faxina\n* {xyz} inválido\n");
        assert_eq!(it.len(), 3);
        // 2026-11-09 é segunda: 20766 dias desde 1970-01-01.
        let seg = 20766;
        assert_eq!(weekday(seg), 0);
        assert!(it[0].on(seg) && !it[0].on(seg + 1) && it[0].on(seg + 2));
        assert_eq!(it[1].at, Some(8 * 60));
        assert!(it[1].on(seg + 4) && !it[1].on(seg + 5));
        assert!(it[2].on(seg + 5));
        assert_eq!(it[0].days_label(), "seg, qua, sex");
        assert_eq!(it[1].days_label(), "dias úteis");
    }
    #[test]
    fn sequencia_pula_dias_em_que_nao_vale() {
        // Vale em dias pares; feitos 10, 8, 6 (9 e 7 não valem); hoje 11 não vale.
        let feitos = [10, 8, 6];
        assert_eq!(streak_on(|d| feitos.contains(&d), |d| d % 2 == 0, 11), 3);
        assert_eq!(streak_on(|d| feitos.contains(&d), |d| d % 2 == 0, 12), 3); // 12 ainda não feito
    }
    #[test]
    fn sequencia_conta_hoje_ou_vem_de_ontem() {
        let feitos = [10, 9, 8, 6];
        assert_eq!(streak(|d| feitos.contains(&d), 10), 3);
        assert_eq!(streak(|d| feitos.contains(&d), 11), 3); // hoje (11) ainda não, vale até ontem
        assert_eq!(streak(|d| feitos.contains(&d), 12), 0);
    }
}
