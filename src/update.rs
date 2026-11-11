//! Atualização pelo próprio Kindle: procura a última versão publicada no GitHub,
//! baixa, confere o SHA-256 e instala; o session.sh reinicia o Verso Ink (saída 43).

use crate::sys::{log, sh};

pub const REPO: &str = "pedronvitoriano/verso-ink";
pub const ASSET: &str = "verso-ink-kindle.tar.gz";
pub const EXIT_UPDATED: i32 = 43;

pub enum Check {
    UpToDate(String),
    Available { version: String, url: String, sha_url: String },
    Failed(String),
}

/// Primeiro valor de texto de `"chave": "valor"` a partir de `from`.
fn str_after(json: &str, from: usize, key: &str) -> Option<String> {
    let rest = &json[from..];
    let i = rest.find(&format!("\"{key}\""))?;
    let after = &rest[i + key.len() + 2..];
    let q1 = after.find('"')? + 1;
    let q2 = after[q1..].find('"')? + q1;
    Some(after[q1..q2].to_string())
}

fn newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| v.trim_start_matches('v').split('.').map(|p| p.parse::<u32>().unwrap_or(0)).collect::<Vec<_>>();
    parse(remote) > parse(local)
}

pub fn check() -> Check {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let Some(json) = sh("curl", &["-sL", "-m", "30", "-H", "Accept: application/vnd.github+json", &url]) else {
        return Check::Failed("Sem resposta do GitHub. Confira o Wi-Fi.".into());
    };
    let Some(tag) = str_after(&json, 0, "tag_name") else {
        return Check::Failed("Nenhuma versão publicada ainda.".into());
    };
    let local = env!("CARGO_PKG_VERSION");
    if !newer(&tag, local) {
        return Check::UpToDate(local.into());
    }
    let find = |name: &str| {
        let i = json.find(&format!("\"name\": \"{name}\"")).or_else(|| json.find(&format!("\"name\":\"{name}\"")))?;
        str_after(&json, i, "browser_download_url")
    };
    match (find(ASSET), find(&format!("{ASSET}.sha256"))) {
        (Some(url), Some(sha_url)) => Check::Available { version: tag.trim_start_matches('v').into(), url, sha_url },
        _ => Check::Failed(format!("A versão {tag} não tem o pacote para Kindle.")),
    }
}

/// Baixa, confere e instala em /mnt/us. Devolve a mensagem de erro, se houver.
pub fn install(url: &str, sha_url: &str) -> Result<(), String> {
    let tmp = "/tmp/verso-ink-update.tar.gz";
    let _ = std::fs::remove_file(tmp);
    sh("curl", &["-sL", "-m", "120", "-o", tmp, url]).ok_or("Falha ao baixar o pacote.")?;
    let expected = sh("curl", &["-sL", "-m", "30", sha_url]).ok_or("Falha ao baixar a soma de verificação.")?;
    let expected = expected.split_whitespace().next().unwrap_or("").to_lowercase();
    let got = sh("sha256sum", &[tmp]).and_then(|s| s.split_whitespace().next().map(str::to_string)).unwrap_or_default();
    if expected.len() != 64 || got != expected {
        log(&format!("atualização: sha256 não confere ({got} != {expected})"));
        return Err("O pacote baixado não confere. Nada foi instalado.".into());
    }
    sh("tar", &["-xzf", tmp, "-C", "/mnt/us"]).ok_or("Falha ao extrair o pacote.")?;
    let _ = sh("sh", &["-c", "chmod +x /mnt/us/verso-ink/bin/*; sync"]);
    log("atualização instalada");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compara_versoes_e_le_o_json() {
        assert!(newer("v0.5.0", "0.4.0"));
        assert!(newer("0.10.0", "0.9.9"));
        assert!(!newer("v0.4.0", "0.4.0"));
        let j = r#"{"tag_name": "v0.5.0", "assets": [{"name": "verso-ink-kindle.tar.gz", "browser_download_url": "https://x/a.tar.gz"}]}"#;
        assert_eq!(str_after(j, 0, "tag_name").as_deref(), Some("v0.5.0"));
        let i = j.find("\"name\": \"verso-ink-kindle.tar.gz\"").unwrap();
        assert_eq!(str_after(j, i, "browser_download_url").as_deref(), Some("https://x/a.tar.gz"));
    }
}
