//! Everything that talks to yt-dlp: argument building, running jobs, search.

use crate::config::*;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Field separator for our machine-readable lines (ASCII unit separator).
const SEP: char = '\u{1f}';
const TAG: &str = "MUS\u{1f}";

pub enum AppEvent {
    Job(u64, JobEvent),
    Search(u64, Result<Vec<SearchResult>, String>),
    Version(Result<String, String>),
}

pub enum JobEvent {
    Progress(Progress),
    PostProcess { status: String, name: String },
    File(String),
    Log { line: String, err: bool },
    Exit { ok: bool, cancelled: bool, spawn_error: Option<String> },
}

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub downloaded: Option<f64>,
    pub total: Option<f64>,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub index: Option<u32>,
    pub count: Option<u32>,
    pub audio_stream: bool,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub url: String,
    pub title: String,
    pub duration: Option<f64>,
    pub channel: String,
    pub views: Option<f64>,
}

/// Heuristic: does this URL point at a playlist/album/channel rather than one video?
pub fn looks_like_playlist(url: &str) -> bool {
    let u = url.to_lowercase();
    ["list=", "/playlist", "/album/", "/sets/", "/channel/", "/@", "/c/", "/user/", "/browse/"]
        .iter()
        .any(|p| u.contains(p))
}

pub fn is_url(s: &str) -> bool {
    let s = s.trim();
    s.contains("://")
        || s.starts_with("www.")
        || s.starts_with("youtu.be/")
        || s.starts_with("youtube.com/")
        || s.starts_with("music.youtube.com/")
        || s.starts_with("soundcloud.com/")
        || s.starts_with("ytsearch")
        || s.starts_with("scsearch")
}

