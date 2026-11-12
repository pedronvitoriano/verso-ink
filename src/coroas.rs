//! Coroas: uma coroa por linha, por coluna e por reino (região), e coroas não podem se tocar,
//! nem na diagonal. Grades geradas com solução única. Toque: vazio → X → coroa → vazio.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, Rng, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Empty,
    X,
    Crown,
}

pub struct Coroas {
    pub n: usize,
    pub region: Vec<u8>,
    pub solution: Vec<usize>,
    pub marks: Vec<Mark>,
    undo: Vec<(usize, Mark)>,
    menu: bool,
    /// Menu do treino livre (escolher o tamanho).
    practice: bool,
    started: std::time::Instant,
    /// Segundos jogados antes de sair e voltar.
    before: u64,
    pub won: bool,
    /// Fase em jogo (0 = treino livre).
    level: u32,
    kind: Kind,
    hints: u32,
    stats: Stats,
}

const SIZES: [(usize, &str); 6] = [(5, "5×5"), (6, "6×6"), (7, "7×7"), (8, "8×8"), (10, "10×10"), (12, "12×12")];

// ---------- Fases ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Normal,
    /// Múltiplos de 5 a partir da 10 (exceto os de 10).
    Hard,
    /// Múltiplos de 10.
    Challenge,
}

/// Tamanho e tipo da fase `k`: cresce de 4 em 4 fases; as especiais são maiores e mais duras.
pub fn fase_spec(k: u32) -> (usize, Kind) {
    let base = (5 + (k.saturating_sub(1) / 4) as usize).min(11);
    if k >= 10 && k % 10 == 0 {
        ((base + 2).min(12), Kind::Challenge)
    } else if k >= 10 && k % 5 == 0 {
        ((base + 1).min(12), Kind::Hard)
    } else {
        (base, Kind::Normal)
    }
}

/// A nota de dificuldade (ver `coroas_logic`) serve para a fase?
fn grade_ok(k: u32, kind: Kind, g: u8) -> bool {
    match kind {
        Kind::Challenge => g >= 5,
        Kind::Hard => g >= 4,
        Kind::Normal if k <= 4 => g <= 2,
        Kind::Normal if k < 10 => g <= 3,
        Kind::Normal => g <= 4,
    }
}

/// Gera a grade da fase `k` dentro de `budget`; se não achar a dificuldade ideal, fica com a mais próxima.
pub fn generate_fase(k: u32, budget: std::time::Duration) -> (Vec<u8>, Vec<usize>) {
    let (n, kind) = fase_spec(k);
    let t = std::time::Instant::now();
    let mut best: Option<(i32, Vec<u8>, Vec<usize>)> = None;
    for _ in 0..300 {
        let (reg, sol) = generate(n);
        let g = crate::coroas_logic::grade(n, &reg);
        if grade_ok(k, kind, g) {
            return (reg, sol);
        }
        let score = if kind == Kind::Normal { -(g as i32) } else { g as i32 };
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, reg, sol));
        }
        if t.elapsed() > budget {
            break;
        }
    }
    let b = best.expect("ao menos uma grade");
    (b.1, b.2)
}

fn dir() -> std::path::PathBuf {
    let d = crate::home_dir().join("data/coroas");
    let _ = std::fs::create_dir_all(&d);
    d
}

fn enc_reg(reg: &[u8]) -> String {
    reg.iter().map(|&g| (b'a' + g) as char).collect()
}
fn dec_reg(s: &str) -> Vec<u8> {
    s.bytes().map(|b| b.wrapping_sub(b'a')).collect()
}
fn enc_sol(sol: &[usize]) -> String {
    sol.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
}
fn dec_sol(s: &str) -> Vec<usize> {
    s.split(',').filter_map(|x| x.parse().ok()).collect()
}

/// Grade pronta guardada (fase, tamanho, reinos, solução), se válida.
fn read_puzzle(name: &str) -> Option<(u32, usize, Vec<u8>, Vec<usize>, Vec<String>)> {
    let t = std::fs::read_to_string(dir().join(name)).ok()?;
    let l: Vec<&str> = t.lines().collect();
    let level = l.first()?.parse().ok()?;
    let n: usize = l.get(1)?.parse().ok()?;
    let reg = dec_reg(l.get(2)?);
    let sol = dec_sol(l.get(3)?);
    (reg.len() == n * n && sol.len() == n && reg.iter().all(|&g| (g as usize) < n)).then(|| (level, n, reg, sol, l[4..].iter().map(|s| s.to_string()).collect()))
}

fn write_atomic(name: &str, text: &str) {
    let tmp = dir().join(format!("{name}.tmp"));
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(tmp, dir().join(name));
    }
}

