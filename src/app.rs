//! Application state and input handling.

use crate::config::{expand, Config, PRESETS};
use crate::input::TextInput;
use crate::options::{field, Kind, F, FIELDS, QUICK};
use crate::queue::{Job, Phase, Status};
use crate::ytdlp::{self, AppEvent, JobEvent, SearchResult};
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Download,
    Search,
    Queue,
    Options,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Download, Tab::Search, Tab::Queue, Tab::Options];

    pub fn index(self) -> usize {
        Tab::ALL.iter().position(|t| *t == self).unwrap()
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Download => "Download",
            Tab::Search => "Search",
            Tab::Queue => "Queue",
            Tab::Options => "Options",
        }
    }
}

/// Something on screen the mouse can hit. Recorded by the UI on each draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Tab(Tab),
    UrlInput,
    SearchInput,
    /// Option row on the Download tab; `true` when the value part was hit.
    Quick(usize, bool),
    /// Option row on the Options tab (index into FIELDS).
    Opt(usize, bool),
    QueueRow(usize),
    /// Search result; `true` when the select marker was hit.
    SearchRow(usize, bool),
    /// A clickable key hint: acts like pressing that key.
    Key(KeyCode, KeyModifiers),
    Preset(usize),
    PopupKey(KeyCode),
    PopupArea,
    Scroll(ScrollArea),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollArea {
    Quick,
    Options,
    Queue,
    Search,
    Presets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Input,
    List,
}

pub struct EditState {
    pub field: F,
    pub input: TextInput,
    pub completions: Vec<String>,
}

pub enum Popup {
    Help,
    Edit(EditState),
    Presets(usize),
    ConfirmQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Ok,
    Warn,
    Error,
}

pub struct SearchState {
    pub input: TextInput,
    pub focus: Focus,
    pub results: Vec<SearchResult>,
    pub sel: usize,
    pub marked: BTreeSet<usize>,
    pub loading: bool,
    pub error: Option<String>,
    pub seq: u64,
    pub query: String,
}

pub struct App {
    pub cfg: Config,
    pub tab: Tab,
    pub url: TextInput,
    pub dl_focus: Focus,
    pub quick_sel: usize,
    pub opt_sel: usize,
    pub jobs: Vec<Job>,
    pub queue_sel: usize,
    pub paused: bool,
    pub search: SearchState,
    pub popup: Option<Popup>,
    pub toast: Option<(String, Level, Instant)>,
    pub version: Option<Result<String, String>>,
    pub should_quit: bool,
    pub started: Instant,
    pub hits: RefCell<Vec<(Rect, Target)>>,
    tx: Sender<AppEvent>,
    next_id: u64,
}

impl App {
    pub fn new(cfg: Config, tx: Sender<AppEvent>) -> Self {
        Self {
            cfg,
            tab: Tab::Download,
            url: TextInput::default(),
            dl_focus: Focus::Input,
            quick_sel: 0,
            opt_sel: 0,
            jobs: Vec::new(),
            queue_sel: 0,
            paused: false,
            search: SearchState {
                input: TextInput::default(),
                focus: Focus::Input,
                results: Vec::new(),
                sel: 0,
                marked: BTreeSet::new(),
                loading: false,
                error: None,
                seq: 0,
                query: String::new(),
            },
            popup: None,
            toast: None,
            version: None,
            should_quit: false,
            started: Instant::now(),
            hits: RefCell::new(Vec::new()),
            tx,
            next_id: 1,
        }
    }

    // ---------- helpers ----------

    pub fn toast(&mut self, msg: impl Into<String>, level: Level) {
        self.toast = Some((msg.into(), level, Instant::now()));
    }

    fn save_cfg(&mut self) {
        if let Err(e) = self.cfg.save() {
            self.toast(format!("Could not save config: {e}"), Level::Error);
        }
    }

    pub fn quick_fields(&self) -> Vec<F> {
        QUICK.iter().copied().filter(|f| self.cfg.relevant(*f)).collect()
    }

