//! Stockfish (UCI) rodando no Kindle: lance do computador e avaliação de posições.

use crate::sys::log;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct Engine {
    child: Child,
    stdin: ChildStdin,
    out: BufReader<ChildStdout>,
}

/// Avaliação do ponto de vista de quem joga: centipeões, ou mate em N (positivo = quem joga dá mate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Score {
    Cp(i32),
    Mate(i32),
}

impl Score {
    /// Em centipeões, com mate valendo ±10000 (mais perto = maior).
    pub fn cp(self) -> i32 {
        match self {
            Score::Cp(c) => c,
            Score::Mate(m) if m > 0 => 10_000 - m * 10,
            Score::Mate(m) => -10_000 - m * 10,
        }
    }
    pub fn neg(self) -> Score {
        match self {
            Score::Cp(c) => Score::Cp(-c),
            Score::Mate(m) => Score::Mate(-m),
        }
    }
}

pub struct Analysis {
    pub best: String,
    pub score: Score,
}

fn binary() -> std::path::PathBuf {
    // Copiado para /tmp com o resto do Verso Ink (o modo USB desmonta /mnt/us).
    let t = std::path::PathBuf::from("/tmp/verso-ink/stockfish");
    if t.exists() { t } else { crate::home_dir().join("bin/stockfish") }
}

impl Engine {
    pub fn start() -> Option<Engine> {
        let mut child = Command::new(binary()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
        let stdin = child.stdin.take()?;
        let out = BufReader::new(child.stdout.take()?);
        let mut e = Engine { child, stdin, out };
        e.send("uci");
        e.wait_for("uciok")?;
        e.send("setoption name Hash value 16");
        e.ready()?;
        log("stockfish pronto");
        Some(e)
    }

    fn send(&mut self, cmd: &str) {
        let _ = writeln!(self.stdin, "{cmd}");
        let _ = self.stdin.flush();
    }

    fn wait_for(&mut self, prefix: &str) -> Option<String> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.out.read_line(&mut line).ok()? == 0 {
                return None;
            }
            if line.starts_with(prefix) {
                return Some(line.trim().to_string());
            }
        }
    }

    fn ready(&mut self) -> Option<()> {
        self.send("isready");
        self.wait_for("readyok").map(|_| ())
    }

    /// Força: abaixo de 1400 usa o nível 0 (e o treinador mistura lances fracos); acima, limita pelo Elo.
    pub fn set_strength(&mut self, elo: u32) {
        if elo >= 2900 {
            self.send("setoption name UCI_LimitStrength value false");
            self.send("setoption name Skill Level value 20");
        } else if elo >= 1400 {
            self.send("setoption name Skill Level value 20");
            self.send("setoption name UCI_LimitStrength value true");
            self.send(&format!("setoption name UCI_Elo value {}", elo.clamp(1350, 2850)));
        } else {
            self.send("setoption name UCI_LimitStrength value false");
            self.send("setoption name Skill Level value 0");
        }
        let _ = self.ready();
    }

    fn full_strength(&mut self) {
        self.send("setoption name UCI_LimitStrength value false");
        self.send("setoption name Skill Level value 20");
    }

    fn search(&mut self, moves: &[String], go: &str) -> Option<Analysis> {
        let pos = if moves.is_empty() { "position startpos".to_string() } else { format!("position startpos moves {}", moves.join(" ")) };
        self.send(&pos);
        self.send(go);
        let mut score = Score::Cp(0);
        let mut line = String::new();
        loop {
            line.clear();
            if self.out.read_line(&mut line).ok()? == 0 {
                return None;
            }
            if line.starts_with("info") && line.contains(" score ") {
                let w: Vec<&str> = line.split_whitespace().collect();
                if let Some(i) = w.iter().position(|x| *x == "score") {
                    let v: i32 = w.get(i + 2).and_then(|x| x.parse().ok()).unwrap_or(0);
                    score = if w.get(i + 1) == Some(&"mate") { Score::Mate(v) } else { Score::Cp(v) };
                }
            } else if let Some(rest) = line.strip_prefix("bestmove ") {
                let best = rest.split_whitespace().next().unwrap_or("").to_string();
                return Some(Analysis { best, score });
            }
        }
    }

    /// Lance do computador com a força configurada.
    pub fn play(&mut self, moves: &[String], millis: u32) -> Option<String> {
        self.search(moves, &format!("go movetime {millis}")).map(|a| a.best).filter(|b| b != "(none)")
    }

    /// Avaliação com força total (para treinador e revisão), do ponto de vista de quem joga.
    pub fn analyse(&mut self, moves: &[String], millis: u32) -> Option<Analysis> {
        self.full_strength();
        self.search(moves, &format!("go movetime {millis}"))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.send("quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
