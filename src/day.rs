//! O dia: prioridades, tarefas, lembretes e hábitos, com sequências (streaks).
//!
//! A lista vem de `data/dia.txt`, escrito pelo PC (tools/dia.sh) ou pelo Claude:
//!
//! ```text
//! ! Terminar o relatório        ← prioridade (até 3)
//! - Comprar pão                 ← tarefa
//! @ 14:30 Reunião com a equipe  ← lembrete com hora
//! * Ler 20 min                  ← hábito (vale todo dia)
//! # linhas com # são comentários
//! ```
//!
//! O que foi marcado em cada dia fica em `data/dias/<dia>.txt` (uma chave por linha).

use crate::data::today;
use crate::home_dir;
use std::collections::HashSet;
use std::path::PathBuf;

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
    pub fn time_label(&self) -> String {
        self.at.map(|m| format!("{:02}:{:02}", m / 60, m % 60)).unwrap_or_default()
    }
}

pub fn parse(src: &str) -> Vec<Item> {
    src.lines()
        .filter_map(|l| {
            let l = l.trim();
            let (mark, rest) = (l.chars().next()?, l.get(1..)?.trim());
            if rest.is_empty() {
                return None;
            }
            match mark {
                '!' => Some(Item { kind: Kind::Priority, text: rest.into(), at: None }),
                '-' => Some(Item { kind: Kind::Task, text: rest.into(), at: None }),
                '*' => Some(Item { kind: Kind::Habit, text: rest.into(), at: None }),
                '@' => {
                    let (time, text) = rest.split_once(' ')?;
                    let (h, m) = time.split_once(':')?;
                    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
                    (h < 24 && m < 60).then(|| Item { kind: Kind::Reminder, text: text.trim().into(), at: Some(h * 60 + m) })
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

pub fn load_items() -> Vec<Item> {
    std::fs::read_to_string(plan_path()).map(|s| parse(&s)).unwrap_or_default()
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

/// Dia completo: todas as prioridades e tarefas marcadas (e havia alguma).
fn day_complete(items: &[Item], done: &HashSet<String>) -> bool {
    let todo: Vec<_> = items.iter().filter(|i| matches!(i.kind, Kind::Priority | Kind::Task)).collect();
    !todo.is_empty() && todo.iter().all(|i| done.contains(&i.key()))
}

/// Dias seguidos até hoje (hoje conta se já cumprido; se ainda não, a sequência vem de ontem).
fn streak(mut ok: impl FnMut(i64) -> bool, today: i64) -> u32 {
    let start = if ok(today) { today } else { today - 1 };
    let mut n = 0;
    while n < 3650 && ok(start - n as i64) {
        n += 1;
    }
    n
}

pub fn todo_streak(items: &[Item], offset_min: i64) -> u32 {
    streak(|d| day_complete(items, &checked(d)), today(offset_min))
}

pub fn habit_streak(key: &str, offset_min: i64) -> u32 {
    streak(|d| checked(d).contains(key), today(offset_min))
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

pub fn reading_streak(offset_min: i64) -> u32 {
    let goal = reading_goal_min() as i64 * 60;
    let mut per_day = std::collections::HashMap::<i64, i64>::new();
    if let Ok(s) = std::fs::read_to_string(home_dir().join("data/reading.log")) {
        for l in s.lines() {
            let mut it = l.split_whitespace();
            if let (Some(Ok(start)), Some(Ok(dur))) = (it.next().map(str::parse::<i64>), it.next().map(str::parse::<i64>)) {
                *per_day.entry(crate::data::day_of(start, offset_min)).or_default() += dur.max(0);
            }
        }
    }
    streak(|d| per_day.get(&d).copied().unwrap_or(0) >= goal, today(offset_min))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn le_o_formato() {
        let it = parse("# comentário\n! Relatório\n- Pão\n@ 14:30 Reunião com a equipe\n@ 25:00 inválido\n* Ler 20 min\n-\n");
        assert_eq!(it.len(), 4);
        assert_eq!(it[2].at, Some(14 * 60 + 30));
        assert_eq!(it[2].text, "Reunião com a equipe");
        assert_eq!(it[3].kind, Kind::Habit);
    }
    #[test]
    fn sequencia_conta_hoje_ou_vem_de_ontem() {
        let feitos = [10, 9, 8, 6];
        assert_eq!(streak(|d| feitos.contains(&d), 10), 3);
        assert_eq!(streak(|d| feitos.contains(&d), 11), 3); // hoje (11) ainda não, vale até ontem
        assert_eq!(streak(|d| feitos.contains(&d), 12), 0);
    }
}
