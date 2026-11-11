//! Toque (cyttsp4_mt, protocolo multitoque tipo B) e eventos de energia (powerd).

use crate::sys::{ioctl, log};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::Sender;

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;
const SYN_REPORT: u16 = 0;
const BTN_TOUCH: u16 = 0x14a;
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;

const fn eviocgabs(abs: u16) -> u32 {
    0x8018_4540 + abs as u32
}
const EVIOCGRAB: u32 = 0x4004_4590;

pub const TOUCH_DEV: &str = "/dev/input/event1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    Down(i32, i32),
    Move(i32, i32),
    Up(i32, i32),
}

#[derive(Debug)]
pub enum Event {
    Touch(Touch),
    /// powerd vai apagar a tela (botão ou inatividade).
    Sleep,
    /// powerd acordou.
    Wake,
}

fn abs_range(f: &File, code: u16) -> (i32, i32) {
    // struct input_absinfo { value, minimum, maximum, fuzz, flat, resolution }
    let mut info = [0i32; 6];
    let rc = unsafe { ioctl(f.as_raw_fd(), eviocgabs(code), info.as_mut_ptr()) };
    if rc == 0 && info[2] > info[1] {
        (info[1], info[2])
    } else {
        (0, 0)
    }
}

/// Converte a coordenada do sensor para pixels da tela.
fn scale(v: i32, (min, max): (i32, i32), screen: i32) -> i32 {
    if max <= min {
        return v;
    }
    ((v - min) as i64 * (screen - 1) as i64 / (max - min) as i64) as i32
}

