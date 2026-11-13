//! Próximo lembrete para despertar o Kindle pelo RTC.

use crate::day::{Item, Kind};
use std::collections::HashSet;
use std::io;
use std::path::Path;

/// Permite ao laço distinguir uma hora futura de um aviso que já precisa ser exibido.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proximo {
    Vencido,
    Futuro(i64),
    Nenhum,
}

fn read_optional(path: &Path) -> io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        value => value,
    }
}

fn checked_at(root: &Path, day: i64) -> io::Result<HashSet<String>> {
    let path = root.join("data/dias").join(format!("{day}.txt"));
    Ok(read_optional(&path)?.lines().map(str::to_string).collect())
}

/// Próxima ocorrência em UTC, considerando o fuso local e os dias programados.
fn calcular(items: &[Item], avulsos: &[crate::lembretes_ui::Avulso], now: i64, offset_min: i64, mut checked: impl FnMut(i64) -> io::Result<HashSet<String>>) -> io::Result<Proximo> {
    let mut next = avulsos.iter().map(|item| item.epoch).min();
    if next.is_some_and(|time| time <= now) { return Ok(Proximo::Vencido); }
    let offset = offset_min * 60;
    let today = (now + offset).div_euclid(86_400);
    let reminders: Vec<_> = items.iter().filter(|item| item.kind == Kind::Reminder && item.at.is_some_and(|m| m < 1440)).collect();
    if !reminders.is_empty() {
        for day in today..=today + 7 {
            if !reminders.iter().any(|item| item.on(day)) { continue; }
            let done = checked(day)?;
            for item in reminders.iter().filter(|item| item.on(day) && !done.contains(&item.key())) {
                let epoch = day * 86_400 - offset + i64::from(item.at.unwrap()) * 60;
                if epoch <= now { return Ok(Proximo::Vencido); }
                next = Some(next.map_or(epoch, |old| old.min(epoch)));
            }
            // Todo horário num dia posterior vem depois do primeiro horário já encontrado.
            if next.is_some_and(|epoch| epoch < (day + 1) * 86_400 - offset) { break; }
        }
    }
    Ok(next.map_or(Proximo::Nenhum, Proximo::Futuro))
}

/// Reavalia o disco: erro de leitura é diferente de não haver lembretes.
/// Uma data implausível não deve programar nem liberar o repouso silenciosamente.
pub fn proximo(now: i64, offset_min: i64) -> io::Result<Proximo> {
    let root = crate::home_dir();
    let plan = read_optional(&root.join("data/dia.txt"))?;
    let items = crate::day::parse(&plan);
    let avulsos = crate::lembretes_ui::tentar_pendentes()?;
    if avulsos.is_empty() && !items.iter().any(|item| item.kind == Kind::Reminder && item.at.is_some()) { return Ok(Proximo::Nenhum); }
    if now < 1_735_689_600 { return Err(io::Error::new(io::ErrorKind::InvalidData, "Relógio ainda não ajustado.")); }
    calcular(&items, &avulsos, now, offset_min, |day| checked_at(&root, day))
}

/// Programa o RTC no Kindle; no computador, apenas calcula o estado.
/// Uma ocorrência vencida deve ser exibida antes de permitir a suspensão.
pub fn programar(offset_min: i64) -> io::Result<Proximo> {
    let next = proximo(crate::data::epoch(), offset_min)?;
    if !cfg!(target_arch = "arm") || next == Proximo::Vencido { return Ok(next); }
    let rtc = Path::new("/sys/class/rtc/rtc0/wakealarm");
    if !rtc.exists() { return Ok(next); }
    std::fs::write(rtc, "0")?;
    if let Proximo::Futuro(epoch) = next { std::fs::write(rtc, epoch.to_string())?; }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lembretes_ui::Avulso;

    #[test]
    fn horario_local_pendente_e_repeticao() {
        let items = crate::day::parse("@ 08:30 Remédio\n@ {seg} 09:00 Consulta\n");
        let offset = -240;
        let monday = 20_766;
        assert_eq!(crate::day::weekday(monday), 0);
        let base = monday * 86_400 - offset * 60;
        let empty = |_| Ok(HashSet::new());
        assert_eq!(calcular(&items, &[], base + 8 * 3600, offset, empty).unwrap(), Proximo::Futuro(base + 8 * 3600 + 30 * 60));
        assert_eq!(calcular(&items, &[], base + 8 * 3600 + 31 * 60, offset, empty).unwrap(), Proximo::Vencido);
        assert_eq!(calcular(&items, &[], base + 8 * 3600 + 31 * 60, offset, |d| Ok(if d == monday { ["@ Remédio".to_string()].into_iter().collect() } else { HashSet::new() })).unwrap(), Proximo::Futuro(base + 9 * 3600));
    }

    #[test]
    fn compara_avulsos_e_recorrentes_e_trata_vencido() {
        let items = crate::day::parse("@ 09:00 Consulta\n");
        let day = 20_000;
        let now = day * 86_400 + 8 * 3600;
        let avulsos = [Avulso { epoch: now + 1800, texto: "Ligação".into() }];
        let empty = |_| Ok(HashSet::new());
        assert_eq!(calcular(&items, &avulsos, now, 0, empty).unwrap(), Proximo::Futuro(now + 1800));
        assert_eq!(calcular(&items, &avulsos, now + 1800, 0, empty).unwrap(), Proximo::Vencido);
        assert_eq!(calcular(&items, &[], now, 0, empty).unwrap(), Proximo::Futuro(now + 3600));
        assert_eq!(calcular(&[], &[], now, 0, empty).unwrap(), Proximo::Nenhum);
    }

    #[test]
    fn falha_real_de_leitura_nao_vira_lista_vazia() {
        let root = std::env::temp_dir().join(format!("verso-rtc-leitura-{}", std::process::id()));
        let dir = root.join("data/dias");
        std::fs::create_dir_all(&dir).unwrap();
        let day = 20_000;
        let path = dir.join(format!("{day}.txt"));
        std::fs::write(&path, b"@ Consulta\n\xff").unwrap();
        let items = crate::day::parse("@ 09:00 Consulta\n");
        assert!(calcular(&items, &[], day * 86_400 + 8 * 3600, 0, |d| checked_at(&root, d)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"@ Consulta\n\xff");
        std::fs::remove_file(path).unwrap();
        assert_eq!(calcular(&items, &[], day * 86_400 + 8 * 3600, 0, |d| checked_at(&root, d)).unwrap(), Proximo::Futuro(day * 86_400 + 9 * 3600));
        let _ = std::fs::remove_dir_all(root);
    }
}
