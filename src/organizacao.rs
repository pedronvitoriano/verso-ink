//! Hábitos e tarefas do plano do dia, editados sem apagar comentários ou outros itens.

use crate::day::{self, Item, Kind};
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};
use std::path::PathBuf;

const PER_PAGE: usize = 6;

#[derive(Clone)]
struct Row {
    line: usize,
    item: Item,
    quick: bool,
    complete: bool,
    raw: String,
}

fn rows(source: &str) -> Vec<Row> {
    source.lines().enumerate().filter_map(|(line, text)| {
        let item = day::parse(text).into_iter().next()?;
        matches!(item.kind, Kind::Task | Kind::Habit).then_some(Row { line, item, quick: false, complete: false, raw: text.into() })
    }).collect()
}

fn quick_rows(source: &str) -> Vec<Row> {
    source.lines().enumerate().filter_map(|(line, text)| {
        let (state, name) = text.split_once('\t')?;
        if !matches!(state, "0" | "1") || name.trim().is_empty() || name.chars().any(char::is_control) { return None; }
        Some(Row { line, item: Item { kind: Kind::Task, text: name.into(), at: None, days: None }, quick: true, complete: state == "1", raw: text.into() })
    }).collect()
}

fn add_quick(source: &str, name: &str) -> Result<String, &'static str> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) { return Err("Escreva um nome simples para a tarefa."); }
    if quick_rows(source).iter().any(|r| r.item.text.eq_ignore_ascii_case(name)) { return Err("Essa tarefa já está na lista."); }
    let mut result = source.to_string();
    if !result.is_empty() && !result.ends_with('\n') { result.push('\n'); }
    result.push_str("0\t");
    result.push_str(name);
    result.push('\n');
    Ok(result)
}

fn edit_quick(source: &str, line: usize, remove: bool) -> Option<String> {
    let row = quick_rows(source).into_iter().find(|row| row.line == line)?;
    let mut result = String::new();
    for (i, text) in source.split_inclusive('\n').enumerate() {
        if i != line { result.push_str(text); }
        else if !remove {
            result.push_str(if row.complete { "0\t" } else { "1\t" });
            result.push_str(&row.item.text);
            if text.ends_with("\r\n") { result.push_str("\r\n"); }
            else if text.ends_with('\n') { result.push('\n'); }
        }
    }
    Some(result)
}

fn add_to(source: &str, kind: Kind, value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.is_empty() || value.chars().any(char::is_control) || value.starts_with('{') {
        return Err("Escreva um nome simples para o item.");
    }
    let marker = match kind {
        Kind::Habit => '*',
        Kind::Task => '-',
        _ => return Err("Não foi possível criar este item."),
    };
    if rows(source).iter().any(|r| r.item.kind == kind && r.item.text.eq_ignore_ascii_case(value)) {
        return Err("Esse item já está na lista.");
    }
    let mut result = source.to_string();
    if !result.is_empty() && !result.ends_with('\n') { result.push('\n'); }
    result.push(marker);
    result.push(' ');
    result.push_str(value);
    result.push('\n');
    Ok(result)
}

fn remove_from(source: &str, line: usize) -> Option<String> {
    if !rows(source).iter().any(|r| r.line == line) { return None; }
    Some(source.split_inclusive('\n').enumerate().filter_map(|(i, text)| (i != line).then_some(text)).collect())
}

fn path() -> PathBuf { crate::home_dir().join("data/dia.txt") }
fn quick_path() -> PathBuf { crate::home_dir().join("data/tarefas_rapidas.txt") }

/// Total e concluídas, para o resumo na tela inicial.
pub fn resumo_rapidas() -> (usize, usize) {
    let list = quick_rows(&read_file(quick_path()).unwrap_or_default());
    (list.len(), list.iter().filter(|r| r.complete).count())
}

fn read_file(path: PathBuf) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        other => other,
    }
}

fn write_file(path: PathBuf, value: &str) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    let temp = path.with_extension("txt.tmp");
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(value.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() { let _ = std::fs::remove_file(&temp); }
    result
}

fn checked_path(day: i64) -> PathBuf { crate::home_dir().join("data/dias").join(format!("{day}.txt")) }

