//! Verso Ink: sistema próprio para o Kindle Paperwhite 3.
//!
//! Subcomandos:
//!   run            abre a interface (a interface da Amazon deve estar parada)
//!   gate           portão do boot: sai com 0 para abrir o Verso Ink, 3 para pular
//!   tap X Y [ms]   injeta um toque (testes pelo SSH)
//!   info           mostra tela, toque e relógio

mod clock;
mod fb;
mod input;
mod sys;
mod text;
mod ui;

use fb::{Framebuffer, Wave};
use input::{Event, Touch};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use sys::{log, read_trim, sh};
use text::Fonts;
use ui::{Module, Screen, Status};

const BATTERY: &str = "/sys/class/power_supply/max77696-battery";
/// Depois desse tempo aberto sem cair, o boot é considerado bem-sucedido.
const HEALTHY_AFTER: Duration = Duration::from_secs(20);

fn home_dir() -> PathBuf {
    std::env::var_os("VERSO_INK_HOME").map(PathBuf::from).unwrap_or_else(|| "/mnt/us/verso-ink".into())
}

fn offset_min() -> i64 {
    read_trim(home_dir().join("settings/utc_offset_min").to_str().unwrap_or(""))
        .and_then(|s| s.parse().ok())
        .unwrap_or(-240) // Manaus (UTC-4), sem horário de verão. Mude em settings/utc_offset_min.
}

fn read_status() -> Status {
    let battery = read_trim(&format!("{BATTERY}/capacity")).and_then(|s| s.parse().ok());
    let charging = read_trim(&format!("{BATTERY}/status")).is_some_and(|s| s == "Charging" || s == "Full");
    let wifi = sh("lipc-get-prop", &["com.lab126.wifid", "cmState"]).is_some_and(|s| s == "CONNECTED");
    Status { battery, charging, wifi, offset_min: offset_min() }
}

fn set_wifi(on: bool) {
    // Modo de desenvolvimento: mantém o Wi-Fi (e o SSH) ao dormir, para testes remotos.
    if !on && home_dir().join("settings/dev_keep_wifi").exists() {
        log("wifi mantido (dev_keep_wifi)");
        return;
    }
    let v = if on { "1" } else { "0" };
    let _ = sh("lipc-set-prop", &["com.lab126.cmd", "wirelessEnable", v]);
    log(&format!("wifi {}", if on { "ligado" } else { "desligado" }));
}

