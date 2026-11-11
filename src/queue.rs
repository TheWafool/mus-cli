//! Download jobs and their state.

use crate::config::Config;
use crate::ytdlp::{JobEvent, Progress};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

const LOG_CAP: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Pending,
    Running,
    Done,
    Failed(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Starting,
    Downloading,
    Processing(String),
}

pub struct Job {
    pub id: u64,
    pub url: String,
    /// Options snapshot taken when the job was queued.
    pub cfg: Config,
    pub title: Option<String>,
    pub status: Status,
    pub phase: Phase,
    pub progress: Progress,
    pub files: Vec<String>,
    pub log: VecDeque<(String, bool)>,
    pub last_error: Option<String>,
    pub warnings: usize,
    pub started: Option<Instant>,
    pub finished: Option<Instant>,
    pub cancel: Arc<AtomicBool>,
    pub handle: Option<JoinHandle<()>>,
}

impl Job {
    pub fn new(id: u64, url: String, cfg: Config, title: Option<String>) -> Self {
        Self {
            id,
            url,
            cfg,
            title,
            status: Status::Pending,
            phase: Phase::Starting,
            progress: Progress::default(),
            files: Vec::new(),
            log: VecDeque::new(),
            last_error: None,
            warnings: 0,
            started: None,
            finished: None,
            cancel: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.url)
    }

    pub fn is_active(&self) -> bool {
        self.status == Status::Running
    }

    pub fn is_finished(&self) -> bool {
        matches!(self.status, Status::Done | Status::Failed(_) | Status::Cancelled)
    }

    /// Progress of the current file, 0..1.
    pub fn file_fraction(&self) -> f64 {
        match (self.progress.downloaded, self.progress.total) {
            (Some(d), Some(t)) if t > 0.0 => (d / t).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Overall progress across playlist items, 0..1.
    pub fn fraction(&self) -> f64 {
        if self.status == Status::Done {
            return 1.0;
        }
        let file = if matches!(self.phase, Phase::Processing(_)) { 1.0 } else { self.file_fraction() };
        match (self.progress.index, self.progress.count) {
            (Some(i), Some(n)) if n > 1 && i >= 1 => ((i - 1) as f64 + file) / n as f64,
            _ => file,
        }
    }

    pub fn elapsed(&self) -> Option<f64> {
        let start = self.started?;
        let end = self.finished.unwrap_or_else(Instant::now);
        Some(end.duration_since(start).as_secs_f64())
    }

    pub fn push_log(&mut self, line: String, err: bool) {
        if self.log.len() >= LOG_CAP {
            self.log.pop_front();
        }
        self.log.push_back((line, err));
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Reset a finished job so it can run again.
    pub fn reset(&mut self) {
        self.status = Status::Pending;
        self.phase = Phase::Starting;
        self.progress = Progress::default();
        self.files.clear();
        self.last_error = None;
        self.warnings = 0;
        self.started = None;
        self.finished = None;
        self.cancel = Arc::new(AtomicBool::new(false));
        self.handle = None;
        self.push_log("── retry ──".into(), false);
    }

    pub fn apply(&mut self, ev: JobEvent) {
        match ev {
            JobEvent::Progress(p) => {
                if let Some(t) = &p.title {
                    // For playlists, keep the item title fresh.
                    self.title = Some(t.clone());
                }
                self.phase = Phase::Downloading;
                self.progress = Progress { title: None, ..p };
            }
            JobEvent::PostProcess { status, name } => {
                if status == "started" {
                    self.phase = Phase::Processing(name);
                }
            }
            JobEvent::File(path) => {
                self.push_log(format!("Saved: {path}"), false);
                self.files.push(path);
            }
            JobEvent::Log { line, err } => {
                if let Some(e) = line.strip_prefix("ERROR: ") {
                    self.last_error = Some(e.to_string());
                } else if line.starts_with("WARNING:") {
                    self.warnings += 1;
                }
                self.push_log(line, err);
            }
            JobEvent::Exit { ok, cancelled, spawn_error } => {
                self.finished = Some(Instant::now());
                self.handle = None;
                self.status = if cancelled {
                    Status::Cancelled
                } else if let Some(e) = spawn_error {
                    self.push_log(e.clone(), true);
                    Status::Failed(e)
                } else if ok || !self.files.is_empty() {
                    // Playlists with a few unavailable items still count as done.
                    Status::Done
                } else {
                    Status::Failed(
                        self.last_error.clone().unwrap_or_else(|| "yt-dlp exited with an error".into()),
                    )
                };
            }
        }
    }
}
