//! Busca por posição nos dicionários TSV ordenados, sem carregar o arquivo inteiro.
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

#[derive(Clone, Debug)]
pub struct Verbete { pub palavra: String, pub definicao: String, pub portugues: bool }

fn chave(s: &str) -> String {
    s.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase().nfd().filter(|c| !is_combining_mark(*c)).collect()
}

fn variantes(s: &str) -> Vec<String> {
    let palavra = s.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase();
    let mut v = vec![chave(&palavra)];
    for (sufixo, troca) in [("ões", "ão"), ("inho", "o"), ("inha", "a"), ("es", ""), ("s", "")] {
        if let Some(raiz) = palavra.strip_suffix(sufixo) {
            if raiz.chars().count() >= 2 { let x = chave(&format!("{raiz}{troca}")); if !v.contains(&x) { v.push(x); } }
        }
    }
    v.retain(|x| !x.is_empty());
    v
}

/// A cada passo, alinha ao início da próxima linha e mantém a metade pertinente.
fn buscar(path: &Path, chave: &str) -> std::io::Result<Option<Verbete>> {
    let mut f = BufReader::new(std::fs::File::open(path)?);
    let mut lo = 0;
    let mut hi = f.get_ref().metadata()?.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        f.seek(SeekFrom::Start(mid.saturating_sub(1)))?;
        if mid > 0 { let mut resto = Vec::new(); f.read_until(b'\n', &mut resto)?; }
        let inicio = f.stream_position()?;
        if inicio >= hi { hi = mid; continue; }
        let mut linha = String::new();
        f.read_line(&mut linha)?;
        let fim = f.stream_position()?;
        let Some((k, resto)) = linha.trim_end_matches(['\r', '\n']).split_once('\t') else { lo = fim; continue };
        match k.cmp(chave) {
            std::cmp::Ordering::Less => lo = fim,
            std::cmp::Ordering::Greater => hi = mid,
            std::cmp::Ordering::Equal => {
                return Ok(resto.split_once('\t').map(|(palavra, definicao)| Verbete { palavra: palavra.to_string(), definicao: definicao.to_string(), portugues: false }));
            }
        }
    }
    Ok(None)
}

fn diretorio() -> PathBuf {
    let local = crate::home_dir().join("dict");
    if local.is_dir() || Path::new("/mnt/us/verso-ink").is_dir() { local } else { Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dict") }
}

pub fn consultar(palavra: &str, ingles: bool) -> Option<Verbete> {
    consultar_em(&diretorio(), palavra, ingles)
}

fn consultar_em(dir: &Path, palavra: &str, ingles: bool) -> Option<Verbete> {
    let variantes = variantes(palavra);
    let fontes = if ingles { ["en-pt.tsv", "pt.tsv"] } else { ["pt.tsv", "en-pt.tsv"] };
    for fonte in fontes {
        for variante in &variantes {
            if let Ok(Some(mut resultado)) = buscar(&dir.join(fonte), variante) {
                resultado.portugues = fonte == "pt.tsv";
                return Some(resultado);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busca_nos_dicionarios_reais_quando_disponiveis() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dict");
        if !dir.join("pt.tsv").is_file() || !dir.join("en-pt.tsv").is_file() { return; }
        assert_eq!(consultar_em(&dir, "corações", false).unwrap().palavra, "Coração");
        assert_eq!(consultar_em(&dir, "house", false).unwrap().palavra, "house");
        assert_eq!(consultar_em(&dir, "casinha", false).unwrap().palavra, "Casinha");
    }

    #[test]
    fn busca_ordenada_por_bytes_e_flexoes() {
        let dir = std::env::temp_dir().join(format!("verso-dicionario-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pt.tsv"), "acao\tAção\tAto de agir\ncasa\tCasa\tMoradia\ncoracao\tCoração\tÓrgão\nflor\tFlor\tParte da planta\nultimo\tÚltimo\tNo fim\n").unwrap();
        std::fs::write(dir.join("en-pt.tsv"), "house\thouse\tcasa\n").unwrap();
        for (p, esperado) in [("AÇÃO", "Ação"), ("casinha", "Casa"), ("corações", "Coração"), ("flores", "Flor"), ("último", "Último"), ("house", "house")] {
            assert_eq!(consultar_em(&dir, p, false).unwrap().palavra, esperado, "{p}");
        }
        assert!(consultar_em(&dir, "inexistente", false).is_none());
        std::fs::write(dir.join("en-pt.tsv"), "casa\tcasa em inglês\tteste\nhouse\thouse\tcasa\n").unwrap();
        assert_eq!(consultar_em(&dir, "casa", true).unwrap().palavra, "casa em inglês");
        let _ = std::fs::remove_dir_all(dir);
    }
}
