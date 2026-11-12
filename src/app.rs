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
    /// Escolha do relógio das partidas.
    ChessClock,
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
    ClaudeFiles,
    /// Escolher arquivos do Kindle para anexar à pergunta.
    ClaudeAttach,
    /// Coleção de achados raros (das frases da Início).
    Achados,
    Reader,
    ReaderToc,
    ClaudeDoc,
    Update,
    Wifi,
    WifiPass,
    LibSearch,
    /// Texto pedido por um minijogo (ex.: o rumo novo de uma história).
    MiniText,
    /// Edição da manhã.
    Edicao,
    /// Artigos mandados do celular.
    Artigos,
    /// Porta-retrato (fotos mandadas do celular).
    Retrato,
    /// Terminal: comandos de um toque.
    Terminal,
    /// Terminal: digitar um comando.
    TermType,
    /// Pedido de permissão de um agente do Claude Code.
    Aprovar,
    /// Missões para os agentes do PC.
    Missoes,
    /// Missão livre: digitar o pedido.
    MissaoText,
    Calendar,
    Tutorial,
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
    Search,
    Sort,
    /// Modo avião (Luz e tela).
    Plane,
    /// Botão da frase da Início.
    Phrase,
    Help,
    Ctx,
}

/// Código de saída que pede ao `session.sh` para abrir um livro no KOReader e voltar.
pub const EXIT_OPEN_BOOK: i32 = 42;
pub const OPEN_FILE: &str = "/tmp/verso-ink.open-file";

/// Toque em andamento numa casa de minijogo.
struct Gesture {
    start: u32,
    last: u32,
    at: Instant,
    dragging: bool,
    held: bool,
}

/// Tempo segurando para contar como "segurar".
const HOLD: Duration = Duration::from_millis(450);

pub struct App {
    pub fb: Framebuffer,
    pub fonts: Fonts,
    pub screen: Screen,
    pressed: Option<Target>,
    /// Dedo numa casa de minijogo: para segurar e arrastar.
    gesture: Option<Gesture>,
    swipe_from: Option<(i32, i32)>,
    /// Anexos da pergunta ao Claude: o que dá para anexar, os escolhidos e a página.
    attach_items: Vec<crate::chat::AttachItem>,
    attach_sel: Vec<usize>,
    attach_page: usize,
    /// Lugar do módulo aberto no cache de minijogos, quando não é o padrão da tela de volta.
    mini_slot: Option<usize>,
    /// Achado que acabou de entrar na coleção (destaque na tela Achados).
    achado_new: Option<usize>,
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
    sudoku_practice: bool,
    sudoku_prog: crate::fases::Progress,
    chess: Option<crate::chess::Game>,
    sprites: Option<crate::chess::Sprites>,
    engine: Option<crate::engine::Engine>,
    coach_msg: String,
    pre_eval: Option<(crate::engine::Score, String)>,
    /// Partida: confirmação pendente (0 empate, 1 desistir) e quando foi pedida.
    chess_confirm: Option<(u8, Instant)>,
    /// Jogo local: empate proposto, esperando o outro aceitar.
    chess_offer: bool,
    /// Jogo local encerrado (desistência ou empate combinado): recado final.
    chess_over: Option<String>,
    records: Vec<crate::coach::Record>,
    coach_page: usize,
    end_info: (String, String, Option<(u32, u32)>, Option<String>),
    saved_games: Vec<crate::coach::Saved>,
    games_page: usize,
    review_k: usize,
    rng: crate::sudoku::Rng,
    mini: Option<Box<dyn crate::minigame::MiniGame>>,
    /// Partidas deixadas no meio (por jogo): voltar ao jogo continua de onde parou.
    mini_cache: std::collections::HashMap<usize, Box<dyn crate::minigame::MiniGame>>,
    /// Botão arriscado (novo jogo, recomeçar) esperando o segundo toque.
    mini_confirm: Option<u32>,
    /// O que os relógios do xadrez mostram agora (para redesenhar só quando muda).
    clock_shown: String,
    /// Título do pedido de texto de um minijogo.
    mini_prompt: String,
    /// Painel ligado à VPS: página da edição, dos artigos, foto atual, terminal, aprovações.
    ed_page: usize,
    art_page: usize,
    photo_k: usize,
    photo_at: Instant,
    term: (String, String),
    approvals: Vec<crate::painel::Approval>,
    approve_back: Screen,
    /// Para onde volta o documento aberto (arquivos do Claude ou fim de partida).
    doc_back: Screen,
    /// Alvos da página atual da Edição da manhã.
    ed_hits: Vec<(Rect, crate::painel::EdAction)>,
    /// Missões: o que veio da VPS, a etapa da escolha e os alvos da tela.
    missions: Option<crate::painel::Missions>,
    mstage: crate::painel::MStage,
    m_hits: Vec<(Rect, crate::painel::MAct)>,
    panel_note: String,
    /// Sincronização em segundo plano com a VPS: última vez (rápida e completa) e o resultado.
    sync_fast: Instant,
    sync_full: Option<Instant>,
    sync_rx: Option<std::sync::mpsc::Receiver<(Option<crate::painel::Sync>, bool, (usize, usize))>>,
    /// O que estava na tela quando o Kindle dormiu (para a tela de repouso).
    sleep_scene: Option<ui::SleepScene>,
    /// Para onde o botão Início leva ao sair do minijogo/módulo.
    mini_back: Screen,
    /// Índice (na tela Jogar) do minijogo aberto.
    mini_game: usize,
    /// Para onde volta o painel Luz e tela (aberto de qualquer tela deslizando de baixo).
    light_return: Option<Screen>,
    /// De onde se abriu a tela de Wi-Fi (Luz e tela ou Ajustes).
    wifi_back: Screen,
    /// Tutorial: jogo, página, para onde voltar e se abre o jogo ao terminar.
    tut: (usize, usize, Screen, bool),
    clips: Vec<crate::clippings::BookClips>,
    /// Destaques para revisar hoje.
    clip_due: usize,
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
    chat_pages: Vec<Vec<crate::chat::Line>>,
    /// Pergunta ao Claude a partir de uma tela: contexto que vai junto e para onde voltar.
    claude_ctx: Option<String>,
    /// Leitor próprio: livro aberto, fontes (carregadas na primeira vez) e página do sumário.
    reader: Option<crate::reader::Reader>,
    rfonts: Option<crate::reader::RFonts>,
    toc_page: usize,
    claude_return: Option<Screen>,
    /// Cartão "vai junto" (título, detalhe) e perguntas prontas da tela de pergunta.
    claude_about: Option<(String, String)>,
    claude_sugg: Vec<String>,
    /// Leitor de texto (arquivo do Claude, análise): título, páginas, página atual, aviso.
    doc: (String, Vec<Vec<crate::chat::Line>>, usize),
    files_note: String,
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
        Target::Night => 32,
        Target::OpenSettings | Target::Wifi => 28,
        Target::Key(_) => 18,
        // Cartas (alvo mais alto que largo, como o monte do UNO): canto de carta.
        Target::Mini(_) if r.h > r.w => r.w / 9,
        // Botões e cartões do sistema: o mesmo raio do desenho (meia altura, até 40).
        _ => (r.h.min(r.w) / 2).min(40),
    }
}