/// Normalize user input like `youtu.be/xyz` into a full URL.
pub fn normalize_url(s: &str) -> String {
    let s = s.trim();
    if s.contains("://") || s.starts_with("ytsearch") || s.starts_with("scsearch") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

fn filename_template(cfg: &Config, url: &str) -> String {
    let mut t = String::new();
    let playlist = cfg.playlist == PlaylistMode::Whole;
    if playlist && cfg.playlist_folder && looks_like_playlist(url) {
        t.push_str("%(playlist_title,playlist)s/");
    }
    if playlist && cfg.number_playlist {
        t.push_str("%(playlist_index&{} - |)s");
    }
    let body = match cfg.filename_style {
        FilenameStyle::Title => "%(title)s.%(ext)s".to_string(),
        FilenameStyle::ArtistTitle => "%(artist,creator,uploader)s - %(title)s.%(ext)s".into(),
        FilenameStyle::UploaderTitle => "%(uploader)s - %(title)s.%(ext)s".into(),
        FilenameStyle::TitleId => "%(title)s [%(id)s].%(ext)s".into(),
        FilenameStyle::Custom => {
            let c = cfg.custom_template.trim();
            if c.is_empty() {
                "%(title)s.%(ext)s".into()
            } else if c.contains("%(ext)") {
                c.to_string()
            } else {
                format!("{c}.%(ext)s")
            }
        }
    };
    t.push_str(&body);
    t
}

/// The user-facing yt-dlp arguments for a URL (shown in the command preview).
pub fn build_args(cfg: &Config, url: &str) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| a.extend(items.iter().map(|s| s.to_string()));
    let audio = cfg.mode == Mode::Audio;

    // Format selection
    if audio {
        push(&["-f", "ba/b", "-x"]);
        if let Some(fmt) = cfg.audio_format.arg() {
            push(&["--audio-format", fmt]);
            if cfg.audio_format.takes_quality() {
                push(&["--audio-quality", cfg.audio_quality.arg()]);
            }
        }
    } else {
        push(&["-f", "bv*+ba/b"]);
        let mut sort = Vec::new();
        if let Some(h) = cfg.max_height.px() {
            sort.push(format!("res:{h}"));
        }
        match cfg.video_codec {
            VideoCodec::Any => {}
            VideoCodec::H264 => sort.push("vcodec:h264".into()),
            VideoCodec::Vp9 => sort.push("vcodec:vp9".into()),
            VideoCodec::Av1 => sort.push("vcodec:av01".into()),
        }
        match cfg.video_container {
            VideoContainer::Mp4 => sort.push("ext:mp4:m4a".into()),
            VideoContainer::Webm => sort.push("ext:webm:webm".into()),
            VideoContainer::Mkv => {}
        }
        if !sort.is_empty() {
            push(&["-S", &sort.join(",")]);
        }
        let c = cfg.video_container.arg();
        push(&["--merge-output-format", c, "--remux-video", c]);
    }

    // Output
    let out_dir = expand(&cfg.output_dir).display().to_string();
    push(&["-P", &out_dir, "-o", &filename_template(cfg, url)]);
    if cfg.restrict_filenames {
        push(&["--restrict-filenames"]);
    }
    if cfg.overwrite {
        push(&["--force-overwrites"]);
    }

    // Metadata & extras
    if cfg.embed_metadata {
        push(&["--embed-metadata"]);
    }
    let thumb_ok = if audio {
        cfg.audio_format != AudioFormat::Wav
    } else {
        cfg.video_container != VideoContainer::Webm
    };
    if cfg.embed_thumbnail && thumb_ok {
        push(&["--embed-thumbnail"]);
        if audio && cfg.square_cover {
            push(&[
                "--convert-thumbnails",
                "jpg",
                "--ppa",
                "ThumbnailsConvertor:-qmin 1 -q:v 1 -vf crop=\"'if(gt(ih,iw),iw,ih)':'if(gt(iw,ih),ih,iw)'\"",
            ]);
        }
    }
    if cfg.embed_chapters {
        push(&["--embed-chapters"]);
    }
    if !audio && cfg.subtitles != Subtitles::Off {
        if matches!(cfg.subtitles, Subtitles::Embed | Subtitles::Both) {
            push(&["--embed-subs"]);
        }
        if matches!(cfg.subtitles, Subtitles::File | Subtitles::Both) {
            push(&["--write-subs"]);
        }
        if cfg.auto_subs {
            push(&["--write-auto-subs"]);
        }
        let langs = if cfg.sub_langs.trim().is_empty() { "en.*" } else { cfg.sub_langs.trim() };
        push(&["--sub-langs", langs]);
    }
    if cfg.split_chapters {
        push(&[
            "--split-chapters",
            "-o",
            "chapter:%(title)s/%(section_number)02d - %(section_title)s.%(ext)s",
        ]);
    }
    let cats = if cfg.sponsor_categories.trim().is_empty() {
        "all"
    } else {
        cfg.sponsor_categories.trim()
    };
    match cfg.sponsorblock {
        SponsorBlock::Off => {}
        SponsorBlock::Mark => push(&["--sponsorblock-mark", cats]),
        SponsorBlock::Remove => push(&["--sponsorblock-remove", cats]),
    }

    // Playlist
    match cfg.playlist {
        PlaylistMode::Whole => {
            push(&["--yes-playlist"]);
            if !cfg.playlist_items.trim().is_empty() {
                push(&["-I", cfg.playlist_items.trim()]);
            }
        }
        PlaylistMode::Single => push(&["--no-playlist"]),
    }
    if cfg.use_archive {
        let p = expand(&cfg.archive_file).display().to_string();
        push(&["--download-archive", &p]);
    }

    // Network
    if let Some(b) = cfg.cookies_browser.arg() {
        push(&["--cookies-from-browser", b]);
    }
    if !cfg.rate_limit.trim().is_empty() {
        push(&["-r", cfg.rate_limit.trim()]);
    }
    let retries = if cfg.retries == 0 { "infinite".to_string() } else { cfg.retries.to_string() };
    push(&["-R", &retries]);
    if cfg.fragments > 1 {
        push(&["-N", &cfg.fragments.to_string()]);
    }
    if !cfg.proxy.trim().is_empty() {
        push(&["--proxy", cfg.proxy.trim()]);
    }

    if let Some(extra) = shlex::split(&cfg.extra_args) {
        a.extend(extra);
    }
    a.push("--".into());
    a.push(url.to_string());
    a
}

