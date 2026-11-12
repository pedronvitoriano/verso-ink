//! Frase da tela inicial: um toque pessoal que muda com a hora, o dia, o clima e o que você
//! anda fazendo no Kindle. Muitas trazem um botão que abre a tela certa ("Abrir a aula ›").
//! De vez em quando aparece um achado raro (e, bem de vez em quando, um lendário): guarde na
//! coleção; alguns dão bônus de verdade (vida no Coroas, protetor da ofensiva, XP).
//!
//! O nome vem de settings/nome (sem ele, as frases ficam sem nome). A escolha é fixa durante a
//! hora (não pisca a cada minuto) e varia de hora em hora.

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
    /// Destaques esperando revisão hoje.
    pub review_due: usize,
}

/// Para onde o botão da frase leva.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Act {
    None,
    Aprender,
    Aula,
    Revisao,
    Edicao,
    Semana,
    Jogo,
    Ler,
    Foco,
    Claude,
    Hoje,
    Jogar,
    Colecao,
    /// Achado raro (índice em ITEMS): o botão guarda na coleção.
    Achado(usize),
}

impl Act {
    pub fn label(self) -> &'static str {
        match self {
            Act::None => "",
            Act::Aprender => "Estudar agora ›",
            Act::Aula => "Abrir a aula ›",
            Act::Revisao => "Revisar ›",
            Act::Edicao => "Ler a edição ›",
            Act::Semana => "Fechar a semana ›",
            Act::Jogo => "Voltar ao jogo ›",
            Act::Ler => "Continuar lendo ›",
            Act::Foco => "Começar um foco ›",
            Act::Claude => "Chamar o Claude ›",
            Act::Hoje => "Ver o meu dia ›",
            Act::Jogar => "Escolher um jogo ›",
            Act::Colecao => "Ver a coleção ›",
            Act::Achado(_) => "Guardar na coleção ›",
        }
    }
}

/// Raridade da frase escolhida.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Rarity {
    Common,
    Rare,
    Legendary,
}

pub struct Phrase {
    pub text: String,
    pub act: Act,
    pub rarity: Rarity,
}

/// Bônus de um achado.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Bonus {
    None,
    /// Uma vida a mais no Coroas.
    Life,
    /// Um protetor da ofensiva.
    Shield,
    /// XP para o dia.
    Xp(u32),
}

/// Achados: (nome, frase de quando aparece, desenho, bônus, lendário?).
pub const ITEMS: [(&str, &str, &str, Bonus, bool); 16] = [
    ("Palheta dourada", "Uma palheta dourada caiu entre as páginas. Dizem que afina sozinha.", "achado-palheta", Bonus::Xp(5), false),
    ("Coroa perdida", "Achei uma coroa esquecida num reino do Coroas. Ninguém reclamou.", "peca-coroa", Bonus::None, false),
    ("Coração extra", "Um coração extra rolou para fora do Coroas. Vale uma vida.", "coracao", Bonus::Life, false),
    ("Escudo de gelo", "Um escudo de gelo apareceu. Ele segura a ofensiva num dia ruim.", "escudo", Bonus::Shield, false),
    ("Bandeira fincada", "Alguém fincou uma bandeira bem aqui. Pelo menos não era mina.", "peca-bandeira", Bonus::None, false),
    ("Rei de marfim", "Um rei de marfim fugiu do tabuleiro e veio parar na sua tela.", "xadrez-rei-b", Bonus::None, false),
    ("Clave de prata", "Uma clave de sol de prata. Agora a pauta inteira te deve um favor.", "clave-sol", Bonus::Xp(5), false),
    ("Alvo certeiro", "Um alvo com a flecha no meio. Hoje a mira está boa.", "alvo", Bonus::Xp(5), false),
    ("Mina desarmada", "Uma mina desarmada, de lembrança. Pode guardar, não explode.", "peca-mina", Bonus::None, false),
    ("Copas da sorte", "Um ás de copas escapou do baralho da Paciência.", "naipe-copas", Bonus::None, false),
    ("Disco da vitória", "Um disco do Lig 4 rolou até aqui, ainda quente da última partida.", "peca-disco-p", Bonus::None, false),
    ("Livro sem fim", "Um livro que nunca acaba apareceu. Só não abra antes de dormir.", "icone-livro", Bonus::Xp(5), false),
    ("Chama eterna", "LENDÁRIO: a chama eterna. Ela não apaga, e ainda vale 20 XP.", "chama", Bonus::Xp(20), true),
    ("Coroa do rei antigo", "LENDÁRIO: a coroa do rei antigo. Dizem que só aparece uma vez por ano.", "peca-coroa", Bonus::Life, true),
    ("Violão de cristal", "LENDÁRIO: um violão de cristal. Toca qualquer acorde sem errar.", "icone-violao", Bonus::Shield, true),
    ("Kindle de ouro", "LENDÁRIO: um Kindle de ouro. Este aqui já é especial, mas olha só.", "icone-ler", Bonus::Xp(20), true),
];

