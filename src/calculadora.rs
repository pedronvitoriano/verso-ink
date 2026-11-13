//! Calculadora, conversor de unidades e divisão de conta, sem conexão.

use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

/// Parser de expressões: porcentagem é uma operação pós-fixa (10% = 0,1).
struct Parser<'a> { chars: &'a [u8], pos: usize }
impl<'a> Parser<'a> {
    fn eat(&mut self, c: u8) -> bool {
        if self.chars.get(self.pos) == Some(&c) { self.pos += 1; true } else { false }
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop { if self.eat(b'+') { v += self.term()?; } else if self.eat(b'-') { v -= self.term()?; } else { break; } }
        Some(v)
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            if self.eat(b'*') { v *= self.factor()?; }
            else if self.eat(b'/') { let d = self.factor()?; if d == 0.0 { return None; } v /= d; }
            else { break; }
        }
        Some(v)
    }
    fn factor(&mut self) -> Option<f64> {
        let mut v = if self.eat(b'-') { -self.factor()? }
            else if self.eat(b'+') { self.factor()? }
            else if self.eat(b'(') { let v = self.expr()?; if !self.eat(b')') { return None; } v }
            else {
                let start = self.pos;
                while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit() || *c == b'.') { self.pos += 1; }
                if self.eat(b'e') || self.eat(b'E') {
                    if !self.eat(b'+') { self.eat(b'-'); }
                    let exponent = self.pos;
                    while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit()) { self.pos += 1; }
                    if self.pos == exponent { return None; }
                }
                std::str::from_utf8(&self.chars[start..self.pos]).ok()?.parse().ok()?
            };
        while self.eat(b'%') { v /= 100.0; }
        Some(v)
    }
}
fn calculate(text: &str) -> Option<f64> {
    if text.len() > 128 { return None; }
    let mut p = Parser { chars: text.as_bytes(), pos: 0 };
    let v = p.expr()?;
    (p.pos == p.chars.len() && v.is_finite()).then_some(v)
}
fn number(v: f64) -> String {
    if !v.is_finite() { return "Indisponível".into(); }
    let v = if v == 0.0 { 0.0 } else { v };
    let s = if v.abs() >= 1e12 || (v != 0.0 && v.abs() < 1e-6) { format!("{v:.8e}") } else { format!("{v:.12}").trim_end_matches('0').trim_end_matches('.').to_string() };
    s.replace('.', ",")
}
fn display_expr(s: &str) -> String { s.replace('*', "×").replace('/', "÷").replace('.', ",") }
fn history_path() -> std::path::PathBuf { crate::home_dir().join("data/calculadora-historico.txt") }
fn history() -> Vec<String> {
    std::fs::read_to_string(history_path()).unwrap_or_default().lines().rev().take(8).map(str::to_string).collect()
}
fn add_history(expr: &str, result: &str) {
    let path = history_path();
    if let Some(d) = path.parent() { let _ = std::fs::create_dir_all(d); }
    let mut lines: Vec<String> = history().into_iter().rev().collect();
    lines.push(format!("{} = {result}", display_expr(expr)));
    let _ = std::fs::write(path, lines.join("\n"));
}