    pub fn active_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.is_active()).count()
    }

    pub fn pending_count(&self) -> usize {
        self.jobs.iter().filter(|j| j.status == Status::Pending).count()
    }

    fn is_typing(&self) -> bool {
        (self.tab == Tab::Download && self.dl_focus == Focus::Input)
            || (self.tab == Tab::Search && self.search.focus == Focus::Input)
    }

    fn focused_input_empty(&self) -> bool {
        match self.tab {
            Tab::Download => self.url.value().is_empty(),
            Tab::Search => self.search.input.value().is_empty(),
            _ => false,
        }
    }

    fn switch_tab(&mut self, dir: i32) {
        let i = (self.tab.index() as i32 + dir).rem_euclid(4) as usize;
        self.tab = Tab::ALL[i];
    }

    // ---------- queue ----------

    pub fn enqueue(&mut self, url: &str, title: Option<String>) -> bool {
        let url = ytdlp::normalize_url(url);
        if self.jobs.iter().any(|j| j.url == url && !j.is_finished()) {
            return false;
        }
        let job = Job::new(self.next_id, url, self.cfg.clone(), title);
        self.next_id += 1;
        self.jobs.push(job);
        true
    }

    fn enqueue_many(&mut self, items: Vec<(String, Option<String>)>) {
        let total = items.len();
        let added = items.into_iter().filter(|(u, t)| self.enqueue(u, t.clone())).count();
        let mode = match self.cfg.mode {
            crate::config::Mode::Audio => self.cfg.audio_format.label(),
            crate::config::Mode::Video => self.cfg.video_container.label(),
        };
        match (added, total - added) {
            (0, _) => self.toast("Already in the queue", Level::Warn),
            (n, 0) => self.toast(
                format!("Queued {n} item{} as {mode}", if n == 1 { "" } else { "s" }),
                Level::Ok,
            ),
            (n, d) => self.toast(format!("Queued {n}, skipped {d} duplicate(s)"), Level::Ok),
        }
    }

    fn start_job(&mut self, i: usize) {
        let tx = self.tx.clone();
        let job = &mut self.jobs[i];
        job.status = Status::Running;
        job.phase = Phase::Starting;
        job.started = Some(Instant::now());
        job.push_log(format!("$ {}", ytdlp::preview(&job.cfg, &job.url)), false);
        job.handle = Some(ytdlp::spawn_job(job.id, &job.cfg, &job.url, job.cancel.clone(), tx));
    }

    fn schedule(&mut self) {
        if self.paused {
            return;
        }
        let max = self.cfg.max_concurrent.max(1) as usize;
        let mut running = self.active_count();
        let mut i = 0;
        while running < max && i < self.jobs.len() {
            if self.jobs[i].status == Status::Pending {
                self.start_job(i);
                running += 1;
            }
            i += 1;
        }
    }

    fn submit_url(&mut self) {
        let text = self.url.value().trim().to_string();
        if text.is_empty() {
            self.toast("Paste a link first (Ctrl+Shift+V)", Level::Warn);
            return;
        }
        let words: Vec<&str> = text.split_whitespace().collect();
        let urls: Vec<(String, Option<String>)> = words
            .iter()
            .filter(|w| ytdlp::is_url(w))
            .map(|w| (w.to_string(), None))
            .collect();
        if urls.is_empty() {
            // Not a link: treat as a search query.
            self.search.input.set(text.clone());
            self.url.clear();
            self.tab = Tab::Search;
            self.run_search();
            return;
        }
        self.url.clear();
        self.enqueue_many(urls);
    }

    // ---------- search ----------

    fn run_search(&mut self) {
        let q = self.search.input.value().trim().to_string();
        if q.is_empty() {
            return;
        }
        self.search.seq += 1;
        self.search.loading = true;
        self.search.error = None;
        self.search.query = q.clone();
        ytdlp::spawn_search(self.search.seq, &self.cfg, &q, self.tx.clone());
    }

    fn queue_search_selection(&mut self) {
        let s = &self.search;
        let picks: Vec<usize> = if s.marked.is_empty() {
            if s.results.is_empty() { vec![] } else { vec![s.sel] }
        } else {
            s.marked.iter().copied().collect()
        };
        let items: Vec<(String, Option<String>)> = picks
            .iter()
            .filter_map(|i| s.results.get(*i))
            .map(|r| (r.url.clone(), Some(r.title.clone()).filter(|t| !t.is_empty())))
            .collect();
        if items.is_empty() {
            return;
        }
        self.search.marked.clear();
        self.enqueue_many(items);
    }

    // ---------- events ----------

    pub fn on_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Job(id, ev) => {
                let exit = matches!(ev, JobEvent::Exit { .. });
                let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) else { return };
                job.apply(ev);
                if exit {
                    let title = job.display_title().to_string();
                    let files = job.files.len();
                    let status = job.status.clone();
                    let had_err = job.last_error.is_some();
                    match status {
                        Status::Done => {
                            let msg = match (files, had_err) {
                                (0, _) => format!("Nothing new: {title}"),
                                (1, false) => format!("Downloaded: {title}"),
                                (n, false) => format!("Downloaded {n} files"),
                                (n, true) => format!("Downloaded {n} files (some items failed)"),
                            };
                            self.desktop_notify("Download finished", &msg);
                            self.toast(msg, if had_err { Level::Warn } else { Level::Ok });
                        }
                        Status::Failed(e) => {
                            self.desktop_notify("Download failed", &format!("{title}\n{e}"));
                            self.toast(format!("Failed: {e}"), Level::Error);
                        }
                        Status::Cancelled => self.toast("Cancelled", Level::Info),
                        _ => {}
                    }
                }
            }
            AppEvent::Search(seq, res) => {
                if seq != self.search.seq {
                    return;
                }
                self.search.loading = false;
                match res {
                    Ok(results) => {
                        self.search.sel = 0;
                        self.search.marked.clear();
                        if results.is_empty() {
                            self.search.error = Some("No results".into());
                        } else if self.tab == Tab::Search {
                            self.search.focus = Focus::List;
                        }
                        self.search.results = results;
                    }
                    Err(e) => self.search.error = Some(e),
                }
            }
            AppEvent::Version(res) => self.version = Some(res),
        }
    }

    pub fn on_tick(&mut self) {
        self.schedule();
        if let Some((_, _, t)) = &self.toast
            && t.elapsed().as_secs() >= 5 {
                self.toast = None;
            }
    }

    pub fn on_paste(&mut self, text: &str) {
        if let Some(Popup::Edit(e)) = &mut self.popup {
            e.input.insert_str(text.trim());
            return;
        }
        if self.popup.is_some() {
            return;
        }
        if self.tab == Tab::Search {
            self.search.focus = Focus::Input;
            self.search.input.insert_str(text.trim());
            return;
        }
        self.tab = Tab::Download;
        self.dl_focus = Focus::Input;
        if !self.url.value().is_empty() && !self.url.value().ends_with(' ') {
            self.url.insert(' ');
        }
        self.url.insert_str(text.trim());
    }

    fn paste_clipboard(&mut self) {
        let tools: [(&str, &[&str]); 3] = [
            ("wl-paste", &["-n"]),
            ("xclip", &["-o", "-selection", "clipboard"]),
            ("xsel", &["-ob"]),
        ];
        for (bin, args) in tools {
            if let Ok(out) = Command::new(bin).args(args).stdin(Stdio::null()).output()
                && out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    self.on_paste(&text);
                    return;
                }
        }
        self.toast(
            "No clipboard tool (wl-clipboard/xclip). Use your terminal's paste: Ctrl+Shift+V",
            Level::Warn,
        );
    }

    fn desktop_notify(&self, title: &str, body: &str) {
        if !self.cfg.notify {
            return;
        }
        let (title, body) = (title.to_string(), body.to_string());
        std::thread::spawn(move || {
            let mut cmd = if cfg!(target_os = "macos") {
                let script = format!(
                    "display notification {:?} with title {:?}",
                    body.replace('\n', " "),
                    title
                );
                let mut c = Command::new("osascript");
                c.args(["-e", &script]);
                c
            } else {
                let mut c = Command::new("notify-send");
                c.args(["-a", "mus", "-i", "folder-music", &title, &body]);
                c
            };
            let _ = cmd.stdout(Stdio::null()).stderr(Stdio::null()).status();
        });
    }

    fn open_path(&mut self, path: PathBuf) {
        let shown = path.display().to_string();
        if !path.exists() {
            self.toast(format!("Folder doesn't exist yet: {shown}"), Level::Warn);
            return;
        }
        ytdlp::open_external(&path);
        self.toast(format!("Opening {shown}"), Level::Info);
    }

    pub fn request_quit(&mut self) {
        if self.active_count() + self.pending_count() > 0 {
            self.popup = Some(Popup::ConfirmQuit);
        } else {
            self.should_quit = true;
        }
    }

    /// Stop all downloads and wait for their processes to exit.
    pub fn shutdown(&mut self) {
        for j in &self.jobs {
            if j.is_active() {
                j.request_cancel();
            }
        }
        for j in &mut self.jobs {
            if let Some(h) = j.handle.take() {
                let _ = h.join();
            }
        }
    }

    // ---------- mouse ----------

    fn key(&mut self, code: KeyCode) {
        self.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    pub fn on_mouse(&mut self, ev: MouseEvent) {
        let pos = Position::new(ev.column, ev.row);
        let hits = self.hits.borrow().clone();
        let topmost = |want_scroll: bool| {
            hits.iter()
                .rev()
                .find(|(r, t)| r.contains(pos) && matches!(t, Target::Scroll(_)) == want_scroll)
                .map(|(_, t)| *t)
        };
        match ev.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let up = ev.kind == MouseEventKind::ScrollUp;
                let Some(Target::Scroll(area)) = topmost(true) else { return };
                match area {
                    ScrollArea::Quick => {
                        if up && (self.dl_focus == Focus::Input || self.quick_sel == 0) {
                            return;
                        }
                        self.dl_focus = Focus::List;
                    }
                    ScrollArea::Search => {
                        if self.search.results.is_empty() || (up && self.search.sel == 0) {
                            return;
                        }
                        self.search.focus = Focus::List;
                    }
                    _ => {}
                }
                self.key(if up { KeyCode::Up } else { KeyCode::Down });
            }
            MouseEventKind::Down(button @ (MouseButton::Left | MouseButton::Right)) => {
                let right = button == MouseButton::Right;
                let target = topmost(false);
                if let Some(popup) = &self.popup {
                    match (popup, target) {
                        (Popup::Presets(cur), Some(Target::Preset(i))) => {
                            if *cur == i {
                                self.key(KeyCode::Enter);
                            } else {
                                self.popup = Some(Popup::Presets(i));
                            }
                        }
                        (_, Some(Target::PopupKey(code))) => self.key(code),
                        (Popup::Help, _) => self.popup = None,
                        (_, Some(Target::PopupArea)) | (_, Some(Target::Preset(_))) => {}
                        _ => self.popup = None, // click outside closes
                    }
                    return;
                }
                let Some(target) = target else { return };
                let activate = KeyEvent::new(
                    if right { KeyCode::Left } else { KeyCode::Enter },
                    KeyModifiers::NONE,
                );
                match target {
                    Target::Tab(t) => self.tab = t,
                    Target::UrlInput => self.dl_focus = Focus::Input,
                    Target::SearchInput => self.search.focus = Focus::Input,
                    Target::Quick(i, on_value) => {
                        let again = self.dl_focus == Focus::List && self.quick_sel == i;
                        self.dl_focus = Focus::List;
                        self.quick_sel = i;
                        if (on_value || again || right)
                            && let Some(id) = self.quick_fields().get(i).copied() {
                                self.form_key(id, activate);
                            }
                    }
                    Target::Opt(i, on_value) => {
                        let again = self.opt_sel == i;
                        self.opt_sel = i;
                        if on_value || again || right {
                            self.form_key(FIELDS[i].id, activate);
                        }
                    }
                    Target::QueueRow(i) => {
                        self.tab = Tab::Queue;
                        self.queue_sel = i;
                    }
                    Target::SearchRow(i, on_marker) => {
                        let s = &mut self.search;
                        let again = s.focus == Focus::List && s.sel == i;
                        s.focus = Focus::List;
                        s.sel = i;
                        if (on_marker || again || right)
                            && !s.marked.remove(&i) {
                                s.marked.insert(i);
                            }
                    }
                    Target::Key(code, mods) => self.on_key(KeyEvent::new(code, mods)),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // ---------- keys ----------

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if ctrl && key.code == KeyCode::Char('c') {
            if matches!(self.popup, Some(Popup::ConfirmQuit)) {
                self.should_quit = true;
            } else {
                self.popup = None;
                self.request_quit();
            }
            return;
        }
        if let Some(p) = self.popup.take() {
            self.popup_key(p, key);
            return;
        }
        match key.code {
            KeyCode::Tab => return self.switch_tab(1),
            KeyCode::BackTab => return self.switch_tab(-1),
            KeyCode::F(n @ 1..=4) => {
                self.tab = Tab::ALL[n as usize - 1];
                return;
            }
            KeyCode::Char(c @ '1'..='4') if alt => {
                self.tab = Tab::ALL[c as usize - '1' as usize];
                return;
            }
            KeyCode::Char('v') if ctrl => return self.paste_clipboard(),
            _ => {}
        }
        // In an empty text field, 1-4 still switch tabs.
        if self.is_typing() && !ctrl && !alt && self.focused_input_empty()
            && let KeyCode::Char(c @ '1'..='4') = key.code {
                self.tab = Tab::ALL[c as usize - '1' as usize];
                return;
            }
        if !self.is_typing() && !ctrl && !alt {
            match key.code {
                KeyCode::Char('q') => return self.request_quit(),
                KeyCode::Char('?') => {
                    self.popup = Some(Popup::Help);
                    return;
                }
                KeyCode::Char(c @ '1'..='4') => {
                    self.tab = Tab::ALL[c as usize - '1' as usize];
                    return;
                }
                _ => {}
            }
        }
        match self.tab {
            Tab::Download => self.key_download(key),
            Tab::Search => self.key_search(key),
            Tab::Queue => self.key_queue(key),
            Tab::Options => self.key_options(key),
        }
    }

    /// Shared handling for option-form rows. Returns true if the key was used.
    fn form_key(&mut self, id: F, key: KeyEvent) -> bool {
        let changed = match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.cfg.cycle(id, -1),
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => self.cfg.cycle(id, 1),
            KeyCode::Enter => {
                if matches!(field(id).kind, Kind::Text | Kind::Path) {
                    self.popup = Some(Popup::Edit(EditState {
                        field: id,
                        input: TextInput::new(self.cfg.display(id)),
                        completions: Vec::new(),
                    }));
                    false
                } else {
                    self.cfg.cycle(id, 1)
                }
            }
            KeyCode::Char('r') => {
                self.cfg.reset_field(id);
                self.toast(format!("{} reset to default", field(id).label), Level::Info);
                true
            }
            KeyCode::Char('p') => {
                self.popup = Some(Popup::Presets(0));
                false
            }
            _ => return false,
        };
        if changed {
            self.save_cfg();
        }
        true
    }

    fn key_download(&mut self, key: KeyEvent) {
        if self.dl_focus == Focus::Input {
            match key.code {
                KeyCode::Enter => self.submit_url(),
                KeyCode::Esc | KeyCode::Down => self.dl_focus = Focus::List,
                _ => {
                    self.url.handle_key(key);
                }
            }
            return;
        }
        let fields = self.quick_fields();
        self.quick_sel = self.quick_sel.min(fields.len().saturating_sub(1));
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if self.quick_sel == 0 {
                    self.dl_focus = Focus::Input;
                } else {
                    self.quick_sel -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.quick_sel = (self.quick_sel + 1).min(fields.len().saturating_sub(1))
            }
            KeyCode::Char('i') | KeyCode::Char('/') | KeyCode::Esc => self.dl_focus = Focus::Input,
            KeyCode::Char('d') => self.submit_url(),
            _ => {
                if let Some(id) = fields.get(self.quick_sel).copied() {
                    self.form_key(id, key);
                }
            }
        }
    }

    fn key_search(&mut self, key: KeyEvent) {
        let s = &mut self.search;
        if s.focus == Focus::Input {
            match key.code {
                KeyCode::Enter => self.run_search(),
                KeyCode::Esc => s.focus = Focus::List,
                KeyCode::Down if !s.results.is_empty() => s.focus = Focus::List,
                _ => {
                    s.input.handle_key(key);
                }
            }
            return;
        }
        let n = s.results.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if s.sel == 0 {
                    s.focus = Focus::Input;
                } else {
                    s.sel -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => s.sel = (s.sel + 1).min(n.saturating_sub(1)),
            KeyCode::PageUp => s.sel = s.sel.saturating_sub(10),
            KeyCode::PageDown => s.sel = (s.sel + 10).min(n.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => s.sel = 0,
            KeyCode::End | KeyCode::Char('G') => s.sel = n.saturating_sub(1),
            KeyCode::Char(' ') => {
                if n > 0 {
                    if !s.marked.remove(&s.sel) {
                        s.marked.insert(s.sel);
                    }
                    s.sel = (s.sel + 1).min(n - 1);
                }
            }
            KeyCode::Char('a') => {
                if s.marked.len() == n {
                    s.marked.clear();
                } else {
                    s.marked = (0..n).collect();
                }
            }
            KeyCode::Enter => self.queue_search_selection(),
            KeyCode::Char('o') => {
                if let Some(r) = s.results.get(s.sel) {
                    ytdlp::open_external(&r.url);
                }
            }
            KeyCode::Char('i') | KeyCode::Char('/') => s.focus = Focus::Input,
            KeyCode::Char(c)
                if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && c.is_alphanumeric() =>
            {
                // Typing a letter jumps back into the search box.
                s.focus = Focus::Input;
                s.input.clear();
                s.input.insert(c);
            }
            _ => {}
        }
    }

    fn key_queue(&mut self, key: KeyEvent) {
        let n = self.jobs.len();
        let sel = self.queue_sel.min(n.saturating_sub(1));
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.queue_sel = sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.queue_sel = (sel + 1).min(n.saturating_sub(1)),
            KeyCode::Home | KeyCode::Char('g') => self.queue_sel = 0,
            KeyCode::End | KeyCode::Char('G') => self.queue_sel = n.saturating_sub(1),
            KeyCode::Char('p') => {
                self.paused = !self.paused;
                let msg = if self.paused {
                    "Queue paused: running downloads continue, no new ones start"
                } else {
                    "Queue resumed"
                };
                self.toast(msg, Level::Info);
            }
            KeyCode::Char('C') => {
                let before = self.jobs.len();
                self.jobs.retain(|j| !j.is_finished());
                let removed = before - self.jobs.len();
                self.queue_sel = self.queue_sel.min(self.jobs.len().saturating_sub(1));
                self.toast(format!("Cleared {removed} finished"), Level::Info);
            }
            KeyCode::Char('R') => {
                let mut count = 0;
                for j in &mut self.jobs {
                    if matches!(j.status, Status::Failed(_) | Status::Cancelled) {
                        j.reset();
                        count += 1;
                    }
                }
                self.toast(format!("Retrying {count} job(s)"), Level::Info);
            }
            _ => {}
        }
        if n == 0 {
            return;
        }
        match key.code {
            KeyCode::Char('c') => {
                let j = &mut self.jobs[sel];
                match j.status {
                    Status::Running => {
                        j.request_cancel();
                        self.toast("Cancelling…", Level::Info);
                    }
                    Status::Pending => {
                        j.status = Status::Cancelled;
                        j.finished = Some(Instant::now());
                    }
                    _ => {}
                }
            }
            KeyCode::Char('r') => {
                let j = &mut self.jobs[sel];
                if j.is_finished() {
                    j.reset();
                    self.toast("Retrying", Level::Info);
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if self.jobs[sel].is_active() {
                    self.toast("Still running: cancel it first with c", Level::Warn);
                } else {
                    self.jobs.remove(sel);
                    self.queue_sel = sel.min(self.jobs.len().saturating_sub(1));
                }
            }
            KeyCode::Char('o') | KeyCode::Enter => {
                let j = &self.jobs[sel];
                let dir = j
                    .files
                    .last()
                    .and_then(|f| Path::new(f).parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| expand(&j.cfg.output_dir));
                self.open_path(dir);
            }
            KeyCode::Char('K') if sel > 0 => {
                self.jobs.swap(sel, sel - 1);
                self.queue_sel = sel - 1;
            }
            KeyCode::Char('J') if sel + 1 < n => {
                self.jobs.swap(sel, sel + 1);
                self.queue_sel = sel + 1;
            }
            _ => {}
        }
    }

    fn key_options(&mut self, key: KeyEvent) {
        let n = FIELDS.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.opt_sel = self.opt_sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.opt_sel = (self.opt_sel + 1).min(n - 1),
            KeyCode::PageUp => self.opt_sel = self.opt_sel.saturating_sub(8),
            KeyCode::PageDown => self.opt_sel = (self.opt_sel + 8).min(n - 1),
            KeyCode::Home | KeyCode::Char('g') => self.opt_sel = 0,
            KeyCode::End | KeyCode::Char('G') => self.opt_sel = n - 1,
            KeyCode::Char('[') => self.jump_section(-1),
            KeyCode::Char(']') => self.jump_section(1),
            _ => {
                let id = FIELDS[self.opt_sel].id;
                self.form_key(id, key);
            }
        }
    }

    fn jump_section(&mut self, dir: i32) {
        let cur = FIELDS[self.opt_sel].section;
        if dir > 0 {
            if let Some(i) = FIELDS.iter().skip(self.opt_sel).position(|f| f.section != cur) {
                self.opt_sel += i;
            }
        } else {
            // Go to the start of this section, or of the previous one if already there.
            let mut i = self.opt_sel;
            while i > 0 && FIELDS[i - 1].section == cur {
                i -= 1;
            }
            if i == self.opt_sel && i > 0 {
                let prev = FIELDS[i - 1].section;
                i -= 1;
                while i > 0 && FIELDS[i - 1].section == prev {
                    i -= 1;
                }
            }
            self.opt_sel = i;
        }
    }

    fn popup_key(&mut self, popup: Popup, key: KeyEvent) {
        match popup {
            Popup::Help => {}
            Popup::ConfirmQuit => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => self.should_quit = true,
                _ => {}
            },
            Popup::Presets(mut i) => match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    i = i.checked_sub(1).unwrap_or(PRESETS.len() - 1);
                    self.popup = Some(Popup::Presets(i));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.popup = Some(Popup::Presets((i + 1) % PRESETS.len()));
                }
                KeyCode::Enter => {
                    let p = &PRESETS[i];
                    (p.apply)(&mut self.cfg);
                    self.save_cfg();
                    self.toast(format!("Preset applied: {}", p.name), Level::Ok);
                }
                KeyCode::Esc | KeyCode::Char('q') => {}
                _ => self.popup = Some(Popup::Presets(i)),
            },
            Popup::Edit(mut e) => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let value = e.input.value().trim().to_string();
                    self.cfg.set_text(e.field, value);
                    self.save_cfg();
                    if e.field == F::YtdlpPath {
                        self.version = None;
                        ytdlp::check_version(&self.cfg.ytdlp_path, self.tx.clone());
                    }
                    self.toast(format!("{} saved", field(e.field).label), Level::Ok);
                }
                KeyCode::Tab if field(e.field).kind == Kind::Path => {
                    complete_path(&mut e);
                    self.popup = Some(Popup::Edit(e));
                }
                _ => {
                    e.input.handle_key(key);
                    e.completions.clear();
                    self.popup = Some(Popup::Edit(e));
                }
            },
        }
    }
}