/// Arguments that make yt-dlp's output machine-readable for us.
fn internal_args() -> Vec<String> {
    let s = SEP;
    let dl = format!(
        "download:MUS{s}dl{s}%(progress.downloaded_bytes)s{s}%(progress.total_bytes)s{s}\
         %(progress.total_bytes_estimate)s{s}%(progress.speed)s{s}%(progress.eta)s{s}\
         %(info.playlist_autonumber)s{s}%(info.n_entries)s{s}%(info.vcodec)s{s}%(info.title)s"
    );
    let pp = format!("postprocess:MUS{s}pp{s}%(progress.status)s{s}%(progress.postprocessor)s");
    let done = format!("after_move:MUS{s}file{s}%(filepath)s");
    [
        "--newline",
        "--progress",
        "--no-simulate",
        "--progress-template",
        &dl,
        "--progress-template",
        &pp,
        "--print",
        &done,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Shell-quoted command line for display.
pub fn preview(cfg: &Config, url: &str) -> String {
    let mut parts = vec![cfg.ytdlp_path.clone()];
    parts.extend(build_args(cfg, url));
    parts
        .iter()
        .map(|p| {
            if p == "<URL>" {
                p.clone()
            } else {
                shlex::try_quote(p).map(|c| c.into_owned()).unwrap_or_else(|_| p.clone())
            }
        })
        .map(|p| contract_home(&p))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Show paths under $HOME as ~/… (display only).
pub fn contract_home(s: &str) -> String {
    if let Some(home) = dirs::home_dir().map(|h| h.display().to_string())
        && let Some(rest) = s.strip_prefix(&home)
            && (rest.is_empty() || rest.starts_with('/')) {
                return format!("~{rest}");
            }
    s.to_string()
}

fn num(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

fn parse_line(line: &str, err: bool) -> JobEvent {
    if let Some(rest) = line.find(TAG).map(|i| &line[i + TAG.len()..]) {
        let parts: Vec<&str> = rest.splitn(11, SEP).collect();
        match parts.first().copied() {
            Some("dl") if parts.len() >= 10 => {
                let title = parts[9].trim();
                return JobEvent::Progress(Progress {
                    downloaded: num(parts[1]),
                    total: num(parts[2]).or_else(|| num(parts[3])),
                    speed: num(parts[4]),
                    eta: num(parts[5]),
                    index: num(parts[6]).map(|v| v as u32),
                    count: num(parts[7]).map(|v| v as u32),
                    audio_stream: parts[8].trim() == "none",
                    title: (!title.is_empty() && title != "NA").then(|| title.to_string()),
                });
            }
            Some("pp") if parts.len() >= 3 => {
                return JobEvent::PostProcess {
                    status: parts[1].trim().to_string(),
                    name: parts[2].trim().to_string(),
                };
            }
            Some("file") if parts.len() >= 2 => return JobEvent::File(parts[1].trim().to_string()),
            _ => {}
        }
    }
    JobEvent::Log { line: line.to_string(), err }
}

fn spawn_reader<R: Read + Send + 'static>(
    stream: R,
    err: bool,
    id: u64,
    tx: Sender<AppEvent>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buf);
                    // yt-dlp may also use \r for in-place updates
                    for line in text.split(['\r', '\n']) {
                        let line = line.trim_end();
                        if !line.is_empty() {
                            let _ = tx.send(AppEvent::Job(id, parse_line(line, err)));
                        }
                    }
                }
            }
        }
    })
}

