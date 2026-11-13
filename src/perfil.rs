//! Perfil simples para o bench: tempo e chamadas por primitiva de desenho, ligado só com
//! `perfil::on()` (o perf do sistema pode estar bloqueado, e assim mede também no Kindle).
//! Tempo inclusivo: um cartão conta também os discos que ele desenha.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

static ON: AtomicBool = AtomicBool::new(false);
static DATA: Mutex<Vec<(&'static str, u64, u32)>> = Mutex::new(Vec::new());

pub fn on() {
    ON.store(true, Ordering::Relaxed);
    DATA.lock().unwrap().clear();
}

pub struct Guard(Option<(&'static str, Instant)>);

impl Guard {
    #[inline]
    pub fn new(name: &'static str) -> Guard {
        Guard(ON.load(Ordering::Relaxed).then(|| (name, Instant::now())))
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some((name, t)) = self.0 {
            let ns = t.elapsed().as_nanos() as u64;
            let mut d = DATA.lock().unwrap();
            match d.iter_mut().find(|e| e.0 == name) {
                Some(e) => {
                    e.1 += ns;
                    e.2 += 1;
                }
                None => d.push((name, ns, 1)),
            }
        }
    }
}

/// Imprime e zera: total em ms dividido por `frames`, e chamadas por quadro.
pub fn report(frames: u32) {
    let mut d = std::mem::take(&mut *DATA.lock().unwrap());
    d.sort_by(|a, b| b.1.cmp(&a.1));
    for (name, ns, calls) in d {
        println!("    {name:<14} {:>7.2} ms/quadro · {:>5} chamadas/quadro", ns as f64 / 1e6 / frames as f64, calls / frames.max(1));
    }
    ON.store(false, Ordering::Relaxed);
}