/// Prepara a grade da fase `level` em segundo plano, enquanto a pessoa joga a atual.
fn prepare(level: u32) {
    if read_puzzle("proxima.txt").is_some_and(|p| p.0 == level) {
        return;
    }
    std::thread::spawn(move || {
        let (reg, sol) = generate_fase(level, std::time::Duration::from_secs(40));
        let n = fase_spec(level).0;
        write_atomic("proxima.txt", &format!("{level}\n{n}\n{}\n{}\n", enc_reg(&reg), enc_sol(&sol)));
    });
}

/// Progresso nas fases e sequência de dias com fase resolvida.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Stats {
    level: u32,
    streak: u32,
    best: u32,
    last_day: i64,
    solved: u32,
}

impl Stats {
    fn load() -> Self {
        let mut s = Stats { level: 1, streak: 0, best: 0, last_day: i64::MIN, solved: 0 };
        for l in std::fs::read_to_string(dir().join("progresso.txt")).unwrap_or_default().lines() {
            let Some((k, v)) = l.split_once('=') else { continue };
            match (k, v.parse::<i64>()) {
                ("fase", Ok(v)) => s.level = v.max(1) as u32,
                ("sequencia", Ok(v)) => s.streak = v as u32,
                ("melhor", Ok(v)) => s.best = v as u32,
                ("ultimo_dia", Ok(v)) => s.last_day = v,
                ("resolvidas", Ok(v)) => s.solved = v as u32,
                _ => {}
            }
        }
        s
    }
    fn save(&self) {
        write_atomic("progresso.txt", &format!("fase={}\nsequencia={}\nmelhor={}\nultimo_dia={}\nresolvidas={}\n", self.level, self.streak, self.best, self.last_day, self.solved));
    }
    /// Sequência que vale hoje (zera se passou um dia sem resolver).
    fn current_streak(&self, today: i64) -> u32 {
        if self.last_day >= today - 1 { self.streak } else { 0 }
    }
    fn record_win(&mut self, today: i64) {
        if self.last_day != today {
            self.streak = if self.last_day == today - 1 { self.streak + 1 } else { 1 };
        }
        self.last_day = today;
        self.best = self.best.max(self.streak);
        self.solved += 1;
        self.level += 1;
    }
}

fn today() -> i64 {
    crate::data::today(crate::offset_min())
}

fn mmss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// Uma coroa por linha, colunas sem repetir e sem encostar na linha vizinha.
fn place(n: usize, rng: &mut Rng) -> Vec<usize> {
    fn go(r: usize, n: usize, cols: &mut Vec<usize>, used: &mut [bool], rng: &mut Rng) -> bool {
        if r == n {
            return true;
        }
        let mut order: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut order);
        for c in order {
            if used[c] || (r > 0 && cols[r - 1].abs_diff(c) <= 1) {
                continue;
            }
            used[c] = true;
            cols.push(c);
            if go(r + 1, n, cols, used, rng) {
                return true;
            }
            cols.pop();
            used[c] = false;
        }
        false
    }
    let mut cols = Vec::new();
    go(0, n, &mut cols, &mut vec![false; n], rng);
    cols
}

/// Reinos crescendo a partir de cada coroa até cobrir a grade.
fn grow(n: usize, sol: &[usize], rng: &mut Rng) -> Vec<u8> {
    let mut reg = vec![u8::MAX; n * n];
    for (r, &c) in sol.iter().enumerate() {
        reg[r * n + c] = r as u8;
    }
    let mut left = n * n - n;
    while left > 0 {
        let i = (rng.next() % (n * n) as u64) as usize;
        if reg[i] != u8::MAX {
            continue;
        }
        let (r, c) = (i / n, i % n);
        let mut nb = Vec::new();
        if r > 0 { nb.push(i - n); }
        if r + 1 < n { nb.push(i + n); }
        if c > 0 { nb.push(i - 1); }
        if c + 1 < n { nb.push(i + 1); }
        let owned: Vec<u8> = nb.iter().map(|&j| reg[j]).filter(|&g| g != u8::MAX).collect();
        if !owned.is_empty() {
            reg[i] = owned[(rng.next() % owned.len() as u64) as usize];
            left -= 1;
        }
    }
    reg
}

/// Conta soluções (até `limit`). Usada nos testes para conferir a unicidade.
#[cfg_attr(not(test), allow(dead_code))]
pub fn count(n: usize, reg: &[u8], limit: u32) -> u32 {
    fn go(r: usize, n: usize, reg: &[u8], cols: &mut [bool], regs: &mut [bool], prev: Option<usize>, limit: u32) -> u32 {
        if r == n {
            return 1;
        }
        let mut t = 0;
        for c in 0..n {
            let g = reg[r * n + c] as usize;
            if cols[c] || regs[g] || prev.is_some_and(|p| p.abs_diff(c) <= 1) {
                continue;
            }
            cols[c] = true;
            regs[g] = true;
            t += go(r + 1, n, reg, cols, regs, Some(c), limit - t);
            cols[c] = false;
            regs[g] = false;
            if t >= limit {
                break;
            }
        }
        t
    }
    go(0, n, reg, &mut vec![false; n], &mut vec![false; n], None, limit)
}

