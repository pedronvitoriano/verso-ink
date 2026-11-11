//! Clima do dia pelo Open-Meteo (sem conta nem chave), buscado em segundo plano a cada 30 min.
//! Local em `settings/local`: "latitude longitude Nome" (padrão: Manaus, o fuso do Pedro).

use crate::home_dir;
use crate::sys::{log, read_trim, sh};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct Weather {
    pub temp: i32,
    pub max: i32,
    pub min: i32,
    pub rain_pct: i32,
    pub code: i32,
}

impl Weather {
    pub fn describe(&self) -> &'static str {
        match self.code {
            0 => "Céu limpo",
            1 | 2 => "Poucas nuvens",
            3 => "Nublado",
            45 | 48 => "Neblina",
            51..=57 => "Garoa",
            61..=67 => "Chuva",
            71..=77 => "Neve",
            80..=82 => "Pancadas de chuva",
            95..=99 => "Trovoadas",
            _ => "",
        }
    }
}

fn location() -> (String, String, String) {
    let s = read_trim(home_dir().join("settings/local").to_str().unwrap_or("")).unwrap_or_default();
    let mut it = s.split_whitespace();
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => (a.into(), b.into(), it.collect::<Vec<_>>().join(" ")),
        _ => ("-3.10".into(), "-60.02".into(), "Manaus".into()),
    }
}

/// Primeiro número depois de `"chave":` (aceita `[n]`), a partir de `from`.
fn num_after(json: &str, from: &str, key: &str) -> Option<f64> {
    let base = json.find(from)?;
    let rest = &json[base..];
    let i = rest.find(&format!("\"{key}\":"))? + key.len() + 3;
    let v: String = rest[i..].trim_start_matches(['[', ' ']).chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    v.parse().ok()
}

fn cache() -> std::path::PathBuf {
    home_dir().join("data/weather.txt")
}

pub fn load() -> Option<Weather> {
    let s = std::fs::read_to_string(cache()).ok()?;
    let mut l = s.lines();
    let nums: Vec<i64> = l.next()?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    let [fetched, temp, max, min, rain, code] = nums[..] else { return None };
    // Previsão de mais de 6 horas atrás não é mostrada.
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs() as i64;
    (now - fetched < 6 * 3600).then_some(Weather { temp: temp as i32, max: max as i32, min: min as i32, rain_pct: rain as i32, code: code as i32 })
}

fn fetch() -> bool {
    let (lat, lon, place) = location();
    let url = format!(
        "http://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,weather_code&daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max&timezone=auto&forecast_days=1"
    );
    let Some(json) = sh("wget", &["-q", "-T", "15", "-O", "-", &url]) else { return false };
    let get = |from: &str, k: &str| num_after(&json, from, k);
    let (Some(t), Some(c), Some(mx), Some(mn)) = (
        get("\"current\":", "temperature_2m"),
        get("\"current\":", "weather_code"),
        get("\"daily\":", "temperature_2m_max"),
        get("\"daily\":", "temperature_2m_min"),
    ) else {
        return false;
    };
    let rain = get("\"daily\":", "precipitation_probability_max").unwrap_or(0.0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let line = format!("{now} {} {} {} {} {}\n{place}\n", t.round(), mx.round(), mn.round(), rain.round(), c as i64);
    std::fs::write(cache(), line).is_ok()
}

pub fn spawn() {
    std::thread::spawn(|| loop {
        let ok = fetch();
        log(if ok { "clima atualizado" } else { "clima: sem resposta" });
        std::thread::sleep(Duration::from_secs(if ok { 30 * 60 } else { 5 * 60 }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn le_a_resposta() {
        let j = r#"{"current":{"time":"x","temperature_2m":27.0,"weather_code":3},"daily":{"temperature_2m_max":[32.7],"temperature_2m_min":[26.7],"precipitation_probability_max":[40]}}"#;
        assert_eq!(num_after(j, "\"current\":", "temperature_2m"), Some(27.0));
        assert_eq!(num_after(j, "\"daily\":", "temperature_2m_max"), Some(32.7));
        assert_eq!(num_after(j, "\"daily\":", "precipitation_probability_max"), Some(40.0));
    }
}