const CATEGORIES: [&str; 6] = ["Comprimento", "Massa", "Temperatura", "Volume", "Tempo", "Moedas"];
/// Fatores para a unidade base. Temperaturas usam tratamento próprio.
fn units(category: usize) -> Vec<(&'static str, f64)> {
    match category {
        0 => vec![("milímetro", 0.001), ("centímetro", 0.01), ("metro", 1.0), ("quilômetro", 1000.0), ("polegada", 0.0254), ("pé", 0.3048), ("milha", 1609.344)],
        1 => vec![("miligrama", 0.001), ("grama", 1.0), ("quilograma", 1000.0), ("tonelada", 1_000_000.0), ("libra", 453.59237)],
        2 => vec![("Celsius", 1.0), ("Fahrenheit", 1.0), ("Kelvin", 1.0)],
        3 => vec![("mililitro", 0.001), ("litro", 1.0), ("metro cúbico", 1000.0), ("galão americano", 3.785411784)],
        _ => vec![("segundo", 1.0), ("minuto", 60.0), ("hora", 3600.0), ("dia", 86400.0), ("semana", 604800.0)],
    }
}
fn convert_temperature(v: f64, from: usize, to: usize) -> f64 {
    let c = match from { 1 => (v - 32.0) * 5.0 / 9.0, 2 => v - 273.15, _ => v };
    match to { 1 => c * 9.0 / 5.0 + 32.0, 2 => c + 273.15, _ => c }
}
/// Cada linha: código e valor de uma unidade em reais, por exemplo USD=5.20.
fn parse_rates(s: &str) -> Vec<(String, f64)> {
    let mut out = vec![("BRL".to_string(), 1.0)];
    for l in s.lines() {
        let l = l.trim();
        if l.is_empty() || l.starts_with('#') { continue; }
        let parts: Vec<_> = l.split(|c: char| c == '=' || c == ';' || c.is_whitespace()).filter(|x| !x.is_empty()).collect();
        if parts.len() != 2 { continue; }
        let code = parts[0].to_ascii_uppercase();
        let Ok(v) = parts[1].replace(',', ".").parse::<f64>() else { continue };
        if code.len() != 3 || !code.bytes().all(|c| c.is_ascii_uppercase()) || !v.is_finite() || v <= 0.0 { continue; }
        if code == "BRL" { continue; }
        if let Some(item) = out.iter_mut().find(|(name, _)| name == &code) { item.1 = v; } else { out.push((code, v)); }
    }
    out
}
fn rates() -> Vec<(String, f64)> {
    parse_rates(&std::fs::read_to_string(crate::home_dir().join("data/cotacoes.txt")).unwrap_or_default())
}
fn decimal(s: &str) -> Option<f64> { s.parse::<f64>().ok().filter(|v| v.is_finite()) }
/// Número decimal não negativo, como fração exata, sem passar por ponto flutuante.
fn decimal_fraction(text: &str) -> Option<(u128, u128)> {
    let (whole, frac) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) || whole.len() + frac.len() > 15 { return None; }
    let numerator = format!("{whole}{frac}").parse::<u128>().ok()?;
    Some((numerator, 10u128.checked_pow(frac.len() as u32)?))
}
fn round_div(value: u128, divisor: u128) -> Option<u128> {
    if divisor == 0 { return None; }
    let (q, r) = (value / divisor, value % divisor);
    q.checked_add(u128::from(r >= divisor / 2 + divisor % 2))
}
/// Total e cota mínima em centavos, e quantas pessoas pagam um centavo a mais.
fn bill(total: &str, percent: &str, people: u32) -> Option<(u128, u128, u32)> {
    if people == 0 { return None; }
    let (amount, amount_scale) = decimal_fraction(total)?;
    let (rate, rate_scale) = decimal_fraction(if percent.is_empty() { "0" } else { percent })?;
    let factor = 100u128.checked_mul(rate_scale)?.checked_add(rate)?;
    let cents = round_div(amount.checked_mul(factor)?, amount_scale.checked_mul(rate_scale)?)?;
    Some((cents, cents / people as u128, (cents % people as u128) as u32))
}

#[derive(Clone, Copy, PartialEq)]
enum View { Menu, Calc, History, Categories, Convert, Tip }
const CALC_KEYS: [&str; 20] = ["7", "8", "9", "÷", "4", "5", "6", "×", "1", "2", "3", "−", "0", ",", "%", "+", "(", ")", "Apagar", "="];
const NUM_KEYS: [&str; 12] = ["7", "8", "9", "4", "5", "6", "1", "2", "3", ",", "0", "Apagar"];

