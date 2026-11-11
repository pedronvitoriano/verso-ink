//! Dificuldade de uma grade de Coroas medida como uma pessoa resolveria: aplica regras de
//! dedução em ordem de esforço e devolve a mais difícil que foi preciso usar.
//!
//! 1. Casa única: linha, coluna ou reino com uma só casa possível.
//! 2. Confinamento: um reino cabe numa linha (ou coluna) só, e o contrário.
//! 3. Grupos: 2 ou 3 reinos presos no mesmo tanto de linhas/colunas, e o contrário.
//! 4. Contradição curta: pôr coroa ali esvaziaria alguma linha, coluna ou reino.
//! 5. Nada disso basta (precisa de tentativa mais longa).

pub fn grade(n: usize, reg: &[u8]) -> u8 {
    let mut s = Solver::new(n, reg);
    let mut hardest = 1u8;
    loop {
        if s.solved() {
            return hardest;
        }
        let step = if s.singles() {
            1
        } else if s.confine(1) {
            2
        } else if s.confine(2) || s.confine(3) {
            3
        } else if s.contradiction() {
            4
        } else {
            return 5;
        };
        hardest = hardest.max(step);
    }
}

struct Solver {
    n: usize,
    cand: Vec<bool>,
    crown: Vec<bool>,
    /// Unidades: linhas, colunas e reinos (listas de casas), com o tipo (0, 1, 2).
    units: Vec<(u8, Vec<usize>)>,
}

impl Solver {
    fn new(n: usize, reg: &[u8]) -> Self {
        let mut units = Vec::new();
        for r in 0..n {
            units.push((0, (0..n).map(|c| r * n + c).collect()));
        }
        for c in 0..n {
            units.push((1, (0..n).map(|r| r * n + c).collect()));
        }
        for g in 0..n as u8 {
            units.push((2, (0..n * n).filter(|&i| reg[i] == g).collect()));
        }
        Solver { n, cand: vec![true; n * n], crown: vec![false; n * n], units }
    }

    fn solved(&self) -> bool {
        self.crown.iter().filter(|&&c| c).count() == self.n
    }

    /// Casas que uma coroa em `i` tira do jogo.
    fn shadow(&self, i: usize) -> Vec<usize> {
        let n = self.n;
        let mut v = Vec::new();
        for u in &self.units {
            if u.1.contains(&i) {
                v.extend(u.1.iter().copied().filter(|&j| j != i));
            }
        }
        let (r, c) = ((i / n) as i32, (i % n) as i32);
        for dr in -1..=1 {
            for dc in -1..=1 {
                let (rr, cc) = (r + dr, c + dc);
                if (dr, dc) != (0, 0) && rr >= 0 && cc >= 0 && (rr as usize) < n && (cc as usize) < n {
                    v.push(rr as usize * n + cc as usize);
                }
            }
        }
        v
    }

    fn place(&mut self, i: usize) {
        self.crown[i] = true;
        self.cand[i] = false;
        for j in self.shadow(i) {
            self.cand[j] = false;
        }
    }

    fn open_units(&self) -> Vec<usize> {
        (0..self.units.len()).filter(|&k| !self.units[k].1.iter().any(|&i| self.crown[i])).collect()
    }

    fn singles(&mut self) -> bool {
        for k in self.open_units() {
            let c: Vec<usize> = self.units[k].1.iter().copied().filter(|&i| self.cand[i]).collect();
            if c.len() == 1 {
                self.place(c[0]);
                return true;
            }
        }
        false
    }

    /// `k` unidades de um tipo cujas casas possíveis cabem em `k` unidades de outro tipo:
    /// essas outras unidades ficam reservadas, e o resto delas sai do jogo.
    fn confine(&mut self, k: usize) -> bool {
        let open = self.open_units();
        for a in 0..3u8 {
            let us: Vec<usize> = open.iter().copied().filter(|&u| self.units[u].0 == a).collect();
            for b in 0..3u8 {
                if a == b || us.len() < k {
                    continue;
                }
                for combo in combos(us.len(), k) {
                    let cells: Vec<usize> = combo.iter().flat_map(|&x| self.units[us[x]].1.iter().copied().filter(|&i| self.cand[i])).collect();
                    if cells.is_empty() {
                        continue;
                    }
                    let targets: Vec<usize> = open.iter().copied().filter(|&u| self.units[u].0 == b && self.units[u].1.iter().any(|i| cells.contains(i))).collect();
                    if targets.len() != k {
                        continue;
                    }
                    let mut changed = false;
                    for &t in &targets {
                        for &i in &self.units[t].1.clone() {
                            if self.cand[i] && !cells.contains(&i) {
                                self.cand[i] = false;
                                changed = true;
                            }
                        }
                    }
                    if changed {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn contradiction(&mut self) -> bool {
        let open = self.open_units();
        for i in 0..self.n * self.n {
            if !self.cand[i] {
                continue;
            }
            let gone = self.shadow(i);
            let breaks = open.iter().any(|&u| !self.units[u].1.contains(&i) && self.units[u].1.iter().all(|j| !self.cand[*j] || gone.contains(j)));
            if breaks {
                self.cand[i] = false;
                return true;
            }
        }
        false
    }
}

/// Combinações de `k` índices entre `0..m`.
fn combos(m: usize, k: usize) -> Vec<Vec<usize>> {
    fn go(start: usize, m: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..m {
            cur.push(i);
            go(i + 1, m, k, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    go(0, m, k, &mut Vec::new(), &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grades_geradas_sao_resolvidas_ou_marcadas_como_dificeis() {
        for n in 5..=9 {
            let (reg, _) = crate::coroas::generate(n);
            let g = grade(n, &reg);
            assert!((1..=5).contains(&g));
        }
    }

    #[test]
    fn combinacoes() {
        assert_eq!(combos(4, 2).len(), 6);
        assert_eq!(combos(5, 3).len(), 10);
    }
}

#[cfg(test)]
mod perfil {
    #[test]
    #[ignore]
    fn distribuicao_de_dificuldade() {
        for n in 5..=12 {
            let mut h = [0u32; 6];
            let t = std::time::Instant::now();
            let k = if n >= 11 { 8 } else { 30 };
            for _ in 0..k {
                let (reg, _) = crate::coroas::generate(n);
                h[super::grade(n, &reg) as usize] += 1;
            }
            eprintln!("{n}x{n}: notas 1..5 = {:?} ({:?} por grade+nota)", &h[1..], t.elapsed() / k);
        }
    }
}