/// Poda: todo reino ainda sem coroa precisa de uma casa livre nas linhas que faltam.
fn regions_still_fit(r: usize, n: usize, reg: &[u8], cols: &[bool], regs: &[bool]) -> bool {
    let mut ok = vec![false; n];
    for rr in r..n {
        for c in 0..n {
            if !cols[c] {
                ok[reg[rr * n + c] as usize] = true;
            }
        }
    }
    (0..n).all(|g| regs[g] || ok[g])
}

/// Uma solução diferente de `sol`, se houver.
fn other_solution(n: usize, reg: &[u8], sol: &[usize]) -> Option<Vec<usize>> {
    fn go(r: usize, n: usize, reg: &[u8], sol: &[usize], cols: &mut [bool], regs: &mut [bool], cur: &mut Vec<usize>) -> bool {
        if r == n {
            return cur.as_slice() != sol;
        }
        if !regions_still_fit(r, n, reg, cols, regs) {
            return false;
        }
        for c in 0..n {
            let g = reg[r * n + c] as usize;
            if cols[c] || regs[g] || cur.last().is_some_and(|&p| p.abs_diff(c) <= 1) {
                continue;
            }
            cols[c] = true;
            regs[g] = true;
            cur.push(c);
            if go(r + 1, n, reg, sol, cols, regs, cur) {
                return true;
            }
            cur.pop();
            cols[c] = false;
            regs[g] = false;
        }
        false
    }
    let mut cur = Vec::new();
    go(0, n, reg, sol, &mut vec![false; n], &mut vec![false; n], &mut cur).then_some(cur)
}

fn neighbors(n: usize, i: usize) -> impl Iterator<Item = usize> {
    let (r, c) = (i / n, i % n);
    [(r > 0).then(|| i - n), (r + 1 < n).then(|| i + n), (c > 0).then(|| i - 1), (c + 1 < n).then(|| i + 1)].into_iter().flatten()
}

/// O reino `g` continua inteiro (ligado) sem a casa `skip`?
fn connected_without(n: usize, reg: &[u8], g: u8, skip: usize) -> bool {
    let cells: Vec<usize> = (0..n * n).filter(|&i| reg[i] == g && i != skip).collect();
    let Some(&first) = cells.first() else { return false };
    let mut seen = vec![false; n * n];
    let mut stack = vec![first];
    let mut k = 0;
    while let Some(i) = stack.pop() {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        k += 1;
        stack.extend(neighbors(n, i).filter(|&j| reg[j] == g && j != skip && !seen[j]));
    }
    k == cells.len()
}

/// Gera reinos com solução única: cresce os reinos a partir das coroas e, enquanto houver
/// outra solução, passa uma casa dela para um reino vizinho (o que mata aquela solução).
pub fn generate(n: usize) -> (Vec<u8>, Vec<usize>) {
    let mut rng = Rng::new();
    loop {
        let sol = place(n, &mut rng);
        let mut reg = grow(n, &sol, &mut rng);
        for _ in 0..n * n * 2 {
            let Some(alt) = other_solution(n, &reg, &sol) else { return (reg, sol) };
            let rows: Vec<usize> = (0..n).filter(|&r| alt[r] != sol[r]).collect();
            let mut moved = false;
            let mut order = rows.clone();
            rng.shuffle(&mut order);
            for r in order {
                let a = r * n + alt[r];
                let g = reg[a];
                let mut opts: Vec<u8> = neighbors(n, a).map(|j| reg[j]).filter(|&h| h != g).collect();
                opts.dedup();
                if opts.is_empty() || !connected_without(n, &reg, g, a) {
                    continue;
                }
                reg[a] = opts[(rng.next() % opts.len() as u64) as usize];
                moved = true;
                break;
            }
            if !moved {
                break;
            }
        }
    }
}

