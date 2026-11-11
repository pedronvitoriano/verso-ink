//! Estado da interface e navegação: cada tela declara seus alvos de toque.

use crate::apps::{self, Kind, Library, Pomodoro, SysInfo};
use crate::fb::{Framebuffer, Rect, Wave};
use crate::input::Touch;
use crate::sys::{log, sh};
use crate::text::Fonts;
use crate::ui::{self, Module, Status};
use crate::{clock, data, home_dir, read_status, set_wifi};
use std::time::{Duration, Instant, SystemTime};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Home,
    Module(Module),
    Light,
    Library,
    Focus,
    Settings,
    LowBattery,
    Today,
    Reminder,
    Sleep,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Tile(usize),
    Back,
    Header,
    Minus,
    Plus,
    Bar,
    Night,
    OpenSettings,
    Row(usize),
    Prev,
    Next,
    Primary,
    Secondary,
    Wifi,
    Reboot,
    PowerOff,
    Item(usize),
}

/// Código de saída que pede ao `session.sh` para abrir um livro no KOReader e voltar.
pub const EXIT_OPEN_BOOK: i32 = 42;
pub const OPEN_FILE: &str = "/tmp/verso-ink.open-file";

pub struct App {
    pub fb: Framebuffer,
    pub fonts: Fonts,
    pub screen: Screen,
    pressed: Option<Target>,
    swipe_from: Option<(i32, i32)>,
    pub status: Status,
    light: ui::Light,
    lib: Library,
    pub pomo: Pomodoro,
    sys: SysInfo,
    confirm: Option<(u8, Instant)>,
    pub last_minute: u32,
    low_warned: bool,
    items: Vec<crate::day::Item>,
    today_hits: Vec<(usize, Rect)>,
    reminder: Option<crate::day::Item>,
    /// Tela para onde voltar depois de um lembrete.
    before_reminder: Screen,
    /// Quando definido, o laço principal sai com este código.
    pub exit_code: Option<i32>,
}

fn night_flag() -> std::path::PathBuf {
    home_dir().join("settings/night_mode")
}

fn read_light() -> ui::Light {
    let get = |p: &str| sh("lipc-get-prop", &["com.lab126.powerd", p]).and_then(|s| s.parse().ok());
    ui::Light { level: get("flIntensity").unwrap_or(0), max: get("flMaxIntensity").unwrap_or(25), night: night_flag().exists() }
}

/// Impede o Kindle de dormir durante um Pomodoro (o modo de teste mantém sempre).
fn keep_awake(on: bool) {
    let on = on || home_dir().join("settings/dev_keep_awake").exists();
    let _ = sh("lipc-set-prop", &["com.lab126.powerd", "preventScreenSaver", if on { "1" } else { "0" }]);
}

fn round_radius(t: Target, r: Rect) -> i32 {
    match t {
        Target::Tile(_) => 40,
        Target::Row(_) | Target::Bar | Target::Header => 0,
        Target::Item(_) => 0,
        Target::Night | Target::OpenSettings | Target::Wifi => 32,
        _ => r.h.min(r.w) / 2,
    }
}

impl App {
    pub fn new(fb: Framebuffer) -> Self {
        App {
            fb,
            fonts: Fonts::new(),
            screen: Screen::Home,
            pressed: None,
            swipe_from: None,
            status: read_status(),
            light: read_light(),
            lib: Library { books: Vec::new(), store_only: 0, page: 0 },
            pomo: Pomodoro::Idle,
            sys: SysInfo { wifi_on: true, connected: false, essid: None, ip: None, firmware: String::new() },
            confirm: None,
            last_minute: 99,
            low_warned: false,
            items: Vec::new(),
            today_hits: Vec::new(),
            reminder: None,
            before_reminder: Screen::Home,
            exit_code: None,
        }
    }