/// Lê o toque em uma thread e manda `Event::Touch` pelo canal.
/// Captura o dispositivo com EVIOCGRAB para que outro programa não reaja junto.
pub fn spawn_touch(tx: Sender<Event>, width: i32, height: i32) -> std::io::Result<()> {
    let mut f = OpenOptions::new().read(true).open(TOUCH_DEV)?;
    let rx = abs_range(&f, ABS_MT_POSITION_X);
    let ry = abs_range(&f, ABS_MT_POSITION_Y);
    log(&format!("toque: faixa x={rx:?} y={ry:?}"));
    unsafe {
        ioctl(f.as_raw_fd(), EVIOCGRAB, 1i32);
    }
    std::thread::spawn(move || {
        let mut ev = [0u8; 16];
        let (mut x, mut y) = (0, 0);
        let mut slot = 0;
        let mut down = false;
        let mut pending_down = false;
        let mut pending_up = false;
        let mut moved = false;
        while f.read_exact(&mut ev).is_ok() {
            let typ = u16::from_le_bytes([ev[8], ev[9]]);
            let code = u16::from_le_bytes([ev[10], ev[11]]);
            let val = i32::from_le_bytes([ev[12], ev[13], ev[14], ev[15]]);
            match (typ, code) {
                (EV_ABS, ABS_MT_SLOT) => slot = val,
                // Só o primeiro dedo (slot 0) conta, por enquanto.
                (EV_ABS, ABS_MT_TRACKING_ID) if slot == 0 => {
                    if val >= 0 {
                        pending_down = !down;
                    } else {
                        pending_up = down;
                    }
                }
                (EV_ABS, ABS_MT_POSITION_X) if slot == 0 => {
                    x = scale(val, rx, width);
                    moved = true;
                }
                (EV_ABS, ABS_MT_POSITION_Y) if slot == 0 => {
                    y = scale(val, ry, height);
                    moved = true;
                }
                (EV_KEY, BTN_TOUCH) => {
                    if val == 1 {
                        pending_down = !down;
                    } else {
                        pending_up = down;
                    }
                }
                (EV_SYN, SYN_REPORT) => {
                    let t = if pending_down {
                        down = true;
                        Some(Touch::Down(x, y))
                    } else if pending_up {
                        down = false;
                        Some(Touch::Up(x, y))
                    } else if down && moved {
                        Some(Touch::Move(x, y))
                    } else {
                        None
                    };
                    pending_down = false;
                    pending_up = false;
                    moved = false;
                    if let Some(t) = t {
                        if tx.send(Event::Touch(t)).is_err() {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
    });
    Ok(())
}

/// Escuta os eventos do powerd pelo `lipc-wait-event` e traduz em Sleep/Wake.
pub fn spawn_power(tx: Sender<Event>) {
    std::thread::spawn(move || loop {
        let child = std::process::Command::new("lipc-wait-event")
            .args(["-m", "com.lab126.powerd", "goingToScreenSaver,outOfScreenSaver"])
            .stdout(std::process::Stdio::piped())
            .spawn();
        let Ok(mut child) = child else {
            log("lipc-wait-event indisponível; sem eventos de energia");
            return;
        };
        let out = child.stdout.take().unwrap();
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let ev = if line.starts_with("goingToScreenSaver") {
                Event::Sleep
            } else if line.starts_with("outOfScreenSaver") {
                Event::Wake
            } else {
                continue;
            };
            if tx.send(ev).is_err() {
                let _ = child.kill();
                return;
            }
        }
        let _ = child.wait();
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

/// Injeta um toque no dispositivo (usado nos testes automáticos pelo SSH).
/// Coordenadas em pixels da tela; convertidas de volta para a faixa do sensor.
pub fn inject_tap(x: i32, y: i32, hold_ms: u64, width: i32, height: i32) -> std::io::Result<()> {
    let mut f = OpenOptions::new().read(true).write(true).open(TOUCH_DEV)?;
    let rx = abs_range(&f, ABS_MT_POSITION_X);
    let ry = abs_range(&f, ABS_MT_POSITION_Y);
    let unscale = |v: i32, (min, max): (i32, i32), screen: i32| {
        if max <= min { v } else { min + (v as i64 * (max - min) as i64 / (screen - 1) as i64) as i32 }
    };
    let (sx, sy) = (unscale(x, rx, width), unscale(y, ry, height));
    let mut put = |typ: u16, code: u16, val: i32| -> std::io::Result<()> {
        let mut ev = [0u8; 16];
        ev[8..10].copy_from_slice(&typ.to_le_bytes());
        ev[10..12].copy_from_slice(&code.to_le_bytes());
        ev[12..16].copy_from_slice(&val.to_le_bytes());
        f.write_all(&ev)
    };
    put(EV_ABS, ABS_MT_SLOT, 0)?;
    put(EV_ABS, ABS_MT_TRACKING_ID, 4242)?;
    put(EV_ABS, ABS_MT_POSITION_X, sx)?;
    put(EV_ABS, ABS_MT_POSITION_Y, sy)?;
    put(EV_KEY, BTN_TOUCH, 1)?;
    put(EV_SYN, SYN_REPORT, 0)?;
    std::thread::sleep(std::time::Duration::from_millis(hold_ms));
    put(EV_ABS, ABS_MT_TRACKING_ID, -1)?;
    put(EV_KEY, BTN_TOUCH, 0)?;
    put(EV_SYN, SYN_REPORT, 0)?;
    Ok(())
}

/// Lê o estado atual do toque, sem capturar: devolve a posição se há dedo na tela
/// dentro de `timeout_ms` (usado pelo portão do boot).
pub fn finger_in(region: crate::fb::Rect, timeout_ms: u64, width: i32, height: i32) -> bool {
    let Ok(mut f) = OpenOptions::new().read(true).open(TOUCH_DEV) else { return false };
    let rx = abs_range(&f, ABS_MT_POSITION_X);
    let ry = abs_range(&f, ABS_MT_POSITION_Y);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let (mut x, mut y, mut touching) = (-1, -1, false);
    let mut held_since: Option<std::time::Instant> = None;
    let mut ev = [0u8; 16];
    loop {
        // Dedo parado não gera eventos: o tempo segurado é conferido também sem evento novo.
        if touching && held_since.is_some_and(|s| s.elapsed().as_millis() >= 1000) {
            return true;
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return false;
        }
        let left = deadline.saturating_duration_since(now).as_millis() as i32;
        if !crate::sys::wait_readable(f.as_raw_fd(), left.clamp(1, 50)) {
            continue;
        }
        if f.read_exact(&mut ev).is_err() {
            return false;
        }
        let typ = u16::from_le_bytes([ev[8], ev[9]]);
        let code = u16::from_le_bytes([ev[10], ev[11]]);
        let val = i32::from_le_bytes([ev[12], ev[13], ev[14], ev[15]]);
        match (typ, code) {
            (EV_ABS, ABS_MT_POSITION_X) => x = scale(val, rx, width),
            (EV_ABS, ABS_MT_POSITION_Y) => y = scale(val, ry, height),
            (EV_ABS, ABS_MT_TRACKING_ID) => touching = val >= 0,
            (EV_KEY, BTN_TOUCH) => touching = val == 1,
            (EV_SYN, SYN_REPORT) => {
                if touching && region.contains(x, y) {
                    held_since.get_or_insert_with(std::time::Instant::now);
                } else {
                    held_since = None;
                }
            }
            _ => {}
        }
    }
}
