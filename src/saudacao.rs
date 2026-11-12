//! Frase da tela inicial: um toque pessoal que muda com a hora, o dia, o clima e o que você
//! anda fazendo no Kindle. O nome vem de settings/nome (sem ele, as frases ficam sem nome).
//! A escolha é fixa durante a hora (não pisca a cada minuto) e varia de hora em hora.

/// O que a frase pode levar em conta.
pub struct Context {
    pub hour: u32,
    pub weekday: usize,
    pub day: i64,
    pub raining: bool,
    pub book: Option<(String, Option<f32>)>,
    pub game: Option<String>,
    pub read_today: u32,
    pub edition: bool,
    pub notice: bool,
    pub battery: Option<u8>,
    /// Já treinou violão hoje (e quantos dias seguidos).
    pub guitar: Option<(bool, u32)>,
    /// Aprendizado: já fez a lição de hoje e a ofensiva em dias.
    pub learn: Option<(bool, u32)>,
}

pub fn name() -> Option<String> {
    std::fs::read_to_string(crate::home_dir().join("settings/nome")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Troca {nome}, {livro} e {jogo}; "{nome}" some com a vírgula quando não há nome.
fn fill(t: &str, c: &Context, nome: Option<&str>) -> String {
    let mut s = t.to_string();
    match nome {
        Some(n) => s = s.replace("{nome}", n),
        None => {
            s = s.replace(", {nome}", "").replace("{nome}, ", "").replace(" {nome}", "").replace("{nome}", "");
            // Sem nome, a primeira letra volta a ser maiúscula.
            let mut ch = s.chars();
            if let Some(first) = ch.next() {
                s = first.to_uppercase().collect::<String>() + ch.as_str();
            }
        }
    }
    let livro = c.book.as_ref().map(|(t, _)| short_title(t)).unwrap_or_default();
    s.replace("{livro}", &livro).replace("{jogo}", c.game.as_deref().unwrap_or("")).replace("{min}", &c.read_today.to_string()).replace("{dias}", &c.learn.map(|(_, n)| n).unwrap_or(0).to_string())
}

fn short_title(t: &str) -> String {
    let t = t.split(" - ").next().unwrap_or(t).trim_end_matches(" PDF").trim();
    if t.chars().count() > 28 { format!("{}…", t.chars().take(27).collect::<String>()) } else { t.to_string() }
}

/// Frases candidatas para o momento, das mais específicas às gerais.
fn candidates(c: &Context) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    if c.notice {
        v.extend(["Tem um aviso para você, {nome}. Dá uma olhada.", "Um agente deixou um recado, {nome}."]);
        return v;
    }
    if c.battery.is_some_and(|b| b <= 15) {
        v.extend(["Bateria no fim, {nome}. Que tal carregar enquanto lê?", "Pouca bateria: hora de achar a tomada, {nome}."]);
        return v;
    }
    let weekend = c.weekday == 0 || c.weekday == 6;
    match c.hour {
        0..=4 => v.extend([
            "Ainda acordado, {nome}? Um capítulo e cama.",
            "Madrugada boa para uma partida rápida, {nome}.",
            "O mundo dorme. O que vamos fazer, {nome}?",
            "Silêncio total. Ler ou jogar, {nome}?",
        ]),
        5..=11 => {
            v.extend(["Bom dia, {nome}! O que vamos fazer hoje?", "Bom dia, {nome}. Por onde começamos?", "Café na mão? Bom dia, {nome}."]);
            if c.edition {
                v.extend(["Bom dia, {nome}! A edição de hoje já chegou.", "Sua edição da manhã está pronta, {nome}."]);
            }
            if weekend {
                v.extend(["Manhã de folga, {nome}. Sem pressa hoje.", "Sábado de manhã pede um livro, {nome}."]);
            }
        }
        12..=17 => {
            v.extend(["Boa tarde, {nome}! O que vamos fazer agora?", "Uma pausa na tarde, {nome}?", "Boa tarde, {nome}. Dez minutos só para você?"]);
            if weekend {
                v.push("Tarde livre, {nome}. Uma partida ou um capítulo?");
            }
        }
        _ => {
            v.extend(["Boa noite, {nome}. Como foi o dia?", "Fim de dia, {nome}. Um capítulo antes de dormir?", "Boa noite, {nome}! Ler um pouco ou jogar?"]);
            if !weekend && c.weekday == 5 {
                v.push("Sexta à noite, {nome}! O fim de semana começou.");
            }
        }
    }
    if c.raining {
        v.extend(["Chuva lá fora, {nome}. Tempo de livro.", "Dia de chuva combina com leitura, {nome}."]);
    }
    if let Some((_, p)) = &c.book {
        match p {
            Some(p) if *p > 0.85 => v.push("Falta pouco para terminar {livro}, {nome}!"),
            Some(p) if *p > 0.0 => v.extend(["{livro} está esperando você, {nome}.", "Vamos continuar {livro}, {nome}?"]),
            _ => v.push("Que tal começar {livro} hoje, {nome}?"),
        }
    }
    if let Some(g) = &c.game {
        if !g.is_empty() {
            v.extend(["A partida de {jogo} ficou pela metade, {nome}.", "Revanche no {jogo}, {nome}?"]);
        }
    }
    if let Some((done, streak)) = c.guitar {
        if !done && (17..=22).contains(&c.hour) {
            v.extend(["Já pegou o violão hoje, {nome}?", "Um minuto de trocas no violão, {nome}?"]);
            if streak >= 2 {
                v.push("Não quebra a sequência do violão, {nome}!");
            }
        }
    }
    if let Some((done, streak)) = c.learn {
        if !done && streak >= 2 && c.hour >= 17 {
            // Ofensiva em risco: vira prioridade da frase.
            return vec!["Sua ofensiva de {dias} dias espera uma lição, {nome}.", "Não deixa a chama apagar: {dias} dias, {nome}!", "Uma lição rápida segura os {dias} dias, {nome}."];
        }
        if done && streak >= 3 {
            v.push("{dias} dias seguidos aprendendo. Bonito, {nome}!");
        }
    }
    if c.read_today >= 20 {
        v.push("Já foram {min} minutos de leitura hoje. Bonito, {nome}!");
    }
    v
}

/// A frase do momento (estável durante a hora).
pub fn phrase(c: &Context) -> String {
    let list = candidates(c);
    if list.is_empty() {
        return String::new();
    }
    // Mistura dia e hora para não repetir sempre a mesma no mesmo horário.
    let seed = (c.day as u64).wrapping_mul(2654435761).wrapping_add(c.hour as u64 * 40503);
    let k = (seed ^ (seed >> 13)) as usize % list.len();
    fill(list[k], c, name().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(hour: u32) -> Context {
        Context { hour, weekday: 3, day: 20000, raining: false, book: Some(("Perfil de três reis - Gene Edwards PDF".into(), Some(0.4))), game: Some("Xadrez".into()), read_today: 5, edition: true, notice: false, battery: Some(80), guitar: Some((false, 3)), learn: Some((false, 4)) }
    }

    #[test]
    fn frases_preenchidas_e_sem_nome() {
        for h in 0..24 {
            let c = ctx(h);
            for t in candidates(&c) {
                let com = fill(t, &c, Some("Pedro"));
                let sem = fill(t, &c, None);
                assert!(!com.contains('{') && !sem.contains('{'), "{t}");
                assert!(!sem.contains(", .") && !sem.contains(" ,") && !sem.starts_with(','), "{sem}");
                assert!(sem.chars().next().unwrap().is_uppercase() || sem.starts_with('{'), "{sem}");
                assert!(com.contains("Pedro"), "{com}");
            }
        }
        assert!(fill("{livro} está esperando você, {nome}.", &ctx(9), None).starts_with("Perfil de três reis"));
    }
}