    pub fn render(&mut self, flash: bool) {
        self.status = read_status();
        // O repouso fica sempre claro, mesmo no modo noturno (escuro parecia aparelho apagado).
        self.fb.invert = self.light.night && self.screen != Screen::Sleep;
        let (fb, f, st) = (&mut self.fb, &self.fonts, &self.status);
        match self.screen {
            Screen::Home => ui::draw_home(fb, f, st, &data::summary(st.offset_min)),
            Screen::Module(m) => ui::draw_module(fb, f, m, st),
            Screen::Light => ui::draw_light(fb, f, &self.light, st),
            Screen::Library => apps::draw_library(fb, f, &self.lib, st),
            Screen::Focus => apps::draw_focus(fb, f, self.pomo, data::focus_today(st.offset_min), st),
            Screen::Settings => apps::draw_settings(fb, f, &self.sys, st, self.confirm.map(|c| c.0)),
            Screen::LowBattery => ui::draw_low_battery(fb, f, st),
            Screen::Today => {
                let done = crate::day::checked(data::today(st.offset_min));
                self.today_hits = crate::today_ui::draw_today(fb, f, &self.items, &done, st, data::focus_today(st.offset_min));
            }
            Screen::Reminder => {
                if let Some(it) = &self.reminder {
                    crate::today_ui::draw_reminder(fb, f, it);
                }
            }
            Screen::Sleep => ui::draw_sleep(fb, f, st, data::last_book().as_ref()),
        }
        self.fb.present(Wave::Quality, flash);
        self.last_minute = clock::now(self.status.offset_min).minute;
    }