impl Coroas {
    pub fn new() -> Self {
        let stats = Stats::load();
        let mut g = Coroas {
            n: 0,
            region: Vec::new(),
            solution: Vec::new(),
            marks: Vec::new(),
            undo: Vec::new(),
            menu: true,
            practice: false,
            started: std::time::Instant::now(),
            before: 0,
            won: false,
            level: 0,
            kind: Kind::Normal,
            hints: 0,
            stats,
        };
        // Fase deixada no meio: continua de onde parou.
        if let Some((level, n, reg, sol, rest)) = read_puzzle("andamento.txt") {
            let marks: Vec<Mark> = rest.first().map(|m| m.chars().map(|c| match c { 'x' => Mark::X, 'c' => Mark::Crown, _ => Mark::Empty }).collect()).unwrap_or_default();
            if level == stats.level && marks.len() == n * n {
                let nums: Vec<u64> = rest.get(1).map(|t| t.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
                g.n = n;
                g.region = reg;
                g.solution = sol;
                g.marks = marks;
                g.level = level;
                g.kind = fase_spec(level).1;
                g.before = nums.first().copied().unwrap_or(0);
                g.hints = nums.get(1).copied().unwrap_or(0) as u32;
            }
        }
        if g.level == 0 {
            prepare(stats.level);
        }
        g
    }

    fn has_saved(&self) -> bool {
        self.level > 0 && self.level == self.stats.level && !self.won && self.n > 0
    }

    /// Treino livre: tamanho escolhido, sem fase nem sequência.
    fn start(&mut self, n: usize) {
        let (region, solution) = generate(n);
        self.load_grid(0, n, region, solution);
    }

    fn load_grid(&mut self, level: u32, n: usize, region: Vec<u8>, solution: Vec<usize>) {
        self.n = n;
        self.region = region;
        self.solution = solution;
        self.marks = vec![Mark::Empty; n * n];
        self.undo.clear();
        self.menu = false;
        self.practice = false;
        self.started = std::time::Instant::now();
        self.before = 0;
        self.won = false;
        self.level = level;
        self.kind = if level > 0 { fase_spec(level).1 } else { Kind::Normal };
        self.hints = 0;
    }

    /// Abre a fase atual (a grade já preparada em segundo plano, se houver).
    fn start_fase(&mut self) {
        let k = self.stats.level;
        let n = fase_spec(k).0;
        let (reg, sol) = match read_puzzle("proxima.txt") {
            Some((l, nn, r, s, _)) if l == k && nn == n => {
                let _ = std::fs::remove_file(dir().join("proxima.txt"));
                (r, s)
            }
            _ => generate_fase(k, std::time::Duration::from_secs(6)),
        };
        self.load_grid(k, n, reg, sol);
        prepare(k + 1);
    }

    fn elapsed(&self) -> u64 {
        self.before + self.started.elapsed().as_secs()
    }

    /// Conferiu a vitória: registra a fase e a sequência (uma vez só).
    fn check_win(&mut self) {
        let was = self.won;
        self.won = self.crowns().len() == self.n && self.conflicts().is_empty();
        if self.won && !was {
            self.before = self.elapsed();
            if self.level > 0 && self.level == self.stats.level {
                self.stats.record_win(today());
                self.stats.save();
                let _ = std::fs::remove_file(dir().join("andamento.txt"));
                prepare(self.stats.level);
            }
        }
    }

    fn fase_title(&self) -> String {
        match (self.level, self.kind) {
            (0, _) => format!("Treino · {0}×{0}", self.n),
            (k, Kind::Normal) => format!("Fase {k}"),
            (k, Kind::Hard) => format!("Fase {k} · Difícil"),
            (k, Kind::Challenge) => format!("Fase {k} · Desafio"),
        }
    }

    /// Coroas de nota: 3 sem dica, 2 com uma, 1 com mais.
    fn stars(&self) -> usize {
        match self.hints {
            0 => 3,
            1 => 2,
            _ => 1,
        }
    }

    /// Muda uma casa (com desfazer) e confere a vitória.
    fn set(&mut self, i: usize, m: Mark) -> Redraw {
        self.undo.push((i, self.marks[i]));
        self.marks[i] = m;
        self.check_win();
        if self.won {
            Redraw::Full
        } else {
            Redraw::Part(Rect::new(0, 170, W, self.board_rect().y + self.board_rect().h - 170))
        }
    }

    fn crowns(&self) -> Vec<usize> {
        (0..self.n * self.n).filter(|&i| self.marks[i] == Mark::Crown).collect()
    }

    /// Coroas que quebram alguma regra (mesma linha, coluna, reino ou vizinhas).
    pub fn conflicts(&self) -> Vec<usize> {
        let n = self.n;
        let cr = self.crowns();
        cr.iter()
            .copied()
            .filter(|&a| {
                cr.iter().any(|&b| {
                    a != b && (a / n == b / n || a % n == b % n || self.region[a] == self.region[b] || ((a / n).abs_diff(b / n) <= 1 && (a % n).abs_diff(b % n) <= 1))
                })
            })
            .collect()
    }

    fn geom(&self) -> (i32, i32, i32) {
        minigame::grid_geom(self.n as i32, self.n as i32, 290, 940)
    }

    fn cell_rect(&self, i: usize) -> Rect {
        let (x0, y0, c) = self.geom();
        Rect::new(x0 + (i % self.n) as i32 * c, y0 + (i / self.n) as i32 * c, c, c)
    }

    fn board_rect(&self) -> Rect {
        let (x0, y0, c) = self.geom();
        Rect::new(x0 - 6, y0 - 6, c * self.n as i32 + 12, c * self.n as i32 + 12)
    }

    /// Estilo de cada reino (tom e textura), vizinhos sempre diferentes. No e-ink só há cinzas,
    /// então a textura (pontos, listras) ajuda a separar reinos quando são muitos.
    fn tones(&self) -> Vec<u8> {
        const STYLES: usize = 6;
        let n = self.n;
        let mut tone = vec![usize::MAX; n];
        // Reinos com mais vizinhos escolhem primeiro (coloração gulosa mais estável).
        let mut adj = vec![vec![false; n]; n];
        for i in 0..n * n {
            for j in neighbors(n, i) {
                let (a, b) = (self.region[i] as usize, self.region[j] as usize);
                if a != b {
                    adj[a][b] = true;
                }
            }
        }
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&g| std::cmp::Reverse(adj[g].iter().filter(|&&x| x).count()));
        for g in order {
            let mut used = [false; STYLES];
            for o in 0..n {
                if adj[g][o] && tone[o] < STYLES {
                    used[tone[o]] = true;
                }
            }
            tone[g] = (0..STYLES).find(|&k| !used[k]).unwrap_or(g % STYLES);
        }
        tone.iter().map(|&t| t as u8).collect()
    }