fn kill_group(pid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([signal, "--", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Run one download in a background thread.
pub fn spawn_job(
    id: u64,
    cfg: &Config,
    url: &str,
    cancel: Arc<AtomicBool>,
    tx: Sender<AppEvent>,
) -> JoinHandle<()> {
    let bin = expand(&cfg.ytdlp_path).display().to_string();
    let mut args = internal_args();
    args.extend(build_args(cfg, url));
    if cfg.use_archive
        && let Some(dir) = expand(&cfg.archive_file).parent() {
            let _ = std::fs::create_dir_all(dir);
        }

    thread::spawn(move || {
        let send = |ev: JobEvent| {
            let _ = tx.send(AppEvent::Job(id, ev));
        };
        let mut child = match Command::new(&bin)
            .args(&args)
            .env("PYTHONUNBUFFERED", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                let msg = if e.kind() == std::io::ErrorKind::NotFound {
                    format!("'{bin}' not found. Install yt-dlp or set its path in Options.")
                } else {
                    format!("failed to start {bin}: {e}")
                };
                send(JobEvent::Exit { ok: false, cancelled: false, spawn_error: Some(msg) });
                return;
            }
        };
        let pid = child.id();
        let r1 = spawn_reader(child.stdout.take().unwrap(), false, id, tx.clone());
        let r2 = spawn_reader(child.stderr.take().unwrap(), true, id, tx.clone());

        let mut killed_at: Option<Instant> = None;
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) => {}
                Err(_) => break None,
            }
            if cancel.load(Ordering::Relaxed) {
                match killed_at {
                    None => {
                        kill_group(pid, "-TERM");
                        killed_at = Some(Instant::now());
                    }
                    Some(t) if t.elapsed() > Duration::from_secs(3) => {
                        kill_group(pid, "-KILL");
                        let _ = child.kill();
                    }
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(80));
        };
        let _ = r1.join();
        let _ = r2.join();
        let cancelled = cancel.load(Ordering::Relaxed);
        let ok = status.map(|s| s.success()).unwrap_or(false);
        send(JobEvent::Exit { ok: ok && !cancelled, cancelled, spawn_error: None });
    })
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Search in the background; results arrive as `AppEvent::Search`.
pub fn spawn_search(seq: u64, cfg: &Config, query: &str, tx: Sender<AppEvent>) {
    let bin = expand(&cfg.ytdlp_path).display().to_string();
    let n = cfg.search_count;
    let s = SEP;
    let mut args: Vec<String> = vec![
        "--flat-playlist".into(),
        "--no-warnings".into(),
        "--ignore-errors".into(),
        "--print".into(),
        format!("%(url)s{s}%(title)s{s}%(duration)s{s}%(channel,uploader,artist)s{s}%(view_count)s"),
    ];
    match cfg.search_source {
        SearchSource::YouTube => args.push(format!("ytsearch{n}:{query}")),
        SearchSource::SoundCloud => args.push(format!("scsearch{n}:{query}")),
        SearchSource::YouTubeMusic => {
            args.extend(["-I".into(), format!("1:{n}")]);
            args.push(format!("https://music.youtube.com/search?q={}#songs", url_encode(query)));
        }
    }
    thread::spawn(move || {
        let result = match Command::new(&bin)
            .args(&args)
            .stdin(Stdio::null())
            .output()
        {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(format!("'{bin}' not found. Install yt-dlp or set its path in Options."))
            }
            Err(e) => Err(e.to_string()),
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let results: Vec<SearchResult> = stdout
                    .lines()
                    .filter_map(|l| {
                        let p: Vec<&str> = l.split(SEP).collect();
                        if p.len() < 5 || p[0].trim().is_empty() || p[0] == "NA" {
                            return None;
                        }
                        let na = |v: &str| if v == "NA" { String::new() } else { v.to_string() };
                        Some(SearchResult {
                            url: p[0].to_string(),
                            title: na(p[1]),
                            duration: num(p[2]),
                            channel: na(p[3]),
                            views: num(p[4]),
                        })
                    })
                    .collect();
                if results.is_empty() && !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    let line = err
                        .lines()
                        .rev()
                        .find(|l| l.contains("ERROR"))
                        .unwrap_or("search failed")
                        .trim_start_matches("ERROR: ")
                        .to_string();
                    Err(line)
                } else {
                    Ok(results)
                }
            }
        };
        let _ = tx.send(AppEvent::Search(seq, result));
    });
}

pub fn check_version(bin: &str, tx: Sender<AppEvent>) {
    let bin = expand(bin).display().to_string();
    thread::spawn(move || {
        let res = match Command::new(&bin).arg("--version").stdin(Stdio::null()).output() {
            Ok(o) if o.status.success() => Ok(String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Ok(_) => Err("yt-dlp error".to_string()),
            Err(_) => Err("yt-dlp not found".to_string()),
        };
        let _ = tx.send(AppEvent::Version(res));
    });
}

// ---------- formatting helpers ----------

pub fn fmt_bytes(b: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{v:.0} {}", UNITS[i]) } else { format!("{v:.1} {}", UNITS[i]) }
}

pub fn fmt_duration(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

pub fn fmt_count(v: f64) -> String {
    if v >= 1e9 {
        format!("{:.1}B", v / 1e9)
    } else if v >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if v >= 1e3 {
        format!("{:.1}K", v / 1e3)
    } else {
        format!("{v:.0}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_progress() {
        let line = format!(
            "MUS{SEP}dl{SEP}1024{SEP}4096{SEP}NA{SEP}512.5{SEP}6{SEP}2{SEP}10{SEP}none{SEP}My | Song"
        );
        match parse_line(&line, false) {
            JobEvent::Progress(p) => {
                assert_eq!(p.downloaded, Some(1024.0));
                assert_eq!(p.total, Some(4096.0));
                assert_eq!(p.index, Some(2));
                assert_eq!(p.count, Some(10));
                assert!(p.audio_stream);
                assert_eq!(p.title.as_deref(), Some("My | Song"));
            }
            _ => panic!("not progress"),
        }
    }

    #[test]
    fn audio_args() {
        let cfg = Config::default();
        let a = build_args(&cfg, "https://x");
        assert!(a.windows(2).any(|w| w[0] == "--audio-format" && w[1] == "mp3"));
        assert_eq!(a.last().unwrap(), "https://x");
    }

    #[test]
    fn playlist_detection() {
        assert!(looks_like_playlist("https://www.youtube.com/playlist?list=PL1"));
        assert!(!looks_like_playlist("https://youtu.be/abc"));
    }
}