    fn targets(&self) -> Vec<(Target, Rect)> {
        use Target::*;
        match self.screen {
            Screen::Home => {
                let mut v: Vec<_> = (0..Module::ALL.len()).map(|i| (Tile(i), ui::tile_rect(i))).collect();
                v.push((Header, ui::header_rect()));
                v
            }
            Screen::Module(_) => vec![(Back, ui::back_rect())],
            Screen::Light => vec![
                (Back, ui::back_rect()),
                (Minus, ui::minus_rect()),
                (Plus, ui::plus_rect()),
                (Bar, ui::bar_rect()),
                (Night, ui::night_rect()),
                (OpenSettings, ui::settings_row_rect()),
            ],
            Screen::Library => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..self.lib.visible().len()).map(|i| (Row(i), apps::lib_row_rect(i))));
                if self.lib.page > 0 {
                    v.push((Prev, apps::lib_prev_rect()));
                }
                if self.lib.page + 1 < self.lib.pages() {
                    v.push((Next, apps::lib_next_rect()));
                }
                v
            }
            Screen::Focus => {
                let mut v = vec![(Back, ui::back_rect()), (Secondary, apps::focus_secondary_rect())];
                if !matches!(self.pomo, Pomodoro::Running { .. }) {
                    v.push((Primary, apps::focus_primary_rect()));
                }
                v
            }
            Screen::Settings => vec![
                (Back, ui::back_rect()),
                (Wifi, apps::set_wifi_row()),
                (Reboot, apps::set_reboot_rect()),
                (PowerOff, apps::set_off_rect()),
            ],
            Screen::LowBattery => vec![(Primary, ui::low_battery_ok_rect())],
            Screen::Today => {
                let mut v = vec![(Back, ui::back_rect()), (Primary, crate::today_ui::pomodoro_rect())];
                v.extend(self.today_hits.iter().map(|(i, r)| (Item(*i), *r)));
                v
            }
            Screen::Reminder => vec![(Primary, crate::today_ui::reminder_ok_rect())],
            Screen::Sleep => vec![],
        }
    }

    fn hit(&self, x: i32, y: i32) -> Option<Target> {
        self.targets().into_iter().find(|(_, r)| r.contains(x, y)).map(|(t, _)| t)
    }

    fn rect_of(&self, t: Target) -> Option<Rect> {
        self.targets().into_iter().find(|(k, _)| *k == t).map(|(_, r)| r)
    }

    /// Destaque do toque: inverte o alvo (forma arredondada) e mostra rápido (DU).
    /// Inverter de novo restaura exatamente o desenho anterior.
    fn flip(&mut self, t: Target, pressed: bool) {
        if matches!(t, Target::Header | Target::Bar) {
            return; // a resposta é a própria mudança
        }
        let Some(r) = self.rect_of(t) else { return };
        self.fb.invert_round(r, round_radius(t, r));
        self.fb.refresh(r, if pressed { Wave::Fast } else { Wave::Quality }, false);
    }

    /// Prepara as capas que faltam na página visível, uma a uma, mostrando cada uma ao ficar pronta.
    fn fill_covers(&mut self) {
        for i in 0..self.lib.visible().len() {
            let b = &self.lib.visible()[i];
            if crate::covers::cached(&b.path, apps::THUMB_W, apps::THUMB_H).is_some() {
                continue;
            }
            let t0 = Instant::now();
            let thumb = crate::covers::generate(&b.path, apps::THUMB_W, apps::THUMB_H);
            log(&format!("capa {} em {} ms ({})", b.title, t0.elapsed().as_millis(), if thumb.is_some() { "ok" } else { "sem capa" }));
            let r = apps::cover_rect(i);
            apps::draw_cover(&mut self.fb, &self.fonts, r, b, thumb.as_ref());
            self.fb.refresh(r, Wave::Quality, false);
        }
    }

    pub fn go(&mut self, screen: Screen) {
        match screen {
            Screen::Light => self.light = read_light(),
            Screen::Library => self.lib = Library::scan(),
            Screen::Settings => {
                self.sys = SysInfo::read();
                self.confirm = None;
            }
            Screen::Today => self.items = crate::day::load_items(),
            _ => {}
        }
        self.screen = screen;
        log(&format!("tela: {screen:?}"));
        self.render(true);
        if screen == Screen::Library {
            self.fill_covers();
        }
    }

    fn set_light(&mut self, level: i32) {
        let level = level.clamp(0, self.light.max);
        if level != self.light.level {
            let _ = sh("lipc-set-prop", &["com.lab126.powerd", "flIntensity", &level.to_string()]);
            self.light.level = level;
            log(&format!("luz {level}"));
        }
        ui::draw_light_level(&mut self.fb, &self.fonts, &self.light);
        self.fb.refresh(Rect::new(0, 440, 1072, 380), Wave::Quality, false);
    }

    fn start_pomodoro(&mut self, kind: Kind) {
        // settings/dev_pomodoro_secs encurta a duração (teste do fim do Pomodoro).
        let secs = crate::sys::read_trim(home_dir().join("settings/dev_pomodoro_secs").to_str().unwrap_or(""))
            .and_then(|s| s.parse().ok())
            .unwrap_or(kind.minutes() * 60);
        let ends = SystemTime::now() + Duration::from_secs(secs);
        self.pomo = Pomodoro::Running { kind, ends };
        keep_awake(true);
        log(&format!("pomodoro {kind:?} por {} min", kind.minutes()));
        self.go(Screen::Focus);
    }

    fn activate(&mut self, t: Target, x: i32) {
        use Target::*;
        match (self.screen, t) {
            (_, Back) => self.go(Screen::Home),
            (Screen::Home, Header) => self.go(Screen::Light),
            (Screen::Home, Tile(i)) => match Module::ALL[i] {
                Module::Ler => self.go(Screen::Library),
                Module::Foco => self.go(Screen::Today),
                m => self.go(Screen::Module(m)),
            },
            (Screen::Light, Minus) => self.set_light(self.light.level - 1),
            (Screen::Light, Plus) => self.set_light(self.light.level + 1),
            (Screen::Light, Bar) => {
                let r = ui::bar_rect();
                self.set_light(((x - r.x) * self.light.max + r.w - 1) / r.w);
            }
            (Screen::Light, Night) => {
                self.light.night = !self.light.night;
                if self.light.night {
                    let _ = std::fs::write(night_flag(), "1");
                } else {
                    let _ = std::fs::remove_file(night_flag());
                }
                log(&format!("modo noturno {}", self.light.night));
                self.render(true);
            }
            (Screen::Light, OpenSettings) => self.go(Screen::Settings),
            (Screen::Library, Row(i)) => {
                let Some(b) = self.lib.visible().get(i) else { return };
                let (path, title) = (b.path.clone(), b.title.clone());
                data::set_last_book(&path, &title);
                let _ = std::fs::write(OPEN_FILE, path.to_string_lossy().as_bytes());
                apps::draw_opening_book(&mut self.fb, &self.fonts, &title);
                self.fb.present(Wave::Quality, true);
                self.fb.wait();
                log(&format!("abrindo no KOReader: {}", path.display()));
                self.exit_code = Some(EXIT_OPEN_BOOK);
            }
            (Screen::Library, Prev) => {
                self.lib.page = self.lib.page.saturating_sub(1);
                self.render(true);
                self.fill_covers();
            }
            (Screen::Library, Next) => {
                self.lib.page = (self.lib.page + 1).min(self.lib.pages() - 1);
                self.render(true);
                self.fill_covers();
            }
            (Screen::Focus, Primary) => match self.pomo {
                Pomodoro::Done { kind: Kind::Focus } => self.start_pomodoro(Kind::Break),
                _ => self.start_pomodoro(Kind::Focus),
            },
            (Screen::Focus, Secondary) => match self.pomo {
                Pomodoro::Idle => self.start_pomodoro(Kind::Break),
                Pomodoro::Running { .. } => {
                    self.pomo = Pomodoro::Idle;
                    keep_awake(false);
                    log("pomodoro parado");
                    self.render(true);
                }
                Pomodoro::Done { .. } => {
                    self.pomo = Pomodoro::Idle;
                    self.go(Screen::Home);
                }
            },
            (Screen::Settings, Wifi) => {
                set_wifi(!self.sys.wifi_on);
                std::thread::sleep(Duration::from_millis(600));
                self.sys = SysInfo::read();
                self.render(false);
            }
            (Screen::Settings, Reboot) | (Screen::Settings, PowerOff) => {
                let which = if t == Reboot { 0 } else { 1 };
                let confirmed = self.confirm.is_some_and(|(w, at)| w == which && at.elapsed() < Duration::from_secs(5));
                if !confirmed {
                    self.confirm = Some((which, Instant::now()));
                    self.render(false);
                    return;
                }
                let reboot = which == 0;
                log(if reboot { "reiniciando pelo Ajustes" } else { "desligando pelo Ajustes" });
                self.fb.invert = false;
                apps::draw_powered_off(&mut self.fb, &self.fonts, reboot);
                self.fb.present(Wave::Quality, true);
                self.fb.wait();
                let _ = sh("sync", &[]);
                let _ = std::process::Command::new(if reboot { "reboot" } else { "poweroff" }).status();
            }
            (Screen::LowBattery, Primary) => self.go(Screen::Home),
            (Screen::Today, Primary) => self.go(Screen::Focus),
            (Screen::Today, Item(i)) => {
                if let Some(it) = self.items.get(i) {
                    let on = crate::day::toggle(data::today(self.status.offset_min), &it.key());
                    log(&format!("{} {}", if on { "marcado" } else { "desmarcado" }, it.key()));
                    self.render(false);
                }
            }
            (Screen::Reminder, Primary) => {
                if let Some(it) = self.reminder.take() {
                    crate::day::toggle(data::today(self.status.offset_min), &it.key());
                }
                let back = self.before_reminder;
                self.go(back);
            }
            _ => {}
        }
    }

    pub fn on_touch(&mut self, t: Touch) {
        match t {
            Touch::Down(x, y) => {
                self.swipe_from = Some((x, y));
                if let Some(h) = self.hit(x, y) {
                    self.pressed = Some(h);
                    self.flip(h, true);
                }
            }
            Touch::Move(x, y) => {
                // Arrastar para fora cancela o toque, como num botão nativo.
                if let Some(h) = self.pressed {
                    if self.hit(x, y) != Some(h) {
                        self.pressed = None;
                        self.flip(h, false);
                    }
                }
            }
            Touch::Up(x, y) => {
                let from = self.swipe_from.take();
                // Deslizar do topo para baixo na tela inicial abre Luz e tela.
                if self.screen == Screen::Home && from.is_some_and(|(_, fy)| fy < 180 && y - fy > 250) {
                    if let Some(h) = self.pressed.take() {
                        self.flip(h, false);
                    }
                    return self.go(Screen::Light);
                }
                let Some(h) = self.pressed.take() else { return };
                self.flip(h, false);
                if self.hit(x, y) == Some(h) {
                    self.activate(h, x);
                }
            }
        }
    }

    /// Chamado várias vezes por segundo: relógio, Pomodoro, confirmação e bateria.
    pub fn tick(&mut self) {
        let now = clock::now(self.status.offset_min);
        let minute_changed = now.minute != self.last_minute;

        if let Pomodoro::Running { kind, ends } = self.pomo {
            if SystemTime::now() >= ends {
                if kind == Kind::Focus {
                    data::add_focus();
                }
                self.pomo = Pomodoro::Done { kind };
                keep_awake(false);
                log(&format!("pomodoro {kind:?} concluído"));
                self.go(Screen::Focus);
                return;
            }
            if self.screen == Screen::Focus && minute_changed {
                self.last_minute = now.minute;
                apps::draw_focus_live(&mut self.fb, &self.fonts, self.pomo, self.status.offset_min);
                self.fb.refresh(apps::focus_live_rect(), Wave::Quality, false);
                return;
            }
        }
        if self.screen == Screen::Settings {
            if let Some((_, at)) = self.confirm {
                if at.elapsed() >= Duration::from_secs(5) {
                    self.confirm = None;
                    self.render(false);
                }
            }
        }
        if !minute_changed {
            return;
        }
        self.last_minute = now.minute;
        self.status = read_status();
        // Lembretes: na hora (ou ao acordar, se passou), em tela cheia.
        if !matches!(self.screen, Screen::Reminder | Screen::Sleep) && now.plausible() {
            let items = crate::day::load_items();
            if let Some(it) = crate::day::due_reminder(&items, self.status.offset_min, now.hour * 60 + now.minute) {
                log(&format!("lembrete: {}", it.text));
                self.before_reminder = if matches!(self.screen, Screen::LowBattery) { Screen::Home } else { self.screen };
                self.reminder = Some(it);
                return self.go(Screen::Reminder);
            }
        }
        // Aviso de bateria fraca: uma vez por descarga, só fora do carregador.
        match self.status.battery {
            Some(p) if p <= 10 && !self.status.charging && !self.low_warned && self.screen != Screen::Sleep => {
                self.low_warned = true;
                log(&format!("bateria fraca: {p}%"));
                return self.go(Screen::LowBattery);
            }
            _ if self.status.charging => self.low_warned = false,
            _ => {}
        }
        if self.screen == Screen::Home {
            let st = &self.status;
            ui::draw_clock(&mut self.fb, &self.fonts, st);
            ui::draw_header(&mut self.fb, &self.fonts, st);
            ui::draw_footer(&mut self.fb, &self.fonts, st);
            // A cada hora cheia, uma atualização completa limpa os fantasmas do e-ink.
            if now.minute == 0 {
                self.render(true);
            } else {
                let c = ui::clock_rect();
                let d = ui::date_rect();
                self.fb.refresh(Rect::new(0, d.y, 1072, c.y + c.h - d.y), Wave::Quality, false);
                self.fb.refresh(Rect::new(0, 1300, 1072, 148), Wave::Quality, false);
            }
        }
    }
}
