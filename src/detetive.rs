//! Um caso por dia, com depoimentos e registros que podem ser conferidos sem sorte.
use crate::fb::{Framebuffer, Rect};
use crate::minigame::{self, MiniGame, Redraw, MARGIN, W};
use crate::text::{Face, Fonts};
use crate::ui::{self, INK, MUTED, PAPER};

const N: usize = 5;
const HOURS: u32 = 12;

#[derive(Clone, Debug)]
struct Person { name: String, trait_: String }

#[derive(Clone, Debug)]
struct Case {
    people: Vec<Person>, places: Vec<String>, objects: Vec<String>, motives: Vec<String>, time: String,
    culprit: usize, object: usize, motive: usize, object_place: usize, motive_place: usize,
}

fn entries(section: &str) -> Vec<String> {
    let mut active = false;
    let mut out = Vec::new();
    for line in include_str!("../assets/detetive.txt").lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            active = &line[1..line.len() - 1] == section;
        } else if active && !line.is_empty() {
            out.push(line.to_string());
        }
    }
    out
}

impl Case {
    fn generate(day: i64) -> Self {
        let mut rng = minigame::Rng::seeded((day as u64).wrapping_add(0x5de7ec71));
        let mut people: Vec<Person> = entries("suspeitos").into_iter().filter_map(|s| {
            let (name, trait_) = s.split_once('|')?;
            Some(Person { name: name.into(), trait_: trait_.into() })
        }).collect();
        let mut places = entries("locais");
        let mut objects = entries("objetos");
        let mut motives = entries("motivos");
        let mut times = entries("horarios");
        rng.shuffle(&mut people); rng.shuffle(&mut places); rng.shuffle(&mut objects);
        rng.shuffle(&mut motives); rng.shuffle(&mut times);
        people.truncate(N); places.truncate(N); objects.truncate(N); motives.truncate(N);
        let culprit = (rng.next() % N as u64) as usize;
        let object = (rng.next() % N as u64) as usize;
        let motive = (rng.next() % N as u64) as usize;
        let object_place = (rng.next() % N as u64) as usize;
        let motive_place = (object_place + 1 + (rng.next() % (N as u64 - 1)) as usize) % N;
        Self { people, places, objects, motives, time: times[0].clone(), culprit, object, motive, object_place, motive_place }
    }
    fn testimony(&self, i: usize) -> String {
        format!("{} diz que estava no local {} às {}.", self.people[i].name, self.places[i], self.time)
    }
    fn evidence(&self, i: usize) -> String {
        let mut s = format!("O registro do local {} {} a presença de {} às {}.", self.places[i], if i == self.culprit { "desmente" } else { "confirma" }, self.people[i].name, self.time);
        if i == self.object_place { s.push_str(&format!(" O objeto usado foi {}.", self.objects[self.object])); }
        if i == self.motive_place { s.push_str(&format!(" Uma carta revela o motivo: {}.", self.motives[self.motive])); }
        s
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Menu, People, Places, Detail, Notes, AccusePerson, AccuseObject, AccuseMotive, Result }

pub struct Detetive {
    case: Case, day: i64, page: Page, return_page: Page,
    people_seen: u32, places_seen: u32, hours_used: u32, detail: String, note_page: usize,
    accused: usize, object: usize, motive: usize, result: u32,
    streak: u32, best: u32, last_win: i64,
}

fn spend_hour(used: &mut u32) -> bool {
    if *used >= HOURS { return false; }
    *used += 1;
    true
}

fn path() -> std::path::PathBuf {
    let d = crate::home_dir().join("data");
    let _ = std::fs::create_dir_all(&d);
    d.join("detetive.txt")
}

impl Detetive {
    pub fn new() -> Self {
        let day = crate::fases::today();
        let mut g = Self { case: Case::generate(day), day, page: Page::Menu, return_page: Page::People, people_seen: 0, places_seen: 0, hours_used: 0, detail: String::new(), note_page: 0, accused: 0, object: 0, motive: 0, result: 0, streak: 0, best: 0, last_win: i64::MIN };
        let data = std::fs::read_to_string(path()).unwrap_or_default();
        let mut saved_day = i64::MIN;
        for line in data.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let Ok(v) = value.parse::<i64>() else { continue };
            match key {
                "dia" => saved_day = v,
                "pessoas" => g.people_seen = (v as u32) & 31,
                "locais" => g.places_seen = (v as u32) & 31,
                "horas" => g.hours_used = v.clamp(0, HOURS as i64) as u32,
                "resultado" => g.result = (v as u32).min(2),
                "acusado" => g.accused = (v as usize).min(N - 1),
                "objeto" => g.object = (v as usize).min(N - 1),
                "motivo" => g.motive = (v as usize).min(N - 1),
                "sequencia" => g.streak = (v.max(0) as u32).min(100000),
                "melhor" => g.best = (v.max(0) as u32).min(100000),
                "ultima_vitoria" => g.last_win = v,
                _ => {}
            }
        }
        if saved_day != day { g.people_seen = 0; g.places_seen = 0; g.hours_used = 0; g.result = 0; }
        else { g.hours_used = g.hours_used.max(g.people_seen.count_ones() + g.places_seen.count_ones()); }
        g
    }
    #[cfg(test)]
    pub fn final_para_print() -> Self {
        let mut g = Self::new();
        g.accused = g.case.culprit; g.object = g.case.object; g.motive = g.case.motive;
        g.result = 1; g.page = Page::Result;
        g
    }
    fn store(&self) {
        let target = path();
        let tmp = target.with_extension("tmp");
        let s = format!("dia={}\npessoas={}\nlocais={}\nhoras={}\nresultado={}\nacusado={}\nobjeto={}\nmotivo={}\nsequencia={}\nmelhor={}\nultima_vitoria={}\n", self.day, self.people_seen, self.places_seen, self.hours_used, self.result, self.accused, self.object, self.motive, self.streak, self.best, self.last_win);
        if std::fs::write(&tmp, s).is_ok() { let _ = std::fs::rename(tmp, target); }
    }
    fn hours(&self) -> u32 { HOURS.saturating_sub(self.hours_used) }
    fn streak_today(&self) -> u32 { if self.last_win >= self.day - 1 { self.streak } else { 0 } }
    fn row(i: usize) -> Rect { Rect::new(MARGIN, 370 + i as i32 * 140, W - MARGIN * 2, 118) }
    fn button(i: usize, n: usize) -> Rect {
        let r = minigame::bottom_button(i, n);
        Rect::new(r.x, 1274, r.w, r.h)
    }
    fn show_detail(&mut self, i: usize, person: bool) {
        if !spend_hour(&mut self.hours_used) { return; }
        let mask = 1 << i;
        let bits = if person { &mut self.people_seen } else { &mut self.places_seen };
        *bits |= mask;
        self.detail = if person { self.case.testimony(i) } else { self.case.evidence(i) };
        self.return_page = if person { Page::People } else { Page::Places };
        self.page = Page::Detail;
        self.store();
    }
    fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        for i in 0..N {
            if self.people_seen & (1 << i) != 0 { notes.push(self.case.testimony(i)); }
            if self.places_seen & (1 << i) != 0 { notes.push(self.case.evidence(i)); }
        }
        notes
    }
    fn finish(&mut self) {
        let won = self.accused == self.case.culprit && self.object == self.case.object && self.motive == self.case.motive;
        self.result = if won { 1 } else { 2 };
        if won && self.last_win != self.day {
            self.streak = if self.last_win == self.day - 1 { self.streak + 1 } else { 1 };
            self.best = self.best.max(self.streak);
            self.last_win = self.day;
        }
        self.page = Page::Result;
        self.store();
    }
    fn wrapped(f: &Fonts, text: &str, width: i32, size: f32) -> Vec<String> {
        let mut lines = vec![String::new()];
        for word in text.split_whitespace() {
            let line = lines.last().unwrap();
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if f.measure(Face::Body, size, &next) > width && !line.is_empty() { lines.push(word.to_string()); }
            else { *lines.last_mut().unwrap() = next; }
        }
        lines
    }
    fn paragraph(fb: &mut Framebuffer, f: &Fonts, text: &str, y: i32, size: f32) {
        for (k, line) in Self::wrapped(f, text, W - 2 * MARGIN - 64, size).iter().enumerate() {
            f.draw(fb, Face::Body, size, MARGIN + 32, y + k as i32 * (size as i32 + 14), INK, line);
        }
    }
    fn status(&self) -> String { format!("Restam {} horas", self.hours()) }
    fn list_labels(&self) -> Vec<String> {
        match self.page {
            Page::People => (0..N).map(|i| format!("{} · {}{}", self.case.people[i].name, self.case.people[i].trait_, if self.people_seen & (1 << i) != 0 { " · ouvido" } else { "" })).collect(),
            Page::Places => (0..N).map(|i| format!("{}{}", self.case.places[i], if self.places_seen & (1 << i) != 0 { " · visitado" } else { "" })).collect(),
            Page::AccusePerson => self.case.people.iter().map(|p| p.name.clone()).collect(),
            Page::AccuseObject => self.case.objects.clone(),
            Page::AccuseMotive => self.case.motives.clone(),
            _ => vec![],
        }
    }
}

