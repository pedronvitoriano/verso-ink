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
    SudokuWon,
    ChessHub,
    Coaches,
    Chess,
    ChessEnd,
    ChessGames,
    ChessReview,
    ChessInsights,
    Mini,
    Clips,
    ClipBook,
    Compose,
    Claude,
    ClaudeLive,
    ClaudeChat,
    ClaudeCompose,
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
    Mini(u32),
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
    sudoku_won: (u64, bool),
    chess: Option<crate::chess::Game>,
    sprites: Option<crate::chess::Sprites>,
    engine: Option<crate::engine::Engine>,
    coach_msg: String,
    pre_eval: Option<(crate::engine::Score, String)>,
    records: Vec<crate::coach::Record>,
    coach_page: usize,
    end_info: (String, String, Option<(u32, u32)>, Option<String>),
    saved_games: Vec<crate::coach::Saved>,
    games_page: usize,
    review_k: usize,
    rng: crate::sudoku::Rng,
    mini: Option<Box<dyn crate::minigame::MiniGame>>,
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
    chats: Result<Vec<crate::chat::ChatInfo>, String>,
    cur_chat: Option<crate::chat::Chat>,
    chat_pages: Vec<Vec<(String, bool, bool)>>,
    chat_page: usize,
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
            sudoku_won: (0, false),
            chess: None,
            sprites: None,
            engine: None,
            coach_msg: String::new(),
            pre_eval: None,
            records: Vec::new(),
            coach_page: 0,
            end_info: (String::new(), String::new(), None, None),
            saved_games: Vec::new(),
            games_page: 0,
            review_k: 0,
            rng: crate::sudoku::Rng::new(),
            mini: None,
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
            chats: Ok(Vec::new()),
            cur_chat: None,
            chat_pages: Vec::new(),
            chat_page: 0,
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
        self.fb.rot180 = self.screen == Screen::Chess && self.local_rotated();
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
            Screen::SudokuMenu => crate::sudoku::draw_resume(fb, f, crate::sudoku::Game::load().filter(|g| !g.solved()).as_ref()),
            Screen::Sudoku => {
                if let Some(g) = &self.sudoku {
                    crate::sudoku::draw_board(fb, f, g, &self.sudoku_note);
                }
            }
            Screen::SudokuWon => {
                if let Some(g) = &self.sudoku {
                    crate::sudoku::draw_won(fb, f, g, self.sudoku_won.0, self.sudoku_won.1);
                }
            }
            Screen::ChessHub => {
                let resume = crate::chess::Game::load().filter(|g| g.in_progress()).map(|g| match g.mode {
                    crate::chess::Mode::Coach { level, .. } => {
                        let c = crate::coach::coach(level);
                        format!("Contra {} ({}) · {} lances", c.name, c.rating, g.history.len())
                    }
                    crate::chess::Mode::Local => format!("Duas pessoas · {} lances", g.history.len()),
                });
                crate::chess_ui::draw_hub(fb, f, resume);
            }
            Screen::Coaches => crate::chess_ui::draw_coaches(fb, f, self.coach_page),
            Screen::Chess => {
                if let (Some(g), Some(sp)) = (&self.chess, &self.sprites) {
                    crate::chess::draw(fb, f, g, sp);
                    if let crate::chess::Mode::Coach { level, .. } = g.mode {
                        let c = crate::coach::coach(level);
                        crate::chess_ui::draw_coach_line(fb, f, &format!("{} ({})", c.name, c.rating), &self.coach_msg);
                    }
                }
            }
            Screen::ChessEnd => {
                let (t, d, r, u) = &self.end_info;
                crate::chess_ui::draw_end(fb, f, t, d, *r, u.clone());
            }
            Screen::ChessGames => crate::chess_ui::draw_games(fb, f, &self.saved_games, self.games_page, st.offset_min),
            Screen::ChessReview => {
                if let (Some(g), Some(sp)) = (self.saved_games.first(), &self.sprites) {
                    crate::chess_ui::draw_review(fb, f, sp, g, self.review_k);
                }
            }
            Screen::ChessInsights => {
                if let Some(g) = self.saved_games.first() {
                    // Dica de treino: a frase mais acionável dos insights (motivo recorrente).
                    let lines = crate::coach::insights(g, &self.saved_games[1..]);
                    let tip = lines.iter().find(|l| l.contains("Antes de jogar") || l.contains("Olhe sempre") || l.contains("Procure xeques")).cloned();
                    crate::chess_ui::draw_insights(fb, f, g, &self.saved_games[1..], tip.as_deref());
                }
            }
            Screen::Clips => crate::clippings::draw_books(fb, f, &self.clips, self.clip_page),
            Screen::Mini => {
                if let Some(g) = &self.mini {
                    g.draw(fb, f);
                }
            }
            Screen::Claude => crate::chat::draw_hub(fb, f, &self.chats, self.claude.len()),
            Screen::ClaudeLive => crate::claude_ui::draw(fb, f, &self.claude),
            Screen::ClaudeChat => {
                if let Some(c) = &self.cur_chat {
                    crate::chat::draw_chat(fb, f, c, &self.chat_pages, self.chat_page);
                }
            }
            Screen::ClaudeCompose => {
                let title = self.cur_chat.as_ref().map(|c| c.title.clone()).unwrap_or_else(|| "Nova conversa".into());
                crate::chat::draw_compose_header(fb, f, &title);
                crate::today_ui::draw_field(fb, f, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
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
            Screen::Claude => {
                let mut v = vec![(Back, ui::back_rect()), (NewGame, crate::chat::new_rect()), (Continue, crate::chat::live_rect())];
                if let Ok(c) = &self.chats {
                    v.extend((0..c.len().min(crate::chat::ROWS)).map(|i| (Row(i), crate::chat::chat_row(i))));
                }
                v
            }
            Screen::ClaudeLive => vec![(Back, ui::back_rect())],
            Screen::ClaudeChat => {
                let mut v = vec![(Back, ui::back_rect()), (Primary, crate::chat::reply_rect())];
                if self.chat_page > 0 {
                    v.push((Prev, crate::chat::prev_rect()));
                }
                if self.chat_page + 1 < self.chat_pages.len() {
                    v.push((Next, crate::chat::next_rect()));
                }
                v
            }
            Screen::ClaudeCompose => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend(self.keys.iter().enumerate().map(|(i, k)| (Key(i as u8), k.rect)));
                v
            }
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
            Screen::Games => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..apps::GAMES.len()).map(|i| (Game(i), apps::games_row(i))));
                v
            }
            Screen::Mini => {
                let mut v = vec![(Back, ui::back_rect())];
                if let Some(g) = &self.mini {
                    v.extend(g.targets().into_iter().map(|(id, r)| (Mini(id), r)));
                }
                v
            }
            Screen::SudokuMenu => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..3).map(|k| (Level(k), crate::sudoku::resume_level_rect(k))));
                if crate::sudoku::Game::load().is_some_and(|g| !g.solved()) {
                    v.push((Continue, crate::sudoku::continue_rect()));
                }
                v
            }
            Screen::Sudoku => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..4).map(|k| (Game(k), crate::sudoku::action_rect(k))));
                v.extend((0..81).map(|i| (Cell(i), crate::sudoku::cell_rect(i))));
                v.extend((0..=9u8).map(|n| (Key(n), crate::sudoku::key_rect(n))));
                v
            }
            Screen::SudokuWon => vec![(Back, ui::back_rect()), (NewGame, crate::sudoku::won_new_rect())],
            Screen::ChessHub => {
                let has_resume = crate::chess::Game::load().is_some_and(|g| g.in_progress());
                let n = if has_resume { 4 } else { 3 };
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..n).map(|k| (Game(k + if has_resume { 0 } else { 1 }), crate::chess_ui::hub_row(k))));
                v
            }
            Screen::Coaches => {
                let mut v = vec![(Back, ui::back_rect())];
                for k in 0..crate::chess_ui::PER_PAGE {
                    let i = self.coach_page * crate::chess_ui::PER_PAGE + k;
                    if i < crate::coach::LADDER && i <= crate::coach::unlocked() {
                        v.push((Row(i), crate::chess_ui::coach_row(k)));
                    }
                }
                if self.coach_page > 0 {
                    v.push((Prev, crate::chess_ui::prev_rect()));
                }
                if (self.coach_page + 1) * crate::chess_ui::PER_PAGE < crate::coach::LADDER {
                    v.push((Next, crate::chess_ui::next_rect()));
                }
                v
            }
            Screen::Chess => {
                let mut v = vec![(Back, ui::back_rect()), (Undo, crate::chess::undo_rect()), (NewGame, crate::chess::new_rect())];
                if self.coach_level().is_none() {
                    v.push((Check, crate::chess::rotate_rect()));
                }
                let flip = self.chess.as_ref().is_some_and(|g| g.flipped());
                if self.chess.as_ref().is_some_and(|g| g.pending_promo.is_some()) {
                    v.extend((0..4).map(|k| (Level(k), crate::chess::promo_rect(k))));
                } else {
                    v.extend((0..64).map(|i| (Cell(i), crate::chess::square_rect(i, flip))));
                }
                v
            }
            Screen::ChessEnd => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..3).map(|k| (Game(k), crate::chess_ui::end_row(k))));
                v
            }
            Screen::ChessGames => {
                let mut v = vec![(Back, ui::back_rect())];
                let n = self.saved_games.len().saturating_sub(self.games_page * crate::chess_ui::PER_PAGE).min(crate::chess_ui::PER_PAGE);
                v.extend((0..n).map(|k| (Row(k), crate::chess_ui::game_row(k))));
                if self.games_page > 0 {
                    v.push((Prev, crate::chess_ui::prev_rect()));
                }
                if (self.games_page + 1) * crate::chess_ui::PER_PAGE < self.saved_games.len() {
                    v.push((Next, crate::chess_ui::next_rect()));
                }
                v
            }
            Screen::ChessReview => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..3).map(|k| (Game(k), crate::chess_ui::review_btn(k))));
                v
            }
            Screen::ChessInsights => {
                let mut v = vec![(Back, ui::back_rect())];
                if let Some(g) = self.saved_games.first() {
                    v.extend(crate::chess_ui::key_moments(g).iter().enumerate().map(|(i, _)| (Row(i), crate::chess_ui::moment_rect(i))));
                }
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
        // Casas de jogo não piscam: a resposta é a própria jogada.
        if matches!(t, Target::Header | Target::Bar | Target::Cell(_)) || matches!(t, Target::Mini(id) if id < 800) {
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
                if self.screen == Screen::ClaudeCompose {
                    return self.send_claude();
                }
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

    fn start_claude_compose(&mut self) {
        self.compose_text.clear();
        self.compose_hint = "Escreva e toque em OK para enviar.".into();
        self.kb_mode = crate::keyboard::Mode::Letters;
        self.kb_shift = true;
        self.go(Screen::ClaudeCompose);
    }

    fn open_chat(&mut self, id: &str) {
        match crate::chat::open(id) {
            Ok(c) => {
                self.chat_pages = crate::chat::paginate(&self.fonts, &c);
                self.chat_page = self.chat_pages.len().saturating_sub(1);
                self.cur_chat = Some(c);
                self.go(Screen::ClaudeChat);
            }
            Err(e) => {
                self.chats = Err(e);
                self.render(false);
            }
        }
    }

    /// Envia a mensagem e mostra a espera; a resposta abre na última página da conversa.
    fn send_claude(&mut self) {
        let text = self.compose_text.trim().to_string();
        if text.is_empty() {
            self.compose_hint = "Escreva alguma coisa antes do OK.".into();
            return self.render(false);
        }
        let id = self.cur_chat.as_ref().map(|c| c.id.clone());
        self.update_msg = ("Claude".into(), vec!["Pensando… pode levar até um minuto.".into(), "".into(), text.chars().take(80).collect()], None);
        self.screen = Screen::Update;
        self.render(true);
        self.fb.wait();
        match crate::chat::send(id.as_deref(), &text) {
            Ok(new_id) => {
                self.compose_text.clear();
                self.open_chat(&new_id);
            }
            Err(e) => {
                // Mantém o texto para tentar de novo.
                self.compose_hint = e;
                self.screen = Screen::ClaudeCompose;
                self.render(true);
            }
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

    /// Redesenha grade, teclado, ações e linha de estado do Sudoku (atualização parcial).
    fn redraw_sudoku_grid(&mut self) {
        let Some(g) = &self.sudoku else { return };
        crate::sudoku::draw_status(&mut self.fb, &self.fonts, g, &self.sudoku_note);
        for i in 0..81 {
            crate::sudoku::draw_cell(&mut self.fb, &self.fonts, g, i);
        }
        crate::sudoku::draw_grid_lines(&mut self.fb);
        crate::sudoku::draw_keys(&mut self.fb, &self.fonts, g);
        crate::sudoku::draw_actions(&mut self.fb, &self.fonts, g);
        self.fb.refresh(Rect::new(0, 170, 1072, 1448 - 170), Wave::Quality, false);
    }

    /// Depois de uma jogada: vitória (com recorde) ou só redesenho.
    fn after_sudoku_move(&mut self) {
        let won = self.sudoku.as_ref().is_some_and(|g| g.solved());
        if !won {
            return self.redraw_sudoku_grid();
        }
        let (lv, secs, hints) = self.sudoku.as_ref().map(|g| (g.level, g.seconds(), g.hints)).unwrap();
        // Recorde só vale sem dicas.
        let rec = hints == 0 && crate::sudoku::submit_record(lv, secs);
        crate::sudoku::Game::clear_saved();
        log(&format!("sudoku {} resolvido em {} s (recorde: {rec})", lv.name(), secs));
        self.sudoku_won = (secs, rec);
        self.go(Screen::SudokuWon);
    }

    fn start_sudoku(&mut self, lv: crate::sudoku::Level) {
        let t = Instant::now();
        let g = crate::sudoku::Game::new(lv);
        log(&format!("sudoku {} gerado em {} ms", lv.name(), t.elapsed().as_millis()));
        g.save();
        crate::sudoku::set_last_level(lv);
        self.sudoku = Some(g);
        self.sudoku_note = "Toque numa casa e depois num número.".into();
        self.go(Screen::Sudoku);
    }

    /// Jogo local com "Girar" ligado e vez das pretas: tela de cabeça para baixo.
    fn local_rotated(&self) -> bool {
        self.coach_level().is_none() && crate::chess::rotate_enabled() && self.chess.as_ref().is_some_and(|g| !g.pos.white_to_move)
    }

    /// Toque num minijogo: redesenha o que ele pedir e, se for a vez da CPU, joga por ela.
    fn mini_tap(&mut self, id: u32, x: i32) {
        let Some(g) = self.mini.as_mut() else { return };
        let r = g.tap(id, x, 0);
        self.apply_mini_redraw(r);
        let mut guard = 0;
        while self.mini.as_ref().is_some_and(|g| g.cpu_turn()) && guard < 200 {
            guard += 1;
            if let Some(g) = self.mini.as_mut() {
                g.cpu_move();
            }
            self.apply_mini_redraw(crate::minigame::Redraw::Part(Rect::new(0, 170, 1072, 1448 - 170)));
        }
        if let Some(g) = &self.mini {
            g.save();
        }
    }

    fn apply_mini_redraw(&mut self, r: crate::minigame::Redraw) {
        use crate::minigame::Redraw;
        match r {
            Redraw::None => {}
            Redraw::Full => self.render(true),
            Redraw::Part(rect) => {
                if let Some(g) = &self.mini {
                    g.draw(&mut self.fb, &self.fonts);
                }
                self.fb.refresh(rect, Wave::Quality, false);
            }
        }
    }

    fn coach_level(&self) -> Option<usize> {
        match self.chess.as_ref().map(|g| g.mode) {
            Some(crate::chess::Mode::Coach { level, .. }) => Some(level),
            _ => None,
        }
    }

    fn ensure_engine(&mut self) -> bool {
        if self.engine.is_none() {
            self.engine = crate::engine::Engine::start();
            if self.engine.is_none() {
                self.coach_msg = "Não consegui abrir o motor de xadrez.".into();
                log("stockfish não iniciou");
            }
        }
        self.engine.is_some()
    }

    /// Fala do treinador com a tela atualizada na hora (antes de cálculos demorados).
    fn say(&mut self, text: &str) {
        self.coach_msg = text.to_string();
        if let Some(level) = self.coach_level() {
            let c = crate::coach::coach(level);
            crate::chess_ui::draw_coach_line(&mut self.fb, &self.fonts, &format!("{} ({})", c.name, c.rating), text);
            self.fb.refresh(crate::chess_ui::coach_line_rect(), Wave::Quality, false);
        }
    }

    fn start_coach_game(&mut self, level: usize) {
        let (_, played) = crate::coach::rating();
        let user_white = played % 2 == 0;
        let g = crate::chess::Game::new_coach(level, user_white);
        g.save();
        self.chess = Some(g);
        self.records.clear();
        crate::coach::clear_current();
        self.pre_eval = None;
        let c = crate::coach::coach(level);
        self.coach_msg = format!("Olá, sou {}. {}. Você joga de {}.", c.name, c.style, if user_white { "brancas" } else { "pretas" });
        self.go(Screen::Chess);
        if !self.ensure_engine() {
            return self.render(false);
        }
        if self.chess.as_ref().is_some_and(|g| g.engine_turn()) {
            self.engine_reply();
        } else {
            self.analyse_pre();
        }
    }

    fn resume_chess(&mut self) {
        let Some(g) = crate::chess::Game::load() else { return };
        let coach = matches!(g.mode, crate::chess::Mode::Coach { .. });
        self.chess = Some(g);
        self.records = if coach { crate::coach::load_current() } else { Vec::new() };
        self.coach_msg = "Partida retomada. Sua vez.".into();
        self.go(Screen::Chess);
        if coach && self.ensure_engine() {
            if self.chess.as_ref().is_some_and(|g| g.engine_turn()) {
                self.engine_reply();
            } else {
                self.analyse_pre();
            }
        }
    }

    /// Avaliação da posição na sua vez (base para medir o seu próximo lance).
    fn analyse_pre(&mut self) {
        let moves = self.chess.as_ref().map(|g| g.moves.clone()).unwrap_or_default();
        self.pre_eval = self.engine.as_mut().and_then(|e| e.analyse(&moves, 600)).map(|a| (a.score, a.best));
    }

    /// Mede o lance que você acabou de fazer, comenta e passa a vez ao treinador.
    fn after_user_move(&mut self) {
        let Some(level) = self.coach_level() else { return };
        let Some(g) = self.chess.as_ref() else { return };
        let Some(before) = g.history.last().cloned() else { return };
        let (Some(last), Some(uci)) = (g.last, g.moves.last().cloned()) else { return };
        let user_white = before.white_to_move;
        let ply = g.moves.len() - 1;
        let promo = chess_promo(&uci);
        let san = crate::chess::san(&before, last, promo);
        let moves = g.moves.clone();
        let rating = crate::coach::coach(level).rating;
        if let Some((pre, best)) = self.pre_eval.take() {
            self.say("Analisando o seu lance…");
            let after = self.engine.as_mut().and_then(|e| e.analyse(&moves, 600)).map(|a| a.score.neg()).unwrap_or(pre);
            let loss = (pre.cp() - after.cp()).max(0);
            let class = crate::coach::Class::from_loss(loss, uci == best);
            let best_san = crate::coach::best_san(&before, &best);
            let why = if class as u8 >= crate::coach::Class::Inaccuracy as u8 { crate::coach::explain(&before, last, &best, user_white, pre) } else { String::new() };
            log(&format!("lance {san}: {} (perda {loss})", class.name()));
            self.records.push(crate::coach::Record { ply, san: san.clone(), class, loss, best_uci: best, best_san, why: why.clone() });
            crate::coach::save_current(&self.records);
            self.coach_msg = if crate::coach::should_comment(rating, class) {
                format!("{}: {why}", class.name())
            } else {
                crate::coach::praise(rating, class).map(str::to_string).unwrap_or_else(|| format!("{san}. Agora é minha vez."))
            };
        }
        if self.chess.as_ref().is_some_and(|g| g.result().is_some()) {
            return self.finish_coach_game(None);
        }
        self.engine_reply();
    }

    /// Lance do treinador, na força dele (abaixo de 1400 mistura lances aleatórios).
    fn engine_reply(&mut self) {
        let Some(level) = self.coach_level() else { return };
        let rating = crate::coach::coach(level).rating;
        let comment = self.coach_msg.clone();
        self.say(&format!("{comment} Pensando…"));
        let (moves, pos) = match &self.chess {
            Some(g) => (g.moves.clone(), g.pos.clone()),
            None => return,
        };
        let random = (self.rng.next() % 1000) as f32 / 1000.0 < crate::coach::random_move_chance(rating);
        let mv = if random {
            crate::coach::random_legal(&pos, &mut self.rng)
        } else {
            self.engine.as_mut().and_then(|e| {
                e.set_strength(rating);
                e.play(&moves, crate::coach::think_millis(rating))
            })
        };
        let san = mv.and_then(|m| self.chess.as_mut().and_then(|g| g.play_uci(&m)));
        self.coach_msg = match san {
            Some(s) => format!("{comment} Joguei {s}.").trim().to_string(),
            None => "Não consegui jogar.".into(),
        };
        if let Some(g) = &self.chess {
            g.save();
        }
        self.redraw_chess();
        if self.chess.as_ref().is_some_and(|g| g.result().is_some()) {
            return self.finish_coach_game(None);
        }
        self.analyse_pre();
        self.say(&self.coach_msg.clone());
    }

    fn undo_chess(&mut self) {
        let coach = self.coach_level().is_some();
        if let Some(g) = &mut self.chess {
            g.undo();
            // Contra o treinador, desfaz também o lance dele, para voltar à sua vez.
            if coach && g.engine_turn() {
                g.undo();
            }
        }
        if coach {
            let ply = self.chess.as_ref().map(|g| g.moves.len()).unwrap_or(0);
            self.records.retain(|r| r.ply < ply);
            crate::coach::save_current(&self.records);
            self.coach_msg = "Lance desfeito. Sua vez.".into();
            self.redraw_chess();
            self.say(&self.coach_msg.clone());
            self.analyse_pre();
        } else if crate::chess::rotate_enabled() {
            self.render(true);
        } else {
            self.redraw_chess();
        }
    }

    /// Fim da partida contra o treinador: rating, escada, partida salva e tela de resultado.
    /// `forced`: placar imposto (desistência = 0).
    fn finish_coach_game(&mut self, forced: Option<f32>) {
        let Some(level) = self.coach_level() else { return };
        let Some(g) = self.chess.as_ref() else { return };
        let Some(user_white) = (match g.mode {
            crate::chess::Mode::Coach { user_white, .. } => Some(user_white),
            _ => None,
        }) else {
            return;
        };
        let result = g.result();
        let score = forced.unwrap_or_else(|| {
            let mate = !g.pos.has_moves() && g.pos.in_check(g.pos.white_to_move);
            if mate {
                // Quem está na vez levou mate.
                if g.pos.white_to_move == user_white { 0.0 } else { 1.0 }
            } else {
                0.5
            }
        });
        let c = crate::coach::coach(level);
        let ratings = crate::coach::update_rating(c.rating, score);
        let label = if score >= 1.0 { "vitória" } else if score <= 0.0 { "derrota" } else { "empate" };
        crate::coach::save_game(level, user_white, label, ratings, &g.moves, &self.records);
        crate::coach::clear_current();
        let mut unlocked = None;
        if score >= 1.0 {
            let before = crate::coach::unlocked();
            crate::coach::unlock_after_win(level);
            if crate::coach::unlocked() > before {
                let n = crate::coach::coach(crate::coach::unlocked());
                unlocked = Some(format!("Liberado: {} ({}).", n.name, n.rating));
            }
        }
        let title = match label {
            "vitória" => "Vitória!",
            "derrota" => "Derrota",
            _ => "Empate",
        };
        let detail = match (forced, result) {
            (Some(_), _) => format!("Você desistiu contra {}.", c.name),
            (_, Some(r)) => format!("{r} Contra {} ({}).", c.name, c.rating),
            _ => String::new(),
        };
        log(&format!("partida contra {} terminou: {label}, rating {} → {}", c.name, ratings.0, ratings.1));
        let _ = std::fs::remove_file(home_dir().join("data/xadrez.txt"));
        self.engine = None;
        self.end_info = (title.into(), detail, Some(ratings), unlocked);
        self.go(Screen::ChessEnd);
    }

    fn open_review(&mut self) {
        // Começa no primeiro deslize real; se não houver, no primeiro lance.
        self.review_k = self.saved_games.first().and_then(|g| g.records.iter().position(|r| r.class as u8 >= crate::coach::Class::Mistake as u8)).unwrap_or(0);
        self.go(Screen::ChessReview);
    }

    fn redraw_chess(&mut self) {
        let (Some(g), Some(sp)) = (&self.chess, &self.sprites) else { return };
        for i in 0..64 {
            crate::chess::draw_square(&mut self.fb, g, sp, i);
        }
        crate::chess::draw_status(&mut self.fb, &self.fonts, g, sp);
        if self.coach_level().is_some() {
            let msg = self.coach_msg.clone();
            if let Some(level) = self.coach_level() {
                let c = crate::coach::coach(level);
                crate::chess_ui::draw_coach_line(&mut self.fb, &self.fonts, &format!("{} ({})", c.name, c.rating), &msg);
            }
        }
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
                self.chats = crate::chat::list();
            }
            Screen::ClaudeLive => {
                self.claude = crate::claude_ui::load();
                self.claude_stamp = crate::claude_ui::stamp();
            }
            Screen::Clips => {
                self.clips = crate::clippings::load();
                self.clip_page = 0;
            }
            Screen::Chess | Screen::ChessReview => {
                if self.sprites.is_none() {
                    let t = Instant::now();
                    self.sprites = Some(crate::chess::Sprites::new());
                    log(&format!("peças desenhadas em {} ms", t.elapsed().as_millis()));
                }
                if screen == Screen::Chess && self.chess.is_none() {
                    self.chess = Some(crate::chess::Game::new());
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
            (Screen::Sudoku, Back) => {
                if let Some(g) = &self.sudoku {
                    g.save(); // guarda o tempo jogado
                }
                self.go(Screen::Games);
            }
            (Screen::SudokuMenu | Screen::SudokuWon | Screen::ChessHub, Back) => self.go(Screen::Games),
            (Screen::Chess, Back) => {
                self.engine = None; // fecha o Stockfish; a partida fica salva para continuar
                self.go(Screen::ChessHub);
            }
            (Screen::Coaches | Screen::ChessEnd | Screen::ChessGames, Back) => self.go(Screen::ChessHub),
            (Screen::ChessReview, Back) => self.go(Screen::ChessGames),
            (Screen::ChessInsights, Back) => self.go(Screen::ChessReview),
            (Screen::Clips, Back) => self.go(Screen::Library),
            (Screen::ClipBook, Back) => self.go(Screen::Clips),
            (Screen::Compose, Back) => self.go(Screen::Today),
            (Screen::ClaudeLive | Screen::ClaudeChat, Back) => self.go(Screen::Claude),
            (Screen::ClaudeCompose, Back) => {
                if self.cur_chat.is_some() {
                    self.screen = Screen::ClaudeChat;
                    self.render(true);
                } else {
                    self.go(Screen::Claude);
                }
            }
            (Screen::Claude, Continue) => self.go(Screen::ClaudeLive),
            (Screen::Claude, NewGame) => {
                self.cur_chat = None;
                self.start_claude_compose();
            }
            (Screen::Claude, Row(i)) => {
                let id = self.chats.as_ref().ok().and_then(|c| c.get(i)).map(|c| c.id.clone());
                if let Some(id) = id {
                    self.open_chat(&id);
                }
            }
            (Screen::ClaudeChat, Primary) => self.start_claude_compose(),
            (Screen::ClaudeChat, Prev) => {
                self.chat_page = self.chat_page.saturating_sub(1);
                self.render(true);
            }
            (Screen::ClaudeChat, Next) => {
                self.chat_page += 1;
                self.render(true);
            }
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
            (Screen::Compose | Screen::WifiPass | Screen::ClaudeCompose, Key(i)) => self.press_key(i as usize),
            (Screen::Wifi, Back) => self.go(Screen::Settings),
            (Screen::WifiPass, Back) => self.go(Screen::Wifi),
            (Screen::Mini, Back) => {
                if let Some(g) = &self.mini {
                    g.save();
                }
                self.go(Screen::Games);
            }
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
            // Sudoku: com partida pela metade, pergunta; sem ela, já começa um jogo novo.
            (Screen::Games, Game(0)) => {
                if crate::sudoku::Game::load().is_some_and(|g| !g.solved()) {
                    self.go(Screen::SudokuMenu);
                } else {
                    self.start_sudoku(crate::sudoku::last_level());
                }
            }
            (Screen::Games, Game(1)) => self.go(Screen::ChessHub),
            (Screen::Games, Game(i)) => {
                self.mini = match i {
                    2 => Some(Box::new(crate::coroas::Coroas::new())),
                    _ => None,
                };
                if self.mini.is_some() {
                    self.go(Screen::Mini);
                }
            }
            (Screen::Mini, Mini(id)) => self.mini_tap(id, x),
            (Screen::SudokuMenu, Level(k)) => self.start_sudoku(crate::sudoku::LEVELS[k.min(2)]),
            (Screen::SudokuMenu, Continue) => {
                self.sudoku = crate::sudoku::Game::load();
                self.sudoku_note = "Partida retomada.".into();
                self.go(Screen::Sudoku);
            }
            (Screen::Sudoku, Cell(i)) => {
                if let Some(g) = &mut self.sudoku {
                    g.selected = if g.selected == Some(i) { None } else { Some(i) };
                }
                self.redraw_sudoku_grid();
            }
            (Screen::Sudoku, Key(n)) => {
                if let Some(g) = &mut self.sudoku {
                    if g.selected.is_none() {
                        self.sudoku_note = "Escolha uma casa primeiro.".into();
                    } else {
                        let before = g.mistakes;
                        g.press(n);
                        g.save();
                        self.sudoku_note = if g.mistakes > before { "Esse número não vai aí.".into() } else { String::new() };
                    }
                }
                self.after_sudoku_move();
            }
            (Screen::Sudoku, Game(0)) => {
                if let Some(g) = &mut self.sudoku {
                    g.notes_mode = !g.notes_mode;
                    self.sudoku_note = if g.notes_mode { "Anotando: os números entram pequenos, como lembrete.".into() } else { String::new() };
                }
                self.redraw_sudoku_grid();
            }
            (Screen::Sudoku, Game(1)) => {
                if let Some(g) = &mut self.sudoku {
                    self.sudoku_note = if g.hint().is_some() { "Dica: a casa destacada recebeu o número certo.".into() } else { String::new() };
                    g.save();
                }
                self.after_sudoku_move();
            }
            (Screen::Sudoku, Game(2)) => {
                if let Some(g) = &mut self.sudoku {
                    self.sudoku_note = if g.undo() { "Jogada desfeita.".into() } else { "Nada para desfazer.".into() };
                    g.save();
                }
                self.redraw_sudoku_grid();
            }
            (Screen::Sudoku, Game(_)) => {
                if let Some(g) = &self.sudoku {
                    g.save();
                }
                self.go(Screen::SudokuMenu);
            }
            (Screen::SudokuWon, NewGame) => {
                let lv = self.sudoku.as_ref().map(|g| g.level).unwrap_or_else(crate::sudoku::last_level);
                self.start_sudoku(lv);
            }
            // Central do xadrez: 0 continuar, 1 treinadores, 2 duas pessoas, 3 partidas anteriores.
            (Screen::ChessHub, Game(0)) => self.resume_chess(),
            (Screen::ChessHub, Game(1)) => {
                self.coach_page = crate::coach::unlocked() / crate::chess_ui::PER_PAGE;
                self.go(Screen::Coaches);
            }
            (Screen::ChessHub, Game(2)) => {
                self.engine = None;
                let g = crate::chess::Game::new();
                g.save();
                self.chess = Some(g);
                self.go(Screen::Chess);
            }
            (Screen::ChessHub, Game(_)) => {
                self.saved_games = crate::coach::list_saved();
                self.games_page = 0;
                self.go(Screen::ChessGames);
            }
            (Screen::Coaches, Row(i)) => self.start_coach_game(i),
            (Screen::Coaches, Prev) => {
                self.coach_page = self.coach_page.saturating_sub(1);
                self.render(true);
            }
            (Screen::Coaches, Next) => {
                self.coach_page += 1;
                self.render(true);
            }
            (Screen::Chess, Cell(i)) => {
                let busy = self.chess.as_ref().is_some_and(|g| g.engine_turn());
                if busy {
                    return;
                }
                let moved = self.chess.as_mut().is_some_and(|g| g.tap(i));
                if moved && self.coach_level().is_none() && crate::chess::rotate_enabled() {
                    // Jogo local girando: a tela inteira vira para o próximo jogador.
                    return self.render(true);
                }
                self.redraw_chess();
                if moved {
                    self.after_user_move();
                }
            }
            (Screen::Chess, Level(k)) => {
                if let Some(g) = &mut self.chess {
                    g.promote(crate::chess::PROMO[k.min(3)]);
                }
                if self.coach_level().is_none() && crate::chess::rotate_enabled() {
                    return self.render(true);
                }
                self.redraw_chess();
                self.after_user_move();
            }
            (Screen::Chess, Undo) => self.undo_chess(),
            (Screen::Chess, Check) => {
                crate::chess::set_rotate(!crate::chess::rotate_enabled());
                self.render(true);
            }
            (Screen::Chess, NewGame) => {
                let coach = self.chess.as_ref().is_some_and(|g| matches!(g.mode, crate::chess::Mode::Coach { .. }));
                if coach {
                    self.finish_coach_game(Some(0.0)); // desistir conta como derrota
                } else {
                    let g = crate::chess::Game::new();
                    g.save();
                    self.chess = Some(g);
                    self.render(true);
                }
            }
            (Screen::ChessEnd, Game(0)) => {
                self.saved_games = crate::coach::list_saved();
                self.open_review();
            }
            (Screen::ChessEnd, Game(1)) => {
                let lv = self.saved_games.first().map(|g| g.level).or(match self.chess.as_ref().map(|g| g.mode) {
                    Some(crate::chess::Mode::Coach { level, .. }) => Some(level),
                    _ => None,
                });
                self.start_coach_game(lv.unwrap_or(0));
            }
            (Screen::ChessEnd, Game(_)) => {
                self.coach_page = crate::coach::unlocked() / crate::chess_ui::PER_PAGE;
                self.go(Screen::Coaches);
            }
            (Screen::ChessGames, Row(k)) => {
                let idx = self.games_page * crate::chess_ui::PER_PAGE + k;
                if idx < self.saved_games.len() {
                    let g = self.saved_games.remove(idx);
                    self.saved_games.insert(0, g);
                    self.open_review();
                }
            }
            (Screen::ChessGames, Prev) => {
                self.games_page = self.games_page.saturating_sub(1);
                self.render(true);
            }
            (Screen::ChessGames, Next) => {
                self.games_page += 1;
                self.render(true);
            }
            (Screen::ChessReview, Game(0)) => {
                self.review_k = self.review_k.saturating_sub(1);
                self.render(false);
            }
            (Screen::ChessReview, Game(1)) => self.go(Screen::ChessInsights),
            (Screen::ChessInsights, Row(i)) => {
                if let Some(k) = self.saved_games.first().and_then(|g| crate::chess_ui::key_moments(g).get(i).copied()) {
                    self.review_k = k;
                    self.go(Screen::ChessReview);
                }
            }
            (Screen::ChessReview, Game(_)) => {
                let n = self.saved_games.first().map(|g| g.records.len()).unwrap_or(0);
                self.review_k = (self.review_k + 1).min(n.saturating_sub(1));
                self.render(false);
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
        // Tela girada: o toque também gira.
        let t = if self.fb.rot180 {
            let (w, h) = (self.fb.width - 1, self.fb.height - 1);
            match t {
                Touch::Down(x, y) => Touch::Down(w - x, h - y),
                Touch::Move(x, y) => Touch::Move(w - x, h - y),
                Touch::Up(x, y) => Touch::Up(w - x, h - y),
            }
        } else {
            t
        };
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
        if self.screen == Screen::ClaudeLive && self.claude_checked.elapsed() >= Duration::from_secs(2) {
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

fn chess_promo(uci: &str) -> Option<crate::chess::Kind> {
    crate::chess::parse_uci(uci).and_then(|(_, p)| p)
}
