//! Data e hora em português, sem depender do fuso do Kindle (o do PW3 vem corrompido).

use std::time::{SystemTime, UNIX_EPOCH};

const WEEKDAYS: [&str; 7] = ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"];
const MONTHS: [&str; 12] = [
    "janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro",
];

pub struct Now {
    pub hour: u32,
    pub minute: u32,
    pub day: u32,
    pub month: u32,
    pub year: i64,
    pub weekday: usize,
}

/// Dias desde 1970-01-01 → (ano, mês 1..=12, dia). Algoritmo de Howard Hinnant.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn now(offset_min: i64) -> Now {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) + offset_min * 60;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Now {
        hour: (rem / 3600) as u32,
        minute: ((rem % 3600) / 60) as u32,
        day,
        month,
        year,
        // 1970-01-01 foi quinta-feira (índice 4).
        weekday: (days + 4).rem_euclid(7) as usize,
    }
}

impl Now {
    pub fn time(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
    pub fn date(&self) -> String {
        format!("{}, {} de {}", WEEKDAYS[self.weekday], self.day, MONTHS[(self.month - 1) as usize])
    }
    /// O relógio do sistema é plausível? (o PW3 perde a hora sem Wi-Fi)
    pub fn plausible(&self) -> bool {
        self.year >= 2025
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn datas_conhecidas() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2026-11-11 = 20768 dias desde a época.
        assert_eq!(civil_from_days(20_768), (2026, 11, 11));
        assert_eq!((20_768 + 4) % 7, 3); // quarta-feira
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }
}