pub fn name() -> Option<String> {
    std::fs::read_to_string(crate::home_dir().join("settings/nome")).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

// ---------- Coleção ----------

fn coll_path() -> std::path::PathBuf {
    crate::home_dir().join("data/achados.txt")
}

/// Achados guardados: (índice, dia em que achou).
pub fn collected() -> Vec<(usize, i64)> {
    std::fs::read_to_string(coll_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (a, b) = l.split_once('|')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .filter(|(i, _)| *i < ITEMS.len())
        .collect()
}

/// Guarda o achado e aplica o bônus. Devolve false se já estava na coleção hoje.
pub fn collect(i: usize, day: i64) -> bool {
    if i >= ITEMS.len() || collected().iter().any(|(k, d)| *k == i && *d == day) {
        return false;
    }
    let mut s = std::fs::read_to_string(coll_path()).unwrap_or_default();
    s.push_str(&format!("{i}|{day}\n"));
    let _ = std::fs::create_dir_all(crate::home_dir().join("data"));
    let _ = std::fs::write(coll_path(), s);
    match ITEMS[i].3 {
        Bonus::Life => crate::coroas::bonus_life(),
        Bonus::Shield => crate::aprender::add_freeze(),
        Bonus::Xp(n) => {
            let _ = crate::aprender::add(n, "achado");
        }
        Bonus::None => {}
    }
    true
}

pub fn bonus_text(b: Bonus) -> String {
    match b {
        Bonus::None => "Só para a coleção.".into(),
        Bonus::Life => "Bônus: +1 vida no Coroas.".into(),
        Bonus::Shield => "Bônus: +1 protetor da ofensiva.".into(),
        Bonus::Xp(n) => format!("Bônus: +{n} XP hoje."),
    }
}

// ---------- Frases ----------

/// Troca {nome}, {livro}, {jogo}, {min}, {dias} e {rev}; "{nome}" some com a vírgula
/// quando não há nome.
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
    s.replace("{livro}", &livro)
        .replace("{jogo}", c.game.as_deref().unwrap_or(""))
        .replace("{min}", &c.read_today.to_string())
        .replace("{dias}", &c.learn.map(|(_, n)| n).unwrap_or(0).to_string())
        .replace("{rev}", &c.review_due.to_string())
}

fn short_title(t: &str) -> String {
    let t = t.split(" - ").next().unwrap_or(t).trim_end_matches(" PDF").trim();
    if t.chars().count() > 28 { format!("{}…", t.chars().take(27).collect::<String>()) } else { t.to_string() }
}

type C = (&'static str, Act);

/// Frases candidatas para o momento, das mais específicas às gerais.
fn candidates(c: &Context) -> Vec<C> {
    use Act::*;
    let mut v: Vec<C> = Vec::new();
    if c.notice {
        v.extend([("Tem um aviso para você, {nome}. Dá uma olhada.", Edicao), ("Um agente deixou um recado, {nome}.", Edicao)]);
        return v;
    }
    if c.battery.is_some_and(|b| b <= 15) {
        v.extend([("Bateria no fim, {nome}. Que tal carregar enquanto lê?", None), ("Pouca bateria: hora de achar a tomada, {nome}.", None)]);
        return v;
    }
    // Ofensiva em risco à noite: vira prioridade.
    if let Some((done, streak)) = c.learn {
        if !done && streak >= 2 && c.hour >= 17 {
            return vec![
                ("Sua ofensiva de {dias} dias espera uma lição, {nome}.", Aprender),
                ("Não deixa a chama apagar: {dias} dias, {nome}!", Aprender),
                ("Uma lição rápida segura os {dias} dias, {nome}.", Aprender),
                ("A chama está tremendo, {nome}. Cinco minutos resolvem.", Aprender),
            ];
        }
    }
    let weekend = c.weekday == 0 || c.weekday == 6;
    match c.hour {
        0..=4 => v.extend([
            ("Ainda acordado, {nome}? Um capítulo e cama.", Ler),
            ("Madrugada boa para uma partida rápida, {nome}.", Jogar),
            ("O mundo dorme. O que vamos fazer, {nome}?", None),
            ("Silêncio total. Ler ou jogar, {nome}?", None),
            ("Até as corujas foram dormir, {nome}.", None),
            ("Hora perfeita para um sudoku sem testemunhas.", Jogar),
            ("Os agentes trabalham de madrugada. E você, {nome}?", Edicao),
        ]),
        5..=11 => {
            v.extend([
                ("Bom dia, {nome}! O que vamos fazer hoje?", Hoje),
                ("Bom dia, {nome}. Por onde começamos?", Hoje),
                ("Café na mão? Bom dia, {nome}.", None),
                ("Manhã nova, página nova, {nome}.", Ler),
                ("Bom dia! Um foco de 25 minutos para abrir o dia?", Foco),
                ("Bom dia, {nome}. Cinco minutos de estudo antes do caos?", Aprender),
                ("Dia começando. A Edição já sabe o que rolou à noite.", Edicao),
            ]);
            if c.edition {
                v.extend([("Bom dia, {nome}! A edição de hoje já chegou.", Edicao), ("Sua edição da manhã está pronta, {nome}.", Edicao), ("Os agentes deixaram o relatório da noite, {nome}.", Edicao)]);
            }
            if weekend {
                v.extend([("Manhã de folga, {nome}. Sem pressa hoje.", None), ("Sábado de manhã pede um livro, {nome}.", Ler), ("Fim de semana: tempo de aprender algo só por diversão.", Aprender)]);
            }
        }
        12..=17 => {
            v.extend([
                ("Boa tarde, {nome}! O que vamos fazer agora?", None),
                ("Uma pausa na tarde, {nome}?", Jogar),
                ("Boa tarde, {nome}. Dez minutos só para você?", Aprender),
                ("Tarde pede um foco. Um pomodoro, {nome}?", Foco),
                ("Depois do almoço, uma partida leve de xadrez?", Jogar),
                ("Calor de Manaus lá fora; aqui, só papel e tinta.", Ler),
                ("Dúvida no meio da tarde? O Claude está acordado.", Claude),
            ]);
            if weekend {
                v.extend([("Tarde livre, {nome}. Uma partida ou um capítulo?", Jogar), ("Tarde de domingo combina com violão, {nome}.", Aula)]);
            }
        }
        _ => {
            v.extend([
                ("Boa noite, {nome}. Como foi o dia?", Hoje),
                ("Fim de dia, {nome}. Um capítulo antes de dormir?", Ler),
                ("Boa noite, {nome}! Ler um pouco ou jogar?", None),
                ("Noite tranquila. Revisar o que você leu esta semana?", Revisao),
                ("Antes de dormir, uma lição curta fixa melhor, {nome}.", Aprender),
                ("A tela não brilha, os olhos agradecem. Boa noite, {nome}.", Ler),
            ]);
            if !weekend && c.weekday == 5 {
                v.extend([("Sexta à noite, {nome}! O fim de semana começou.", Jogar), ("Sexta! Uma partida contra o Claude para comemorar?", Jogar)]);
            }
        }
    }
    if c.weekday == 0 && (10..=21).contains(&c.hour) {
        v.extend([("Domingo, {nome}: que tal fechar a semana?", Semana), ("Três minutos para fechar a semana, {nome}?", Semana), ("Domingo é dia de olhar a semana e planejar a próxima.", Semana)]);
    }
    if c.raining {
        v.extend([("Chuva lá fora, {nome}. Tempo de livro.", Ler), ("Dia de chuva combina com leitura, {nome}.", Ler), ("Chovendo em Manaus: violão e janela, {nome}?", Aula)]);
    }
    if let Some((_, p)) = &c.book {
        match p {
            Some(p) if *p > 0.85 => v.extend([("Falta pouco para terminar {livro}, {nome}!", Ler), ("Reta final de {livro}. Bora?", Ler)]),
            Some(p) if *p > 0.0 => v.extend([("{livro} está esperando você, {nome}.", Ler), ("Vamos continuar {livro}, {nome}?", Ler), ("Onde paramos em {livro}?", Ler)]),
            _ => v.push(("Que tal começar {livro} hoje, {nome}?", Ler)),
        }
    }
    if let Some(g) = &c.game {
        if !g.is_empty() {
            v.extend([("A partida de {jogo} ficou pela metade, {nome}.", Jogo), ("Revanche no {jogo}, {nome}?", Jogo), ("O {jogo} está do jeitinho que você deixou.", Jogo)]);
        }
    }
    if let Some((done, streak)) = c.guitar {
        if !done && (17..=22).contains(&c.hour) {
            v.extend([("Já pegou o violão hoje, {nome}?", Aula), ("Um minuto de trocas no violão, {nome}?", Aula), ("A aula de violão de hoje está montada, {nome}.", Aula)]);
            if streak >= 2 {
                v.push(("Não quebra a sequência do violão, {nome}!", Aula));
            }
        }
    }
    if c.review_due >= 3 {
        v.extend([("{rev} destaques querem ser lembrados hoje, {nome}.", Revisao), ("Lembra do que você marcou nos livros? {rev} esperam revisão.", Revisao)]);
    }
    if let Some((done, streak)) = c.learn {
        if done && streak >= 3 {
            v.extend([("{dias} dias seguidos aprendendo. Bonito, {nome}!", Aprender), ("A chama está firme: {dias} dias, {nome}.", Aprender)]);
        }
        if !done && c.hour < 17 {
            v.push(("A lição de hoje ainda está esperando, {nome}.", Aprender));
        }
    }
    if c.read_today >= 20 {
        v.extend([("Já foram {min} minutos de leitura hoje. Bonito, {nome}!", None), ("{min} minutos lidos hoje. O livro agradece.", Ler)]);
    }
    if !collected().is_empty() {
        v.push(("Sua coleção de achados está crescendo, {nome}.", Colecao));
    }
    // Umas soltas, para variar.
    v.extend([
        ("Pequeno todo dia vence grande de vez em quando.", Aprender),
        ("Dizem que o e-ink descansa os olhos. Os seus agradecem.", None),
        ("Um foco curto agora vale por uma hora distraída depois.", Foco),
        ("Tem coisa rara escondida por aqui. Fique de olho nas frases.", Colecao),
    ]);
    v
}

fn seed(c: &Context, salt: u64) -> u64 {
    let s = (c.day as u64).wrapping_mul(2654435761).wrapping_add(c.hour as u64 * 40503).wrapping_add(salt.wrapping_mul(0x9E37_79B9));
    let s = s ^ (s >> 13);
    let s = s.wrapping_mul(0xff51afd7ed558ccd);
    s ^ (s >> 33)
}

/// Sorteio de raridade da hora: ~1 em 30 horas um raro, ~1 em 600 um lendário. Itens já
/// guardados hoje não voltam no mesmo dia.
fn rare_pick(c: &Context) -> Option<usize> {
    let roll = seed(c, 7) % 600;
    let today: Vec<usize> = collected().iter().filter(|(_, d)| *d == c.day).map(|(i, _)| *i).collect();
    let legendary = roll == 0;
    if !legendary && roll % 30 != 1 {
        return None;
    }
    let pool: Vec<usize> = (0..ITEMS.len()).filter(|i| ITEMS[*i].4 == legendary && !today.contains(i)).collect();
    (!pool.is_empty()).then(|| pool[(seed(c, 11) % pool.len() as u64) as usize])
}

/// A frase do momento (estável durante a hora).
pub fn pick(c: &Context) -> Phrase {
    if !c.notice && c.battery.is_none_or(|b| b > 15) {
        if let Some(i) = rare_pick(c) {
            let rarity = if ITEMS[i].4 { Rarity::Legendary } else { Rarity::Rare };
            return Phrase { text: ITEMS[i].1.to_string(), act: Act::Achado(i), rarity };
        }
    }
    let list = candidates(c);
    if list.is_empty() {
        return Phrase { text: String::new(), act: Act::None, rarity: Rarity::Common };
    }
    let k = (seed(c, 0) % list.len() as u64) as usize;
    Phrase { text: fill(list[k].0, c, name().as_deref()), act: list[k].1, rarity: Rarity::Common }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(hour: u32) -> Context {
        Context { hour, weekday: 0, day: 20000, raining: true, book: Some(("Perfil de três reis - Gene Edwards PDF".into(), Some(0.4))), game: Some("Xadrez".into()), read_today: 25, edition: true, notice: false, battery: Some(80), guitar: Some((false, 3)), learn: Some((true, 4)), review_due: 5 }
    }

    #[test]
    fn frases_preenchidas_e_sem_nome() {
        for h in 0..24 {
            let c = ctx(h);
            for (t, _) in candidates(&c) {
                let com = fill(t, &c, Some("Pedro"));
                let sem = fill(t, &c, None);
                assert!(!com.contains('{') && !sem.contains('{'), "{t}");
                assert!(!sem.contains(", .") && !sem.contains(" ,") && !sem.starts_with(','), "{sem}");
                assert!(sem.chars().next().unwrap().is_uppercase() || sem.starts_with(char::is_numeric), "{sem}");
            }
        }
        assert!(fill("{livro} está esperando você, {nome}.", &ctx(9), None).starts_with("Perfil de três reis"));
        // Muitas frases possíveis ao longo do dia.
        let total: std::collections::HashSet<&str> = (0..24).flat_map(|h| candidates(&ctx(h)).into_iter().map(|(t, _)| t)).collect();
        assert!(total.len() >= 60, "{}", total.len());
    }

    #[test]
    fn raros_aparecem_as_vezes() {
        let (mut rare, mut legend) = (0, 0);
        for day in 0..2000 {
            for hour in 0..24 {
                let mut c = ctx(hour);
                c.day = 30000 + day;
                if let Some(i) = rare_pick(&c) {
                    if ITEMS[i].4 { legend += 1 } else { rare += 1 }
                }
            }
        }
        let hours = 2000.0 * 24.0;
        assert!((rare as f64 / hours) > 0.015 && (rare as f64 / hours) < 0.06, "{rare}");
        assert!(legend > 0 && (legend as f64 / hours) < 0.005, "{legend}");
    }
}