pub struct Calculadora {
    view: View,
    expr: String,
    result: Option<String>,
    last_value: Option<f64>,
    records: Vec<String>,
    category: usize,
    from: usize,
    to: usize,
    rates: Vec<(String, f64)>,
    input: String,
    tip: [String; 3],
    field: usize,
}
impl Calculadora {
    pub fn new() -> Self {
        Self { view: View::Menu, expr: String::new(), result: None, last_value: None, records: history(), category: 0, from: 0, to: 1, rates: vec![], input: String::new(), tip: [String::new(), "10".into(), "1".into()], field: 0 }
    }
    fn key(k: usize, cols: usize, top: i32, height: i32) -> Rect {
        let gap = 14;
        let w = (W - 2 * MARGIN - (cols as i32 - 1) * gap) / cols as i32;
        Rect::new(MARGIN + (k % cols) as i32 * (w + gap), top + (k / cols) as i32 * (height + gap), w, height)
    }
    fn category_button(k: usize) -> Rect { Self::key(k, 2, 440, 130) }
    fn select_button(k: usize) -> Rect { Self::key(k, 2, 340, 100) }
    fn numeric_button(k: usize) -> Rect { Self::key(k, 3, 760, 115) }
    fn calc_button(k: usize) -> Rect { Self::key(k, 4, 630, 112) }
    fn bottom(k: usize, n: usize) -> Rect {
        let w = (W - 2 * MARGIN - (n as i32 - 1) * 16) / n as i32;
        Rect::new(MARGIN + k as i32 * (w + 16), 1288, w, 80)
    }
    fn options(&self) -> Vec<(String, f64)> {
        if self.category == 5 { self.rates.clone() }
        else { units(self.category).into_iter().map(|(s, v)| (s.to_string(), v)).collect() }
    }
    fn convert_value(&self) -> Option<f64> {
        let v = decimal(&self.input)?;
        let options = self.options();
        if self.category == 5 && options.len() < 2 { return None; }
        let (_, a) = options.get(self.from)?;
        let (_, b) = options.get(self.to)?;
        let n = if self.category == 2 { convert_temperature(v, self.from, self.to) } else { v * a / b };
        n.is_finite().then_some(n)
    }
    fn edit_number(s: &mut String, k: usize, integer: bool) {
        match k {
            11 => { s.pop(); }
            9 if !integer && !s.contains('.') => { if s.is_empty() { s.push('0'); } s.push('.'); }
            0..=8 | 10 => {
                let digit = if k == 10 { '0' } else { ['7','8','9','4','5','6','1','2','3'][k] };
                if s.len() < 15 { s.push(digit); }
            }
            _ => {}
        }
    }
    fn display(fb: &mut Framebuffer, f: &Fonts, r: Rect, caption: &str, value: &str) {
        ui::card(fb, r, 28, PAPER, false);
        f.draw(fb, Face::Strong, 26.0, r.x + 26, r.y + 45, MUTED, caption);
        let mut size = 52.0;
        while f.measure(Face::Display, size, value) > r.w - 52 && size > 28.0 { size -= 2.0; }
        let value = crate::apps::fit(f, Face::Display, size, value, r.w - 52);
        f.draw(fb, Face::Display, size, r.x + 26, r.y + r.h - 26, INK, &value);
    }
}

