//! Verso Ink: sistema próprio para o Kindle Paperwhite 3.
//!
//! Subcomandos:
//!   run                 abre a interface (a interface da Amazon deve estar parada)
//!   gate                portão do boot: sai com 0 para abrir o Verso Ink, 3 para pular
//!   splash-amazon       tela "Abrindo o Kindle original"
//!   preview off|reboot  desenha a tela de desligado/reiniciando (testes)
//!   tap X Y [ms]        injeta um toque (testes pelo SSH)
//!   swipe X Y0 Y1       injeta um deslize vertical
//!   info                mostra tela, relógio, bateria e Wi-Fi

mod app;
mod apps;
mod clock;
mod covers;
mod data;
mod fb;
mod input;
mod sys;
mod text;
mod ui;

use app::{App, Screen};
use fb::{Framebuffer, Wave};
use input::Event;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use sys::{log, read_trim, sh};
use text::Fonts;
use ui::Status;

const BATTERY: &str = "/sys/class/power_supply/max77696-battery";
/// Depois desse tempo aberto sem cair, o boot é considerado bem-sucedido.
const HEALTHY_AFTER: Duration = Duration::from_secs(20);

pub fn home_dir() -> PathBuf {
    std::env::var_os("VERSO_INK_HOME").map(PathBuf::from).unwrap_or_else(|| "/mnt/us/verso-ink".into())
}

fn offset_min() -> i64 {
    read_trim(home_dir().join("settings/utc_offset_min").to_str().unwrap_or(""))
        .and_then(|s| s.parse().ok())
        .unwrap_or(-240) // Manaus (UTC-4), sem horário de verão. Mude em settings/utc_offset_min.
}

pub fn read_status() -> Status {
    // settings/dev_battery simula um nível de bateria (testes do aviso de bateria fraca).
    let fake: Option<u8> = read_trim(home_dir().join("settings/dev_battery").to_str().unwrap_or("")).and_then(|s| s.parse().ok());
    let battery = fake.or_else(|| read_trim(&format!("{BATTERY}/capacity")).and_then(|s| s.parse().ok()));
    let charging = fake.is_none() && read_trim(&format!("{BATTERY}/status")).is_some_and(|s| s == "Charging" || s == "Full");
    let wifi = sh("lipc-get-prop", &["com.lab126.wifid", "cmState"]).is_some_and(|s| s == "CONNECTED");
    Status { battery, charging, wifi, offset_min: offset_min() }
}

pub fn set_wifi(on: bool) {
    // Modo de desenvolvimento: mantém o Wi-Fi (e o SSH) ao dormir, para testes remotos.
    if !on && home_dir().join("settings/dev_keep_wifi").exists() {
        log("wifi mantido (dev_keep_wifi)");
        return;
    }
    let _ = sh("lipc-set-prop", &["com.lab126.cmd", "wirelessEnable", if on { "1" } else { "0" }]);
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

/// Gancho de teste: `echo sleep|wake|home > /tmp/verso-ink.cmd`.
fn injected_command() -> Option<String> {
    let c = std::fs::read_to_string("/tmp/verso-ink.cmd").ok()?;
    let _ = std::fs::remove_file("/tmp/verso-ink.cmd");
    Some(c.trim().to_string())
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

    let mut app = App::new(fb);
    // Voltando do KOReader, reabre a biblioteca, onde a pessoa estava.
    if std::env::var_os("VERSO_INK_RETURN").is_some() {
        app.go(Screen::Library);
    }
    let t0 = Instant::now();
    if app.screen == Screen::Home {
        app.render(true);
    }
    log(&format!("primeira tela em {} ms", t0.elapsed().as_millis()));

    // A Amazon ainda limpa a tela alguns instantes depois de parar; redesenhamos uma vez.
    let mut redraw_at = Some(t0 + Duration::from_millis(2500));
    let mut healthy_marked = false;
    let mut clock_was_plausible = clock::now(app.status.offset_min).plausible();

    loop {
        let next = match injected_command().as_deref() {
            Some("sleep") => Ok(Event::Sleep),
            Some("wake") => Ok(Event::Wake),
            Some("home") => {
                app.go(Screen::Home);
                continue;
            }
            Some("library") => {
                app.go(Screen::Library);
                continue;
            }
            Some("refresh") => {
                if app.screen == Screen::Library {
                    app.go(Screen::Library);
                }
                continue;
            }
            _ => rx.recv_timeout(Duration::from_millis(250)),
        };
        match next {
            Ok(Event::Touch(t)) => {
                if app.screen != Screen::Sleep {
                    app.on_touch(t);
                }
            }
            Ok(Event::Sleep) => {
                log("dormindo");
                app.go(Screen::Sleep);
                app.fb.wait();
                set_wifi(false);
            }
            Ok(Event::Wake) => {
                log("acordando");
                set_wifi(true);
                sync_clock_async();
                app.go(Screen::Home);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                log("canal de eventos fechado");
                return 1;
            }
        }
        if let Some(code) = app.exit_code {
            return code;
        }
        if redraw_at.is_some_and(|t| Instant::now() >= t) {
            redraw_at = None;
            if app.screen != Screen::Sleep {
                app.render(true);
            }
        }
        if !healthy_marked && t0.elapsed() >= HEALTHY_AFTER {
            let _ = std::fs::write(home_dir().join("state/fail_count"), "0");
            healthy_marked = true;
            log("boot saudável: contador de falhas zerado");
        }
        // Quando o relógio é acertado pela rede, redesenha a data na hora.
        let plausible = clock::now(app.status.offset_min).plausible();
        if plausible != clock_was_plausible {
            clock_was_plausible = plausible;
            app.last_minute = 99;
        }
        if app.screen != Screen::Sleep {
            app.tick();
        }
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

    let touch = input::open_touch();
    if let (Ok(mut fb), Some(touch)) = (Framebuffer::open(), touch) {
        let fonts = Fonts::new();
        let t = Instant::now();
        ui::draw_gate_hint(&mut fb, &fonts);
        fb.present(Wave::Quality, true);
        log(&format!("portão: tela desenhada em {} ms", t.elapsed().as_millis()));
        let (w, h) = (fb.width, fb.height);
        if input::finger_in(touch, ui::gate_corner(), 3000, w, h) {
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
    // Com panic = "abort" a mensagem se perderia: grava no log antes de cair.
    std::panic::set_hook(Box::new(|info| log(&format!("PANIC: {info}"))));
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("run") => run(),
        Some("gate") => gate(),
        Some("info") => info(),
        Some("preview") => match Framebuffer::open() {
            Ok(mut fb) => {
                apps::draw_powered_off(&mut fb, &Fonts::new(), args.get(2).is_some_and(|a| a == "reboot"));
                fb.present(Wave::Quality, true);
                fb.wait();
                0
            }
            Err(_) => 1,
        },
        Some("splash-amazon") => match Framebuffer::open() {
            Ok(mut fb) => {
                ui::draw_amazon_splash(&mut fb, &Fonts::new());
                fb.present(Wave::Quality, true);
                fb.wait();
                0
            }
            Err(_) => 1,
        },
        Some("swipe") => {
            let n = |i: usize| args.get(i).and_then(|s| s.parse::<i32>().ok());
            match (n(2), n(3), n(4)) {
                (Some(x), Some(y0), Some(y1)) => match input::inject_drag(x, y0, y1, 400, 1072, 1448) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("swipe: {e}");
                        1
                    }
                },
                _ => {
                    eprintln!("uso: verso-ink swipe X Y0 Y1");
                    2
                }
            }
        }
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