impl MiniGame for Detetive {
    fn at_menu(&self) -> bool { self.page == Page::Menu }
    fn save(&self) { self.store(); }
    fn resume(&mut self) {
        if crate::fases::today() != self.day { *self = Self::new(); }
    }
    fn summary(&self) -> String { if self.page == Page::Menu { String::new() } else { self.status() } }
    fn draw(&self, fb: &mut Framebuffer, f: &Fonts) {
        fb.fill(fb.screen(), PAPER);
        ui::draw_back(fb, f, false);
        if self.page == Page::Menu {
            minigame::draw_menu(fb, f, "Detetive", "Um mistério novo a cada dia.", &[if self.result == 0 { "Investigar o caso de hoje" } else { "Ver resultado de hoje" }]);
            f.draw(fb, Face::Display, 50.0, MARGIN, 750, INK, &format!("Sequência: {} dias", self.streak_today()));
            f.draw(fb, Face::Body, 30.0, MARGIN, 810, MUTED, &format!("Melhor sequência: {} dias", self.best));
            minigame::menu_art(fb, "Detetive", 880);
            return;
        }
        let title = match self.page { Page::People => "Interrogar", Page::Places => "Visitar locais", Page::Detail => "Descoberta", Page::Notes => "Caderno", Page::AccusePerson => "Quem foi?", Page::AccuseObject => "Qual objeto?", Page::AccuseMotive => "Qual motivo?", Page::Result => "Caso encerrado", Page::Menu => "Detetive" };
        let status = if self.page == Page::Result { if self.result == 1 { "Caso resolvido!".to_string() } else { "A acusação não bateu.".to_string() } } else { self.status() };
        minigame::header(fb, f, "Detetive", &status);
        f.draw(fb, Face::Display, 52.0, MARGIN, 320, INK, title);
        match self.page {
            Page::People | Page::Places | Page::AccusePerson | Page::AccuseObject | Page::AccuseMotive => {
                for (i, label) in self.list_labels().iter().enumerate() {
                    minigame::pill(fb, f, Self::row(i), &crate::apps::fit(f, Face::Strong, 31.0, label, W - 2 * MARGIN - 60), false);
                }
                if self.page == Page::People || self.page == Page::Places {
                    let hint = if self.hours() == 0 { "Sem horas: consulte o caderno ou acuse." } else if self.page == Page::People { "Cada interrogatório custa 1 hora." } else { "Cada visita custa 1 hora." };
                    f.draw(fb, Face::Body, 28.0, MARGIN, 1125, MUTED, hint);
                    minigame::pill(fb, f, Self::button(0, 3), if self.page == Page::People { "Locais" } else { "Pessoas" }, false);
                    minigame::pill(fb, f, Self::button(1, 3), "Caderno", false);
                    minigame::pill(fb, f, Self::button(2, 3), "Acusar", true);
                } else { minigame::pill(fb, f, Self::button(0, 1), "Voltar", false); }
            }
            Page::Detail => {
                ui::card(fb, Rect::new(MARGIN, 370, W - 2 * MARGIN, 430), 28, PAPER, false);
                Self::paragraph(fb, f, &self.detail, 435, 36.0);
                f.draw(fb, Face::Body, 30.0, MARGIN + 32, 740, MUTED, "As descobertas ficam no caderno.");
                minigame::pill(fb, f, Self::button(0, 1), "Continuar", true);
            }
            Page::Notes => {
                let notes = self.notes();
                let shown = notes.get(self.note_page);
                ui::card(fb, Rect::new(MARGIN, 370, W - 2 * MARGIN, 530), 28, PAPER, false);
                let msg = shown.map(String::as_str).unwrap_or("Suas descobertas aparecerão aqui.");
                Self::paragraph(fb, f, msg, 440, 36.0);
                f.draw(fb, Face::Strong, 30.0, MARGIN, 1010, INK, &format!("Anotação {} de {}", if notes.is_empty() { 0 } else { self.note_page + 1 }, notes.len()));
                minigame::pill(fb, f, Self::button(0, 3), "Anterior", false);
                minigame::pill(fb, f, Self::button(1, 3), "Próxima", false);
                minigame::pill(fb, f, Self::button(2, 3), "Voltar", true);
            }
            Page::Result => {
                ui::card(fb, Rect::new(MARGIN, 370, W - 2 * MARGIN, 790), 28, PAPER, false);
                let text = format!("{} mentiu: disse que estava no local {} às {}, mas o registro desmente o álibi. No local {}, a pista revelou que o objeto usado foi {}. No local {}, uma carta revelou o motivo: {}.", self.case.people[self.case.culprit].name, self.case.places[self.case.culprit], self.case.time, self.case.places[self.case.object_place], self.case.objects[self.case.object], self.case.places[self.case.motive_place], self.case.motives[self.case.motive]);
                Self::paragraph(fb, f, &text, 440, 32.0);
                let answer = format!("Sua acusação: {}, {} e {}.", self.case.people[self.accused].name, self.case.objects[self.object], self.case.motives[self.motive]);
                Self::paragraph(fb, f, &answer, 950, 30.0);
                minigame::pill(fb, f, Self::button(0, 1), "Menu", true);
            }
            Page::Menu => {}
        }
    }
    fn targets(&self) -> Vec<(u32, Rect)> {
        match self.page {
            Page::Menu => vec![(900, minigame::menu_row(0))],
            Page::People | Page::Places => {
                let mut v: Vec<_> = if self.hours() > 0 { (0..N).map(|i| (i as u32, Self::row(i))).collect() } else { Vec::new() };
                v.extend((0..3).map(|i| (800 + i as u32, Self::button(i, 3)))); v
            }
            Page::AccusePerson | Page::AccuseObject | Page::AccuseMotive => {
                let mut v: Vec<_> = (0..N).map(|i| (i as u32, Self::row(i))).collect();
                v.push((803, Self::button(0, 1))); v
            }
            Page::Notes => (0..3).map(|i| (810 + i as u32, Self::button(i, 3))).collect(),
            Page::Detail | Page::Result => vec![(803, Self::button(0, 1))],
        }
    }
    fn tap(&mut self, id: u32, _x: i32, _y: i32) -> Redraw {
        match (self.page, id) {
            (Page::Menu, 900) => {
                if self.result == 0 {
                    self.detail = format!("Uma joia desapareceu às {}. Os inocentes dizem a verdade; só quem a roubou mentiu sobre o álibi. Confira os relatos e os registros. Encontre também o objeto e o motivo.", self.case.time);
                    self.return_page = Page::People;
                    self.page = Page::Detail;
                } else { self.page = Page::Result; }
            }
            (Page::People, 0..=4) => self.show_detail(id as usize, true),
            (Page::Places, 0..=4) => self.show_detail(id as usize, false),
            (Page::People, 800) => self.page = Page::Places,
            (Page::Places, 800) => self.page = Page::People,
            (Page::People | Page::Places, 801) => { self.return_page = self.page; self.note_page = 0; self.page = Page::Notes; }
            (Page::People | Page::Places, 802) => self.page = Page::AccusePerson,
            (Page::Detail | Page::Notes, 803 | 812) => self.page = self.return_page,
            (Page::Notes, 810) => self.note_page = self.note_page.saturating_sub(1),
            (Page::Notes, 811) => self.note_page = (self.note_page + 1).min(self.notes().len().saturating_sub(1)),
            (Page::AccusePerson, 0..=4) => { self.accused = id as usize; self.page = Page::AccuseObject; }
            (Page::AccuseObject, 0..=4) => { self.object = id as usize; self.page = Page::AccuseMotive; }
            (Page::AccuseMotive, 0..=4) => { self.motive = id as usize; self.finish(); }
            (Page::AccusePerson, 803) => self.page = Page::People,
            (Page::AccuseObject, 803) => self.page = Page::AccusePerson,
            (Page::AccuseMotive, 803) => self.page = Page::AccuseObject,
            (Page::Result, 803) => self.page = Page::Menu,
            _ => return Redraw::None,
        }
        Redraw::Full
    }
    fn back(&mut self) -> bool {
        match self.page {
            Page::Menu => false,
            Page::Detail | Page::Notes => { self.page = self.return_page; true }
            Page::AccuseObject => { self.page = Page::AccusePerson; true }
            Page::AccuseMotive => { self.page = Page::AccuseObject; true }
            _ => { self.page = Page::Menu; true }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duzentos_casos_tem_uma_unica_solucao() {
        for day in 1..=200 {
            let c = Case::generate(day);
            assert_eq!(c.people.len(), N); assert_eq!(c.places.len(), N);
            assert_ne!(c.object_place, c.motive_place);
            // Reconstrói todas as combinações compatíveis apenas a partir dos textos disponíveis.
            let depositions: Vec<_> = (0..N).map(|i| c.testimony(i)).collect();
            let clues: Vec<_> = (0..N).map(|i| c.evidence(i)).collect();
            let mut solutions = Vec::new();
            for person in 0..N {
                let alibi_false = clues.iter().any(|s| s.contains(&format!("desmente a presença de {} às", c.people[person].name)));
                let alibi_known = depositions[person].contains(&c.places[person]);
                for object in 0..N {
                    let object_found = clues.iter().any(|s| s.contains(&format!("O objeto usado foi {}.", c.objects[object])));
                    for motive in 0..N {
                        let motive_found = clues.iter().any(|s| s.contains(&format!("Uma carta revela o motivo: {}.", c.motives[motive])));
                        if alibi_false && alibi_known && object_found && motive_found { solutions.push((person, object, motive)); }
                    }
                }
            }
            assert_eq!(solutions, vec![(c.culprit, c.object, c.motive)]);
            assert!((N * 2) as u32 <= HOURS);
            for i in 0..N { assert!(c.testimony(i).contains(&c.places[i])); }
        }
    }
    #[test]
    fn cada_acao_gasta_uma_hora_e_para_no_limite() {
        let mut used = 0;
        for _ in 0..HOURS { assert!(spend_hour(&mut used)); }
        assert!(!spend_hour(&mut used));
        assert_eq!(used, HOURS);
    }
    #[test]
    fn sem_horas_desativa_novas_visitas_e_preserva_caderno() {
        let mut g = Detetive::new();
        g.page = Page::Places;
        g.hours_used = HOURS;
        let targets = g.targets();
        assert!(targets.iter().all(|(id, _)| *id >= 800));
        assert!(targets.iter().any(|(id, _)| *id == 801));
        assert!(targets.iter().any(|(id, _)| *id == 802));
        assert_eq!(Detetive::button(0, 3).y + Detetive::button(0, 3).h + 6, 1448 - MARGIN);
    }
    #[test]
    fn mesma_data_gera_o_mesmo_caso() {
        let a = Case::generate(100); let b = Case::generate(100);
        assert_eq!(a.people[0].name, b.people[0].name);
        assert_eq!((a.culprit, a.object, a.motive), (b.culprit, b.object, b.motive));
    }
}