impl MiniGame for Calculadora {
    fn at_menu(&self) -> bool { self.view == View::Menu }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        match self.view {
            View::Menu => minigame::draw_menu(fb, f, "Contas", "Calcule, converta e divida.", &["Calculadora", "Converter unidades", "Gorjeta e divisão"]),
            View::Calc => {
                minigame::header(fb, f, "Calculadora", "Faça sua conta");
                let expression = if self.expr.is_empty() { "0".to_string() } else { display_expr(&self.expr) };
                Self::display(fb, f, Rect::new(MARGIN, 300, W - 2 * MARGIN, 280), "CONTA", &expression);
                if let Some(result) = &self.result {
                    let width = f.measure(Face::Strong, 32.0, result);
                    f.draw(fb, Face::Strong, 32.0, W - MARGIN - 26 - width.min(W - 2 * MARGIN - 52), 375, MUTED, result);
                }
                for (k, label) in CALC_KEYS.iter().enumerate() { minigame::pill(fb, f, Self::calc_button(k), label, *label == "="); }
                minigame::pill(fb, f, Self::bottom(0, 2), "Limpar", false);
                minigame::pill(fb, f, Self::bottom(1, 2), "Histórico", false);
            }
            View::History => {
                minigame::header(fb, f, "Calculadora", "Contas recentes");
                if self.records.is_empty() { f.draw(fb, Face::Body, 32.0, MARGIN, 390, MUTED, "Suas contas aparecerão aqui."); }
                for (i, line) in self.records.iter().enumerate() {
                    let r = Rect::new(MARGIN, 300 + i as i32 * 110, W - 2 * MARGIN, 95);
                    ui::card(fb, r, 22, PAPER, false);
                    let text = crate::apps::fit(f, Face::Strong, 32.0, line, r.w - 44);
                    f.draw(fb, Face::Strong, 32.0, r.x + 22, r.y + 60, INK, &text);
                }
                minigame::pill(fb, f, Self::bottom(0, 1), "Voltar à calculadora", false);
            }
            View::Categories => {
                minigame::header(fb, f, "Conversor", "O que você quer converter?");
                for (k, title) in CATEGORIES.iter().enumerate() { minigame::pill(fb, f, Self::category_button(k), title, false); }
                minigame::pill(fb, f, Self::bottom(0, 1), "Voltar", false);
            }
            View::Convert => {
                minigame::header(fb, f, "Conversor", CATEGORIES[self.category]);
                let options = self.options();
                if self.category == 5 && options.len() < 2 {
                    f.draw(fb, Face::Strong, 34.0, MARGIN, 395, INK, "Arquivo de cotações ausente ou sem valores válidos.");
                    f.draw(fb, Face::Body, 30.0, MARGIN, 470, MUTED, "Sincronize as cotações e tente novamente.");
                } else {
                    for (k, index) in [self.from, self.to].iter().enumerate() {
                        let label = format!("{}: {}", if k == 0 { "De" } else { "Para" }, options.get(*index).map(|x| x.0.as_str()).unwrap_or(""));
                        let label = crate::apps::fit(f, Face::Strong, 30.0, &label, Self::select_button(k).w - 44);
                        minigame::pill(fb, f, Self::select_button(k), &label, false);
                    }
                    let value = if self.input.is_empty() { "0".to_string() } else { display_expr(&self.input) };
                    Self::display(fb, f, Rect::new(MARGIN, 470, W - 2 * MARGIN, 130), "VALOR", &value);
                    let result = self.convert_value().map(number).unwrap_or_else(|| "—".to_string());
                    let text = format!("Resultado: {result}");
                    f.draw(fb, Face::Strong, 32.0, MARGIN, 660, INK, &crate::apps::fit(f, Face::Strong, 32.0, &text, W - 2 * MARGIN));
                    minigame::pill(fb, f, Rect::new(MARGIN, 680, 250, 62), "Trocar sinal", false);
                    for (k, label) in NUM_KEYS.iter().enumerate() { minigame::pill(fb, f, Self::numeric_button(k), label, false); }
                }
                minigame::pill(fb, f, Self::bottom(0, 1), "Outras unidades", false);
            }
            View::Tip => {
                minigame::header(fb, f, "Dividir conta", "Toque em um campo para preencher");
                let labels = ["Conta (R$)", "Gorjeta (%)", "Pessoas"];
                for k in 0..3 {
                    let r = Self::key(k, 3, 310, 145);
                    let label = format!("{}: {}", labels[k], if self.tip[k].is_empty() { "0".to_string() } else { display_expr(&self.tip[k]) });
                    let label = crate::apps::fit(f, Face::Strong, 26.0, &label, r.w - 32);
                    minigame::pill(fb, f, r, &label, self.field == k);
                }
                let result = self.tip[2].parse::<u32>().ok().and_then(|people| bill(&self.tip[0], &self.tip[1], people));
                let money = |n: u128| format!("{},{:02}", n / 100, n % 100);
                let total = result.map(|(a, _, _)| money(a)).unwrap_or_else(|| "—".into());
                Self::display(fb, f, Rect::new(MARGIN, 490, W - 2 * MARGIN, 190), "TOTAL COM GORJETA", &format!("R$ {total}"));
                if let Some((_, _, extra)) = result.filter(|(_, _, extra)| *extra > 0) {
                    let note = if extra == 1 { "1 pessoa paga 1 centavo a mais".to_string() } else { format!("{extra} pessoas pagam 1 centavo a mais") };
                    f.draw(fb, Face::Strong, 26.0, MARGIN + 26, 585, MUTED, &crate::apps::fit(f, Face::Strong, 26.0, &note, W - 2 * MARGIN - 52));
                }
                let per_person = match result {
                    Some((_, each, 0)) => format!("Por pessoa: R$ {}", money(each)),
                    Some((_, each, _)) => format!("Por pessoa: R$ {} ou R$ {}", money(each), money(each + 1)),
                    None => "Por pessoa: R$ —".into(),
                };
                f.draw(fb, Face::Strong, 34.0, MARGIN, 735, INK, &crate::apps::fit(f, Face::Strong, 34.0, &per_person, W - 2 * MARGIN));
                for (k, label) in NUM_KEYS.iter().enumerate() { minigame::pill(fb, f, Self::numeric_button(k), label, false); }
                minigame::pill(fb, f, Self::bottom(0, 1), "Voltar", false);
            }
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.view {
            View::Menu => (0..3).map(|k| (900 + k as u32, minigame::menu_row(k))).collect(),
            View::Calc => (0..20).map(|k| (k as u32, Self::calc_button(k))).chain([(800, Self::bottom(0, 2)), (801, Self::bottom(1, 2))]).collect(),
            View::History => vec![(802, Self::bottom(0, 1))],
            View::Categories => (0..6).map(|k| (910 + k as u32, Self::category_button(k))).chain([(803, Self::bottom(0, 1))]).collect(),
            View::Convert => {
                let mut v = vec![(804, Self::bottom(0, 1))];
                if self.category != 5 || self.rates.len() >= 2 {
                    v.extend([(820, Self::select_button(0)), (821, Self::select_button(1)), (822, Rect::new(MARGIN, 680, 250, 62))]);
                    v.extend((0..12).map(|k| (100 + k as u32, Self::numeric_button(k))));
                }
                v
            }
            View::Tip => (0..3).map(|k| (830 + k as u32, Self::key(k, 3, 310, 145))).chain((0..12).map(|k| (100 + k as u32, Self::numeric_button(k)))).chain([(803, Self::bottom(0, 1))]).collect(),
        }
    }
    fn tap(&mut self, id: u32, _: i32, _: i32) -> Redraw {
        match (self.view, id) {
            (View::Menu, 900) => self.view = View::Calc,
            (View::Menu, 901) => self.view = View::Categories,
            (View::Menu, 902) => self.view = View::Tip,
            (View::Calc, 0..=19) => {
                let k = id as usize;
                if k == 19 {
                    if let Some(v) = calculate(&self.expr) {
                        let n = number(v);
                        add_history(&self.expr, &n);
                        self.records = history();
                        self.result = Some(n);
                        self.last_value = Some(v);
                    } else { self.result = Some("Conta incompleta ou inválida".into()); self.last_value = None; }
                } else if k == 18 { self.expr.pop(); self.result = None; self.last_value = None; }
                else {
                    let key = match k { 3 => '/', 7 => '*', 11 => '-', 15 => '+', 13 => '.', 14 => '%', 16 => '(', 17 => ')', _ => CALC_KEYS[k].chars().next().unwrap() };
                    if self.result.take().is_some() {
                        if key.is_ascii_digit() || key == '(' { self.expr.clear(); }
                        else if let Some(value) = self.last_value { self.expr = format!("{value:e}"); }
                        self.last_value = None;
                    }
                    if self.expr.len() < 128 { self.expr.push(key); }
                }
            }
            (View::Calc, 800) => { self.expr.clear(); self.result = None; self.last_value = None; }
            (View::Calc, 801) => { self.records = history(); self.view = View::History; }
            (View::History, 802) => self.view = View::Calc,
            (View::Categories, 803) | (View::Tip, 803) => self.view = View::Menu,
            (View::Categories, 910..=915) => {
                self.category = (id - 910) as usize;
                self.rates = rates(); self.from = 0; self.to = 1; self.input.clear(); self.view = View::Convert;
            }
            (View::Convert, 804) => self.view = View::Categories,
            (View::Convert, 820..=821) => {
                let n = self.options().len();
                if n > 0 { if id == 820 { self.from = (self.from + 1) % n; } else { self.to = (self.to + 1) % n; } }
            }
            (View::Convert, 822) => { if self.input.starts_with('-') { self.input.remove(0); } else if !self.input.is_empty() { self.input.insert(0, '-'); } }
            (View::Convert, 100..=111) => Self::edit_number(&mut self.input, (id - 100) as usize, false),
            (View::Tip, 830..=832) => self.field = (id - 830) as usize,
            (View::Tip, 100..=111) => Self::edit_number(&mut self.tip[self.field], (id - 100) as usize, self.field == 2),
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn back(&mut self) -> bool {
        self.view = match self.view {
            View::Menu => return false,
            View::History => View::Calc,
            View::Convert => View::Categories,
            _ => View::Menu,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precedencia_parenteses_porcentagem_e_erros() {
        assert_eq!(calculate("2+3*4"), Some(14.0));
        assert_eq!(calculate("(2+3)*4"), Some(20.0));
        assert_eq!(calculate("200*10%"), Some(20.0));
        assert_eq!(calculate("-(3+2)"), Some(-5.0));
        assert_eq!(calculate("1e-12*2"), Some(2e-12));
        assert_eq!(calculate("1e+20/1e20"), Some(1.0));
        for s in ["3/0", "(2+3", "1..2", "4+", "1/0.0", "2a", "1e+", "1e"] { assert_eq!(calculate(s), None, "{s}"); }
        assert!(calculate(&"(".repeat(129)).is_none());
    }
    #[test]
    fn valores_pequenos_e_contas_seguidas_preservam_precisao() {
        assert_ne!(number(0.000000001), "0");
        assert_ne!(number(1e-12), "0");
        let mut c = Calculadora::new();
        c.view = View::Calc;
        c.expr = "1/3".into();
        let result = calculate(&c.expr).unwrap();
        c.last_value = Some(result);
        c.result = Some(number(result));
        c.tap(7, 0, 0); // Multiplicar o resultado pelo valor seguinte.
        c.tap(10, 0, 0); // 3
        assert!((calculate(&c.expr).unwrap() - 1.0).abs() < 1e-15);
        c.category = 0; c.from = 0; c.to = 6; c.input = "1".into();
        assert!((c.convert_value().unwrap() - 0.001 / 1609.344).abs() < 1e-18);
        assert_ne!(number(c.convert_value().unwrap()), "0");
    }
    #[test]
    fn unidades_cotacoes_e_temperaturas() {
        assert!((convert_temperature(0.0, 0, 1) - 32.0).abs() < 1e-9);
        assert!((convert_temperature(273.15, 2, 0)).abs() < 1e-9);
        assert!((units(0)[3].1 / units(0)[2].1 - 1000.0).abs() < 1e-9);
        let r = parse_rates("USD=5,20\nEUR;6.1\nBRL=2\nUSD=5.3\nXXX=-3\n");
        assert_eq!(r, vec![("BRL".into(), 1.0), ("USD".into(), 5.3), ("EUR".into(), 6.1)]);
        assert_eq!(parse_rates(""), vec![("BRL".into(), 1.0)]);
        let mut c = Calculadora::new();
        c.category = 5; c.rates = r; c.from = 1; c.to = 2; c.input = "10".into();
        assert!((c.convert_value().unwrap() - 10.0 * 5.3 / 6.1).abs() < 1e-12);
        c.from = 2; c.to = 1;
        assert!((c.convert_value().unwrap() - 10.0 * 6.1 / 5.3).abs() < 1e-12);
        c.category = 2; c.from = 1; c.to = 0; c.input = "-40".into();
        assert!((c.convert_value().unwrap() + 40.0).abs() < 1e-12);
    }
    #[test]
    fn conta_dividida_com_gorjeta() {
        assert_eq!(bill("100", "10", 4), Some((11_000, 2_750, 0)));
        assert_eq!(bill("2.67", "0", 1), Some((267, 267, 0)));
        assert_eq!(bill("1.00", "0.5", 1), Some((101, 101, 0)));
        assert_eq!(bill("10.00", "", 4), Some((1_000, 250, 0)));
        assert_eq!(bill("10.01", "0", 2), Some((1_001, 500, 1)));
        assert_eq!(bill("20", "10", 0), None);
        assert_eq!(bill("-1", "10", 1), None);
    }
}