/// Acerta o relógio pela rede, em segundo plano (o PW3 perde a hora).
fn sync_clock_async() {
    std::thread::spawn(|| {
        for _ in 0..12 {
            if sh("ntpdate", &["-s", "pool.ntp.org"]).is_some() || sh("ntpdate", &["-s", "a.ntp.br"]).is_some() {
                log("relógio acertado");
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
        log("não consegui acertar o relógio");
    });
}

struct App {
    fb: Framebuffer,
    fonts: Fonts,
    screen: Screen,
    pressed: Option<usize>, // índice do bloco ou 99 para o botão Início
    status: Status,
    last_minute: u32,
}

const BACK: usize = 99;

impl App {
    fn render(&mut self, flash: bool) {
        self.status = read_status();
        match self.screen {
            Screen::Home => ui::draw_home(&mut self.fb, &self.fonts, &self.status),
            Screen::Module(m) => ui::draw_module(&mut self.fb, &self.fonts, m, &self.status),
            Screen::Sleep => ui::draw_sleep(&mut self.fb, &self.fonts, &self.status),
        }
        self.fb.present(Wave::Quality, flash);
        self.last_minute = clock::now(self.status.offset_min).minute;
    }

    fn hit(&self, x: i32, y: i32) -> Option<usize> {
        match self.screen {
            Screen::Home => (0..Module::ALL.len()).find(|&i| ui::tile_rect(i).contains(x, y)),
            Screen::Module(_) => ui::back_rect().contains(x, y).then_some(BACK),
            Screen::Sleep => None,
        }
    }

    fn draw_pressed(&mut self, target: usize, pressed: bool) {
        if target == BACK {
            ui::draw_back(&mut self.fb, &self.fonts, pressed);
            self.fb.refresh(ui::back_rect(), Wave::Fast, false);
        } else {
            ui::draw_tile(&mut self.fb, &self.fonts, target, pressed);
            self.fb.refresh(ui::tile_rect(target), Wave::Fast, false);
        }
    }

    fn on_touch(&mut self, t: Touch) {
        match t {
            Touch::Down(x, y) => {
                if let Some(h) = self.hit(x, y) {
                    self.pressed = Some(h);
                    self.draw_pressed(h, true);
                }
            }
            Touch::Move(x, y) => {
                // Arrastar para fora cancela o toque, como num botão nativo.
                if let Some(h) = self.pressed {
                    if self.hit(x, y) != Some(h) {
                        self.pressed = None;
                        self.restore(h);
                    }
                }
            }
            Touch::Up(x, y) => {
                let Some(h) = self.pressed.take() else { return };
                if self.hit(x, y) != Some(h) {
                    self.restore(h);
                    return;
                }
                self.screen = if h == BACK { Screen::Home } else { Screen::Module(Module::ALL[h]) };
                log(&format!("tela: {:?}", self.screen));
                self.render(true);
            }
        }
    }

    fn restore(&mut self, target: usize) {
        self.draw_pressed(target, false);
        let r = if target == BACK { ui::back_rect() } else { ui::tile_rect(target) };
        self.fb.refresh(r, Wave::Quality, false);
    }

    fn tick(&mut self) {
        let now = clock::now(self.status.offset_min);
        if self.screen == Screen::Home && now.minute != self.last_minute {
            self.last_minute = now.minute;
            self.status = read_status();
            ui::draw_clock(&mut self.fb, &self.fonts, &self.status);
            ui::draw_header(&mut self.fb, &self.fonts, &self.status);
            ui::draw_footer(&mut self.fb, &self.fonts, &self.status);
            // A cada hora cheia, uma atualização completa limpa os fantasmas do e-ink.
            if now.minute == 0 {
                self.fb.present(Wave::Quality, true);
            } else {
                let c = ui::clock_rect();
                let d = ui::date_rect();
                self.fb.refresh(fb::Rect::new(0, d.y, 1072, c.y + c.h - d.y), Wave::Quality, false);
                self.fb.refresh(fb::Rect::new(0, 1300, 1072, 148), Wave::Quality, false);
            }
        }
    }
}

fn run() -> i32 {
    log(&format!("início, pid {}", std::process::id()));
    // Teste das saídas de emergência: simula um Verso Ink que cai logo ao abrir.
    if home_dir().join("settings/dev_crash").exists() {
        log("dev_crash: saindo com falha de propósito");
        return 1;
    }
    let fb = match Framebuffer::open() {
        Ok(fb) => fb,
        Err(e) => {
            log(&format!("framebuffer: {e}"));
            return 1;
        }
    };
    let (w, h) = (fb.width, fb.height);
    let (tx, rx) = mpsc::channel();
    if let Err(e) = input::spawn_touch(tx.clone(), w, h) {
        log(&format!("toque: {e}"));
        return 1;
    }
    input::spawn_power(tx);
    set_wifi(true);
    sync_clock_async();

    let mut app = App { fb, fonts: Fonts::new(), screen: Screen::Home, pressed: None, status: read_status(), last_minute: 99 };
    let t0 = Instant::now();
    app.render(true);
    log(&format!("primeira tela em {} ms", t0.elapsed().as_millis()));

    // A Amazon ainda limpa a tela alguns instantes depois de parar; redesenhamos uma vez.
    let mut redraw_at = Some(t0 + Duration::from_millis(2500));
    let healthy_flag = home_dir().join("state/fail_count");
    let mut healthy_marked = false;
    let mut clock_was_plausible = clock::now(app.status.offset_min).plausible();

    loop {
        // Gancho de teste: `echo sleep > /tmp/verso-ink.cmd` simula o powerd (sleep|wake).
        let injected = std::fs::read_to_string("/tmp/verso-ink.cmd").ok().and_then(|c| {
            let _ = std::fs::remove_file("/tmp/verso-ink.cmd");
            match c.trim() {
                "sleep" => Some(Event::Sleep),
                "wake" => Some(Event::Wake),
                _ => None,
            }
        });
        let next = match injected {
            Some(ev) => Ok(ev),
            None => rx.recv_timeout(Duration::from_millis(500)),
        };
        match next {
            Ok(Event::Touch(t)) => {
                if app.screen != Screen::Sleep {
                    app.on_touch(t);
                }
            }
            Ok(Event::Sleep) => {
                log("dormindo");
                app.screen = Screen::Sleep;
                app.pressed = None;
                app.render(true);
                app.fb.wait();
                set_wifi(false);
            }
            Ok(Event::Wake) => {
                log("acordando");
                set_wifi(true);
                sync_clock_async();
                app.screen = Screen::Home;
                app.render(true);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                log("canal de eventos fechado");
                return 1;
            }
        }
        if redraw_at.is_some_and(|t| Instant::now() >= t) {
            redraw_at = None;
            if app.pressed.is_none() && app.screen != Screen::Sleep {
                app.render(true);
            }
        }
        if !healthy_marked && t0.elapsed() >= HEALTHY_AFTER {
            let _ = std::fs::write(&healthy_flag, "0");
            healthy_marked = true;
            log("boot saudável: contador de falhas zerado");
        }
        // Quando o relógio é acertado pela rede, redesenha a data na hora.
        let plausible = clock::now(app.status.offset_min).plausible();
        if plausible != clock_was_plausible {
            clock_was_plausible = plausible;
            app.last_minute = 99;
        }
        app.tick();
    }
}

/// Portão do boot. Pula o Verso Ink (sai com 3) se existir o arquivo DISABLE,
/// se houve 3 falhas seguidas ou se a pessoa segurar o canto da tela.
fn gate() -> i32 {
    let home = home_dir();
    if home.join("DISABLE").exists() {
        log("portão: DISABLE presente, pulando");
        return 3;
    }
    let count_path = home.join("state/fail_count");
    let fails: u32 = read_trim(count_path.to_str().unwrap_or("")).and_then(|s| s.parse().ok()).unwrap_or(0);
    if fails >= 3 {
        log(&format!("portão: {fails} falhas seguidas, pulando"));
        return 3;
    }
    let _ = std::fs::create_dir_all(home.join("state"));
    let _ = std::fs::write(&count_path, (fails + 1).to_string());

    if let Ok(mut fb) = Framebuffer::open() {
        let fonts = Fonts::new();
        ui::draw_gate_hint(&mut fb, &fonts);
        fb.present(Wave::Quality, true);
        let (w, h) = (fb.width, fb.height);
        if input::finger_in(ui::gate_corner(), 3000, w, h) {
            log("portão: gesto de emergência, pulando");
            let _ = std::fs::write(&count_path, fails.to_string());
            return 3;
        }
    }
    0
}

fn info() -> i32 {
    match Framebuffer::open() {
        Ok(fb) => println!("tela {}x{}", fb.width, fb.height),
        Err(e) => println!("tela: erro {e}"),
    }
    let st = read_status();
    let now = clock::now(st.offset_min);
    println!("hora local {} ({}) plausível={}", now.time(), now.date(), now.plausible());
    println!("bateria {:?} carregando={} wifi={}", st.battery, st.charging, st.wifi);
    0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("run") => run(),
        Some("gate") => gate(),
        Some("info") => info(),
        Some("splash-amazon") => match Framebuffer::open() {
            Ok(mut fb) => {
                ui::draw_amazon_splash(&mut fb, &Fonts::new());
                fb.present(Wave::Quality, true);
                fb.wait();
                0
            }
            Err(_) => 1,
        },
        Some("tap") => {
            let n = |i: usize| args.get(i).and_then(|s| s.parse::<i32>().ok());
            match (n(2), n(3)) {
                (Some(x), Some(y)) => {
                    let ms = n(4).unwrap_or(80) as u64;
                    match input::inject_tap(x, y, ms, 1072, 1448) {
                        Ok(()) => 0,
                        Err(e) => {
                            eprintln!("tap: {e}");
                            1
                        }
                    }
                }
                _ => {
                    eprintln!("uso: verso-ink tap X Y [ms]");
                    2
                }
            }
        }
        _ => {
            eprintln!("uso: verso-ink run | gate | info | tap X Y [ms]");
            2
        }
    };
    std::process::exit(code);
}