    /// Pinta uma casa com o estilo do reino; padrões presos à tela para emendarem entre casas.
    fn fill_style(fb: &mut Framebuffer, r: Rect, style: u8) {
        let base = [0xFF, 0xDC, 0xBE, 0xFF, 0xEC, 0xFF][style as usize % 6];
        fb.fill(r, base);
        match style {
            3 => {
                // Pontos.
                let step = 18;
                let mut y = r.y - r.y.rem_euclid(step) + step / 2;
                while y < r.y + r.h {
                    let mut x = r.x - r.x.rem_euclid(step) + step / 2;
                    while x < r.x + r.w {
                        if x - 2 >= r.x && y - 2 >= r.y && x + 2 <= r.x + r.w && y + 2 <= r.y + r.h {
                            fb.fill(Rect::new(x - 2, y - 2, 4, 4), 0x88);
                        }
                        x += step;
                    }
                    y += step;
                }
            }
            4 => {
                // Listras diagonais: em cada linha, trechos de 3 px a cada 16.
                for yy in r.y..r.y + r.h {
                    let mut xx = r.x + (16 - (r.x + yy).rem_euclid(16)) % 16;
                    while xx < r.x + r.w {
                        let w = 3.min(r.x + r.w - xx);
                        fb.fill(Rect::new(xx, yy, w, 1), 0xA8);
                        xx += 16;
                    }
                }
            }
            5 => {
                // Listras horizontais.
                let mut yy = r.y + (14 - r.y.rem_euclid(14)) % 14;
                while yy < r.y + r.h {
                    fb.fill(Rect::new(r.x, yy, r.w, 2.min(r.y + r.h - yy)), 0xA8);
                    yy += 14;
                }
            }
            _ => {}
        }
    }

    fn draw_crown(fb: &mut Framebuffer, r: Rect, v: u8) {
        // Desenho em 0..1 ampliado a partir do centro, para a coroa encher a casa.
        let k = 1.4;
        let (s, x, y) = (r.w as f32 * k, r.x as f32 - r.w as f32 * (k - 1.0) / 2.0, r.y as f32 - r.w as f32 * (k - 1.0) / 2.0);
        let pts = [(0.2, 0.72), (0.2, 0.38), (0.36, 0.55), (0.5, 0.28), (0.64, 0.55), (0.8, 0.38), (0.8, 0.72)];
        for w in pts.windows(2) {
            ui::stroke(fb, (x + w[0].0 * s, y + w[0].1 * s), (x + w[1].0 * s, y + w[1].1 * s), s * 0.08, v);
        }
        fb.fill(Rect::new((x + 0.2 * s) as i32, (y + 0.66 * s) as i32, (0.6 * s) as i32, (0.1 * s) as i32), v);
        // Preenche o corpo da coroa.
        for k in 0..8 {
            let yy = y + (0.5 + k as f32 * 0.025) * s;
            ui::stroke(fb, (x + 0.24 * s, yy), (x + 0.76 * s, yy), s * 0.05, v);
        }
    }
}

