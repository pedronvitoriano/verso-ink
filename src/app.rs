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
    Games,
    SudokuMenu,
    Sudoku,
    Chess,
    Clips,
    ClipBook,
    Compose,
    Claude,
    Update,
    Wifi,
    WifiPass,
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
    Cell(usize),
    Key(u8),
    Level(usize),
    Continue,
    Check,
    NewGame,
    Undo,
    Game(usize),
    Clips,
    BookRow(usize),
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
    sudoku: Option<crate::sudoku::Game>,
    sudoku_note: String,
    chess: Option<crate::chess::Game>,
    sprites: Option<crate::chess::Sprites>,
    clips: Vec<crate::clippings::BookClips>,
    clip_page: usize,
    clip_book: usize,
    clip_pages: Vec<Vec<(String, bool)>>,
    clip_bpage: usize,
    compose_kind: usize,
    compose_text: String,
    compose_hint: String,
    kb_mode: crate::keyboard::Mode,
    kb_shift: bool,
    keys: Vec<crate::keyboard::Key>,
    claude: Vec<crate::claude_ui::Session>,
    claude_stamp: u64,
    claude_checked: Instant,
    update_msg: (String, Vec<String>, Option<String>),
    nets: Vec<crate::wifi::Net>,
    wifi_cur: Option<String>,
    wifi_on: bool,
    wifi_note: String,
    wifi_target: Option<crate::wifi::Net>,
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
            sudoku: None,
            sudoku_note: String::new(),
            chess: None,
            sprites: None,
            clips: Vec::new(),
            clip_page: 0,
            clip_book: 0,
            clip_pages: Vec::new(),
            clip_bpage: 0,
            compose_kind: 0,
            compose_text: String::new(),
            compose_hint: String::new(),
            kb_mode: crate::keyboard::Mode::Letters,
            kb_shift: true,
            keys: Vec::new(),
            claude: Vec::new(),
            claude_stamp: 0,
            claude_checked: Instant::now(),
            update_msg: (String::new(), Vec::new(), None),
            nets: Vec::new(),
            wifi_cur: None,
            wifi_on: true,
            wifi_note: String::new(),
            wifi_target: None,
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
            Screen::Games => {
                let s = crate::sudoku::Game::load().filter(|g| !g.solved()).map(|g| format!("Jogo em andamento · {} · {}/81", g.level.name(), g.filled()));
                let c = crate::chess::Game::load().filter(|g| !g.history.is_empty()).map(|g| format!("Partida em andamento · {} lances", g.history.len()));
                apps::draw_games(fb, f, s, c);
            }
            Screen::SudokuMenu => crate::sudoku::draw_menu(fb, f, crate::sudoku::Game::load().filter(|g| !g.solved()).as_ref()),
            Screen::Sudoku => {
                if let Some(g) = &self.sudoku {
                    crate::sudoku::draw_board(fb, f, g, &self.sudoku_note);
                }
            }
            Screen::Chess => {
                if let (Some(g), Some(sp)) = (&self.chess, &self.sprites) {
                    crate::chess::draw(fb, f, g, sp);
                }
            }
            Screen::Clips => crate::clippings::draw_books(fb, f, &self.clips, self.clip_page),
            Screen::Claude => crate::claude_ui::draw(fb, f, &self.claude),
            Screen::Wifi => crate::wifi::draw(fb, f, self.wifi_on, self.wifi_cur.as_deref(), &self.nets, &self.wifi_note),
            Screen::WifiPass => {
                let essid = self.wifi_target.as_ref().map(|n| n.essid.clone()).unwrap_or_default();
                crate::wifi::draw_password_header(fb, f, &essid);
                crate::today_ui::draw_field(fb, f, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
            Screen::Update => {
                let (t, l, b) = &self.update_msg;
                apps::draw_update(fb, f, t, l, b.as_deref());
            }
            Screen::Compose => {
                crate::today_ui::draw_compose(fb, f, self.compose_kind, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
            Screen::ClipBook => {
                if let Some(b) = self.clips.get(self.clip_book) {
                    crate::clippings::draw_book(fb, f, b, &self.clip_pages, self.clip_bpage);
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
                let mut v = vec![(Back, ui::back_rect()), (Clips, apps::clips_rect())];
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
                (Check, apps::set_update_rect()),
            ],
            Screen::Claude => vec![(Back, ui::back_rect())],
            Screen::Wifi => {
                let mut v = vec![(Back, ui::back_rect()), (Wifi, crate::wifi::toggle_rect()), (Check, crate::wifi::rescan_rect())];
                if self.wifi_on {
                    v.extend((0..self.nets.len().min(crate::wifi::ROWS)).map(|i| (Row(i), crate::wifi::row_rect(i))));
                }
                v
            }
            Screen::WifiPass => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend(self.keys.iter().enumerate().map(|(i, k)| (Key(i as u8), k.rect)));
                v
            }
            Screen::Update => {
                if self.update_msg.2.is_some() {
                    vec![(Primary, apps::update_ok_rect())]
                } else {
                    vec![]
                }
            }
            Screen::LowBattery => vec![(Primary, ui::low_battery_ok_rect())],
            Screen::Compose => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..4).map(|k| (Level(k), crate::today_ui::chip_rect(k))));
                v.extend(self.keys.iter().enumerate().map(|(i, k)| (Key(i as u8), k.rect)));
                v
            }
            Screen::Today => {
                let mut v = vec![(Back, ui::back_rect()), (Primary, crate::today_ui::pomodoro_rect()), (NewGame, crate::today_ui::add_rect())];
                v.extend(self.today_hits.iter().map(|(i, r)| (Item(*i), *r)));
                v
            }
            Screen::Reminder => vec![(Primary, crate::today_ui::reminder_ok_rect())],
            Screen::Games => vec![(Back, ui::back_rect()), (Game(0), apps::games_row(0)), (Game(1), apps::games_row(1))],
            Screen::SudokuMenu => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..3).map(|k| (Level(k), crate::sudoku::level_rect(k))));
                if crate::sudoku::Game::load().is_some_and(|g| !g.solved()) {
                    v.push((Continue, crate::sudoku::continue_rect()));
                }
                v
            }
            Screen::Sudoku => {
                let mut v = vec![(Back, ui::back_rect()), (Check, crate::sudoku::check_rect()), (NewGame, crate::sudoku::new_game_rect())];
                v.extend((0..81).map(|i| (Cell(i), crate::sudoku::cell_rect(i))));
                v.extend((0..=9u8).map(|n| (Key(n), crate::sudoku::key_rect(n))));
                v
            }
            Screen::Chess => {
                let mut v = vec![(Back, ui::back_rect()), (Undo, crate::chess::undo_rect()), (NewGame, crate::chess::new_rect())];
                v.extend((0..64).map(|i| (Cell(i), crate::chess::square_rect(i))));
                v
            }
            Screen::Clips => {
                let mut v = vec![(Back, ui::back_rect())];
                let n = self.clips.len().saturating_sub(self.clip_page * crate::clippings::BOOKS_PER_PAGE).min(crate::clippings::BOOKS_PER_PAGE);
                v.extend((0..n).map(|i| (BookRow(i), crate::clippings::book_row(i))));
                if self.clip_page > 0 {
                    v.push((Prev, crate::clippings::prev_rect()));
                }
                if (self.clip_page + 1) * crate::clippings::BOOKS_PER_PAGE < self.clips.len() {
                    v.push((Next, crate::clippings::next_rect()));
                }
                v
            }
            Screen::ClipBook => {
                let mut v = vec![(Back, ui::back_rect())];
                if self.clip_bpage > 0 {
                    v.push((Prev, crate::clippings::prev_rect()));
                }
                if self.clip_bpage + 1 < self.clip_pages.len() {
                    v.push((Next, crate::clippings::next_rect()));
                }
                v
            }
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
        if matches!(t, Target::Header | Target::Bar | Target::Cell(_)) {
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

    fn show_update(&mut self, title: &str, lines: &[&str], button: Option<&str>) {
        self.update_msg = (title.into(), lines.iter().map(|s| s.to_string()).collect(), button.map(str::to_string));
        self.screen = Screen::Update;
        self.render(true);
        self.fb.wait();
    }

    /// Procura, baixa e instala a versão nova, mostrando cada etapa na tela.
    fn run_update(&mut self) {
        use crate::update::{check, install, Check, EXIT_UPDATED};
        self.show_update("Atualização", &["Procurando a versão mais nova no GitHub…"], None);
        match check() {
            Check::Failed(msg) => self.show_update("Não deu", &[&msg], Some("Voltar")),
            Check::UpToDate(v) => self.show_update("Tudo em dia", &[&format!("Você já tem a versão mais nova ({v})."), ""], Some("Voltar")),
            Check::Available { version, url, sha_url } => {
                self.show_update("Atualizando", &[&format!("Baixando a versão {version}…"), "Não desligue o Kindle."], None);
                match install(&url, &sha_url) {
                    Ok(()) => {
                        self.show_update("Atualizado", &[&format!("Versão {version} instalada."), "Reiniciando o Verso Ink…"], None);
                        self.exit_code = Some(EXIT_UPDATED);
                    }
                    Err(e) => self.show_update("Não deu", &[&e], Some("Voltar")),
                }
            }
        }
    }

    fn press_key(&mut self, i: usize) {
        use crate::keyboard::{Action, Mode};
        let Some(action) = self.keys.get(i).map(|k| k.action.clone()) else { return };
        let mut relayout = false;
        match action {
            Action::Char(c) => {
                self.compose_text.push(c);
                if self.kb_shift && self.kb_mode != Mode::Numbers {
                    self.kb_shift = false;
                    relayout = true;
                }
            }
            Action::Space => self.compose_text.push(' '),
            Action::Backspace => {
                self.compose_text.pop();
            }
            Action::Shift => {
                self.kb_shift = !self.kb_shift;
                relayout = true;
            }
            Action::Mode(m) => {
                self.kb_mode = m;
                relayout = true;
            }
            Action::Done => {
                if self.screen == Screen::WifiPass {
                    let (Some(n), psk) = (self.wifi_target.clone(), self.compose_text.clone()) else { return };
                    if psk.chars().count() < 8 {
                        self.compose_hint = "Senhas de Wi-Fi têm pelo menos 8 caracteres.".into();
                    } else {
                        return self.connect_wifi(n, Some(psk));
                    }
                } else {
                    return self.save_compose();
                }
            }
        }
        crate::today_ui::draw_field(&mut self.fb, &self.fonts, &self.compose_text, &self.compose_hint);
        self.fb.refresh(Rect::new(0, crate::today_ui::field_rect().y - 4, 1072, 470), Wave::Quality, false);
        if relayout {
            self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
            crate::keyboard::draw(&mut self.fb, &self.fonts, &self.keys);
            self.fb.refresh(crate::keyboard::area(), Wave::Quality, false);
        }
    }

    fn refresh_wifi(&mut self) {
        self.wifi_on = crate::wifi::enabled();
        self.wifi_cur = if crate::wifi::connected() { crate::wifi::current() } else { None };
        self.nets = crate::wifi::networks();
    }

    /// Conecta mostrando a espera na tela; o resultado aparece na lista.
    fn connect_wifi(&mut self, n: crate::wifi::Net, psk: Option<String>) {
        self.wifi_note = format!("Conectando em {}… (até 30 s)", n.essid);
        self.screen = Screen::Wifi;
        self.render(true);
        self.fb.wait();
        self.wifi_note = match crate::wifi::connect(&n.essid, psk.as_deref(), n.secured, Duration::from_secs(30)) {
            Ok(e) => format!("Conectado a {e}."),
            Err(e) => e,
        };
        self.compose_text.clear();
        self.refresh_wifi();
        self.render(false);
    }

    fn save_compose(&mut self) {
        let text = self.compose_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mark = crate::today_ui::KINDS[self.compose_kind].1;
        let line = format!("{mark} {text}");
        if text.is_empty() || crate::day::parse(&line).is_empty() {
            self.compose_hint = if mark == '@' { "Para lembrete, comece com a hora: 14:30 Texto".into() } else { "Escreva alguma coisa antes do OK.".into() };
            crate::today_ui::draw_field(&mut self.fb, &self.fonts, &self.compose_text, &self.compose_hint);
            self.fb.refresh(Rect::new(0, crate::today_ui::field_rect().y - 4, 1072, 470), Wave::Quality, false);
            return;
        }
        let path = home_dir().join("data/dia.txt");
        let mut cur = std::fs::read_to_string(&path).unwrap_or_default();
        if !cur.is_empty() && !cur.ends_with('\n') {
            cur.push('\n');
        }
        cur.push_str(&line);
        cur.push('\n');
        let _ = std::fs::write(&path, cur);
        log(&format!("adicionado ao dia: {line}"));
        self.go(Screen::Today);
    }

    fn redraw_sudoku_grid(&mut self) {
        let Some(g) = &self.sudoku else { return };
        for i in 0..81 {
            crate::sudoku::draw_cell(&mut self.fb, &self.fonts, g, i);
        }
        crate::sudoku::draw_grid_lines(&mut self.fb);
        let n = crate::sudoku::note_rect();
        self.fb.fill(n, ui::PAPER);
        self.fonts.draw(&mut self.fb, crate::text::Face::Body, 32.0, crate::sudoku::GRID_X, crate::sudoku::GRID_Y - 40, ui::MUTED, &self.sudoku_note);
        self.fb.refresh(Rect::new(0, n.y, 1072, crate::sudoku::grid_rect().y + crate::sudoku::grid_rect().h - n.y), Wave::Quality, false);
    }

    fn redraw_chess(&mut self) {
        let (Some(g), Some(sp)) = (&self.chess, &self.sprites) else { return };
        for i in 0..64 {
            crate::chess::draw_square(&mut self.fb, g, sp, i);
        }
        crate::chess::draw_status(&mut self.fb, &self.fonts, g);
        let b = crate::chess::board_rect();
        let s = crate::chess::status_rect();
        self.fb.refresh(Rect::new(0, b.y, 1072, s.y + s.h - b.y), Wave::Quality, false);
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
            Screen::Wifi => self.refresh_wifi(),
            Screen::Claude => {
                self.claude = crate::claude_ui::load();
                self.claude_stamp = crate::claude_ui::stamp();
            }
            Screen::Clips => {
                self.clips = crate::clippings::load();
                self.clip_page = 0;
            }
            Screen::Chess => {
                if self.sprites.is_none() {
                    let t = Instant::now();
                    self.sprites = Some(crate::chess::Sprites::new());
                    log(&format!("peças desenhadas em {} ms", t.elapsed().as_millis()));
                }
                if self.chess.is_none() {
                    self.chess = Some(crate::chess::Game::load().unwrap_or_else(crate::chess::Game::new));
                }
            }
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
        self.set_light_with(level, Wave::Quality);
    }

    /// Nível pela posição do dedo na barra (vale também fora dela na vertical, como um controle deslizante).
    fn bar_level(&self, x: i32) -> i32 {
        let r = ui::bar_rect();
        ((x - r.x).clamp(0, r.w) * self.light.max + r.w - 1) / r.w
    }

    fn set_light_with(&mut self, level: i32, wave: Wave) {
        let level = level.clamp(0, self.light.max);
        if level != self.light.level {
            let _ = sh("lipc-set-prop", &["com.lab126.powerd", "flIntensity", &level.to_string()]);
            self.light.level = level;
            log(&format!("luz {level}"));
        }
        ui::draw_light_level(&mut self.fb, &self.fonts, &self.light);
        self.fb.refresh(Rect::new(0, 440, 1072, 380), wave, false);
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
            (Screen::SudokuMenu | Screen::Sudoku | Screen::Chess, Back) => self.go(Screen::Games),
            (Screen::Clips, Back) => self.go(Screen::Library),
            (Screen::ClipBook, Back) => self.go(Screen::Clips),
            (Screen::Compose, Back) => self.go(Screen::Today),
            (Screen::Settings, Check) => self.run_update(),
            (Screen::Update, Primary) => self.go(Screen::Settings),
            (Screen::Today, NewGame) => {
                self.compose_text.clear();
                self.compose_kind = 0;
                self.kb_mode = crate::keyboard::Mode::Letters;
                self.kb_shift = true;
                self.compose_hint = "Escreva e toque em OK.".into();
                self.go(Screen::Compose);
            }
            (Screen::Compose, Level(k)) => {
                self.compose_kind = k;
                self.compose_hint = if k == 2 { "Comece com a hora, por exemplo: 14:30 Ligar para a Ana".into() } else { "Escreva e toque em OK.".into() };
                if k == 2 && self.compose_text.is_empty() {
                    self.kb_mode = crate::keyboard::Mode::Numbers;
                }
                self.render(false);
            }
            (Screen::Compose | Screen::WifiPass, Key(i)) => self.press_key(i as usize),
            (Screen::Wifi, Back) => self.go(Screen::Settings),
            (Screen::WifiPass, Back) => self.go(Screen::Wifi),
            (_, Back) => self.go(Screen::Home),
            (Screen::Home, Header) => self.go(Screen::Light),
            (Screen::Home, Tile(i)) => match Module::ALL[i] {
                Module::Ler => self.go(Screen::Library),
                Module::Foco => self.go(Screen::Today),
                Module::Jogar => self.go(Screen::Games),
                Module::Claude => self.go(Screen::Claude),
                m => self.go(Screen::Module(m)),
            },
            (Screen::Light, Minus) => self.set_light(self.light.level - 1),
            (Screen::Light, Plus) => self.set_light(self.light.level + 1),
            (Screen::Light, Bar) => self.set_light(self.bar_level(x)),
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
            (Screen::Library, Clips) => self.go(Screen::Clips),
            (Screen::Clips, BookRow(i)) => {
                self.clip_book = self.clip_page * crate::clippings::BOOKS_PER_PAGE + i;
                if let Some(b) = self.clips.get(self.clip_book) {
                    self.clip_pages = crate::clippings::paginate(&self.fonts, b);
                    self.clip_bpage = 0;
                    self.go(Screen::ClipBook);
                }
            }
            (Screen::Clips, Prev) => {
                self.clip_page = self.clip_page.saturating_sub(1);
                self.render(true);
            }
            (Screen::Clips, Next) => {
                self.clip_page += 1;
                self.render(true);
            }
            (Screen::ClipBook, Prev) => {
                self.clip_bpage = self.clip_bpage.saturating_sub(1);
                self.render(true);
            }
            (Screen::ClipBook, Next) => {
                self.clip_bpage += 1;
                self.render(true);
            }
            (Screen::Games, Game(0)) => self.go(Screen::SudokuMenu),
            (Screen::Games, Game(_)) => self.go(Screen::Chess),
            (Screen::SudokuMenu, Level(k)) => {
                let lv = [crate::sudoku::Level::Easy, crate::sudoku::Level::Medium, crate::sudoku::Level::Hard][k.min(2)];
                let t = Instant::now();
                let g = crate::sudoku::Game::new(lv);
                log(&format!("sudoku {} gerado em {} ms", lv.name(), t.elapsed().as_millis()));
                g.save();
                self.sudoku = Some(g);
                self.sudoku_note = "Toque numa casa e depois num número.".into();
                self.go(Screen::Sudoku);
            }
            (Screen::SudokuMenu, Continue) => {
                self.sudoku = crate::sudoku::Game::load();
                self.sudoku_note = "Jogo retomado.".into();
                self.go(Screen::Sudoku);
            }
            (Screen::Sudoku, Cell(i)) => {
                if let Some(g) = &mut self.sudoku {
                    g.selected = if g.selected == Some(i) { None } else { Some(i) };
                }
                self.redraw_sudoku_grid();
            }
            (Screen::Sudoku, Key(n)) => {
                let solved = if let Some(g) = &mut self.sudoku {
                    g.set(n);
                    g.save();
                    g.solved()
                } else {
                    false
                };
                if solved {
                    self.sudoku_note = "Resolvido! Parabéns.".into();
                    crate::sudoku::Game::clear_saved();
                    if let Some(g) = &mut self.sudoku {
                        g.selected = None;
                    }
                    self.render(true);
                } else {
                    self.redraw_sudoku_grid();
                }
            }
            (Screen::Sudoku, Check) => {
                let bad = self.sudoku.as_mut().map(|g| g.check()).unwrap_or(0);
                self.sudoku_note = match bad {
                    0 => "Nenhum erro até aqui.".into(),
                    1 => "1 casa errada, marcada com um traço.".into(),
                    n => format!("{n} casas erradas, marcadas com um traço."),
                };
                self.redraw_sudoku_grid();
            }
            (Screen::Sudoku, NewGame) => self.go(Screen::SudokuMenu),
            (Screen::Chess, Cell(i)) => {
                if let Some(g) = &mut self.chess {
                    g.tap(i);
                }
                self.redraw_chess();
            }
            (Screen::Chess, Undo) => {
                if let Some(g) = &mut self.chess {
                    g.undo();
                }
                self.redraw_chess();
            }
            (Screen::Chess, NewGame) => {
                let g = crate::chess::Game::new();
                g.save();
                self.chess = Some(g);
                self.render(true);
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
                self.wifi_note = "Toque numa rede para conectar.".into();
                self.go(Screen::Wifi);
            }
            (Screen::Wifi, Wifi) => {
                set_wifi(!self.wifi_on);
                self.wifi_note = if self.wifi_on { "Wi-Fi desligado.".into() } else { "Ligando o Wi-Fi…".into() };
                std::thread::sleep(Duration::from_millis(1500));
                self.refresh_wifi();
                self.render(false);
            }
            (Screen::Wifi, Check) => {
                self.wifi_note = "Procurando redes…".into();
                self.render(false);
                self.fb.wait();
                crate::wifi::scan();
                std::thread::sleep(Duration::from_secs(5));
                self.refresh_wifi();
                self.wifi_note = format!("{} redes encontradas.", self.nets.len());
                self.render(false);
            }
            (Screen::Wifi, Row(i)) => {
                let Some(n) = self.nets.get(i).cloned() else { return };
                log(&format!("wifi: tocou em {} (conhecida={}, protegida={})", n.essid, n.known, n.secured));
                if self.wifi_cur.as_deref() == Some(n.essid.as_str()) {
                    return;
                }
                if n.secured && !n.known {
                    self.wifi_target = Some(n);
                    self.compose_text.clear();
                    self.compose_hint = "Digite a senha e toque em OK.".into();
                    self.kb_mode = crate::keyboard::Mode::Letters;
                    self.kb_shift = false;
                    self.go(Screen::WifiPass);
                } else {
                    self.connect_wifi(n, None);
                }
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
                    // Barra de luz: já ajusta no toque, com a atualização rápida do e-ink.
                    if h == Target::Bar {
                        let lv = self.bar_level(x);
                        self.set_light_with(lv, Wave::Fast);
                    }
                }
            }
            Touch::Move(x, _) if self.pressed == Some(Target::Bar) => {
                // Arrastando na barra: a luz acompanha o dedo.
                let lv = self.bar_level(x);
                if lv != self.light.level {
                    self.set_light_with(lv, Wave::Fast);
                }
            }
            Touch::Up(x, _) if self.pressed == Some(Target::Bar) => {
                self.pressed = None;
                self.swipe_from = None;
                let lv = self.bar_level(x);
                self.set_light_with(lv, Wave::Quality);
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
        // Bloco Claude: atualiza sozinho quando chega evento novo do PC.
        if self.screen == Screen::Claude && self.claude_checked.elapsed() >= Duration::from_secs(2) {
            self.claude_checked = Instant::now();
            let s = crate::claude_ui::stamp();
            if s != self.claude_stamp {
                self.claude_stamp = s;
                self.claude = crate::claude_ui::load();
                self.render(false);
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