/// Tab-complete a directory path in the edit popup.
fn complete_path(e: &mut EditState) {
    let raw = e.input.value().to_string();
    let (dir_part, prefix) = match raw.rfind('/') {
        Some(i) => (&raw[..=i], &raw[i + 1..]),
        None => ("", raw.as_str()),
    };
    let dir = if dir_part.is_empty() { PathBuf::from(".") } else { expand(dir_part) };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        e.completions = vec!["(no such folder)".into()];
        return;
    };
    let show_hidden = prefix.starts_with('.');
    let mut names: Vec<String> = rd
        .filter_map(|d| d.ok())
        .filter(|d| d.path().is_dir())
        .filter_map(|d| d.file_name().into_string().ok())
        .filter(|n| n.starts_with(prefix) && (show_hidden || !n.starts_with('.')))
        .collect();
    names.sort();
    match names.len() {
        0 => e.completions = vec!["(no matches)".into()],
        1 => {
            e.input.set(format!("{dir_part}{}/", names[0]));
            e.completions.clear();
        }
        _ => {
            let mut common = names[0].clone();
            for n in &names[1..] {
                while !n.starts_with(&common) {
                    common.pop();
                }
            }
            e.input.set(format!("{dir_part}{common}"));
            e.completions = names;
        }
    }
}