/// Altera apenas a chave escolhida, conservando linhas desconhecidas e o histórico já gravado.
fn toggle_checked(path: PathBuf, key: &str) -> std::io::Result<bool> {
    let source = read_file(path.clone())?;
    let found = source.lines().any(|line| line == key);
    let result = if found {
        source.split_inclusive('\n').filter(|line| line.trim_end_matches(['\r', '\n']) != key).collect::<String>()
    } else {
        let mut text = source;
        if !text.is_empty() && !text.ends_with('\n') { text.push('\n'); }
        text.push_str(key);
        text.push('\n');
        text
    };
    write_file(path, &result)?;
    Ok(!found)
}

pub struct Organizacao {
    page: usize,
    deleting: bool,
    text_kind: Option<Kind>,
    request_text: bool,
    message: String,
    pending_delete: Option<(u32, Row)>,
    displayed: Vec<Row>,
}

impl Organizacao {
    pub fn new() -> Self {
        let mut screen = Self { page: 0, deleting: false, text_kind: None, request_text: false, message: String::new(), pending_delete: None, displayed: Vec::new() };
        screen.refresh_rows();
        screen
    }

    fn row_rect(k: usize) -> Rect {
        Rect::new(MARGIN, 290 + k as i32 * 150, W - 2 * MARGIN, 132)
    }

    fn page_rect(next: bool) -> Rect {
        Rect::new(if next { W - MARGIN - 220 } else { MARGIN }, 1185, 220, 70)
    }
    fn bottom(i: usize) -> Rect { Rect::new(MARGIN + i as i32 * 314, 1288, 298, 80) }

    fn plan_rows(&self) -> Vec<Row> {
        let mut list = rows(&read_file(path()).unwrap_or_default());
        list.extend(quick_rows(&read_file(quick_path()).unwrap_or_default()));
        list
    }

    fn refresh_rows(&mut self) {
        self.displayed = self.plan_rows();
        self.page = self.page.min(self.displayed.len().saturating_sub(1) / PER_PAGE);
    }

    /// O que foi mostrado deve continuar sendo a mesma linha no arquivo atual.
    fn same_row(a: &Row, b: &Row) -> bool { a.line == b.line && a.quick == b.quick && a.raw == b.raw }
    fn visible_row(displayed: &[Row], current: &[Row], position: usize) -> Option<Row> {
        let row = displayed.get(position)?;
        current.get(position).filter(|now| Self::same_row(row, now)).map(|_| row.clone())
    }

    fn added_page(items: &[Row], kind: Kind, text: &str) -> usize {
        items.iter().rposition(|r| r.item.kind == kind && r.quick == (kind == Kind::Task) && r.item.text.eq_ignore_ascii_case(text.trim())).unwrap_or(0) / PER_PAGE
    }

    /// O primeiro toque seleciona; o segundo confirma o mesmo item.
    fn confirm_delete(&mut self, id: u32, row: &Row) -> bool {
        if self.pending_delete.as_ref().is_some_and(|(old, original)| *old == id && Self::same_row(original, row)) {
            self.pending_delete = None;
            true
        } else {
            self.pending_delete = Some((id, row.clone()));
            false
        }
    }

    fn append(&mut self, text: &str) {
        let Some(kind) = self.text_kind.take() else { return };
        let dest = if kind == Kind::Task { quick_path() } else { path() };
        let result = read_file(dest.clone()).map_err(|_| "Não foi possível abrir a lista.").and_then(|source| {
            if kind == Kind::Task { add_quick(&source, text) } else { add_to(&source, kind, text) }
        });
        match result {
            Ok(value) if write_file(dest, &value).is_ok() => {
                let items = self.plan_rows();
                self.page = Self::added_page(&items, kind, text);
                self.displayed = items;
                self.message = "Adicionado à lista.".into();
            }
            Ok(_) => self.message = "Não foi possível salvar a lista.".into(),
            Err(e) => self.message = e.into(),
        }
    }

    fn remove(&mut self, row: &Row) {
        let dest = if row.quick { quick_path() } else { path() };
        let result = read_file(dest.clone()).ok().and_then(|source| {
            // Se o arquivo mudou desde o desenho, não apaga uma linha diferente.
            let current = if row.quick { quick_rows(&source) } else { rows(&source) }.into_iter().find(|r| r.line == row.line)?;
            if current.raw != row.raw { return None; }
            if row.quick { edit_quick(&source, row.line, true) } else { remove_from(&source, row.line) }
        });
        match result {
            Some(value) if write_file(dest, &value).is_ok() => {
                self.page = self.page.min(self.plan_rows().len().saturating_sub(1) / PER_PAGE);
                self.message = "Item apagado.".into();
                self.deleting = false;
            }
            _ => self.message = "Não foi possível apagar. Confira a lista.".into(),
        }
    }