impl MiniGame for Coroas {
    fn risky(&self, id: u32) -> bool {
        id == 803 && self.level == 0 && !self.undo.is_empty()
    }

    fn summary(&self) -> String {
        if self.menu || self.n == 0 { String::new() } else if self.level > 0 { format!("Fase {} · {}×{}", self.level, self.n, self.n) } else { format!("Treino · {}×{}", self.n, self.n) }
    }

    fn context(&self) -> String {
        if self.menu || self.n == 0 {
            return "No menu do Coroas.".into();
        }
        let n = self.n;
        let reg: Vec<String> = (0..n).map(|r| (0..n).map(|c| (b'A' + self.region[r * n + c]) as char).collect()).collect();
        let marks: Vec<String> = (0..n).map(|r| (0..n).map(|c| match self.marks[r * n + c] { Mark::Crown => 'C', Mark::X => 'x', Mark::Empty => '.' }).collect()).collect();
        format!(
            "Coroas {n}×{n} (uma coroa por linha, coluna e reino; coroas não se encostam nem na diagonal). Fase {}.\nReinos (letras):\n{}\nMarcas do Pedro (C coroa, x descartada, . vazia):\n{}\nAjude pelo raciocínio; não entregue a solução inteira se ele não pedir.",
            self.level, reg.join("\n"), marks.join("\n")
        )
    }

