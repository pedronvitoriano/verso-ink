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
mod chat;
mod chess;
mod chess_ui;
mod backup;
mod caixas;
mod coach;
mod lig4;
mod masmorra;
mod minas;
mod nonograma;
mod paciencia;
mod partitura;
mod painel;
mod palavra;
mod reader;
mod saudacao;
mod receitas;
mod sokoban;
mod sprites;
mod naval;
mod coroas;
mod coroas_logic;
mod cripta;
mod minigame;
mod engine;
mod clippings;
mod claude_ui;
mod clock;
mod update;
mod sudoku;
mod covers;
mod damas;
mod data;
mod fases;
mod day;
mod today_ui;
mod weather;
mod wifi;
mod fb;
mod historias;
mod input;
mod keyboard;
mod sys;
mod text;
mod tutorial;
mod uno;
mod violao;
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
    input::spawn_power(tx.clone());
    input::spawn_power_key(tx.clone());
    set_wifi(true);
    sync_clock_async();
    weather::spawn();

    let mut app = App::new(fb);
    // Voltando do KOReader, reabre a biblioteca, onde a pessoa estava.
    if std::env::var("VERSO_INK_RETURN").is_ok_and(|v| !v.is_empty()) {
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
    let mut last_touch = Instant::now();
    let mut slept_at: Option<Instant> = None;
    let mut woke_at = Instant::now() - Duration::from_secs(60);
    let idle_limit = Duration::from_secs(
        read_trim(home_dir().join("settings/repouso_min").to_str().unwrap_or("")).and_then(|s| s.parse::<u64>().ok()).unwrap_or(10) * 60,
    );
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
            Some("today") => {
                app.go(Screen::Today);
                continue;
            }
            Some("refresh") => {
                if app.screen == Screen::Library {
                    app.go(Screen::Library);
                }
                continue;
            }
            // Dedo parado numa casa de jogo: checa "segurar" mais vezes.
            _ => rx.recv_timeout(Duration::from_millis(if app.touch_active() { 60 } else { 250 })),
        };
        match next {
            Ok(Event::Touch(t)) => {
                last_touch = Instant::now();
                if app.screen != Screen::Sleep {
                    app.on_touch(t);
                }
            }
            Ok(Event::PowerKey) => {
                // O aperto que acorda o Kindle também chega como evento: ignorado logo após acordar.
                if woke_at.elapsed() < Duration::from_secs(2) {
                    continue;
                }
                if app.screen == Screen::Sleep {
                    // Botão de novo com a tela de repouso: acorda (se o kernel não chegou a suspender).
                    if slept_at.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) {
                        log("acordando pelo botão");
                        slept_at = None;
                        woke_at = Instant::now();
                        set_wifi(true);
                        sync_clock_async();
                        app.go(Screen::Home);
                        last_touch = Instant::now();
                    }
                } else {
                    log("botão de ligar: dormindo");
                    go_to_sleep(&mut app, &tx);
                    slept_at = Some(Instant::now());
                }
            }
            Ok(Event::Sleep) => {
                log("dormindo");
                app.go(Screen::Sleep);
                app.fb.wait();
                set_wifi(false);
            }
            Ok(Event::Wake) => {
                if app.screen != Screen::Sleep {
                    continue;
                }
                slept_at = None;
                woke_at = Instant::now();
                last_touch = Instant::now();
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
        // Repouso por inatividade (o powerd também não faz isso sem a interface da Amazon).
        let busy = matches!(app.pomo, apps::Pomodoro::Running { .. }) || app.mini_awake() || home_dir().join("settings/dev_keep_awake").exists();
        if app.screen != Screen::Sleep && !busy && last_touch.elapsed() >= idle_limit {
            log("inatividade: dormindo");
            go_to_sleep(&mut app, &tx);
            slept_at = Some(Instant::now());
        }
        if app.screen != Screen::Sleep {
            app.tick();
        }
    }
}

/// Desenha o repouso, desliga o Wi-Fi e suspende o kernel. A escrita em /sys/power/state
/// só retorna quando o aparelho acorda (botão de ligar); aí avisamos o laço com Wake.
fn go_to_sleep(app: &mut App, tx: &mpsc::Sender<Event>) {
    app.go(Screen::Sleep);
    app.fb.wait();
    set_wifi(false);
    if home_dir().join("settings/dev_keep_wifi").exists() {
        return; // modo de teste: não suspende, para o SSH continuar
    }
    let tx = tx.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        let r = std::fs::write("/sys/power/state", "mem");
        log(&format!("voltou da suspensão ({})", if r.is_ok() { "ok" } else { "falhou" }));
        let _ = tx.send(Event::Wake);
    });
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
        // Teste da instalação sem publicar no GitHub: verso-ink update-test URL URL_SHA256
        Some("update-test") => match (args.get(2), args.get(3)) {
            (Some(u), Some(s)) => match update::install(u, s) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("{e}");
                    1
                }
            },
            _ => 2,
        },
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
        // drag X0 Y0 X1 Y1 [ms]: arraste em linha reta (testes)
        Some("drag") => {
            let n = |i: usize| args.get(i).and_then(|s| s.parse::<i32>().ok());
            match (n(2), n(3), n(4), n(5)) {
                (Some(a), Some(b), Some(c), Some(d)) => input::inject_path(a, b, c, d, n(6).unwrap_or(1200) as u64, 1072, 1448).map(|_| 0).unwrap_or(1),
                _ => 2,
            }
        }
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
