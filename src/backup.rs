//! Cópia de segurança diária dos dados e ajustes (fica no próprio Kindle, em `backups/`, as 7
//! mais novas) e aviso de versão nova do Verso Ink (só avisa; instalar é pelo Ajustes).

use crate::home_dir;
use std::path::PathBuf;

const KEEP: usize = 7;

fn backups_dir() -> PathBuf {
    home_dir().join("backups")
}

/// Faz a cópia do dia, se ainda não houver (a partir das 3h, ou na primeira vez acordado depois).
pub fn maybe_backup(offset_min: i64) -> Option<String> {
    let now = crate::clock::now(offset_min);
    if !now.plausible() || now.hour < 3 {
        return None;
    }
    let name = format!("{:04}-{:02}-{:02}.tar.gz", now.year, now.month, now.day);
    let dir = backups_dir();
    if dir.join(&name).exists() {
        return None;
    }
    let _ = std::fs::create_dir_all(&dir);
    let tmp = dir.join(format!("{name}.tmp"));
    let home = home_dir();
    let ok = std::process::Command::new("tar")
        .args(["czf", &tmp.to_string_lossy(), "-C", &home.to_string_lossy(), "data", "settings"])
        .status()
        .is_ok_and(|s| s.success());
    if !ok || std::fs::rename(&tmp, dir.join(&name)).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    prune(&dir);
    Some(name)
}

/// Apaga as cópias mais antigas, ficando com as `KEEP` mais novas (só arquivos .tar.gz desta pasta).
fn prune(dir: &std::path::Path) {
    let mut v = list(dir);
    while v.len() > KEEP {
        let old = v.remove(0);
        let _ = std::fs::remove_file(dir.join(old));
    }
}

/// Nomes das cópias, da mais antiga para a mais nova (o nome é a data).
fn list(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|r| r.flatten().filter_map(|e| e.file_name().into_string().ok()).filter(|n| n.ends_with(".tar.gz") && n.len() == 17).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// "11/11 · 7 guardadas" para o Ajustes.
pub fn summary() -> String {
    let v = list(&backups_dir());
    match v.last() {
        None => "ainda nenhuma".into(),
        Some(last) => format!("{}/{} · {} guardada{}", &last[8..10], &last[5..7], v.len(), if v.len() == 1 { "" } else { "s" }),
    }
}

// ---------- Versão nova ----------

fn state(name: &str) -> PathBuf {
    let d = home_dir().join("state");
    let _ = std::fs::create_dir_all(&d);
    d.join(name)
}

/// Versão nova já encontrada (e ainda não instalada).
pub fn update_available() -> Option<String> {
    let v = std::fs::read_to_string(state("versao_nova")).ok()?;
    let v = v.trim().to_string();
    crate::update::newer(&v, env!("CARGO_PKG_VERSION")).then_some(v)
}

/// Procura versão nova uma vez por dia, em segundo plano (a consulta pode levar até 30 s).
pub fn maybe_check_update(today: i64) {
    let stamp = state("versao_conferida");
    if std::fs::read_to_string(&stamp).ok().and_then(|s| s.trim().parse::<i64>().ok()) == Some(today) {
        return;
    }
    let _ = std::fs::write(&stamp, today.to_string());
    std::thread::spawn(|| match crate::update::check() {
        crate::update::Check::Available { version, .. } => {
            let _ = std::fs::write(state("versao_nova"), version);
        }
        crate::update::Check::UpToDate(_) => {
            let _ = std::fs::remove_file(state("versao_nova"));
        }
        crate::update::Check::Failed(_) => {
            // Sem rede ou sem resposta: tenta de novo amanhã (ou pelo Ajustes).
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn guarda_so_as_sete_mais_novas() {
        let dir = std::env::temp_dir().join(format!("verso-ink-backup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for d in 1..=9 {
            std::fs::write(dir.join(format!("2026-11-{d:02}.tar.gz")), "x").unwrap();
        }
        std::fs::write(dir.join("outra-coisa.txt"), "x").unwrap();
        prune(&dir);
        let v = list(&dir);
        assert_eq!(v.len(), 7);
        assert_eq!(v[0], "2026-11-03.tar.gz");
        assert!(dir.join("outra-coisa.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