    fn at_menu(&self) -> bool {
        self.menu
    }

    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.menu && self.practice {
            let opts: Vec<&str> = SIZES.iter().map(|s| s.1).collect();
            minigame::draw_menu(fb, f, "Treino", "Escolha o tamanho. Não conta para as fases.", &opts);
            return;
        }
        if self.menu {
            let st = &self.stats;
            let streak = st.current_streak(today());
            let main = if self.has_saved() { format!("Continuar fase {}", st.level) } else { format!("Fase {}", st.level) };
            minigame::draw_menu(fb, f, "Coroas", "Uma por linha, coluna e reino. Sem encostar.", &[&main, "Treino livre"]);
            let y = minigame::menu_row(2).y + 40;
            let lines = [
                format!("Sequência: {} dia{}", streak, if streak == 1 { "" } else { "s" }),
                format!("Melhor sequência: {} · Fases resolvidas: {}", st.best, st.solved),
                "O tamanho de cada fase é surpresa. De 5 em 5 vem uma difícil;".to_string(),
                "de 10 em 10, um desafio.".to_string(),
            ];
            f.draw(fb, Face::Display, 56.0, minigame::MARGIN, y + 40, INK, &lines[0]);
            for (k, l) in lines[1..].iter().enumerate() {
                f.draw(fb, Face::Body, 30.0, minigame::MARGIN, y + 110 + k as i32 * 50, MUTED, l);
            }
            return;
        }
        let status = if self.won {
            format!("Resolvida em {}", mmss(self.before))
        } else if self.crowns().is_empty() && self.kind != Kind::Normal {
            format!("{} coroas. {}", self.n, if self.kind == Kind::Challenge { "Um desafio de verdade." } else { "Fase difícil." })
        } else {
            let c = self.conflicts().len();
            let placed = self.crowns().len();
            if c > 0 { "Há coroas em conflito (sublinhadas).".to_string() } else { format!("{placed} de {} coroas", self.n) }
        };
        minigame::header(fb, f, &self.fase_title(), &status);
        if self.won && self.level > 0 {
            // Coroas de nota ao lado do tempo.
            for k in 0..3 {
                let r = Rect::new(W - minigame::MARGIN - 60 - (2 - k as i32) * 66, 176, 60, 60);
                Self::draw_crown(fb, r, if k < self.stars() { INK } else { 0xCC });
            }
        }
        let tones = self.tones();
        for i in 0..self.n * self.n {
            let r = self.cell_rect(i);
            Self::fill_style(fb, r, tones[self.region[i] as usize]);
        }
        // Linhas finas entre casas e grossas entre reinos.
        let n = self.n;
        let (x0, y0, c) = self.geom();
        for i in 0..n * n {
            let r = self.cell_rect(i);
            let (row, col) = (i / n, i % n);
            let right_border = col + 1 == n || self.region[i] != self.region[i + 1];
            let down_border = row + 1 == n || self.region[i] != self.region[i + n];
            fb.fill(Rect::new(r.x + r.w - if right_border { 3 } else { 1 }, r.y, if right_border { 6 } else { 2 }, r.h), INK);
            fb.fill(Rect::new(r.x, r.y + r.h - if down_border { 3 } else { 1 }, r.w, if down_border { 6 } else { 2 }), INK);
        }
        fb.fill(Rect::new(x0 - 3, y0 - 3, c * n as i32 + 6, 6), INK);
        fb.fill(Rect::new(x0 - 3, y0 - 3, 6, c * n as i32 + 6), INK);
        let conflicts = self.conflicts();
        for i in 0..n * n {
            let r = self.cell_rect(i);
            let inner = Rect::new(r.x + c / 8, r.y + c / 8, c - c / 4, c - c / 4);
            match self.marks[i] {
                Mark::X => {
                    let (cx, cy, d) = (r.x as f32 + c as f32 / 2.0, r.y as f32 + c as f32 / 2.0, c as f32 * 0.12);
                    ui::stroke(fb, (cx - d, cy - d), (cx + d, cy + d), 3.0, MUTED);
                    ui::stroke(fb, (cx + d, cy - d), (cx - d, cy + d), 3.0, MUTED);
                }
                Mark::Crown => {
                    Self::draw_crown(fb, inner, INK);
                    if conflicts.contains(&i) {
                        fb.fill(Rect::new(r.x + c / 5, r.y + c - c / 8, c - 2 * c / 5, 6), INK);
                    }
                }
                Mark::Empty => {}
            }
        }
        if self.won && self.level > 0 {
            let streak = self.stats.current_streak(today());
            let next = fase_spec(self.stats.level).1;
            let teaser = match next {
                Kind::Normal => String::new(),
                _ => " · a próxima é especial…".into(),
            };
            minigame::hint(fb, f, &format!("Sequência: {streak} dia{}{teaser}", if streak == 1 { "" } else { "s" }));
            minigame::pill(fb, f, minigame::bottom_button(0, 2), "Menu", false);
            minigame::pill(fb, f, minigame::bottom_button(1, 2), &format!("Fase {}", self.stats.level), true);
            return;
        }
        let last = if self.level > 0 { "Menu" } else { "Novo" };
        for (k, l) in ["Desfazer", "Limpar", "Dica", last].iter().enumerate() {
            minigame::pill(fb, f, minigame::bottom_button(k, 4), l, false);
        }
        minigame::hint(fb, f, "Toque ou arraste: marca X. Segure: coroa.");
    }

    fn save(&self) {
        if self.level == 0 || self.won || self.n == 0 || self.level != self.stats.level {
            return;
        }
        let marks: String = self.marks.iter().map(|m| match m { Mark::X => 'x', Mark::Crown => 'c', Mark::Empty => '.' }).collect();
        let secs = if self.menu { self.before } else { self.elapsed() };
        write_atomic("andamento.txt", &format!("{}\n{}\n{}\n{}\n{marks}\n{secs} {}\n", self.level, self.n, enc_reg(&self.region), enc_sol(&self.solution), self.hints));
    }

    fn back(&mut self) -> bool {
        // Só o submenu de treino volta ao menu; da partida, Início sai (o app guarda a partida).
        if self.menu && self.practice {
            self.practice = false;
            return true;
        }
        if !self.menu {
            self.save();
            self.before = if self.won { self.before } else { self.elapsed() };
        }
        false
    }

    fn resume(&mut self) {
        // O tempo fora do jogo não conta.
        self.started = std::time::Instant::now();
    }

    fn hold(&mut self, id: u32) -> Option<Redraw> {
        let i = id as usize;
        if self.menu || self.won || i >= self.n * self.n {
            return None;
        }
        let next = if self.marks[i] == Mark::Crown { Mark::Empty } else { Mark::Crown };
        Some(self.set(i, next))
    }

    fn drag(&mut self, id: u32) -> Option<Redraw> {
        let i = id as usize;
        if self.menu || self.won || i >= self.n * self.n {
            return None;
        }
        // Passar o dedo só marca X em casas vazias; coroas ficam.
        if self.marks[i] == Mark::Empty {
            self.undo.push((i, Mark::Empty));
            self.marks[i] = Mark::X;
        }
        Some(Redraw::Part(self.cell_rect(i)))
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        if self.menu && self.practice {
            return (0..SIZES.len()).map(|k| (910 + k as u32, minigame::menu_row(k))).collect();
        }
        if self.menu {
            return vec![(900, minigame::menu_row(0)), (901, minigame::menu_row(1))];
        }
        if self.won && self.level > 0 {
            return vec![(803, minigame::bottom_button(0, 2)), (804, minigame::bottom_button(1, 2))];
        }
        let mut v: Vec<(u32, Rect)> = (0..4).map(|k| (800 + k as u32, minigame::bottom_button(k, 4))).collect();
        if !self.won {
            v.extend((0..self.n * self.n).map(|i| (i as u32, self.cell_rect(i))));
        }
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            900 => {
                if self.has_saved() {
                    self.menu = false;
                    self.started = std::time::Instant::now();
                } else {
                    self.start_fase();
                }
                Redraw::Full
            }
            901 => {
                self.practice = true;
                Redraw::Full
            }
            910..=915 => {
                self.start(SIZES[(id - 910) as usize].0);
                Redraw::Full
            }
            804 => {
                self.start_fase();
                Redraw::Full
            }
            800 => {
                if let Some((i, m)) = self.undo.pop() {
                    self.marks[i] = m;
                }
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            801 => {
                self.marks.iter_mut().for_each(|m| *m = Mark::Empty);
                self.undo.clear();
                Redraw::Part(Rect::new(0, 170, W, 1448 - 170))
            }
            802 => {
                // Dica: coloca uma coroa certa que ainda falta (e tira as erradas daquela linha).
                let n = self.n;
                if let Some(r) = (0..n).find(|&r| self.marks[r * n + self.solution[r]] != Mark::Crown) {
                    for c in 0..n {
                        if self.marks[r * n + c] == Mark::Crown {
                            self.marks[r * n + c] = Mark::Empty;
                        }
                    }
                    self.undo.push((r * n + self.solution[r], self.marks[r * n + self.solution[r]]));
                    self.marks[r * n + self.solution[r]] = Mark::Crown;
                    self.hints += 1;
                }
                self.check_win();
                if self.won { Redraw::Full } else { Redraw::Part(Rect::new(0, 170, W, 1448 - 170)) }
            }
            803 => {
                // Sair da fase guarda o tempo; o andamento fica salvo pelo `save`.
                self.before = if self.won { self.before } else { self.elapsed() };
                self.started = std::time::Instant::now();
                self.menu = true;
                self.practice = false;
                Redraw::Full
            }
            i if (i as usize) < self.n * self.n => {
                // Toque: marca X (ou limpa o que houver).
                let i = i as usize;
                let next = if self.marks[i] == Mark::Empty { Mark::X } else { Mark::Empty };
                self.set(i, next)
            }
            _ => Redraw::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fases_crescem_e_as_especiais_sao_maiores() {
        assert_eq!(fase_spec(1), (5, Kind::Normal));
        assert_eq!(fase_spec(5), (6, Kind::Normal)); // 5 ainda não é especial
        assert_eq!(fase_spec(10), (9, Kind::Challenge));
        assert_eq!(fase_spec(15), (9, Kind::Hard));
        assert_eq!(fase_spec(20), (11, Kind::Challenge));
        assert_eq!(fase_spec(30), (12, Kind::Challenge));
        assert_eq!(fase_spec(99).0, 11);
        assert!((1..200).all(|k| fase_spec(k).0 <= 12));
    }

    #[test]
    fn sequencia_conta_dias_seguidos() {
        let mut s = Stats { level: 1, streak: 0, best: 0, last_day: i64::MIN, solved: 0 };
        s.record_win(100);
        s.record_win(100);
        assert_eq!((s.streak, s.level, s.solved), (1, 3, 2));
        s.record_win(101);
        assert_eq!(s.streak, 2);
        assert_eq!(s.current_streak(103), 0);
        s.record_win(103);
        assert_eq!((s.streak, s.best), (1, 2));
    }

    #[test]
    fn fase_gerada_respeita_tamanho() {
        for k in [1, 4, 10, 15] {
            let (reg, sol) = generate_fase(k, std::time::Duration::from_secs(3));
            let n = fase_spec(k).0;
            assert_eq!((reg.len(), sol.len()), (n * n, n));
            assert_eq!(count(n, &reg, 2), 1);
        }
    }
    #[test]
    fn grade_com_solucao_unica_e_regras() {
        for n in (5..=10).chain(5..=10).chain(9..=10) {
            let (reg, sol) = generate(n);
            assert_eq!(count(n, &reg, 2), 1);
            // n reinos, cada um ligado, cada um com uma coroa da solução.
            for g in 0..n as u8 {
                assert!(reg.contains(&g));
                assert!(connected_without(n, &reg, g, usize::MAX));
            }
            assert!(reg.iter().all(|&g| (g as usize) < n));
            let mut g = Coroas::new();
            g.n = n;
            g.region = reg;
            g.solution = sol.clone();
            g.marks = vec![Mark::Empty; n * n];
            for (r, &c) in sol.iter().enumerate() {
                g.marks[r * n + c] = Mark::Crown;
            }
            assert!(g.conflicts().is_empty());
        }
    }
}

#[cfg(test)]
mod tempo {
    #[test]
    #[ignore]
    fn tempo_por_tamanho() {
        for n in 5..=12 {
            let t = std::time::Instant::now();
            for _ in 0..5 {
                super::generate(n);
            }
            eprintln!("{n}x{n}: {:?} por grade", t.elapsed() / 5);
        }
    }
}