    #[cfg(test)]
    pub fn screenshot_delete(&mut self) { self.deleting = true; }
}

impl MiniGame for Organizacao {
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        minigame::header(fb, f, "Mais", "Hábitos e tarefas");
        let items = &self.displayed;
        let today = crate::data::today(crate::offset_min());
        let checked = day::checked(today);
        let read_error = read_file(path()).is_err() || read_file(quick_path()).is_err() || read_file(checked_path(today)).is_err();
        if items.is_empty() {
            let text = "Adicione um hábito ou uma tarefa para começar.";
            for (i, line) in crate::chat::wrap_words(f, Face::Body, 34.0, text, W - 2 * MARGIN).iter().enumerate() {
                f.draw(fb, Face::Body, 34.0, MARGIN, 370 + i as i32 * 45, MUTED, line);
            }
        }
        for (k, row) in items.iter().skip(self.page * PER_PAGE).take(PER_PAGE).enumerate() {
            let r = Self::row_rect(k);
            let selected = self.deleting && self.pending_delete.as_ref().is_some_and(|(id, original)| *id == k as u32 && Self::same_row(original, row));
            ui::card(fb, r, 20, if selected { INK } else { PAPER }, false);
            let foreground = if selected { PAPER } else { INK };
            let box_r = Rect::new(r.x + 22, r.y + 38, 52, 52);
            let complete = if row.quick { row.complete } else { checked.contains(&row.item.key()) };
            fb.round_outline(box_r, 7, 3, foreground, if complete { foreground } else if selected { INK } else { PAPER });
            if complete {
                ui::stroke(fb, (box_r.x as f32 + 11.0, box_r.y as f32 + 27.0), (box_r.x as f32 + 22.0, box_r.y as f32 + 38.0), 4.0, if selected { INK } else { PAPER });
                ui::stroke(fb, (box_r.x as f32 + 22.0, box_r.y as f32 + 38.0), (box_r.x as f32 + 42.0, box_r.y as f32 + 13.0), 4.0, if selected { INK } else { PAPER });
            }
            let text = crate::apps::fit(f, Face::Strong, 31.0, &row.item.text, r.w - 120);
            f.draw(fb, Face::Strong, 31.0, r.x + 96, r.y + 53, foreground, &text);
            let detail = if selected { "Toque outra vez para apagar".into() } else if !row.item.on(today) {
                format!("Não previsto hoje · {}", row.item.days_label())
            } else if row.item.kind == Kind::Habit {
                let streak = day::habit_streak(&row.item, crate::offset_min());
                format!("Hábito · {streak} {} de sequência", if streak == 1 { "dia" } else { "dias" })
            } else if row.quick { "Tarefa rápida".into() } else { "Tarefa do dia".into() };
            f.draw(fb, Face::Body, 25.0, r.x + 96, r.y + 100, if selected { PAPER } else { MUTED }, &detail);
        }
        if self.page > 0 { minigame::pill(fb, f, Self::page_rect(false), "Anterior", false); }
        if (self.page + 1) * PER_PAGE < items.len() { minigame::pill(fb, f, Self::page_rect(true), "Próxima", false); }
        let hint = if !self.message.is_empty() { &self.message } else if read_error { "Não foi possível ler todos os dados." } else if self.deleting { "Toque duas vezes no item para apagar." } else { "Toque em um item para marcar ou desmarcar." };
        let hint = crate::apps::fit(f, Face::Body, 25.0, hint, W - 2 * MARGIN);
        if self.page == 0 && items.len() <= PER_PAGE {
            f.draw(fb, Face::Body, 25.0, MARGIN, 1265, MUTED, &hint);
        } else if !self.message.is_empty() || read_error {
            f.draw(fb, Face::Body, 25.0, MARGIN, 268, MUTED, &hint);
        }
        for (i, label) in ["Novo hábito", "Nova tarefa", if self.deleting { "Cancelar" } else { "Apagar" }].iter().enumerate() {
            minigame::pill(fb, f, Self::bottom(i), label, i == 0);
        }
    }

    fn targets(&self) -> Vec<(u32, Rect)> {
        let items = &self.displayed;
        let mut v: Vec<_> = items.iter().skip(self.page * PER_PAGE).take(PER_PAGE).enumerate().map(|(k, _)| (k as u32, Self::row_rect(k))).collect();
        if self.page > 0 { v.push((810, Self::page_rect(false))); }
        if (self.page + 1) * PER_PAGE < items.len() { v.push((811, Self::page_rect(true))); }
        v.extend((0..3).map(|i| (800 + i as u32, Self::bottom(i))));
        v
    }

    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match id {
            800 | 801 => {
                self.pending_delete = None;
                self.text_kind = Some(if id == 800 { Kind::Habit } else { Kind::Task });
                self.request_text = true;
            }
            802 => { self.pending_delete = None; self.deleting = !self.deleting; self.message.clear(); }
            810 => { self.pending_delete = None; self.page = self.page.saturating_sub(1); }
            811 => { self.pending_delete = None; self.page += 1; }
            id if (id as usize) < PER_PAGE => {
                let position = self.page * PER_PAGE + id as usize;
                let current = self.plan_rows();
                let Some(row) = Self::visible_row(&self.displayed, &current, position) else {
                    self.pending_delete = None;
                    self.displayed = current;
                    self.page = self.page.min(self.displayed.len().saturating_sub(1) / PER_PAGE);
                    self.message = "A lista mudou. Escolha o item novamente.".into();
                    return Redraw::Full;
                };
                if self.deleting {
                    if self.confirm_delete(id, &row) { self.remove(&row); }
                    else { self.message.clear(); }
                }
                else if row.quick {
                    let result = read_file(quick_path()).ok().and_then(|source| {
                        let current = quick_rows(&source).into_iter().find(|r| r.line == row.line)?;
                        if current.raw != row.raw { return None; }
                        edit_quick(&source, row.line, false)
                    });
                    if !result.is_some_and(|s| write_file(quick_path(), &s).is_ok()) { self.message = "Não foi possível salvar a tarefa.".into(); }
                    else { self.message.clear(); }
                } else if row.item.on(crate::data::today(crate::offset_min())) {
                    let day = crate::data::today(crate::offset_min());
                    if toggle_checked(checked_path(day), &row.item.key()).is_ok() { self.message.clear(); }
                    else { self.message = "Não foi possível salvar a marcação.".into(); }
                } else { self.message = "Este item não está previsto para hoje.".into(); }
            }
            _ => return Redraw::None,
        }
        self.refresh_rows();
        Redraw::Full
    }

    fn wants_text(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.request_text) { return None; }
        Some(if self.text_kind == Some(Kind::Habit) { "Nome do hábito" } else { "Nome da tarefa" }.into())
    }

    fn text_input(&mut self, text: &str) -> Redraw { self.append(text); self.refresh_rows(); Redraw::Full }

    fn resume(&mut self) { self.refresh_rows(); self.pending_delete = None; }

    fn back(&mut self) -> bool {
        if !self.deleting { return false; }
        self.deleting = false;
        self.pending_delete = None;
        self.message.clear();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identidade_exibida_e_confirmacao_resistem_a_mudanca_externa() {
        let shown = quick_rows("0\tPão\n0\tLeite\n");
        let reordered = quick_rows("0\tLeite\n0\tPão\n");
        assert!(Organizacao::visible_row(&shown, &reordered, 0).is_none());
        assert!(Organizacao::visible_row(&shown, &shown, 0).is_some());
        let mut screen = Organizacao { page: 0, deleting: true, text_kind: None, request_text: false, message: String::new(), pending_delete: None, displayed: shown.clone() };
        assert!(!screen.confirm_delete(0, &shown[0]));
        assert!(!screen.confirm_delete(1, &shown[1]));
        assert!(screen.pending_delete.as_ref().is_some_and(|(id, _)| *id == 1));
        assert!(screen.confirm_delete(1, &shown[1]));
        assert!(screen.pending_delete.is_none());
    }

    #[test]
    fn pagina_de_item_novo_considera_habitos_antes_das_tarefas() {
        let mut items = rows("* Anterior\n* Hábito novo\n");
        let quick = (0..12).map(|i| format!("0\tTarefa {i}\n")).collect::<String>();
        items.extend(quick_rows(&quick));
        assert_eq!(Organizacao::added_page(&items, Kind::Habit, "Hábito novo"), 0);
        assert_eq!(Organizacao::added_page(&items, Kind::Task, "Tarefa 11"), 2);
    }

    #[test]
    fn acrescenta_sem_perder_outros_itens() {
        let original = "# plano\n! prioridade\n@ 12:30 consulta\nlinha desconhecida";
        let updated = add_to(original, Kind::Habit, "Ler").unwrap();
        assert_eq!(updated, format!("{original}\n* Ler\n"));
        assert_eq!(rows(&updated).len(), 1);
        assert!(add_to(&updated, Kind::Habit, "Ler").is_err());
        assert!(add_to(&updated, Kind::Task, "duas\nlinhas").is_err());
        assert!(add_to(&updated, Kind::Task, "{seg} confunde").is_err());
    }

    #[test]
    fn apaga_somente_linha_escolhida_preservando_o_resto() {
        let original = "# manter\r\n- Pão\r\n@ 08:00 Consulta\r\n* Ler\r\n- Pão\r\nsem formato";
        let list = rows(original);
        assert_eq!(list.len(), 3);
        assert_eq!(remove_from(original, list[2].line).unwrap(), "# manter\r\n- Pão\r\n@ 08:00 Consulta\r\n* Ler\r\nsem formato");
        assert!(remove_from(original, 0).is_none());
    }

    #[test]
    fn tarefas_rapidas_conservam_conclusao_independente_do_dia() {
        let source = "# preservar\nlinha desconhecida\n1\tConcluída\n";
        let source = add_quick(source, "Comprar pão").unwrap();
        let r = quick_rows(&source);
        assert_eq!(r.len(), 2);
        assert!(r[0].complete);
        assert!(!r[1].complete);
        let marked = edit_quick(&source, r[1].line, false).unwrap();
        assert!(quick_rows(&marked)[1].complete);
        assert!(marked.ends_with("1\tComprar pão\n"));
        assert!(add_quick(&marked, "Comprar pão").is_err());
        assert!(add_quick(&marked, "nome\tcom tab").is_err());
        assert!(add_quick(&marked, "nome\u{1}controle").is_err());
        let removed = edit_quick(&marked, r[1].line, true).unwrap();
        assert_eq!(removed, "# preservar\nlinha desconhecida\n1\tConcluída\n");
        assert!(quick_rows("1\tNome\0inválido\n").is_empty());
    }

    #[test]
    fn marcacao_preserva_outras_linhas_e_rejeita_historico_ilegivel() {
        let root = std::env::temp_dir().join(format!("verso-organizacao-marcacao-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("dia.txt");
        std::fs::write(&file, "# manter\r\n* Ler\r\nlinha desconhecida\n* Ler\n- Pão").unwrap();
        assert!(!toggle_checked(file.clone(), "* Ler").unwrap());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# manter\r\nlinha desconhecida\n- Pão");
        assert!(toggle_checked(file.clone(), "* Ler").unwrap());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "# manter\r\nlinha desconhecida\n- Pão\n* Ler\n");
        let invalid = b"* Ler\n\xff";
        std::fs::write(&file, invalid).unwrap();
        assert!(toggle_checked(file.clone(), "* Ler").is_err());
        assert_eq!(std::fs::read(&file).unwrap(), invalid);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn falha_ao_salvar_nao_apaga_lista_existente() {
        let root = std::env::temp_dir().join(format!("verso-organizacao-escrita-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("tarefas.txt");
        std::fs::write(&file, "0\tPão\n# preservar\n").unwrap();
        let blocked = file.with_extension("txt.tmp");
        std::fs::create_dir(&blocked).unwrap();
        let value = edit_quick(&read_file(file.clone()).unwrap(), 0, false).unwrap();
        assert!(write_file(file.clone(), &value).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "0\tPão\n# preservar\n");
        std::fs::remove_dir(blocked).unwrap();
        write_file(file.clone(), &value).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "1\tPão\n# preservar\n");
        let _ = std::fs::remove_dir_all(root);
    }
}