impl App {
    pub fn new(fb: Framebuffer) -> Self {
        App {
            fb,
            fonts: Fonts::new(),
            screen: Screen::Home,
            pressed: None,
            gesture: None,
            swipe_from: None,
            attach_items: Vec::new(),
            attach_sel: Vec::new(),
            attach_page: 0,
            mini_slot: None,
            achado_new: None,
            status: read_status(),
            light: read_light(),
            lib: Library::empty(),
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
            sudoku_practice: false,
            sudoku_prog: crate::fases::Progress::load("sudoku"),
            chess: None,
            sprites: None,
            engine: None,
            coach_msg: String::new(),
            pre_eval: None,
            chess_confirm: None,
            chess_offer: false,
            chess_over: None,
            records: Vec::new(),
            coach_page: 0,
            end_info: (String::new(), String::new(), None, None),
            saved_games: Vec::new(),
            games_page: 0,
            review_k: 0,
            rng: crate::sudoku::Rng::new(),
            mini: None,
            mini_cache: std::collections::HashMap::new(),
            mini_confirm: None,
            clock_shown: String::new(),
            mini_prompt: String::new(),
            ed_page: 0,
            art_page: 0,
            photo_k: 0,
            photo_at: Instant::now(),
            term: (String::new(), String::new()),
            approvals: Vec::new(),
            approve_back: Screen::Home,
            doc_back: Screen::ClaudeFiles,
            ed_hits: Vec::new(),
            missions: None,
            mstage: Default::default(),
            m_hits: Vec::new(),
            panel_note: String::new(),
            sync_fast: Instant::now(),
            sync_full: None,
            sync_rx: None,
            sleep_scene: None,
            mini_back: Screen::Games,
            mini_game: 0,
            light_return: None,
            wifi_back: Screen::Settings,
            tut: (0, 0, Screen::Games, false),
            clips: Vec::new(),
            clip_due: 0,
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
            claude_ctx: None,
            reader: None,
            rfonts: None,
            toc_page: 0,
            claude_return: None,
            claude_about: None,
            claude_sugg: Vec::new(),
            doc: (String::new(), Vec::new(), 0),
            files_note: String::new(),
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
        let chess_btn = if self.screen == Screen::Chess { Some(self.chess_buttons()) } else { None };
        let live_games = if self.screen == Screen::Games { self.live_games() } else { Vec::new() };
        let home_game = if self.screen == Screen::Home { self.home_game() } else { None };
        let vs_claude = self.screen == Screen::ChessEnd && self.coach_level() == Some(crate::coach::CLAUDE);
        let light_claude = self.light_claude_ok();
        let (fb, f, st) = (&mut self.fb, &self.fonts, &self.status);
        match self.screen {
            Screen::Home => ui::draw_home(fb, f, st, &data::summary(st.offset_min), home_game.as_ref()),
            Screen::Module(m) => ui::draw_module(fb, f, m, st),
            Screen::Light => ui::draw_light(fb, f, &self.light, st, light_claude, !self.wifi_on),
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
                apps::draw_games(fb, f, &live_games);
            }
            Screen::SudokuMenu => crate::sudoku::draw_resume(fb, f, crate::sudoku::Game::load().filter(|g| !g.solved()).as_ref(), &self.sudoku_prog, self.sudoku_practice),
            Screen::Sudoku => {
                if let Some(g) = &self.sudoku {
                    crate::sudoku::draw_board(fb, f, g, &self.sudoku_note);
                }
            }
            Screen::SudokuWon => {
                if let Some(g) = &self.sudoku {
                    crate::sudoku::draw_won(fb, f, g, self.sudoku_won.0, self.sudoku_won.1, &self.sudoku_prog);
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
            Screen::ChessClock => crate::chess_ui::draw_clock_picker(fb, f),
            Screen::Chess => {
                if let (Some(g), Some(sp)) = (&self.chess, &self.sprites) {
                    let (left, right, note) = chess_btn.clone().unwrap_or_default();
                    let b = crate::chess::Buttons { left: (left.0.as_str(), left.1), right: (right.0.as_str(), right.1), note: note.as_deref() };
                    crate::chess::draw(fb, f, g, sp, &b);
                    if let crate::chess::Mode::Coach { level, .. } = g.mode {
                        let c = crate::coach::coach(level);
                        crate::chess_ui::draw_coach_line(fb, f, &format!("{} ({})", c.name, c.rating), &self.coach_msg);
                    }
                    crate::chess::draw_clocks(fb, f, g);
                }
            }
            Screen::ChessEnd => {
                let (t, d, r, u) = &self.end_info;
                crate::chess_ui::draw_end(fb, f, t, d, *r, u.clone(), vs_claude);
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
            Screen::Clips => crate::clippings::draw_books(fb, f, &self.clips, self.clip_page, self.clip_due),
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
                let last = if self.claude_about.is_none() { self.cur_chat.as_ref().and_then(|c| c.messages.iter().rev().find(|m| !m.user)).map(|m| m.text.as_str()) } else { None };
                let names: Vec<String> = self.attach_sel.iter().filter_map(|i| self.attach_items.get(*i)).map(|a| a.label.chars().take(40).collect()).collect();
                let c = crate::chat::Compose {
                    attach: &names,
                    chat_title: self.cur_chat.as_ref().map(|c| c.title.as_str()),
                    last_reply: last,
                    about: self.claude_about.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
                    suggestions: &self.claude_sugg,
                    text: &self.compose_text,
                    hint: &self.compose_hint,
                };
                crate::chat::draw_compose(fb, f, &c);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
            Screen::Wifi => crate::wifi::draw(fb, f, self.wifi_on, self.wifi_cur.as_deref(), &self.nets, &self.wifi_note),
            Screen::Calendar => crate::today_ui::draw_calendar(fb, f, st),
            Screen::Reader => {
                if let (Some(r), Some(rf)) = (&self.reader, &self.rfonts) {
                    crate::reader::draw(fb, f, rf, r);
                }
            }
            Screen::ReaderToc => {
                if let Some(r) = &self.reader {
                    crate::reader::draw_toc(fb, f, r, self.toc_page);
                }
            }
            Screen::Achados => ui::draw_achados(fb, f, self.achado_new),
            Screen::ClaudeAttach => crate::chat::draw_attach(fb, f, &self.attach_items, &self.attach_sel, self.attach_page),
            Screen::ClaudeFiles => {
                if let Some(c) = &self.cur_chat {
                    crate::chat::draw_files(fb, f, c, &self.files_note);
                }
            }
            Screen::ClaudeDoc => crate::chat::draw_doc(fb, f, &self.doc.0, &self.doc.1, self.doc.2),
            Screen::Edicao => self.ed_hits = crate::painel::draw_edition(fb, f, crate::painel::edition().as_ref(), self.ed_page, &self.panel_note),
            Screen::Missoes => self.m_hits = crate::painel::draw_missions(fb, f, self.missions.as_ref(), &self.mstage, &self.panel_note),
            Screen::MissaoText => {
                ui::draw_field_header(fb, f, "MISSÃO LIVRE", "O que o agente deve fazer?");
                crate::today_ui::draw_field(fb, f, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
            Screen::Artigos => crate::painel::draw_articles(fb, f, &crate::painel::articles(), self.art_page, &self.panel_note),
            Screen::Retrato => crate::painel::draw_photo(fb, f, &crate::painel::photos(), self.photo_k),
            Screen::Terminal => crate::painel::draw_terminal(fb, f, &self.term.0, &self.term.1),
            Screen::TermType => {
                ui::draw_field_header(fb, f, "TERMINAL", "Comando");
                crate::today_ui::draw_field(fb, f, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
            Screen::Aprovar => {
                if let Some(a) = self.approvals.first() {
                    crate::painel::draw_approval(fb, f, a, self.approvals.len() - 1);
                }
            }
            Screen::MiniText => {
                let kicker = if self.mini_game == 14 { "HISTÓRIAS" } else { "JOGO" };
                ui::draw_field_header(fb, f, kicker, &self.mini_prompt);
                crate::today_ui::draw_field(fb, f, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
            Screen::LibSearch => {
                ui::draw_field_header(fb, f, "BIBLIOTECA", "Buscar livro");
                crate::today_ui::draw_field(fb, f, &self.compose_text, &self.compose_hint);
                self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
                crate::keyboard::draw(fb, f, &self.keys);
            }
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
            Screen::Sleep => ui::draw_sleep(fb, f, st, self.sleep_scene.as_ref()),
            Screen::Tutorial => crate::tutorial::draw(fb, f, self.tut.0, self.tut.1),
        }
        if let Some(g) = self.help_game() {
            crate::tutorial::draw_help_button(&mut self.fb, &self.fonts);
            // Miniatura do jogo ao lado do título, quando o título deixa espaço.
            if self.screen == Screen::Mini {
                let title_w = self.fonts.measure(crate::text::Face::Display, 130.0, apps::GAMES.get(g).map(|x| x.0).unwrap_or(""));
                let x = 72 + title_w + 40;
                if 1072 - 72 - x >= 200 {
                    crate::tutorial::draw_preview(&mut self.fb, &self.fonts, g, Rect::new(x, 196, 1072 - 72 - x, 170));
                }
            }
        }
        self.fb.present(Wave::Quality, flash);
        self.last_minute = clock::now(self.status.offset_min).minute;
    }

    /// Jogo cujo "Como jogar" aparece nesta tela (menus dos jogos).
    fn help_game(&self) -> Option<usize> {
        match self.screen {
            Screen::Mini if self.mini_back == Screen::Games && self.mini.as_ref().is_some_and(|g| g.at_menu()) && !crate::tutorial::pages(self.mini_game).is_empty() => Some(self.mini_game),
            Screen::SudokuMenu if !self.sudoku_practice => Some(0),
            Screen::ChessHub => Some(1),
            _ => None,
        }
    }

    fn open_tutorial(&mut self, game: usize, open_after: bool) {
        self.tut = (game, 0, self.screen, open_after);
        self.go(Screen::Tutorial);
    }

    /// Fim do tutorial (Jogar ou Pular): marca como visto e segue para o jogo ou volta.
    fn finish_tutorial(&mut self) {
        let (game, _, back, open_after) = self.tut;
        crate::tutorial::mark_seen(game);
        if open_after {
            self.screen = Screen::Games;
            self.activate(Target::Game(game), 0);
        } else {
            self.go(back);
        }
    }

    fn targets(&self) -> Vec<(Target, Rect)> {
        let mut v = self.screen_targets();
        if self.help_game().is_some() {
            v.insert(0, (Target::Help, crate::tutorial::help_rect()));
        }
        v
    }

    fn screen_targets(&self) -> Vec<(Target, Rect)> {
        use Target::*;
        match self.screen {
            Screen::Home => {
                let mut v = Vec::new();
                let (book, game) = (data::last_book().is_some(), data::last_game());
                if book {
                    v.push((Continue, ui::continue_book_rect(game.is_some())));
                }
                if let Some(i) = game.filter(|&i| i < apps::GAMES.len()) {
                    v.push((Game(i), ui::continue_game_rect(book)));
                }
                v.extend((0..Module::ALL.len()).map(|i| (Tile(i), ui::tile_rect(i))));
                v.push((Header, ui::header_rect()));
                let act = ui::home_phrase(&self.status).act;
                if act != crate::saudacao::Act::None {
                    v.push((Phrase, ui::phrase_rect(&self.fonts, act)));
                }
                // Cartões abaixo da hora: abrem a Edição da manhã (e os avisos).
                v.push((Row(0), ui::chips_rect()));
                v
            }
            Screen::Module(Module::Mais) => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend((0..ui::COMING.len()).map(|k| (Row(k), ui::more_row(k))));
                v
            }
            Screen::Module(_) => vec![(Back, ui::back_rect())],
            Screen::Light => vec![
                (Back, ui::back_rect()),
                (Minus, ui::minus_rect()),
                (Plus, ui::plus_rect()),
                (Bar, ui::bar_rect()),
                (Night, ui::night_rect()),
                (Plane, ui::plane_rect()),
                (OpenSettings, ui::settings_row_rect()),
                (Wifi, ui::wifi_row_rect()),
            ]
            .into_iter()
            .chain(self.light_claude_ok().then_some((Ctx, ui::light_claude_rect())))
            .collect(),
            Screen::Library => {
                let mut v = vec![(Back, ui::back_rect()), (Clips, apps::clips_rect())];
                if !self.lib.books.is_empty() || !self.lib.query.is_empty() {
                    v.push((Search, apps::lib_search_rect()));
                    v.push((Sort, apps::lib_sort_rect()));
                }
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
                let mut v = vec![(Back, ui::back_rect()), (Primary, crate::chat::reply_rect()), (Secondary, crate::chat::chat_attach_rect())];
                if self.cur_chat.as_ref().is_some_and(|c| !c.files.is_empty()) {
                    v.push((Check, crate::chat::files_rect()));
                }
                if self.chat_page > 0 {
                    v.push((Prev, crate::chat::prev_rect()));
                }
                if self.chat_page + 1 < self.chat_pages.len() {
                    v.push((Next, crate::chat::next_rect()));
                }
                v
            }
            Screen::Reader => {
                let Some(r) = &self.reader else { return vec![] };
                if r.menu {
                    let mut v: Vec<(Target, Rect)> = (0..5).map(|k| (Level(k), crate::reader::menu_btn(k))).collect();
                    if r.chapter > 0 {
                        v.push((Level(5), crate::reader::menu_btn(5)));
                    }
                    if r.chapter + 1 < r.book.chapters() {
                        v.push((Level(6), crate::reader::menu_btn(6)));
                    }
                    v.push((Primary, Rect::new(0, 300, 1072, 880)));
                    return v;
                }
                // Alto: menu. Terço esquerdo: volta. O resto: avança.
                vec![(Header, Rect::new(0, 0, 1072, 170)), (Prev, Rect::new(0, 170, 357, 1278)), (Next, Rect::new(357, 170, 715, 1278))]
            }
            Screen::ReaderToc => {
                let Some(r) = &self.reader else { return vec![] };
                let mut v = vec![(Back, ui::back_rect())];
                for i in 0..crate::reader::TOC_ROWS {
                    if self.toc_page * crate::reader::TOC_ROWS + i < r.book.chapters() {
                        v.push((Row(i), crate::reader::toc_row(i)));
                    }
                }
                v.push((Prev, Rect::new(0, 1300, 360, 148)));
                v.push((Next, Rect::new(712, 1300, 360, 148)));
                v
            }
            Screen::ClaudeFiles => {
                let mut v = vec![(Back, ui::back_rect())];
                let n = self.cur_chat.as_ref().map_or(0, |c| c.files.len().min(crate::chat::FILE_ROWS));
                v.extend((0..n).map(|i| (Row(i), crate::chat::file_row(i))));
                v
            }
            Screen::ClaudeDoc => {
                let mut v = vec![(Back, ui::back_rect())];
                if self.doc.2 > 0 {
                    v.push((Prev, crate::chat::prev_rect()));
                }
                if self.doc.2 + 1 < self.doc.1.len() {
                    v.push((Next, crate::chat::next_rect()));
                }
                v
            }
            Screen::Achados => vec![(Back, ui::back_rect())],
            Screen::ClaudeAttach => {
                let mut v = vec![(Back, ui::back_rect()), (Primary, crate::chat::attach_done_rect())];
                let n = self.attach_items.len().saturating_sub(self.attach_page * crate::chat::ATTACH_ROWS).min(crate::chat::ATTACH_ROWS);
                v.extend((0..n).map(|k| (Row(k), crate::chat::attach_row(k))));
                if self.attach_page > 0 {
                    v.push((Prev, crate::chat::attach_nav(0)));
                }
                if (self.attach_page + 1) * crate::chat::ATTACH_ROWS < self.attach_items.len() {
                    v.push((Next, crate::chat::attach_nav(1)));
                }
                v
            }
            Screen::ClaudeCompose => {
                let mut v = vec![(Back, ui::back_rect()), (Secondary, crate::chat::attach_rect())];
                if self.claude_about.is_some() {
                    v.push((Ctx, crate::chat::context_rect()));
                }
                let has_card = self.claude_about.is_some() || self.cur_chat.is_some();
                v.extend(crate::chat::chip_rects(&self.fonts, &self.claude_sugg, has_card).into_iter().enumerate().map(|(k, r)| (Level(k), r)));
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
            Screen::Edicao => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend(self.ed_hits.iter().enumerate().map(|(k, (r, _))| (Row(k), *r)));
                v
            }
            Screen::Missoes => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend(self.m_hits.iter().enumerate().map(|(k, (r, _))| (Row(k), *r)));
                v
            }
            Screen::Artigos => {
                let mut v = vec![(Back, ui::back_rect())];
                let n = crate::painel::articles().len();
                let k0 = self.art_page * crate::painel::ART_ROWS;
                v.extend((0..n.saturating_sub(k0).min(crate::painel::ART_ROWS)).map(|k| (Row(k0 + k), crate::painel::article_row(k))));
                if self.art_page > 0 {
                    v.push((Prev, crate::painel::edition_nav(0)));
                }
                if k0 + crate::painel::ART_ROWS < n {
                    v.push((Next, crate::painel::edition_nav(1)));
                }
                v
            }
            // Porta-retrato: a foto inteira é o alvo (passa); o canto de cima volta.
            Screen::Retrato => vec![(Back, Rect::new(0, 0, 360, 200)), (Next, Rect::new(0, 200, 1072, 1248))],
            Screen::Terminal => {
                let mut v = vec![(Back, ui::back_rect()), (Continue, crate::painel::type_rect())];
                v.extend((0..crate::painel::COMMANDS.len()).map(|k| (Row(k), crate::painel::command_rect(k))));
                v
            }
            Screen::Aprovar => vec![(Level(0), crate::painel::approve_rect(false)), (Level(1), crate::painel::approve_rect(true))],
            Screen::WifiPass | Screen::LibSearch | Screen::MiniText | Screen::TermType | Screen::MissaoText => {
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
                let mut v = vec![(Back, ui::back_rect()), (Primary, crate::today_ui::pomodoro_rect()), (NewGame, crate::today_ui::add_rect()), (Secondary, crate::today_ui::calendar_rect())];
                v.extend(self.today_hits.iter().map(|(i, r)| (Item(*i), *r)));
                v
            }
            Screen::Reminder => vec![(Primary, crate::today_ui::reminder_ok_rect())],
            Screen::Calendar => vec![(Back, ui::back_rect())],
            Screen::Tutorial => vec![(Back, ui::back_rect()), (Prev, crate::tutorial::prev_rect()), (Next, crate::tutorial::next_rect())],
            Screen::Games => {
                let mut v = vec![(Back, ui::back_rect())];
                v.extend(apps::games_layout().0.into_iter().map(|(i, r)| (Game(i), r)));
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
                let n = if self.sudoku_practice { 3 } else { 2 };
                let saved = !self.sudoku_practice && crate::sudoku::Game::load().is_some_and(|g| !g.solved());
                v.extend((0..n).map(|k| (Level(k), crate::sudoku::resume_level_rect(k, saved))));
                if saved {
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
                let mut v = vec![(Back, ui::back_rect())];
                v.extend(crate::chess_ui::hub_ids(has_resume).into_iter().enumerate().map(|(k, id)| (Game(id), crate::chess_ui::hub_row(k))));
                v.push((Sort, crate::chess_ui::clock_row_rect()));
                v
            }
            Screen::ChessClock => {
                let mut v = vec![(Back, ui::back_rect()), (Level(0), crate::chess_ui::clock_option_rect(None))];
                v.extend((0..crate::chess::CONTROLS.len()).map(|k| (Level(k + 1), crate::chess_ui::clock_option_rect(Some(k)))));
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
                let mut v = vec![(Back, ui::back_rect()), (Undo, crate::chess::undo_rect())];
                if !self.chess_buttons().1 .0.is_empty() {
                    v.push((NewGame, crate::chess::new_rect()));
                }
                if self.chess_over.is_some() {
                    return v;
                }
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
                if !self.clips.is_empty() {
                    v.push((Clips, crate::clippings::review_rect()));
                }
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

    /// Abre EPUB/TXT no leitor próprio (na hora, sem trocar de programa).
    fn open_reader(&mut self, path: std::path::PathBuf, title: String) {
        data::set_last_book(&path, &title);
        if self.rfonts.is_none() {
            self.rfonts = Some(crate::reader::RFonts::load());
        }
        let t0 = Instant::now();
        let rf = self.rfonts.as_ref().unwrap();
        match crate::reader::Reader::open(&path, rf) {
            Ok(r) => {
                log(&format!("leitor: {} aberto em {} ms", title, t0.elapsed().as_millis()));
                self.reader = Some(r);
                self.go(Screen::Reader);
            }
            Err(e) => {
                log(&format!("leitor: {e}; abrindo no KOReader"));
                self.open_book(path, title);
            }
        }
    }

    /// Vira a página no leitor; atualização completa só de vez em quando (limpa rastros).
    fn turn_page(&mut self, forward: bool) {
        let (Some(r), Some(rf)) = (self.reader.as_mut(), self.rfonts.as_ref()) else { return };
        let moved = if forward { r.next(rf) } else { r.prev(rf) };
        if moved {
            let flash = r.turns % 8 == 0;
            self.render(flash);
        }
    }

    /// Abre um livro no KOReader (a sessão troca de app pelo código de saída).
    fn open_book(&mut self, path: std::path::PathBuf, title: String) {
        data::set_last_book(&path, &title);
        let _ = std::fs::write(OPEN_FILE, path.to_string_lossy().as_bytes());
        apps::draw_opening_book(&mut self.fb, &self.fonts, &title);
        self.fb.present(Wave::Quality, true);
        self.fb.wait();
        log(&format!("abrindo no KOReader: {}", path.display()));
        self.exit_code = Some(EXIT_OPEN_BOOK);
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
                if self.screen == Screen::LibSearch {
                    self.lib.query = self.compose_text.trim().to_string();
                    return self.go(Screen::Library);
                }
                if self.screen == Screen::MiniText {
                    return self.mini_text_done();
                }
                if self.screen == Screen::MissaoText {
                    self.mstage.text = self.compose_text.trim().to_string();
                    return self.go(Screen::Missoes);
                }
                if self.screen == Screen::TermType {
                    let cmd = self.compose_text.trim().to_string();
                    self.term = (cmd.clone(), crate::painel::run(&cmd));
                    return self.go(Screen::Terminal);
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
        if self.screen == Screen::LibSearch {
            let q = self.compose_text.trim();
            self.compose_hint = if q.is_empty() {
                "Digite parte do título.".into()
            } else {
                match self.lib.count_matches(q) {
                    0 => "Nenhum livro com esse trecho.".into(),
                    1 => "1 livro encontrado. Toque em OK.".into(),
                    k => format!("{k} livros encontrados. Toque em OK."),
                }
            };
        }
        if self.screen == Screen::ClaudeCompose {
            crate::chat::draw_compose_field(&mut self.fb, &self.fonts, &self.compose_text, &self.compose_hint);
            let r = crate::chat::compose_field_rect();
            self.fb.refresh(Rect::new(0, r.y - 6, 1072, 892 - (r.y - 6)), Wave::Fast, false);
        } else {
            crate::today_ui::draw_field(&mut self.fb, &self.fonts, &self.compose_text, &self.compose_hint);
            self.fb.refresh(Rect::new(0, crate::today_ui::field_rect().y - 4, 1072, 470), Wave::Quality, false);
        }
        if relayout {
            self.keys = crate::keyboard::layout(self.kb_mode, self.kb_shift);
            crate::keyboard::draw(&mut self.fb, &self.fonts, &self.keys);
            self.fb.refresh(crate::keyboard::area(), Wave::Quality, false);
        }
    }

    /// Telas onde deslizar de baixo abre Luz e tela (fora as de teclado, onde o gesto atrapalha).
    fn light_swipe_ok(&self) -> bool {
        !matches!(
            self.screen,
            Screen::Sleep | Screen::Light | Screen::Settings | Screen::Wifi | Screen::WifiPass | Screen::Compose | Screen::ClaudeCompose | Screen::LibSearch | Screen::MiniText | Screen::TermType | Screen::MissaoText | Screen::Aprovar | Screen::Retrato | Screen::Update
        )
    }

    /// O painel Luz e tela mostra o botão do Claude: veio de uma tela que o Claude entende.
    fn light_claude_ok(&self) -> bool {
        // Da Início, o botão abre o Claude; de outra tela, pergunta sobre ela.
        self.light_return.is_none_or(|s| s == Screen::Home || Self::claude_ok_for(s))
    }

    /// Telas sobre as quais dá para perguntar ao Claude (o botão no painel puxado de cima).
    fn claude_ok_for(s: Screen) -> bool {
        !matches!(
            s,
            Screen::Home
                | Screen::Sleep
                | Screen::Claude
                | Screen::ClaudeLive
                | Screen::ClaudeChat
                | Screen::ClaudeCompose
                | Screen::ClaudeFiles
                | Screen::ClaudeDoc
                | Screen::Compose
                | Screen::WifiPass
                | Screen::LibSearch
                | Screen::MiniText
                | Screen::TermType
                | Screen::MissaoText
                | Screen::Aprovar
                | Screen::Update
                | Screen::Reminder
                | Screen::Light
        )
    }

    /// Pergunta ao Claude sobre o que está na tela agora (deslizar de cima para baixo).
    /// Guarda o que estava na tela ao dormir: o trecho da página (livro) ou uma miniatura da
    /// partida (jogos). Nas outras telas, nada (o repouso mostra a ilustração do dia).
    fn capture_sleep(&self) -> Option<ui::SleepScene> {
        let (what, detail) = self.here_label();
        match self.screen {
            Screen::Reader | Screen::ReaderToc => {
                let r = self.reader.as_ref()?;
                Some(ui::SleepScene { what, detail, quote: Some(r.page_text()), shot: None, photo: None })
            }
            Screen::Mini | Screen::Sudoku | Screen::SudokuWon | Screen::Chess => {
                // Miniatura da tela (sem a faixa do botão Início), a 60%.
                let (w, top) = (self.fb.width, 174);
                let h = self.fb.height - top;
                let img = image::GrayImage::from_raw(w as u32, h as u32, self.fb.buf[(top * w) as usize..].to_vec())?;
                let (tw, th) = (624u32, (624.0 * h as f32 / w as f32) as u32);
                let small = image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle);
                Some(ui::SleepScene { what, detail, quote: None, shot: Some((tw as i32, th as i32, small.into_raw())), photo: None })
            }
            Screen::Retrato => {
                let list = crate::painel::photos();
                let p = list.get(self.photo_k % list.len().max(1))?.clone();
                Some(ui::SleepScene { what, detail, quote: None, shot: None, photo: Some(p) })
            }
            _ => None,
        }
    }

    /// O que está na tela, em duas partes curtas: (o quê, detalhe). Usado pelo Claude e pelo repouso.
    fn here_label(&self) -> (String, String) {
        let ctx = self.screen_context();
        let mut lines = ctx.lines();
        let first = lines.next().unwrap_or("").to_string();
        let _ = lines.next();
        let detail = match self.screen {
            Screen::Mini => self.mini.as_ref().map(|g| g.summary()).unwrap_or_default(),
            Screen::Sudoku | Screen::SudokuWon => self.sudoku.as_ref().map(|g| format!("{} · {} de 81", g.label(), g.filled())).unwrap_or_default(),
            Screen::Chess | Screen::ChessEnd | Screen::ChessReview => self.chess.as_ref().map(|g| format!("lance {} · {}", g.moves.len() / 2 + 1, if g.pos.white_to_move { "brancas jogam" } else { "pretas jogam" })).unwrap_or_default(),
            Screen::Reader | Screen::ReaderToc => self.reader.as_ref().map(|r| format!("{} · {}%", r.chapter_title, (r.progress() * 100.0).round() as i32)).unwrap_or_default(),
            _ => data::last_book().map(|b| format!("lendo {}", b.title)).unwrap_or_default(),
        };
        let what = match self.screen {
            Screen::Mini => {
                if self.mini_back == Screen::Games { apps::GAMES.get(self.mini_game).map(|g| g.0).unwrap_or("Jogo").to_string() } else { "Receitas".to_string() }
            }
            Screen::Sudoku | Screen::SudokuWon => "Sudoku".into(),
            Screen::Chess | Screen::ChessEnd | Screen::ChessReview => "Xadrez".into(),
            Screen::Today | Screen::Calendar => "Seu dia".into(),
            Screen::Library => "Biblioteca".into(),
            Screen::Reader | Screen::ReaderToc => self.reader.as_ref().map(|r| r.book.title.clone()).unwrap_or_else(|| "Livro".into()),
            _ => first.chars().take(40).collect(),
        };
        (what, detail)
    }

    fn ask_claude_here(&mut self) {
        let (what, detail) = self.here_label();
        self.claude_about = Some((what.clone(), detail));
        self.claude_sugg = if matches!(self.screen, Screen::Reader | Screen::ReaderToc) {
            ["Explique esta página", "Resuma o capítulo até aqui", "Quem é quem nesta parte?"].iter().map(|s| s.to_string()).collect()
        } else {
            Self::suggestions_for(Some(&what), false)
        };
        self.claude_ctx = Some(self.screen_context());
        self.claude_return = Some(self.screen);
        self.cur_chat = None;
        self.open_compose();
    }

    /// Perguntas prontas para o momento (um toque envia).
    fn suggestions_for(what: Option<&str>, continuing: bool) -> Vec<String> {
        let v: &[&str] = match what {
            Some("UNO") => &["Qual a melhor jogada agora?", "Que cor eu escolho?", "Explique o acúmulo"],
            Some("Xadrez") => &["Analise a posição", "Qual é o plano aqui?", "Estou correndo perigo?"],
            Some("Sudoku") => &["Uma dica sem entregar", "Qual técnica uso aqui?"],
            Some("Coroas") => &["Uma dica sem entregar", "Por onde eu começo?"],
            Some("Minas") => &["Onde é seguro abrir?", "Explique esse padrão"],
            Some("Palavra") => &["Uma dica sem entregar", "Que letras testar?"],
            Some("Lig 4") => &["Qual coluna eu jogo?", "Estou correndo perigo?"],
            Some("Cripta") => &["Vale lutar agora?", "Qual caminho seguir?"],
            Some("Histórias") => &["O que você escolheria?", "Resuma até aqui"],
            Some("Receitas") => &["Posso trocar um ingrediente?", "Dá para dobrar a receita?"],
            Some("Seu dia") => &["Organize meu dia", "O que priorizar?"],
            Some("Biblioteca") => &["O que ler a seguir?", "Resuma o livro que estou lendo"],
            Some(_) => &["Explique o que estou vendo", "Uma dica"],
            None if continuing => &["Explique melhor", "Dê um exemplo", "Resuma em tópicos"],
            None => &["Resuma o livro que estou lendo", "Organize meu dia", "Um desafio de lógica", "Me ensine algo em 2 minutos"],
        };
        v.iter().map(|s| s.to_string()).collect()
    }

    /// O que está na tela, em texto, para o Claude entender a pergunta.
    fn screen_context(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        match self.screen {
            Screen::Mini => {
                let name = apps::GAMES.get(self.mini_game).map(|g| g.0).unwrap_or("jogo");
                let name = if self.mini_back == Screen::Games { name } else { "Receitas" };
                parts.push(format!("Pedro está em {name}, no Kindle."));
                if let Some(g) = &self.mini {
                    let c = g.context();
                    if !c.is_empty() {
                        parts.push(c);
                    }
                }
            }
            Screen::Sudoku | Screen::SudokuWon => {
                if let Some(g) = &self.sudoku {
                    let grid: Vec<String> = (0..9).map(|r| (0..9).map(|c| match g.cells[r * 9 + c] { 0 => '.', n => (b'0' + n) as char }).collect()).collect();
                    parts.push(format!("Pedro está jogando Sudoku ({}), {} erros, {} dicas. Grade atual (. = vazio):\n{}", g.label(), g.mistakes, g.hints, grid.join("\n")));
                }
            }
            Screen::Chess | Screen::ChessEnd | Screen::ChessReview => {
                if let Some(g) = &self.chess {
                    let who = match g.mode {
                        crate::chess::Mode::Coach { level, user_white } => {
                            let c = crate::coach::coach(level);
                            format!("contra o treinador {} ({}), Pedro de {}", c.name, c.rating, if user_white { "brancas" } else { "pretas" })
                        }
                        crate::chess::Mode::Local => "duas pessoas no mesmo Kindle".into(),
                    };
                    parts.push(format!("Pedro está jogando xadrez {who}.\nPosição (FEN): {}\nLances (UCI): {}", g.pos.fen(g.moves.len() / 2 + 1), g.moves.join(" ")));
                }
            }
            Screen::Today | Screen::Calendar => {
                let plan = std::fs::read_to_string(home_dir().join("data/dia.txt")).unwrap_or_default();
                parts.push(format!("Pedro está olhando o plano do dia no Kindle:\n{}", plan.trim()));
            }
            Screen::Library => parts.push("Pedro está na biblioteca de livros do Kindle.".into()),
            Screen::Reader | Screen::ReaderToc => {
                if let Some(r) = &self.reader {
                    parts.push(format!(
                        "Pedro está lendo \"{}\" no Kindle (arquivo {}), capítulo {} de {} (\"{}\"), {}% do livro. Página atual:\n{}",
                        r.book.title,
                        r.book.path.display(),
                        r.chapter + 1,
                        r.book.chapters(),
                        r.chapter_title,
                        (r.progress() * 100.0).round() as i32,
                        r.page_text()
                    ));
                    return parts.join("\n");
                }
            }
            _ => {}
        }
        if let Some(b) = data::last_book() {
            let pct = b.progress.map(|p| format!(", {}% lido", (p * 100.0).round() as i32)).unwrap_or_default();
            parts.push(format!("Livro em leitura: {}{pct} (arquivo {}).", b.title, b.path.display()));
        }
        parts.join("\n")
    }

    /// Baixa e abre um arquivo do Claude: livro/PDF no KOReader, texto aqui mesmo.
    fn open_claude_file(&mut self, i: usize) {
        let Some((id, name)) = self.cur_chat.as_ref().and_then(|c| c.files.get(i).map(|f| (c.id.clone(), f.0.clone()))) else { return };
        self.files_note = format!("Baixando {name}…");
        self.render(false);
        self.fb.wait();
        match crate::chat::download(&id, &name) {
            Ok(path) if crate::chat::opens_in_reader(&name) => {
                self.files_note.clear();
                self.open_book(path, name);
            }
            Ok(path) => {
                let text = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
                let lines = crate::chat::layout_text(&self.fonts, &text, false, 1072 - 144);
                self.doc = (name, crate::chat::pages_of(lines), 0);
                self.files_note.clear();
                self.go(Screen::ClaudeDoc);
            }
            Err(e) => {
                self.files_note = e;
                self.render(false);
            }
        }
    }

    /// Nova conversa ou resposta, sem contexto de tela.
    fn start_claude_compose(&mut self) {
        self.claude_ctx = None;
        self.claude_about = None;
        self.claude_sugg = Self::suggestions_for(None, self.cur_chat.is_some());
        self.open_compose();
    }

    fn open_compose(&mut self) {
        self.compose_text.clear();
        self.compose_hint = "Toque numa pergunta pronta, ou escreva e toque em OK.".into();
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
    /// Botão da frase da Início: abre a tela que a frase sugere (ou guarda o achado raro).
    fn phrase_action(&mut self) {
        use crate::saudacao::Act;
        use Target::*;
        let act = ui::home_phrase(&self.status).act;
        let open_mini = |app: &mut Self, g: Box<dyn crate::minigame::MiniGame>, slot: usize| {
            app.mini = Some(app.mini_cache.remove(&slot).unwrap_or(g));
            app.mini_slot = Some(slot);
            app.mini_back = Screen::Home;
            app.go(Screen::Mini);
        };
        match act {
            Act::None => {}
            Act::Aprender => self.activate(Tile(Module::ALL.iter().position(|m| *m == Module::Violao).unwrap_or(4)), 0),
            Act::Aula => open_mini(self, Box::new(crate::violao::Violao::open_aula()), 103),
            Act::Revisao => open_mini(self, Box::new(crate::revisao::Revisao::new()), 104),
            Act::Semana => open_mini(self, Box::new(crate::semana::Semana::new()), 102),
            Act::Edicao => {
                self.ed_page = 0;
                self.go(Screen::Edicao);
            }
            Act::Jogo => match data::last_game().filter(|&i| i < apps::GAMES.len()) {
                Some(i) => {
                    self.go(Screen::Games);
                    self.activate(Game(i), 0);
                }
                None => self.go(Screen::Games),
            },
            Act::Ler => self.activate(Continue, 0),
            Act::Foco => self.go(Screen::Focus),
            Act::Claude => self.go(Screen::Claude),
            Act::Hoje => self.go(Screen::Today),
            Act::Jogar => self.go(Screen::Games),
            Act::Colecao => {
                self.achado_new = None;
                self.go(Screen::Achados);
            }
            Act::Achado(i) => {
                let day = data::today(self.status.offset_min);
                self.achado_new = crate::saudacao::collect(i, day).then_some(i);
                self.go(Screen::Achados);
            }
        }
    }

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
        // Anexos: sobem antes da pergunta (cada um vira um arquivo na pasta da conversa).
        let mut ids = Vec::new();
        for i in self.attach_sel.clone() {
            let Some((path, name)) = self.attach_items.get(i).and_then(crate::chat::attach_file) else { continue };
            self.update_msg = ("Claude".into(), vec![format!("Mandando {name}…"), "".into(), text.chars().take(80).collect()], None);
            self.render(true);
            match crate::chat::upload(&path, &name) {
                Ok(id) => ids.push(id),
                Err(e) => {
                    self.compose_hint = format!("Anexo {name}: {e}");
                    self.screen = Screen::ClaudeCompose;
                    return self.render(true);
                }
            }
        }
        if !ids.is_empty() {
            self.update_msg = ("Claude".into(), vec!["Lendo os anexos e pensando… pode levar até um minuto.".into(), "".into(), text.chars().take(80).collect()], None);
            self.render(true);
        }
        match crate::chat::send_with(id.as_deref(), &text, self.claude_ctx.as_deref(), &ids) {
            Ok(new_id) => {
                self.compose_text.clear();
                self.claude_ctx = None;
                self.attach_sel.clear();
                self.attach_items.clear();
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
        let (lv, secs, hints, fase) = self.sudoku.as_ref().map(|g| (g.level, g.seconds(), g.hints, g.fase)).unwrap();
        if fase > 0 {
            self.sudoku_prog.record_win(fase, crate::fases::today());
        }
        // Recorde (por nível) só vale no treino e sem dicas.
        let rec = fase == 0 && hints == 0 && crate::sudoku::submit_record(lv, secs);
        crate::sudoku::Game::clear_saved();
        log(&format!("sudoku {} resolvido em {} s (recorde: {rec})", lv.name(), secs));
        self.sudoku_won = (secs, rec);
        self.go(Screen::SudokuWon);
    }

    fn start_sudoku(&mut self, lv: crate::sudoku::Level) {
        crate::sudoku::set_last_level(lv);
        self.begin_sudoku(crate::sudoku::Game::new(lv));
    }

    fn start_sudoku_fase(&mut self) {
        self.sudoku_prog = crate::fases::Progress::load("sudoku");
        self.begin_sudoku(crate::sudoku::Game::new_fase(self.sudoku_prog.level));
    }

    fn begin_sudoku(&mut self, g: crate::sudoku::Game) {
        log(&format!("sudoku {}: {} casas vazias", g.label(), 81 - g.filled()));
        g.save();
        self.sudoku = Some(g);
        self.sudoku_note = "Toque numa casa e depois num número.".into();
        self.go(Screen::Sudoku);
    }

    /// Jogo local com "Girar" ligado e vez das pretas: tela de cabeça para baixo.
    fn local_rotated(&self) -> bool {
        self.coach_level().is_none() && crate::chess::rotate_enabled() && self.chess.as_ref().is_some_and(|g| !g.pos.white_to_move)
    }

    /// Último jogo aberto e o resumo da partida (se houver), para o "Continuar" da tela inicial.
    fn home_game(&self) -> Option<(usize, String)> {
        let i = data::last_game().filter(|&i| i < apps::GAMES.len())?;
        let sum = self.live_games().get(i).cloned().flatten().unwrap_or_else(|| apps::GAMES[i].1.to_string());
        Some((i, sum))
    }

    /// Resumo da partida em andamento de cada jogo (None = sem partida).
    fn live_games(&self) -> Vec<Option<String>> {
        let mut live: Vec<Option<String>> = (0..apps::GAMES.len()).map(|i| self.mini_cache.get(&i).map(|g| g.summary()).filter(|t| !t.is_empty())).collect();
        live[0] = crate::sudoku::Game::load().filter(|g| !g.solved()).map(|g| format!("{} · {}/81", g.level.name(), g.filled()));
        live[1] = crate::chess::Game::load().filter(|g| g.in_progress()).map(|g| match g.mode {
            crate::chess::Mode::Coach { level, .. } => format!("Contra {} · {} lances", crate::coach::coach(level).name, g.history.len()),
            crate::chess::Mode::Local => format!("Duas pessoas · {} lances", g.history.len()),
        });
        live
    }

    pub fn mini_awake(&self) -> bool {
        (self.screen == Screen::Mini && self.mini.as_ref().is_some_and(|g| g.keep_awake())) || (self.screen == Screen::Chess && self.chess.as_ref().is_some_and(|g| g.clock_running()))
    }

    /// Relógio do xadrez: redesenha só os mostradores quando o número muda e encerra a partida
    /// quando cai a bandeira.
    fn chess_clock_tick(&mut self) {
        if self.screen != Screen::Chess || self.chess_over.is_some() {
            return;
        }
        let Some(g) = self.chess.as_mut() else { return };
        if !g.clock_running() {
            return;
        }
        if g.clock_tick() {
            let loser = g.clock.as_ref().and_then(|c| c.flag).unwrap_or(true);
            let winner_has = g.pos.board.iter().flatten().any(|p| p.white != loser && p.kind != crate::chess::Kind::King);
            let user_white = match g.mode {
                crate::chess::Mode::Coach { user_white, .. } => Some(user_white),
                _ => None,
            };
            match user_white {
                Some(uw) => {
                    let score = if !winner_has { 0.5 } else if uw == loser { 0.0 } else { 1.0 };
                    self.finish_coach_game(Some(score));
                }
                None => self.render(true),
            }
            return;
        }
        let shown = format!("{}|{}|{}", g.clock_left(true).map(crate::chess::clock_text).unwrap_or_default(), g.clock_left(false).map(crate::chess::clock_text).unwrap_or_default(), g.pos.white_to_move);
        if shown != self.clock_shown {
            self.clock_shown = shown;
            crate::chess::draw_clocks(&mut self.fb, &self.fonts, g);
            for r in [crate::chess::top_clock_rect(), crate::chess::bottom_clock_rect()] {
                self.fb.refresh(Rect::new(r.x - 6, r.y - 6, r.w + 12, r.h + 12), Wave::Fast, false);
            }
        }
    }

    /// Toque num minijogo: redesenha o que ele pedir e, se for a vez da CPU, joga por ela.
    /// Sincroniza com a VPS em segundo plano: avisos, modo longe e aprovações sempre; edição e
    /// caixa de entrada quando `full`. O resultado chega pelo canal e é aplicado no tick.
    fn background_sync(&mut self, full: bool) {
        if self.sync_rx.is_some() || !self.status.wifi || !crate::chat::configured() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        self.sync_rx = Some(rx);
        self.sync_fast = Instant::now();
        if full {
            self.sync_full = Some(Instant::now());
        }
        std::thread::spawn(move || {
            let s = crate::painel::sync();
            let (ed, inbox) = if full { (crate::painel::fetch_edition(), crate::painel::sync_inbox()) } else { (false, (0, 0)) };
            if full {
                // Perguntas do Claude para a revisão dos destaques (só os que ainda não têm).
                crate::revisao::prepare();
                // Currículos e as próximas lições dos tópicos do Aprender.
                crate::estudo::prepare();
                // Dica do professor de violão para a Aula do dia.
                crate::violao::prepare_tip();
            }
            let _ = tx.send((s, ed, inbox));
        });
    }

    /// Resultado da sincronização: abre o pedido de permissão por cima de tudo, e redesenha a
    /// tela do painel se chegou coisa nova.
    fn apply_sync(&mut self) {
        let Some(rx) = &self.sync_rx else { return };
        let Ok((s, ed, (na, nf))) = rx.try_recv() else { return };
        self.sync_rx = None;
        if let Some(s) = s {
            self.approvals = s.approvals;
        }
        if na + nf > 0 {
            self.panel_note = match (na, nf) {
                (a, 0) => format!("{a} artigo{} novo{}", if a == 1 { "" } else { "s" }, if a == 1 { "" } else { "s" }),
                (0, f) => format!("{f} foto{} nova{}", if f == 1 { "" } else { "s" }, if f == 1 { "" } else { "s" }),
                (a, f) => format!("{a} artigos e {f} fotos novas"),
            };
        }
        if !self.approvals.is_empty() && self.screen != Screen::Aprovar && self.screen != Screen::Sleep {
            self.approve_back = self.screen;
            return self.go(Screen::Aprovar);
        }
        let refresh = match self.screen {
            Screen::Edicao => ed,
            Screen::Artigos => na > 0,
            Screen::Retrato => nf > 0,
            Screen::Home => true,
            _ => false,
        };
        if refresh {
            self.render(false);
        }
    }

    /// Ações da tela de Missões.
    fn mission_action(&mut self, act: crate::painel::MAct) {
        use crate::painel::MAct;
        match act {
            MAct::Pick(i) => {
                self.mstage = crate::painel::MStage { template: Some(i), ..Default::default() };
                let livre = self.missions.as_ref().and_then(|m| m.templates.get(i)).is_some_and(|t| t.id == "livre");
                if livre {
                    self.compose_text.clear();
                    self.compose_hint = "Seja específico: projeto, o que fazer e o que não mexer.".into();
                    return self.go(Screen::MissaoText);
                }
            }
            MAct::Project(p) => self.mstage.project = Some(p),
            MAct::Prev => self.mstage.page = self.mstage.page.saturating_sub(1),
            MAct::Next => self.mstage.page += 1,
            MAct::Cancel => self.mstage = Default::default(),
            MAct::When(night) => {
                let (Some(i), Some(m)) = (self.mstage.template, self.missions.as_ref()) else { return };
                let tid = m.templates.get(i).map(|t| t.id.clone()).unwrap_or_default();
                let project = self.mstage.project.clone().unwrap_or_default();
                self.panel_note = match crate::painel::send_mission(&tid, &project, &self.mstage.text, night) {
                    Ok(t) if night => format!("Combinado: \"{t}\" roda esta noite."),
                    Ok(t) => format!("Mandada: \"{t}\". Os agentes já começam."),
                    Err(e) => format!("Não deu para mandar: {e}"),
                };
                self.mstage = Default::default();
                if let Ok(m) = crate::painel::missions() {
                    self.missions = Some(m);
                }
            }
            MAct::Open(article, title) => {
                match crate::painel::mission_report(&article, &title) {
                    Ok(path) => return self.open_reader(path, title),
                    Err(e) => self.panel_note = format!("Não abriu o relatório: {e}"),
                }
            }
        }
        self.render(true);
    }

    /// Ações da Edição da manhã.
    fn edition_action(&mut self, act: crate::painel::EdAction) {
        use crate::painel::EdAction;
        match act {
            EdAction::Prev => {
                self.ed_page = self.ed_page.saturating_sub(1);
                self.render(true);
            }
            EdAction::Next => {
                self.ed_page += 1;
                self.render(true);
            }
            EdAction::Read(url) => {
                // Buscar e limpar uma notícia leva alguns segundos: diz isso na tela antes.
                self.fb.fill(self.fb.screen(), ui::PAPER);
                ui::draw_field_header(&mut self.fb, &self.fonts, "EDIÇÃO DA MANHÃ", "Buscando a notícia…");
                self.fonts.draw(&mut self.fb, crate::text::Face::Body, 32.0, 72, 460, ui::MUTED, "O texto vem limpo, para ler no Verso Ink. Fica guardado em Artigos.");
                self.fb.refresh(self.fb.screen(), Wave::Quality, true);
                self.fb.wait();
                match crate::painel::fetch_article(&url) {
                    Ok(path) => {
                        let title = std::fs::read_to_string(&path).ok().and_then(|t| t.lines().next().map(str::to_string)).unwrap_or_default();
                        self.open_reader(path, title);
                    }
                    Err(e) => {
                        self.panel_note = format!("Não consegui buscar: {e}");
                        self.go(Screen::Edicao);
                    }
                }
            }
            EdAction::Ask(ctx) => {
                let short: String = ctx.chars().take(60).collect();
                self.claude_ctx = Some(ctx);
                self.claude_about = Some(("Edição da manhã".into(), short));
                self.claude_sugg = ["O que eu faço primeiro?", "Explique melhor", "Resuma em uma frase"].iter().map(|s| s.to_string()).collect();
                self.cur_chat = None;
                self.open_compose();
            }
            EdAction::Dismiss(id) => {
                self.panel_note = match crate::painel::dismiss_notice(&id) {
                    Ok(()) => "Aviso dispensado.".into(),
                    Err(e) => format!("Não deu para dispensar: {e}"),
                };
                self.render(true);
            }
            EdAction::Open(id) => {
                crate::painel::toggle_open(&id);
                self.render(false);
            }
            EdAction::Mail(id, acao) => {
                self.panel_note = match crate::painel::mail_action(&id, acao) {
                    Ok(()) => match acao {
                        "rascunho" => "O PC vai escrever o rascunho; o texto chega em Missões.".into(),
                        "lembrar" => "Volta na edição de amanhã.".into(),
                        "aprovar" => "Commit aprovado.".into(),
                        "corrigir" => "Um agente vai corrigir os pontos; o relatório chega em Missões.".into(),
                        "desfazer" => "Um agente vai desfazer o commit (git revert, sem push).".into(),
                        _ => "E-mail tirado da edição.".into(),
                    },
                    Err(e) => format!("Não deu: {e}"),
                };
                self.render(true);
            }
            EdAction::Article(id, title) => match crate::painel::article_by_id(&id, &title) {
                Ok(path) => self.open_reader(path, title),
                Err(e) => {
                    self.panel_note = format!("Não consegui abrir: {e}");
                    self.render(true);
                }
            },
            EdAction::Regen => {
                self.panel_note = match crate::painel::regen_edition() {
                    Ok(()) => "Montando uma edição nova; leva uns 2 minutos.".into(),
                    Err(e) => format!("Sem conexão com a VPS: {e}"),
                };
                self.sync_full = None;
                self.render(true);
            }
        }
    }

    fn decide_approval(&mut self, allow: bool) {
        if let Some(a) = self.approvals.first().cloned() {
            if let Err(e) = crate::painel::decide(&a.id, allow) {
                log(&format!("aprovação: {e}"));
            }
            self.approvals.remove(0);
        }
        if self.approvals.is_empty() {
            let back = self.approve_back;
            self.go(back);
        } else {
            self.render(true);
        }
    }

    /// Selo "Pensando…" no canto de cima e uma pausa de 0,8 a 1,6 s (a primeira jogada da
    /// sequência pensa mais; as seguintes, como a CPU emendando capturas, menos).
    fn cpu_thinking(&mut self, first: bool) {
        let r = Rect::new(1072 - 72 - 250, 84, 250, 66);
        self.fb.fill(Rect::new(r.x - 4, r.y - 4, r.w + 8, r.h + 12), ui::PAPER);
        ui::card(&mut self.fb, r, r.h / 2, ui::INK, false);
        let t = "Pensando…";
        let w = self.fonts.measure(crate::text::Face::Strong, 30.0, t);
        self.fonts.draw(&mut self.fb, crate::text::Face::Strong, 30.0, r.x + (r.w - w) / 2, r.y + 44, ui::PAPER, t);
        self.fb.refresh(Rect::new(r.x - 4, r.y - 4, r.w + 8, r.h + 12), Wave::Fast, false);
        let ms = if first { 800 + self.rng.next() % 800 } else { 450 + self.rng.next() % 300 };
        std::thread::sleep(Duration::from_millis(ms));
    }

    /// Texto pronto para o minijogo: mostra que está trabalhando (pode levar um minuto, se
    /// for o Claude escrevendo) e devolve ao jogo.
    fn mini_text_done(&mut self) {
        let text = self.compose_text.trim().to_string();
        if text.is_empty() {
            self.compose_hint = "Escreva alguma coisa antes de tocar em OK.".into();
            return self.render(false);
        }
        self.fb.fill(self.fb.screen(), ui::PAPER);
        ui::draw_field_header(&mut self.fb, &self.fonts, "UM MOMENTO", "Escrevendo…");
        let hint = "O Claude está escrevendo a continuação. Leva até um minuto.";
        self.fonts.draw(&mut self.fb, crate::text::Face::Body, 32.0, 72, 460, ui::MUTED, hint);
        self.fb.refresh(self.fb.screen(), Wave::Quality, true);
        self.fb.wait();
        if let Some(g) = self.mini.as_mut() {
            g.text_input(&text);
        }
        self.go(Screen::Mini);
    }

    fn mini_tap(&mut self, id: u32, x: i32) {
        let Some(g) = self.mini.as_mut() else { return };
        // Novo jogo e recomeçar pedem um segundo toque: um esbarrão não apaga a partida.
        let pending = self.mini_confirm.take();
        if g.risky(id) && pending != Some(id) {
            if let Some((_, r)) = g.targets().into_iter().find(|(t, _)| *t == id) {
                self.mini_confirm = Some(id);
                g.draw(&mut self.fb, &self.fonts);
                crate::minigame::pill(&mut self.fb, &self.fonts, r, "Toque de novo", true);
                self.fb.refresh(r, Wave::Quality, false);
                return;
            }
        }
        if let Some(p) = pending.filter(|&p| p != id) {
            // Desfaz o aviso no botão que esperava confirmação.
            if let Some((_, r)) = g.targets().into_iter().find(|(t, _)| *t == p) {
                g.draw(&mut self.fb, &self.fonts);
                self.fb.refresh(r, Wave::Quality, false);
            }
        }
        let Some(g) = self.mini.as_mut() else { return };
        let r = g.tap(id, x, 0);
        if let Some(prompt) = g.wants_text() {
            self.mini_prompt = prompt;
            self.compose_text.clear();
            self.compose_hint = "Escreva o que acontece agora e toque em OK.".into();
            return self.go(Screen::MiniText);
        }
        self.apply_mini_redraw(r);
        let mut guard = 0;
        while self.mini.as_ref().is_some_and(|g| g.cpu_turn()) && guard < 200 {
            guard += 1;
            // A CPU "pensa" um pouco antes de jogar: responder na hora parece pouco natural.
            self.cpu_thinking(guard == 1);
            if let Some(g) = self.mini.as_mut() {
                g.cpu_move();
            }
            if let Some(g) = &self.mini {
                g.draw(&mut self.fb, &self.fonts);
            }
            self.fb.refresh(Rect::new(0, 60, 1072, 1448 - 60), Wave::Quality, false);
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
    /// Confirmação pedida há menos de 5 s?
    fn confirming(&self, kind: u8) -> bool {
        self.chess_confirm.is_some_and(|(k, t)| k == kind && t.elapsed() < Duration::from_secs(5))
    }

    /// Botões de baixo da partida conforme o momento (e o recado no lugar do estado).
    fn chess_buttons(&self) -> ((String, bool), (String, bool), Option<String>) {
        if let Some(over) = &self.chess_over {
            return (("Voltar ao menu".into(), true), (String::new(), false), Some(over.clone()));
        }
        if self.chess.as_ref().is_some_and(|g| g.result().is_some()) {
            // Fim por mate, afogamento ou empate de regra: só sair.
            return (("Voltar ao menu".into(), true), (String::new(), false), None);
        }
        if self.chess_offer {
            let who = if self.chess.as_ref().is_some_and(|g| g.pos.white_to_move) { "As brancas propõem empate." } else { "As pretas propõem empate." };
            return (("Aceitar empate".into(), true), ("Recusar".into(), false), Some(who.into()));
        }
        let left = if self.confirming(0) { ("Confirmar empate".to_string(), true) } else { ("Empate".to_string(), false) };
        let right = if self.confirming(1) { ("Confirmar desistência".to_string(), true) } else { ("Desistir".to_string(), false) };
        let note = if self.confirming(0) || self.confirming(1) { Some("Toque de novo para confirmar.".to_string()) } else { None };
        (left, right, note)
    }

    fn chess_note(&mut self, text: &str) {
        if self.coach_level().is_some() {
            self.say(text);
        } else {
            self.render(false);
        }
    }

    /// Propor empate: o treinador aceita se não estiver na frente (e a partida já andou);
    /// no jogo local, o outro jogador decide.
    fn offer_draw(&mut self) {
        let Some(g) = &self.chess else { return };
        match g.mode {
            crate::chess::Mode::Coach { user_white, .. } => {
                let mat = g.pos.material();
                let coach_ahead = if user_white { -mat } else { mat };
                if g.history.len() >= 20 && coach_ahead <= 0 {
                    self.finish_coach_game(Some(0.5));
                } else if g.history.len() < 20 {
                    self.say("Ainda é cedo para empate. Vamos jogar mais um pouco.");
                } else {
                    self.say("Prefiro continuar: acho que estou melhor.");
                }
            }
            crate::chess::Mode::Local => {
                self.chess_offer = true;
                self.render(false);
            }
        }
    }

    /// Fim do jogo local: apaga a partida salva e mostra o recado com o botão de voltar.
    fn end_local(&mut self, text: &str) {
        crate::chess::Game::clear_saved();
        self.chess_over = Some(text.to_string());
        if let Some(g) = self.chess.as_mut() {
            g.clock_pause();
        }
        self.render(true);
    }

    fn say(&mut self, text: &str) {
        self.coach_msg = text.to_string();
        if let Some(level) = self.coach_level() {
            let c = crate::coach::coach(level);
            crate::chess_ui::draw_coach_line(&mut self.fb, &self.fonts, &format!("{} ({})", c.name, c.rating), text);
            if let Some(g) = &self.chess {
                crate::chess::draw_clocks(&mut self.fb, &self.fonts, g);
            }
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
        self.coach_msg = if level == crate::coach::CLAUDE {
            format!("Oi, Pedro! Vamos jogar. Você está de {}. Vou comentando os lances.", if user_white { "brancas" } else { "pretas" })
        } else {
            format!("Olá, sou {}. {}. Você joga de {}.", c.name, c.style, if user_white { "brancas" } else { "pretas" })
        };
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
        let think_start = Instant::now();
        // Contra o Claude: o lance (e uma frase) vem da ponte; sem ela, o Stockfish forte joga.
        let mut claude_comment = None;
        if level == crate::coach::CLAUDE {
            if let Some((m, c)) = self.claude_move() {
                claude_comment = Some(c);
                let san = self.chess.as_mut().and_then(|g| g.play_uci(&m));
                self.coach_msg = match san {
                    Some(s) => format!("{s}. {}", claude_comment.clone().unwrap_or_default()).trim().to_string(),
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
                return self.say(&self.coach_msg.clone());
            }
        }
        let _ = &claude_comment;
        let random = level != crate::coach::CLAUDE && (self.rng.next() % 1000) as f32 / 1000.0 < crate::coach::random_move_chance(rating);
        let mv = if random {
            crate::coach::random_legal(&pos, &mut self.rng)
        } else {
            self.engine.as_mut().and_then(|e| {
                e.set_strength(rating);
                e.play(&moves, crate::coach::think_millis(rating))
            })
        };
        // Ninguém joga em 0,1 s: o treinador "pensa" ao menos 0,9 a 1,8 s, mais nos níveis altos.
        let min_ms = 900 + (rating as u64).min(3000) / 4 + self.rng.next() % 400;
        let spent = think_start.elapsed().as_millis() as u64;
        if spent < min_ms {
            std::thread::sleep(Duration::from_millis(min_ms - spent));
        }
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

    /// Pede o lance ao Claude pela ponte. None se a ponte falhar (o Stockfish joga no lugar).
    fn claude_move(&mut self) -> Option<(String, String)> {
        let g = self.chess.as_ref()?;
        let user_white = matches!(g.mode, crate::chess::Mode::Coach { user_white: true, .. });
        let legal: Vec<String> = g.pos.legal_uci().iter().map(|m| crate::chat::json_str(m)).collect();
        let moves: Vec<String> = g.moves.iter().map(|m| crate::chat::json_str(m)).collect();
        let body = format!(
            "{{\"fen\": {}, \"moves\": [{}], \"legal\": [{}], \"color\": \"{}\", \"level\": \"forte, mas jogando para ensinar\"}}",
            crate::chat::json_str(&g.pos.fen(g.moves.len() / 2 + 1)),
            moves.join(", "),
            legal.join(", "),
            if user_white { "b" } else { "w" }
        );
        match crate::chat::call("POST", "/xadrez/lance", Some(&body), 200) {
            Ok(j) => {
                let m = j.get("move").and_then(crate::chat::Json::str)?.to_string();
                let c = j.get("comment").and_then(crate::chat::Json::str).unwrap_or("").to_string();
                Some((m, c))
            }
            Err(e) => {
                log(&format!("xadrez com o Claude: {e}; o Stockfish joga"));
                None
            }
        }
    }

    /// Análise da partida pelo Claude, aberta como documento.
    fn claude_review(&mut self) {
        let Some(g) = self.chess.as_ref() else { return };
        let user_white = matches!(g.mode, crate::chess::Mode::Coach { user_white: true, .. });
        let moves: Vec<String> = g.moves.iter().map(|m| crate::chat::json_str(m)).collect();
        let result = self.end_info.0.clone();
        let body = format!(
            "{{\"moves\": [{}], \"evals\": [], \"color\": \"{}\", \"result\": {}, \"coach\": \"Claude\"}}",
            moves.join(", "),
            if user_white { "w" } else { "b" },
            crate::chat::json_str(&result)
        );
        self.fb.fill(self.fb.screen(), ui::PAPER);
        ui::draw_field_header(&mut self.fb, &self.fonts, "XADREZ", "O Claude está analisando…");
        self.fonts.draw(&mut self.fb, crate::text::Face::Body, 32.0, 72, 460, ui::MUTED, "Os lances que decidiram a partida e o que treinar. Leva até um minuto.");
        self.fb.refresh(self.fb.screen(), Wave::Quality, true);
        self.fb.wait();
        let text = match crate::chat::call("POST", "/xadrez/revisao", Some(&body), 320) {
            Ok(j) => j.get("text").and_then(crate::chat::Json::str).unwrap_or("").to_string(),
            Err(e) => format!("Não consegui a análise agora: {e}"),
        };
        let lines = crate::chat::layout_text(&self.fonts, &text, false, 1072 - 144);
        self.doc = ("Análise do Claude".into(), crate::chat::pages_of(lines), 0);
        self.doc_back = Screen::ChessEnd;
        self.go(Screen::ClaudeDoc);
    }

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
        crate::chess::draw_clocks(&mut self.fb, &self.fonts, g);
        let b = crate::chess::board_rect();
        let s = crate::chess::status_rect();
        self.fb.refresh(Rect::new(0, b.y, 1072, s.y + s.h - b.y), Wave::Quality, false);
    }

    pub fn go(&mut self, screen: Screen) {
        // Relógio do xadrez: para ao sair da partida e volta a correr ao entrar.
        if let Some(g) = self.chess.as_mut() {
            if screen == Screen::Chess && self.chess_over.is_none() {
                g.clock_resume();
            } else if self.screen == Screen::Chess {
                g.clock_pause();
                g.save();
            }
        }
        self.clock_shown.clear();
        if screen == Screen::Sleep && self.screen != Screen::Sleep {
            self.sleep_scene = self.capture_sleep();
        }
        match screen {
            Screen::Light => {
                self.light = read_light();
                self.wifi_on = crate::wifi::enabled();
            }
            Screen::Library => self.lib = Library::scan(&self.lib.query.clone()),
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
                self.clip_due = crate::revisao::due_count();
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
        if let (Screen::Games, Game(i)) = (self.screen, t) {
            data::set_last_game(i);
        }
        match (self.screen, t) {
            (Screen::Sudoku, Back) => {
                if let Some(g) = &self.sudoku {
                    g.save(); // guarda o tempo jogado
                }
                self.go(Screen::Games);
            }
            (Screen::SudokuMenu, Back) if self.sudoku_practice => {
                self.sudoku_practice = false;
                self.render(true);
            }
            (Screen::SudokuMenu | Screen::SudokuWon | Screen::ChessHub, Back) => self.go(Screen::Games),
            (Screen::Chess, Back) => {
                self.engine = None; // fecha o Stockfish; a partida fica salva para continuar
                self.go(Screen::ChessHub);
            }
            (Screen::Coaches | Screen::ChessEnd | Screen::ChessGames | Screen::ChessClock, Back) => self.go(Screen::ChessHub),
            (Screen::ChessReview, Back) => self.go(Screen::ChessGames),
            (Screen::ChessInsights, Back) => self.go(Screen::ChessReview),
            (Screen::Clips, Back) => self.go(Screen::Library),
            (Screen::ClipBook, Back) => self.go(Screen::Clips),
            (Screen::Compose, Back) => self.go(Screen::Today),
            (Screen::ClaudeChat | Screen::ClaudeCompose, Back) if self.claude_return.is_some() && (self.screen == Screen::ClaudeChat || self.cur_chat.is_none()) => {
                // Veio de um jogo ou livro: volta para lá.
                let back = self.claude_return.take().unwrap();
                self.claude_ctx = None;
                self.go(back);
            }
            (Screen::ClaudeLive | Screen::ClaudeChat, Back) => self.go(Screen::Claude),
            (Screen::ClaudeFiles, Back) => self.go(Screen::ClaudeChat),
            (Screen::ClaudeDoc, Back) => {
                let back = std::mem::replace(&mut self.doc_back, Screen::ClaudeFiles);
                self.go(back);
            }
            (Screen::ClaudeCompose, Secondary) => {
                self.attach_items = crate::chat::attach_items();
                self.attach_sel.retain(|i| *i < self.attach_items.len());
                self.attach_page = 0;
                self.go(Screen::ClaudeAttach);
            }
            (Screen::ClaudeAttach, Row(k)) => {
                let i = self.attach_page * crate::chat::ATTACH_ROWS + k;
                if let Some(p) = self.attach_sel.iter().position(|x| *x == i) {
                    self.attach_sel.remove(p);
                } else if self.attach_sel.len() < 6 {
                    self.attach_sel.push(i);
                }
                self.render(false);
            }
            (Screen::ClaudeAttach, Prev) => {
                self.attach_page = self.attach_page.saturating_sub(1);
                self.render(true);
            }
            (Screen::ClaudeAttach, Next) => {
                self.attach_page += 1;
                self.render(true);
            }
            (Screen::ClaudeAttach, Primary | Back) => self.go(Screen::ClaudeCompose),
            (Screen::ClaudeCompose, Ctx) => {
                self.claude_ctx = None;
                self.claude_about = None;
                self.claude_sugg = Self::suggestions_for(None, self.cur_chat.is_some());
                self.render(false);
            }
            (Screen::ClaudeCompose, Level(k)) => {
                // Pergunta pronta: envia na hora, sem digitar.
                if let Some(q) = self.claude_sugg.get(k).cloned() {
                    self.compose_text = q;
                    self.send_claude();
                }
            }
            (Screen::ClaudeChat, Check) => {
                self.files_note.clear();
                self.go(Screen::ClaudeFiles);
            }
            (Screen::ClaudeFiles, Row(i)) => self.open_claude_file(i),
            (Screen::ClaudeDoc, Prev) => {
                self.doc.2 = self.doc.2.saturating_sub(1);
                self.render(true);
            }
            (Screen::ClaudeDoc, Next) => {
                self.doc.2 += 1;
                self.render(true);
            }
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
            (Screen::ClaudeChat, Secondary) => {
                // Anexar já na conversa: escolhe os arquivos e volta para escrever a resposta.
                // Prepara a resposta sem desenhar a tela de escrever (evita piscar duas vezes).
                self.claude_ctx = None;
                self.claude_about = None;
                self.claude_sugg = Self::suggestions_for(None, true);
                self.compose_text.clear();
                self.compose_hint = "Escreva o que quer saber dos anexos e toque em OK.".into();
                self.kb_mode = crate::keyboard::Mode::Letters;
                self.kb_shift = true;
                self.attach_items = crate::chat::attach_items();
                self.attach_sel.clear();
                self.attach_page = 0;
                self.go(Screen::ClaudeAttach);
            }
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
            (Screen::Compose | Screen::WifiPass | Screen::ClaudeCompose | Screen::LibSearch | Screen::MiniText | Screen::TermType | Screen::MissaoText, Key(i)) => self.press_key(i as usize),
            (Screen::MiniText, Back) => self.go(Screen::Mini),
            (Screen::Wifi, Back) => {
                let back = self.wifi_back;
                self.go(back);
            }
            (Screen::Settings, Back) => self.go(Screen::Light),
            (Screen::Light, Back) => {
                let back = self.light_return.take().unwrap_or(Screen::Home);
                self.go(back);
            }
            (Screen::WifiPass, Back) => self.go(Screen::Wifi),
            (Screen::Mini, Back) => {
                // Primeiro volta dentro do módulo (ex.: passo → receita → lista).
                if self.mini.as_mut().is_some_and(|g| g.back()) {
                    return self.render(true);
                }
                if let Some(g) = self.mini.take() {
                    g.save();
                    // Guarda a partida: ao voltar ao jogo, continua daqui.
                    let key = self.mini_slot.take().unwrap_or(match self.mini_back {
                        Screen::Games => self.mini_game,
                        Screen::Home => 101,
                        _ => 100,
                    });
                    self.mini_cache.insert(key, g);
                }
                let back = self.mini_back;
                self.go(back);
            }
            (Screen::Home, Row(0)) | (Screen::Module(Module::Mais), Row(0)) => {
                self.ed_page = 0;
                self.panel_note.clear();
                self.go(Screen::Edicao);
                self.background_sync(true);
            }
            (Screen::Module(Module::Mais), Row(1)) => {
                self.mstage = Default::default();
                self.panel_note.clear();
                self.missions = crate::painel::missions().map_err(|e| self.panel_note = e).ok();
                self.go(Screen::Missoes);
            }
            (Screen::Missoes, Back) => {
                if self.mstage.template.is_some() {
                    self.mstage = Default::default();
                    self.render(false);
                } else {
                    self.go(Screen::Module(Module::Mais));
                }
            }
            (Screen::MissaoText, Back) => {
                self.mstage = Default::default();
                self.go(Screen::Missoes);
            }
            (Screen::Missoes, Row(k)) => {
                if let Some((_, act)) = self.m_hits.get(k).cloned() {
                    self.mission_action(act);
                }
            }
            (Screen::Module(Module::Mais), Row(2)) => {
                self.art_page = 0;
                self.panel_note.clear();
                self.go(Screen::Artigos);
                self.background_sync(true);
            }
            (Screen::Module(Module::Mais), Row(3)) => {
                self.photo_k = 0;
                self.photo_at = Instant::now();
                self.go(Screen::Retrato);
                self.background_sync(true);
            }
            (Screen::Module(Module::Mais), Row(5)) => self.go(Screen::Terminal),
            (Screen::Module(Module::Mais), Row(6)) => {
                self.mini = Some(self.mini_cache.remove(&102).unwrap_or_else(|| Box::new(crate::semana::Semana::new())));
                self.mini_slot = Some(102);
                self.mini_back = Screen::Module(Module::Mais);
                self.go(Screen::Mini);
            }
            (Screen::Edicao | Screen::Artigos | Screen::Retrato | Screen::Terminal, Back) => self.go(Screen::Module(Module::Mais)),
            (Screen::Edicao, Row(k)) => {
                if let Some((_, act)) = self.ed_hits.get(k).cloned() {
                    self.edition_action(act);
                }
            }
            (Screen::Artigos, Prev) => {
                self.art_page = self.art_page.saturating_sub(1);
                self.render(false);
            }
            (Screen::Artigos, Next) => {
                self.art_page += 1;
                self.render(false);
            }
            (Screen::Artigos, Row(k)) => {
                if let Some((path, title, _)) = crate::painel::articles().into_iter().nth(k) {
                    self.open_reader(path, title);
                }
            }
            (Screen::Retrato, Next) => {
                self.photo_k += 1;
                self.photo_at = Instant::now();
                self.render(true);
            }
            (Screen::Terminal, Row(k)) => {
                if let Some((label, cmd)) = crate::painel::COMMANDS.get(k) {
                    self.term = (label.to_lowercase(), crate::painel::run(cmd));
                    self.render(false);
                }
            }
            (Screen::Terminal, Continue) => {
                self.compose_text.clear();
                self.compose_hint = "Digite o comando e toque em OK. Roda como root: cuidado.".into();
                self.go(Screen::TermType);
            }
            (Screen::TermType, Back) => self.go(Screen::Terminal),
            (Screen::Aprovar, Level(k)) => self.decide_approval(k == 1),
            (Screen::Module(Module::Mais), Row(4)) => {
                self.mini = Some(self.mini_cache.remove(&100).unwrap_or_else(|| Box::new(crate::receitas::Receitas::new())));
                self.mini_slot = None;
                self.mini_back = Screen::Module(Module::Mais);
                self.go(Screen::Mini);
            }
            (Screen::LibSearch, Back) => self.go(Screen::Library),
            (Screen::Reader, Header) => {
                if let Some(r) = self.reader.as_mut() {
                    r.menu = true;
                }
                self.render(false);
            }
            (Screen::Reader, Primary) => {
                if let Some(r) = self.reader.as_mut() {
                    r.menu = false;
                }
                self.render(false);
            }
            (Screen::Reader, Prev) => self.turn_page(false),
            (Screen::Reader, Next) => self.turn_page(true),
            (Screen::Reader, Level(k)) => {
                let rf = self.rfonts.as_ref();
                match (k, self.reader.as_mut(), rf) {
                    (0, Some(r), _) => {
                        r.save();
                        self.lib.query.clear();
                        return self.go(Screen::Library);
                    }
                    (1, Some(r), Some(rf)) => r.set_size(r.size_k.saturating_sub(1), rf),
                    (2, Some(r), Some(rf)) => r.set_size(r.size_k + 1, rf),
                    (3, Some(r), _) => {
                        self.toc_page = r.chapter / crate::reader::TOC_ROWS;
                        return self.go(Screen::ReaderToc);
                    }
                    (4, Some(r), _) => {
                        r.menu = false;
                        return self.ask_claude_here();
                    }
                    (5, Some(r), Some(rf)) => {
                        let c = r.chapter.saturating_sub(1);
                        r.go_chapter(c, rf);
                    }
                    (6, Some(r), Some(rf)) => {
                        let c = r.chapter + 1;
                        r.go_chapter(c, rf);
                    }
                    _ => {}
                }
                self.render(true);
            }
            (Screen::ReaderToc, Back) => self.go(Screen::Reader),
            (Screen::ReaderToc, Row(i)) => {
                let k = self.toc_page * crate::reader::TOC_ROWS + i;
                if let (Some(r), Some(rf)) = (self.reader.as_mut(), self.rfonts.as_ref()) {
                    r.go_chapter(k, rf);
                    r.menu = false;
                }
                self.go(Screen::Reader);
            }
            (Screen::ReaderToc, Prev) => {
                self.toc_page = self.toc_page.saturating_sub(1);
                self.render(false);
            }
            (Screen::ReaderToc, Next) => {
                let n = self.reader.as_ref().map_or(1, |r| r.book.chapters().div_ceil(crate::reader::TOC_ROWS));
                self.toc_page = (self.toc_page + 1).min(n.saturating_sub(1));
                self.render(false);
            }
            (Screen::Tutorial, Back) => {
                let back = self.tut.2;
                self.go(back);
            }
            (Screen::Tutorial, Prev) if self.tut.1 == 0 => self.finish_tutorial(),
            (Screen::Tutorial, Prev) => {
                self.tut.1 -= 1;
                self.render(false);
            }
            (Screen::Tutorial, Next) if self.tut.1 + 1 >= crate::tutorial::pages(self.tut.0).len() => self.finish_tutorial(),
            (Screen::Tutorial, Next) => {
                self.tut.1 += 1;
                self.render(false);
            }
            (_, Help) => {
                if let Some(g) = self.help_game() {
                    self.open_tutorial(g, false);
                }
            }
            (Screen::Calendar, Back) => self.go(Screen::Today),
            (_, Back) => self.go(Screen::Home),
            (Screen::Home, Header) => self.go(Screen::Light),
            // "Continuar" do jogo na tela inicial: abre a partida direto.
            (Screen::Home, Game(i)) => {
                self.go(Screen::Games);
                self.activate(Game(i), x);
            }
            (Screen::Home, Phrase) => self.phrase_action(),
            (Screen::Achados, Back) => {
                self.achado_new = None;
                self.go(Screen::Home);
            }
            (Screen::Home, Continue) => {
                let Some(b) = data::last_book() else { return };
                if !b.path.exists() {
                    self.go(Screen::Library);
                    return;
                }
                if crate::reader::supported(&b.path) {
                    self.open_reader(b.path, b.title);
                } else {
                    self.open_book(b.path, b.title);
                }
            }
            (Screen::Home, Tile(i)) => match Module::ALL[i] {
                Module::Ler => {
                    self.lib.query.clear();
                    self.go(Screen::Library)
                }
                Module::Foco => self.go(Screen::Today),
                Module::Jogar => self.go(Screen::Games),
                Module::Claude => self.go(Screen::Claude),
                Module::Violao => {
                    self.mini = Some(self.mini_cache.remove(&101).unwrap_or_else(|| Box::new(crate::estudo::Estudo::new())));
                    self.mini_slot = None;
                    self.mini_back = Screen::Home;
                    self.go(Screen::Mini);
                }
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
            (Screen::Light, Plane) => {
                // Modo avião = rádio desligado (o Kindle só tem Wi-Fi).
                set_wifi(!self.wifi_on);
                std::thread::sleep(Duration::from_millis(800));
                self.wifi_on = crate::wifi::enabled();
                log(&format!("modo avião {}", !self.wifi_on));
                self.render(false);
            }
            (Screen::Light, Ctx) => {
                // Volta à tela de onde veio (sem redesenhar) e pergunta ao Claude sobre ela.
                let back = self.light_return.take().unwrap_or(Screen::Home);
                if back == Screen::Home {
                    return self.go(Screen::Claude);
                }
                self.screen = back;
                self.ask_claude_here();
            }
            (Screen::Light, Wifi) => {
                self.wifi_back = Screen::Light;
                self.wifi_note = "Toque numa rede para conectar.".into();
                self.go(Screen::Wifi);
            }
            (Screen::Library, Row(i)) => {
                let Some(b) = self.lib.visible().get(i) else { return };
                let (path, title) = (b.path.clone(), b.title.clone());
                if crate::reader::supported(&path) {
                    self.open_reader(path, title);
                } else {
                    self.open_book(path, title);
                }
            }
            (Screen::Library, Clips) => self.go(Screen::Clips),
            (Screen::Clips, Clips) => {
                self.mini = Some(Box::new(crate::revisao::Revisao::new()));
                self.mini_slot = None;
                self.mini_back = Screen::Clips;
                self.go(Screen::Mini);
            }
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
            // Primeira vez num jogo: o tutorial antes.
            (Screen::Games, Game(i)) if !crate::tutorial::seen(i) => self.open_tutorial(i, true),
            (Screen::Games, Game(0)) => {
                self.sudoku_practice = false;
                self.sudoku_prog = crate::fases::Progress::load("sudoku");
                self.go(Screen::SudokuMenu);
            }
            (Screen::Games, Game(1)) => self.go(Screen::ChessHub),
            (Screen::Games, Game(i)) if self.mini_cache.contains_key(&i) => {
                let mut g = self.mini_cache.remove(&i).unwrap();
                g.resume();
                self.mini = Some(g);
                self.mini_slot = None;
                self.mini_game = i;
                self.mini_back = Screen::Games;
                self.go(Screen::Mini);
            }
            (Screen::Games, Game(i)) => {
                self.mini = crate::minigame::make(i);
                if self.mini.is_some() {
                    self.mini_game = i;
                    self.mini_back = Screen::Games;
                    self.go(Screen::Mini);
                }
            }
            (Screen::Mini, Mini(id)) => self.mini_tap(id, x),
            (Screen::SudokuMenu, Level(k)) if self.sudoku_practice => self.start_sudoku(crate::sudoku::LEVELS[k.min(2)]),
            (Screen::SudokuMenu, Level(0)) => self.start_sudoku_fase(),
            (Screen::SudokuMenu, Level(_)) => {
                self.sudoku_practice = true;
                self.render(true);
            }
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
                if self.sudoku.as_ref().is_some_and(|g| g.fase > 0) {
                    self.start_sudoku_fase();
                } else {
                    let lv = self.sudoku.as_ref().map(|g| g.level).unwrap_or_else(crate::sudoku::last_level);
                    self.start_sudoku(lv);
                }
            }
            // Central do xadrez: 0 continuar, 1 treinadores, 2 duas pessoas, 3 partidas anteriores.
            (Screen::ChessHub, Game(0)) => self.resume_chess(),
            (Screen::ChessHub, Game(1)) => {
                self.coach_page = crate::coach::unlocked() / crate::chess_ui::PER_PAGE;
                self.go(Screen::Coaches);
            }
            (Screen::ChessHub, Game(4)) => self.start_coach_game(crate::coach::CLAUDE),
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
            (Screen::ChessHub, Sort) => self.go(Screen::ChessClock),
            (Screen::ChessClock, Level(k)) => {
                crate::chess::save_control(k.checked_sub(1));
                self.go(Screen::ChessHub);
            }
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
                // Jogar cancela confirmações e propostas pendentes.
                let pending = self.chess_confirm.take().is_some() | std::mem::take(&mut self.chess_offer);
                if pending {
                    self.render(false);
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
            // Botão da esquerda: empate (propor, confirmar ou aceitar) ou, no fim, voltar ao menu.
            (Screen::Chess, Undo) => {
                let ended = self.chess.as_ref().is_some_and(|g| g.result().is_some());
                if self.chess_over.is_some() || ended {
                    self.chess_over = None;
                    if ended && self.coach_level().is_none() {
                        crate::chess::Game::clear_saved();
                    }
                    return self.go(Screen::ChessHub);
                }
                if self.chess_offer {
                    // O outro jogador aceitou o empate.
                    self.chess_offer = false;
                    self.end_local("Empate combinado.");
                    return;
                }
                if self.confirming(0) {
                    self.chess_confirm = None;
                    return self.offer_draw();
                }
                self.chess_confirm = Some((0, Instant::now()));
                self.render(false);
            }
            (Screen::Chess, Check) => {
                crate::chess::set_rotate(!crate::chess::rotate_enabled());
                self.render(true);
            }
            // Botão da direita: desistir (com confirmação) ou recusar o empate proposto.
            (Screen::Chess, NewGame) => {
                if self.chess_offer {
                    self.chess_offer = false;
                    self.chess_note("Empate recusado. O jogo segue.");
                    return;
                }
                if !self.confirming(1) {
                    self.chess_confirm = Some((1, Instant::now()));
                    return self.render(false);
                }
                self.chess_confirm = None;
                let coach = self.chess.as_ref().is_some_and(|g| matches!(g.mode, crate::chess::Mode::Coach { .. }));
                if coach {
                    self.finish_coach_game(Some(0.0)); // desistir conta como derrota
                } else {
                    let side = if self.chess.as_ref().is_some_and(|g| g.pos.white_to_move) { "As brancas desistiram." } else { "As pretas desistiram." };
                    self.end_local(side);
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
            (Screen::ChessEnd, Game(2)) if self.coach_level() == Some(crate::coach::CLAUDE) => self.claude_review(),
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
            (Screen::Library, Sort) => {
                self.lib.toggle_sort();
                self.render(true);
                self.fill_covers();
            }
            (Screen::Library, Search) => {
                if self.lib.query.is_empty() {
                    self.compose_text.clear();
                    self.compose_hint = "Digite parte do título.".into();
                    self.kb_shift = false;
                    self.go(Screen::LibSearch);
                } else {
                    self.lib.query.clear();
                    self.lib.apply();
                    self.render(true);
                    self.fill_covers();
                }
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
                self.wifi_back = Screen::Settings;
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
            (Screen::Today, Secondary) => self.go(Screen::Calendar),
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
                self.gesture = None;
                if let Some(h) = self.hit(x, y) {
                    if let (Screen::Mini, Target::Mini(id)) = (self.screen, h) {
                        if id < 800 {
                            self.gesture = Some(Gesture { start: id, last: id, at: Instant::now(), dragging: false, held: false });
                        }
                    }
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
            Touch::Move(x, y) if self.gesture.is_some() => self.gesture_move(x, y),
            Touch::Move(x, y) => {
                // Arrastar para fora cancela o toque, como num botão nativo.
                if let Some(h) = self.pressed {
                    if self.hit(x, y) != Some(h) {
                        self.pressed = None;
                        self.flip(h, false);
                    }
                }
            }
            Touch::Up(_, _) if self.gesture.as_ref().is_some_and(|g| g.dragging || g.held) => {
                let g = self.gesture.take().unwrap();
                self.pressed = None;
                self.swipe_from = None;
                if g.dragging {
                    if let Some(m) = self.mini.as_mut() {
                        m.drag_end();
                    }
                    // Fim do arrasto: uma passada de qualidade limpa os rastros do modo rápido.
                    self.apply_mini_redraw(crate::minigame::Redraw::Part(Rect::new(0, 170, 1072, 1448 - 170)));
                }
                if let Some(m) = &self.mini {
                    m.save();
                }
            }
            Touch::Up(x, y) => {
                self.gesture = None;
                let from = self.swipe_from.take();
                // Deslizar do topo para baixo, em qualquer tela: o painel Luz e tela (brilho, Wi-Fi,
                // ajustes), com o botão que pergunta ao Claude sobre a tela de onde você veio.
                let swipe_down = from.is_some_and(|(_, fy)| fy < 180 && y - fy > 250);
                if swipe_down && self.screen != Screen::Home && self.light_swipe_ok() {
                    if let Some(h) = self.pressed.take() {
                        self.flip(h, false);
                    }
                    self.light_return = Some(self.screen);
                    return self.go(Screen::Light);
                }
                // Deslizar de baixo para cima: Luz e tela (luz, modo escuro, Wi-Fi, ajustes) de qualquer lugar.
                let swipe_up = from.is_some_and(|(_, fy)| fy > 1290 && fy - y > 250);
                if swipe_up && self.light_swipe_ok() {
                    if let Some(h) = self.pressed.take() {
                        self.flip(h, false);
                    }
                    self.light_return = Some(self.screen);
                    return self.go(Screen::Light);
                }
                if self.screen == Screen::Home && swipe_down {
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

    /// Dedo arrastado sobre casas de minijogo: cada casa nova recebe `drag`.
    fn gesture_move(&mut self, x: i32, y: i32) {
        self.check_hold();
        let Some(Target::Mini(id)) = self.hit(x, y) else { return };
        let Some(g) = self.gesture.as_mut() else { return };
        if id >= 800 || id == g.last || g.held {
            return;
        }
        let first = (!g.dragging).then_some(g.start);
        g.last = id;
        let Some(m) = self.mini.as_mut() else { return };
        let mut rects = Vec::new();
        for c in first.into_iter().chain(std::iter::once(id)) {
            match m.drag(c) {
                Some(r) => rects.push(r),
                None => {
                    // Jogo sem arrasto: comportamento de botão (sair cancela).
                    self.gesture = None;
                    self.pressed = None;
                    return;
                }
            }
        }
        if let Some(g) = self.gesture.as_mut() {
            g.dragging = true;
        }
        self.pressed = None;
        m.draw(&mut self.fb, &self.fonts);
        for r in rects {
            if let crate::minigame::Redraw::Part(r) = r {
                self.fb.refresh(r, Wave::Fast, false);
            }
        }
    }

    pub fn touch_active(&self) -> bool {
        self.gesture.as_ref().is_some_and(|g| !g.held && !g.dragging)
    }

    /// Segurou o dedo parado numa casa: `hold` do jogo.
    fn check_hold(&mut self) {
        let hold = self.mini.as_ref().map(|m| Duration::from_millis(m.hold_ms())).unwrap_or(HOLD);
        let Some(g) = self.gesture.as_mut() else { return };
        if g.dragging || g.held || g.at.elapsed() < hold {
            return;
        }
        let id = g.start;
        let Some(m) = self.mini.as_mut() else { return };
        if let Some(r) = m.hold(id) {
            g.held = true;
            self.pressed = None;
            self.apply_mini_redraw(r);
        } else {
            self.gesture = None;
        }
    }

    /// Chamado várias vezes por segundo: relógio, Pomodoro, confirmação e bateria.
    pub fn tick(&mut self) {
        if self.gesture.is_some() {
            self.check_hold();
        }
        // VPS: avisos e aprovações a cada minuto; edição e caixa de entrada a cada 20 minutos.
        self.apply_sync();
        if self.screen != Screen::Sleep && self.sync_fast.elapsed() >= Duration::from_secs(60) {
            let full = self.sync_full.is_none_or(|t| t.elapsed() >= Duration::from_secs(20 * 60));
            self.background_sync(full);
        }
        // Minijogos com relógio (o treino de violão): redesenham quando pedem.
        if self.screen == Screen::Mini {
            if let Some(r) = self.mini.as_mut().map(|g| g.tick()) {
                if !matches!(r, crate::minigame::Redraw::None) {
                    self.apply_mini_redraw(r);
                }
            }
        }
        // Porta-retrato: troca a foto a cada 15 minutos.
        if self.screen == Screen::Retrato && self.photo_at.elapsed() >= Duration::from_secs(15 * 60) {
            self.photo_k += 1;
            self.photo_at = Instant::now();
            self.render(true);
        }
        self.chess_clock_tick();
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
        if now.plausible() {
            if let Some(name) = crate::backup::maybe_backup(self.status.offset_min) {
                log(&format!("cópia de segurança: {name}"));
            }
            if self.status.wifi {
                crate::backup::maybe_check_update(data::today(self.status.offset_min));
            }
        }
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

#[cfg(test)]
mod raio_x {
    use super::*;

    /// Percorre as telas e salva cada uma (raio-x visual):
    /// Cada lição do curso de partitura, a pergunta dela respondida e os treinos:
    /// `VERSO_INK_SHOTS=pasta cargo test curso_de_partitura -- --ignored --test-threads=1`
    #[test]
    #[ignore]
    fn curso_de_partitura() {
        let home = std::env::temp_dir().join(format!("verso-ink-curso-{}", std::process::id()));
        std::fs::create_dir_all(home.join("settings")).unwrap();
        std::env::set_var("VERSO_INK_HOME", &home);
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        let _ = std::os::unix::fs::symlink(repo.join("sprites"), home.join("sprites"));
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        std::fs::create_dir_all(&out).unwrap();
        let mut app = App::new(Framebuffer::memory());
        app.mini_back = Screen::Home;
        for g in 0..crate::partitura::total() {
            let ok = crate::partitura::lesson(g).check.ok;
            for (check, tag) in [(None, "licao"), (Some((ok + 1) % crate::partitura::lesson(g).check.opts.len()), "pergunta")] {
                let mut v = crate::violao::Violao::new();
                v.open_lesson(g, check);
                app.mini = Some(Box::new(v));
                app.go(Screen::Mini);
                app.render(false);
                app.fb.save_pgm(&out.join(format!("{g:02}-{tag}.pgm")));
            }
        }
        for mode in 0..4 {
            for level in 0..if mode == 0 { crate::partitura::LEVELS.len() } else { 1 } {
                let mut v = crate::violao::Violao::new();
                v.open_quiz(mode, level);
                app.mini = Some(Box::new(v));
                app.go(Screen::Mini);
                app.render(false);
                app.fb.save_pgm(&out.join(format!("treino-{mode}-{level}.pgm")));
            }
        }
        let _ = std::fs::remove_dir_all(&home);
    }

    /// `VERSO_INK_SHOTS=pasta cargo test todas_as_telas -- --ignored --test-threads=1`
    #[test]
    #[ignore]
    fn todas_as_telas() {
        let home = std::env::temp_dir().join(format!("verso-ink-raiox-{}", std::process::id()));
        std::fs::create_dir_all(home.join("data/tutoriais")).unwrap();
        std::fs::create_dir_all(home.join("settings")).unwrap();
        std::env::set_var("VERSO_INK_HOME", &home);
        // Arte do repositório (sprites e rostos), como no Kindle.
        let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for d in ["sprites", "treinadores"] {
            let _ = std::os::unix::fs::symlink(repo.join(d), home.join(d));
        }
        // Tutoriais já vistos, para os jogos abrirem direto no menu.
        for g in 0..apps::GAMES.len() {
            std::fs::write(home.join(format!("data/tutoriais/{g}.visto")), "1").unwrap();
        }
        // Um livro e um jogo para o "Continuar" da tela inicial.
        std::fs::write(home.join("livro.pdf"), "").unwrap();
        std::fs::write(home.join("data/last_book"), format!("{}\nPerfil de três reis - Gene Edwards PDF\n", home.join("livro.pdf").display())).unwrap();
        std::fs::write(home.join("data/ultimo_jogo"), "1").unwrap();
        // Relógio 5+3 nas partidas, para a tela da partida mostrar os dois mostradores.
        std::fs::write(home.join("data/xadrez_ritmo.txt"), "5").unwrap();
        std::fs::write(home.join("data/dia.txt"), "! Revisar o capítulo 3\n- Comprar pão\n@ 08:00 Remédio\n* Ler 20 min\n* {seg,qua,sex} Academia\n").unwrap();
        let out = std::path::PathBuf::from(std::env::var("VERSO_INK_SHOTS").expect("VERSO_INK_SHOTS"));
        let mut app = App::new(Framebuffer::memory());
        let mut n = 0;
        let mut shot = |app: &mut App, name: &str| {
            app.render(false);
            n += 1;
            app.fb.save_pgm(&out.join(format!("{n:02}-{name}.pgm")));
        };
        shot(&mut app, "inicio");
        app.go(Screen::Sleep);
        shot(&mut app, "repouso");
        app.go(Screen::Home);
        for (s, name) in [
            (Screen::Light, "luz"),
            (Screen::Settings, "ajustes"),
            (Screen::Library, "ler"),
            (Screen::Today, "hoje"),
            (Screen::Calendar, "calendario"),
            (Screen::Focus, "foco"),
            (Screen::Module(Module::Mais), "mais"),
            (Screen::Games, "jogar"),
            (Screen::Claude, "claude"),
            (Screen::ChessClock, "xadrez-relogio"),
        ] {
            app.go(s);
            shot(&mut app, name);
        }
        app.cur_chat = None;
        app.start_claude_compose();
        shot(&mut app, "claude-pergunta");
        // Cada jogo: menu e, quando dá, a partida começada.
        for i in 0..apps::GAMES.len() {
            app.go(Screen::Games);
            app.activate(Target::Game(i), 0);
            let name = crate::apps::fold(apps::GAMES[i].0).replace(' ', "-");
            shot(&mut app, &format!("jogo-{name}-menu"));
            if app.screen == Screen::Mini {
                // Primeira opção do menu (fase 1 / contra a CPU / primeira história).
                if let Some((id, _)) = app.mini.as_ref().and_then(|g| g.targets().into_iter().find(|(id, _)| *id == 900)) {
                    app.mini_tap(id, 0);
                    shot(&mut app, &format!("jogo-{name}-partida"));
                    if name == "coroas" {
                        for id in [0, 1, 7] {
                            app.mini_tap(id, 0);
                        }
                        shot(&mut app, "jogo-coroas-marcas");
                        // Vidas: uma das duas primeiras casas da linha 1 é errada; segurar nela custa vida.
                        let _ = app.mini.as_mut().and_then(|g| g.hold(0));
                        let _ = app.mini.as_mut().and_then(|g| g.hold(1));
                        shot(&mut app, "jogo-coroas-vida");
                        for _ in 0..6 {
                            let _ = app.mini.as_mut().and_then(|g| g.hold(0));
                            let _ = app.mini.as_mut().and_then(|g| g.hold(1));
                        }
                        shot(&mut app, "jogo-coroas-sem-vidas");
                        app.mini = Some(Box::new(crate::coroas::Coroas::new()));
                        app.go(Screen::Mini);
                        shot(&mut app, "jogo-coroas-sem-vidas-menu");
                        std::fs::write(home.join("data/coroas/vidas.txt"), "5 0").unwrap();
                    }
                    if name == "masmorra" {
                        app.go(Screen::Sleep);
                        shot(&mut app, "repouso-jogo");
                        app.go(Screen::Mini);
                    }
                }
                app.activate(Target::Back, 0);
            } else if app.screen == Screen::SudokuMenu {
                app.activate(Target::Level(0), 0);
                shot(&mut app, "jogo-sudoku-partida");
            } else if app.screen == Screen::ChessHub {
                app.activate(Target::Game(2), 0);
                if let Some(g) = app.chess.as_mut() {
                    for m in ["e2e4", "d7d5", "e4d5", "d8d5", "b1c3", "d5a5"] {
                        let _ = g.play_uci(m);
                    }
                }
                shot(&mut app, "jogo-xadrez-partida");
                app.go(Screen::Coaches);
                shot(&mut app, "jogo-xadrez-treinadores");
                // Partida contra treinador (sem motor no PC: só a tela).
                let mut g = crate::chess::Game::new_coach(0, true);
                let _ = g.play_uci("e2e4");
                let _ = g.play_uci("e7e5");
                app.chess = Some(g);
                app.coach_msg = "Boa abertura! Peões no centro abrem caminho para os bispos. Sua vez.".into();
                app.go(Screen::Chess);
                shot(&mut app, "jogo-xadrez-treinador-partida");
            }
        }
        // Painel: edição de exemplo, artigos e fotos vazios, terminal e um pedido de permissão.
        std::fs::write(
            home.join("data/edicao.json"),
            r#"{"dia": "2026-11-12", "manchete": "Os agentes terminaram o raio-x; um deles ainda precisa de você.", "secoes": [{"titulo": "Avisos", "tipo": "aviso", "itens": [{"texto": "NÃO DESCONECTAR O KINDLE até o fim da atualização", "id": "a1b2c3d4e5f6", "critico": true}]}, {"titulo": "Os agentes durante a noite", "tipo": "agente", "itens": [{"texto": "verso-ink: refez a Início e a Jogar e parou às 03:10 esperando a sua escolha de direção.", "projeto": "verso-ink"}, {"texto": "aions-brain: atualizou duas notas de retomada e terminou sem pendências.", "projeto": "aions-brain"}]}, {"titulo": "Código de ontem", "tipo": "codigo", "itens": [{"texto": "verso-ink: Aprender no jeito dos apps de idioma · risco médio. XP, meta e ofensiva com protetor. Pontos: 1.", "id": "rev:verso-ink:0bfa0c0", "artigo": "abc"}]}, {"titulo": "Agenda de hoje", "tipo": "agenda", "itens": [{"texto": "09:00 · Daily da equipe (Meet)"}, {"texto": "19:30 · Aula de violão"}]}, {"titulo": "E-mails que importam", "tipo": "email", "itens": [{"texto": "Ana (RH): Contrato de estágio. Pede a assinatura até sexta.", "id": "mail:abc123"}, {"texto": "Banco: Fatura de novembro. Vence dia 20.", "id": "mail:def456"}]}, {"titulo": "Brain e tarefas", "tipo": "tarefa", "itens": [{"texto": "3 retomadas em andamento; a do Verso Ink está em dia."}]}, {"titulo": "Na sua área", "tipo": "noticia", "itens": [{"texto": "Rust 1.92 estabiliza novos recursos de async (blog do Rust).", "url": "https://blog.rust-lang.org/"}, {"texto": "Agentes de código ganham espaço em equipes pequenas, diz pesquisa (InfoQ).", "url": "https://www.infoq.com/"}]}]}"#,
        )
        .unwrap();
        app.go(Screen::Edicao);
        app.ed_page = 1;
        shot(&mut app, "painel-edicao-p2");
        crate::painel::toggle_open("mail:abc123");
        shot(&mut app, "painel-edicao-email-aberto");
        crate::painel::toggle_open("rev:verso-ink:0bfa0c0");
        shot(&mut app, "painel-edicao-codigo-aberto");
        crate::painel::toggle_open("rev:verso-ink:0bfa0c0");
        app.ed_page = 0;
        for (s, name) in [(Screen::Edicao, "painel-edicao"), (Screen::Artigos, "painel-artigos"), (Screen::Retrato, "painel-retrato"), (Screen::Terminal, "painel-terminal")] {
            app.go(s);
            shot(&mut app, name);
        }
        // Violão: início, treino (com o cronômetro e a contagem), músicas, cifra e acordes.
        app.go(Screen::Home);
        app.mini = Some(Box::new(crate::violao::Violao::new()));
        app.mini_back = Screen::Home;
        app.go(Screen::Mini);
        shot(&mut app, "violao-inicio");
        for (id, name) in [(700, "violao-treino"), (780, "violao-treino-cronometro"), (780, "violao-treino-contagem")] {
            app.mini_tap(id, 0);
            shot(&mut app, name);
        }
        app.mini = Some(Box::new(crate::violao::Violao::new()));
        app.mini_tap(701, 0);
        shot(&mut app, "violao-musicas");
        app.mini_tap(100, 0);
        shot(&mut app, "violao-cifra");
        app.mini = Some(Box::new(crate::violao::Violao::new()));
        app.mini_tap(702, 0);
        shot(&mut app, "violao-acordes");
        app.mini = Some(Box::new(crate::violao::Violao::new()));
        app.mini_tap(703, 0);
        shot(&mut app, "violao-curso");
        app.mini_tap(811, 0);
        shot(&mut app, "violao-modulo");
        app.mini_tap(907, 0);
        shot(&mut app, "violao-licao");
        app.mini_tap(721, 0);
        shot(&mut app, "violao-pergunta");
        app.mini_tap(870, 0);
        shot(&mut app, "violao-pergunta-resposta");
        app.mini_tap(721, 0);
        shot(&mut app, "violao-licao-seguinte");
        app.go(Screen::Home);
        app.mini = Some(Box::new(crate::violao::Violao::new()));
        app.go(Screen::Mini);
        app.mini_tap(703, 0);
        app.mini_tap(790, 0);
        shot(&mut app, "violao-leitura");
        app.mini_tap(863, 0);
        shot(&mut app, "violao-leitura-resposta");
        for (id, name) in [(841, "violao-treino-ritmo"), (842, "violao-treino-armaduras"), (843, "violao-treino-intervalos")] {
            app.mini_tap(id, 0);
            shot(&mut app, name);
        }
        // Aprender: ofensiva de 4 dias (hoje ainda não), um protetor, e as comemorações.
        {
            let t = crate::aprender::today();
            std::fs::write(home.join("data/aprender.txt"), format!("{}|12|licao\n{}|15|licao\n{}|30|treino\n{}|10|licao\n", t - 4, t - 3, t - 2, t - 1)).unwrap();
            std::fs::write(home.join("data/aprender_protetor.txt"), "1").unwrap();
            app.go(Screen::Home);
            shot(&mut app, "inicio-ofensiva");
            app.mini = Some(Box::new(crate::violao::Violao::new()));
            app.mini_back = Screen::Home;
            app.go(Screen::Mini);
            app.mini_tap(703, 0);
            shot(&mut app, "aprender-curso");
            for (c, name) in [(crate::aprender::Cel::Streak(5, false), "aprender-ofensiva"), (crate::aprender::Cel::Streak(7, true), "aprender-ofensiva-protetor"), (crate::aprender::Cel::Goal(30), "aprender-meta")] {
                crate::aprender::draw_celebration(&mut app.fb, &app.fonts, c);
                app.fb.save_pgm(&out.join(format!("zz-{name}.pgm")));
            }
        }
        // Aprender: hub com os tópicos, um tópico com currículo e uma lição de exemplo.
        {
            let d = home.join("data/aprender/topicos/ingles");
            std::fs::create_dir_all(d.join("licoes")).unwrap();
            std::fs::write(d.join("tema.txt"), "Inglês para desenvolvedores").unwrap();
            std::fs::write(d.join("curriculo.json"), r#"{"titulo": "Inglês para Devs", "unidades": [{"titulo": "Primeiros passos", "licoes": ["Cumprimentos no trabalho", "Apresentando-se", "Números e horários", "Perguntas simples", "Revisão da unidade"]}, {"titulo": "O vocabulário do código", "licoes": ["Bug, fix e issue", "Commits"]}]}"#).unwrap();
            std::fs::write(d.join("feitas.txt"), "0-0").unwrap();
            std::fs::write(d.join("licoes/0-1.json"), r#"{"cartoes": [{"titulo": "I am, I work", "texto": "Para se apresentar numa daily, use frases curtas: I am Pedro. I work on the frontend. I am a full-stack developer. Em inglês o sujeito nunca some: diga sempre I am, nunca só am."}], "exercicios": [{"pergunta": "Como dizer 'Eu trabalho no backend'?", "opcoes": ["I work on the backend", "Work on backend", "I working backend"], "certa": 0, "explicacao": "Sujeito + verbo: I work. Em inglês o sujeito não some."}, {"pergunta": "Complete: ___ a developer.", "opcoes": ["I am", "I is", "Am"], "certa": 0, "explicacao": "Com I, o verbo to be é am."}]}"#).unwrap();
            std::fs::write(home.join("data/aprender.txt"), format!("{}|12|t:ingles\n{}|15|licao\n", crate::aprender::today() - 1, crate::aprender::today() - 1)).unwrap();
            app.go(Screen::Home);
            shot(&mut app, "aprender-inicio");
            app.mini = Some(Box::new(crate::estudo::Estudo::new()));
            app.mini_back = Screen::Home;
            app.go(Screen::Mini);
            shot(&mut app, "aprender-hub");
            let mut e = crate::estudo::Estudo::new();
            e.open_first_topic();
            app.mini = Some(Box::new(e));
            app.go(Screen::Mini);
            shot(&mut app, "aprender-topico");
            let mut e = crate::estudo::Estudo::new();
            e.open_first_topic();
            e.start_lesson(0, 1);
            app.mini = Some(Box::new(e));
            app.go(Screen::Mini);
            shot(&mut app, "aprender-licao-cartao");
            for (id, name) in [(730, "aprender-exercicio"), (741, "aprender-exercicio-errou"), (730, "aprender-exercicio-2"), (740, "aprender-exercicio-certo"), (730, "aprender-repete-erro"), (740, "aprender-repete-certo"), (730, "aprender-fim")] {
                app.mini_tap(id, 0);
                shot(&mut app, name);
            }
        }
        // Puxar do topo num jogo: Luz e tela com o botão do Claude sobre o jogo.
        app.light_return = Some(Screen::Mini);
        app.go(Screen::Light);
        shot(&mut app, "luz-de-um-jogo");
        app.light_return = None;
        // Anexar à pergunta ao Claude.
        {
            app.cur_chat = None;
            app.claude_about = None;
            app.start_claude_compose();
            shot(&mut app, "claude-pergunta-anexar");
            app.attach_items = ["Perfil de três reis", "Destaques: Perfil de três reis", "Cifra: Primeiros acordes", "Foto: praia"]
                .iter()
                .map(|l| crate::chat::AttachItem { label: l.to_string(), sub: "PDF · 812 KB".into(), path: None, text: Some(("x".into(), "y".into())) })
                .collect();
            app.attach_sel = vec![1];
            app.go(Screen::ClaudeAttach);
            shot(&mut app, "claude-anexar");
            app.go(Screen::ClaudeCompose);
            shot(&mut app, "claude-pergunta-com-anexo");
            app.attach_sel.clear();
            app.attach_items.clear();
        }
        // Conversa já feita com o Claude.
        {
            let c = crate::chat::Chat {
                id: "abcdef0123456789".into(),
                title: "Como treinar a troca de acordes".into(),
                messages: vec![
                    crate::chat::Msg { user: true, text: "Como eu melhoro a troca de Em para G? Ainda travo muito.".into(), context: Some("No módulo de violão do Kindle".into()) },
                    crate::chat::Msg { user: false, text: "Boa pergunta, Pedro. A troca **Em → G** fica fácil com três passos:\n\n1. Deixe o dedo 2 como **pivô**: ele quase não sai do lugar.\n2. Treine só a mão esquerda, sem tocar, 10 vezes devagar.\n3. Depois use o treino de um minuto e tente bater o recorde.\n\n## Dica extra\nOlhe para onde o dedo vai, não para onde ele está.".into(), context: None },
                    crate::chat::Msg { user: true, text: "E se eu ainda errar o tempo?".into(), context: None },
                    crate::chat::Msg { user: false, text: "Diminua o andamento. Conte 1, 2, 3, 4 em voz alta e troque no 4.".into(), context: None },
                ],
                files: vec![("plano-de-treino.md".into(), 2300)],
            };
            app.chat_pages = crate::chat::paginate(&app.fonts, &c);
            app.chat_page = 0;
            app.cur_chat = Some(c);
            app.go(Screen::ClaudeChat);
            shot(&mut app, "claude-conversa");
        }
        // Aula do dia do violão (dica escrita à mão; no Kindle vem do Claude).
        {
            std::fs::create_dir_all(home.join("data/violao")).unwrap();
            let t = crate::data::today(crate::offset_min());
            std::fs::write(home.join("data/violao/dica.txt"), format!("{t}\nSua troca Em ↔ G está em 18 por minuto: hoje, deixe o dedo 2 parado como pivô e faça 10 trocas lentas antes do cronômetro. Na leitura, o ritmo é o ponto fraco: 3 rodadas do treino de ritmo.")).unwrap();
            app.mini = Some(Box::new(crate::violao::Violao::new()));
            app.mini_back = Screen::Home;
            app.go(Screen::Mini);
            app.mini_tap(700, 0);
            shot(&mut app, "violao-aula");
        }
        // Achados: um guardado agora e a coleção.
        {
            let day = crate::data::today(crate::offset_min());
            crate::saudacao::collect(1, day - 3);
            crate::saudacao::collect(5, day - 1);
            app.achado_new = crate::saudacao::collect(0, day).then_some(0);
            app.go(Screen::Achados);
            shot(&mut app, "achados");
            app.achado_new = None;
            app.go(Screen::Today);
            shot(&mut app, "hoje-com-aprender");
        }
        // Fechamento da semana com dados de exemplo.
        {
            let w = crate::semana::Week {
                id: "2026-W46".into(),
                title: "Semana de aprender: partitura todo dia".into(),
                summary: vec!["Você abriu a partitura cinco dias seguidos e passou do módulo de ritmo. A leitura ficou para trás: só dois dias com o livro.".into(), "Os agentes fecharam o raio-x do Verso Ink e prepararam o rascunho do post; o Brain tem duas retomadas paradas.".into()],
                numbers: vec![("dias de ofensiva".into(), "5".into()), ("XP na semana".into(), "184".into()), ("min de leitura".into(), "46".into()), ("dias de violão".into(), "3".into()), ("missões".into(), "4".into()), ("pomodoros".into(), "7".into())],
                questions: vec![("p1".into(), "O que mais importa na próxima semana?".into(), vec!["Violão".into(), "Ler mais".into(), "Fechar o Brain".into(), "Trabalho".into()])],
                plan: vec!["Ler 15 minutos antes de dormir, de segunda a sexta.".into(), "Uma lição de partitura por dia, mesmo nos dias corridos.".into(), "Fechar as duas retomadas do Brain até quarta.".into()],
            };
            app.mini_back = Screen::Home;
            for (v, name) in [(0, "semana-inicio"), (1, "semana-resumo"), (2, "semana-pergunta"), (3, "semana-plano")] {
                app.mini = Some(Box::new(crate::semana::Semana::with(w.clone(), v)));
                app.go(Screen::Mini);
                shot(&mut app, name);
            }
        }
        // Revisão dos destaques com cartões de exemplo (o arquivo do Kindle não existe no PC).
        {
            let cards = vec![
                crate::revisao::Card { id: "c1".into(), book: "Perfil de três reis".into(), text: "Quem é quebrado por dentro aprende a não quebrar os outros; a autoridade verdadeira não precisa se defender.".into() },
                crate::revisao::Card { id: "c2".into(), book: "Hábitos".into(), text: "Pequenas melhorias diárias, somadas ao longo de anos, produzem resultados que parecem impossíveis no começo.".into() },
            ];
            let mut q = std::collections::HashMap::new();
            q.insert("c1".to_string(), ("Segundo o livro, o que mostra que uma autoridade é verdadeira?".to_string(), "Hábitos: a mesma paciência de quem melhora um pouco por dia, sem pressa de provar nada.".to_string()));
            app.go(Screen::Home);
            app.mini = Some(Box::new(crate::revisao::Revisao::with(cards, q)));
            app.mini_back = Screen::Home;
            app.go(Screen::Mini);
            shot(&mut app, "revisao-inicio");
            for (id, name) in [(1, "revisao-pergunta"), (2, "revisao-verso"), (4, "revisao-sem-pergunta"), (2, "revisao-sem-pergunta-verso")] {
                app.mini_tap(id, 0);
                shot(&mut app, name);
            }
        }
        app.approvals = vec![crate::painel::Approval { id: "abc".into(), project: "verso-ink".into(), tool: "Bash".into(), summary: "cargo build --release && tools/deploy.sh (envia o binário novo ao Kindle pelo túnel)".into(), created: 0 }];
        app.go(Screen::Aprovar);
        shot(&mut app, "painel-aprovar");
        let _ = std::fs::remove_dir_all(&home);
    }
}
