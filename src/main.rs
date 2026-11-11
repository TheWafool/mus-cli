mod app;
mod config;
mod input;
mod options;
mod queue;
mod ui;
mod ytdlp;

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::DefaultTerminal;
use std::io;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const USAGE: &str = "mus — a terminal UI for yt-dlp

Usage: mus-cli [URL]...

URLs given on the command line are queued at start with your saved options.
Config: ~/.config/mus-cli/config.toml
Press ? inside the app for keys.";

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("mus-cli {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let (cfg, warning) = config::Config::load();
    let (tx, rx) = mpsc::channel();
    let mut app = app::App::new(cfg, tx.clone());
    ytdlp::check_version(&app.cfg.ytdlp_path, tx);
    if let Some(w) = warning {
        app.toast(w, app::Level::Warn);
    }
    for url in &args {
        app.enqueue(url, None);
    }
    if !args.is_empty() {
        app.tab = app::Tab::Queue;
    }

    let mut terminal = ratatui::init();
    // Undo mouse capture / bracketed paste if we panic (ratatui's hook restores the rest).
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
        prev_hook(info);
    }));
    let _ = execute!(io::stdout(), EnableBracketedPaste, EnableMouseCapture);
    let result = run(&mut terminal, &mut app, &rx);
    let _ = execute!(io::stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    app.shutdown();
    result
}

fn run(terminal: &mut DefaultTerminal, app: &mut app::App, rx: &Receiver<ytdlp::AppEvent>) -> io::Result<()> {
    while !app.should_quit {
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(Duration::from_millis(80))? {
            loop {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => app.on_key(k),
                    Event::Paste(s) => app.on_paste(&s),
                    Event::Mouse(m) if !matches!(m.kind, MouseEventKind::Moved | MouseEventKind::Drag(_)) => {
                        app.on_mouse(m)
                    }
                    _ => {}
                }
                if app.should_quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
        while let Ok(ev) = rx.try_recv() {
            app.on_event(ev);
        }
        app.on_tick();
    }
    Ok(())
}
