//! Rendering.

use crate::app::{App, Focus, Level, Popup, Tab};
use crate::config::{expand, Config, Mode, PRESETS};
use crate::options::{field, Kind, F, FIELDS};
use crate::queue::{Job, Phase, Status};
use crate::ytdlp::{fmt_bytes, fmt_count, fmt_duration, preview};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, BorderType, Clear, List, ListItem, ListState, Padding, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Wrap,
};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Catppuccin Mocha
const ACCENT: Color = Color::Rgb(203, 166, 247);
const PINK: Color = Color::Rgb(245, 194, 231);
const BLUE: Color = Color::Rgb(137, 180, 250);
const TEAL: Color = Color::Rgb(148, 226, 213);
const GREEN: Color = Color::Rgb(166, 227, 161);
const YELLOW: Color = Color::Rgb(249, 226, 175);
const RED: Color = Color::Rgb(243, 139, 168);
const TEXT: Color = Color::Rgb(205, 214, 244);
const SUBTEXT: Color = Color::Rgb(166, 173, 200);
const DIM: Color = Color::Rgb(108, 112, 134);
const SURFACE: Color = Color::Rgb(49, 50, 68);
const BASE: Color = Color::Rgb(30, 30, 46);

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

fn spinner(app: &App) -> &'static str {
    SPINNER[(app.started.elapsed().as_millis() / 80) as usize % SPINNER.len()]
}

fn panel(title: &str, focused: bool) -> Block<'static> {
    let (border, title_style) = if focused {
        (Style::new().fg(ACCENT), Style::new().fg(ACCENT).bold())
    } else {
        (Style::new().fg(SURFACE), Style::new().fg(SUBTEXT))
    };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border)
        .title(Span::styled(format!(" {title} "), title_style))
}

fn key_hint(key: &str, desc: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(key.to_string(), Style::new().fg(ACCENT).bold()),
        Span::styled(format!(" {desc}  "), Style::new().fg(DIM)),
    ]
}

fn hints(items: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (k, d) in items {
        spans.extend(key_hint(k, d));
    }
    Line::from(spans)
}

/// Cut a string to `max` cells, adding an ellipsis.
fn truncate(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw > max - 1 {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push('…');
    out
}

/// A line with `left` spans and `right` spans pushed to the far edge.
/// The last left span is truncated if they don't fit.
fn spread(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: u16) -> Line<'static> {
    let width = width as usize;
    let rw: usize = right.iter().map(|s| s.width()).sum();
    let lw: usize = left.iter().map(|s| s.width()).sum();
    let avail = width.saturating_sub(rw + 1);
    if lw > avail
        && let Some(last) = left.pop() {
            let others: usize = left.iter().map(|s| s.width()).sum();
            let room = avail.saturating_sub(others);
            left.push(Span::styled(truncate(&last.content, room), last.style));
        }
    let lw: usize = left.iter().map(|s| s.width()).sum();
    let pad = width.saturating_sub(lw + rw);
    left.push(Span::raw(" ".repeat(pad)));
    left.extend(right);
    Line::from(left)
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [v] = Layout::vertical([Constraint::Length(h.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [r] = Layout::horizontal([Constraint::Length(w.min(area.width))])
        .flex(Flex::Center)
        .areas(v);
    r
}

pub fn draw(f: &mut Frame, app: &App) {
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_header(f, header, app);
    let body = body.inner(Margin { horizontal: 1, vertical: 0 });
    match app.tab {
        Tab::Download => draw_download(f, body, app),
        Tab::Search => draw_search(f, body, app),
        Tab::Queue => draw_queue(f, body, app),
        Tab::Options => draw_options(f, body, app),
    }
    draw_footer(f, footer, app);

    match &app.popup {
        Some(Popup::Help) => draw_help(f, app),
        Some(Popup::Edit(_)) => draw_edit(f, app),
        Some(Popup::Presets(i)) => draw_presets(f, *i),
        Some(Popup::ConfirmQuit) => draw_confirm_quit(f, app),
        None => {}
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let mut left = vec![
        Span::styled(" ♪ mus ", Style::new().fg(BASE).bg(ACCENT).bold()),
        Span::raw("  "),
    ];
    for (i, tab) in Tab::ALL.iter().enumerate() {
        let mut label = format!(" {} {} ", i + 1, tab.title());
        if *tab == Tab::Queue {
            let (a, p) = (app.active_count(), app.pending_count());
            if a + p > 0 {
                label = format!(" {} {} ({}) ", i + 1, tab.title(), a + p);
            }
        }
        let style = if *tab == app.tab {
            Style::new().fg(BASE).bg(PINK).bold()
        } else {
            Style::new().fg(SUBTEXT)
        };
        left.push(Span::styled(label, style));
        left.push(Span::raw(" "));
    }

    let mut right = Vec::new();
    let active = app.active_count();
    if active > 0 {
        right.push(Span::styled(
            format!("{} {active} downloading ", spinner(app)),
            Style::new().fg(TEAL),
        ));
    }
    if app.paused {
        right.push(Span::styled("⏸ paused ", Style::new().fg(YELLOW)));
    }
    match &app.version {
        None => right.push(Span::styled("checking yt-dlp… ", Style::new().fg(DIM))),
        Some(Ok(v)) => right.push(Span::styled(format!("yt-dlp {v} "), Style::new().fg(DIM))),
        Some(Err(e)) => right.push(Span::styled(format!("⚠ {e} "), Style::new().fg(RED).bold())),
    }
    f.render_widget(Paragraph::new(spread(left, right, area.width)), area);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    if let Some((msg, level, _)) = &app.toast {
        let (icon, color) = match level {
            Level::Info => ("●", BLUE),
            Level::Ok => ("✔", GREEN),
            Level::Warn => ("▲", YELLOW),
            Level::Error => ("✖", RED),
        };
        let line = Line::from(vec![
            Span::styled(format!(" {icon} "), Style::new().fg(color).bold()),
            Span::styled(truncate(msg, area.width.saturating_sub(4) as usize), Style::new().fg(color)),
        ]);
        f.render_widget(Paragraph::new(line), area);
        return;
    }
    let line = match app.tab {
        Tab::Download if app.dl_focus == Focus::Input => hints(&[
            ("enter", "download"),
            ("↓", "options"),
            ("^V", "paste"),
            ("tab", "next tab"),
            ("^C", "quit"),
        ]),
        Tab::Download => hints(&[
            ("↑↓", "move"),
            ("←→", "change"),
            ("enter", "edit"),
            ("p", "presets"),
            ("r", "reset"),
            ("d", "download"),
            ("i", "link"),
            ("?", "help"),
        ]),
        Tab::Search if app.search.focus == Focus::Input => {
            hints(&[("enter", "search"), ("↓", "results"), ("tab", "next tab"), ("^C", "quit")])
        }
        Tab::Search => hints(&[
            ("space", "select"),
            ("a", "all"),
            ("enter", "queue"),
            ("o", "open in browser"),
            ("/", "new search"),
            ("?", "help"),
        ]),
        Tab::Queue => hints(&[
            ("c", "cancel"),
            ("r", "retry"),
            ("d", "remove"),
            ("o", "open folder"),
            ("p", if app.paused { "resume" } else { "pause" }),
            ("C", "clear done"),
            ("J/K", "reorder"),
            ("?", "help"),
        ]),
        Tab::Options => hints(&[
            ("↑↓", "move"),
            ("←→", "change"),
            ("enter", "edit"),
            ("[ ]", "section"),
            ("p", "presets"),
            ("r", "reset"),
            ("?", "help"),
        ]),
    };
    f.render_widget(Paragraph::new(line), area);
}

// ---------- option rows ----------

fn value_spans(cfg: &Config, id: F, selected: bool, relevant: bool) -> Vec<Span<'static>> {
    let def = field(id);
    let dim = !relevant;
    let color = |c: Color| if dim { DIM } else { c };
    match def.kind {
        Kind::Toggle => {
            if cfg.toggle_value(id).unwrap_or(false) {
                vec![Span::styled("● On", Style::new().fg(color(GREEN)))]
            } else {
                vec![Span::styled("○ Off", Style::new().fg(DIM))]
            }
        }
        Kind::Choice => {
            let v = cfg.display(id);
            if selected {
                vec![
                    Span::styled("‹ ", Style::new().fg(ACCENT)),
                    Span::styled(v, Style::new().fg(color(TEXT)).bold()),
                    Span::styled(" ›", Style::new().fg(ACCENT)),
                ]
            } else {
                vec![Span::styled(v, Style::new().fg(color(TEXT)))]
            }
        }
        Kind::Text | Kind::Path => {
            let v = cfg.display(id);
            let mut spans = if v.trim().is_empty() {
                vec![Span::styled(
                    def.placeholder.to_string(),
                    Style::new().fg(DIM).add_modifier(Modifier::ITALIC),
                )]
            } else {
                vec![Span::styled(v, Style::new().fg(color(BLUE)))]
            };
            if selected {
                spans.push(Span::styled("  ⏎ edit", Style::new().fg(DIM)));
            }
            spans
        }
    }
}

fn option_row(cfg: &Config, id: F, selected: bool, label_w: usize, width: u16) -> ListItem<'static> {
    let relevant = cfg.relevant(id);
    let label_style = if !relevant {
        Style::new().fg(DIM)
    } else if selected {
        Style::new().fg(TEXT).bold()
    } else {
        Style::new().fg(SUBTEXT)
    };
    let label = truncate(field(id).label, label_w);
    let mut spans = vec![Span::styled(format!("{label:<label_w$}  "), label_style)];
    let values = value_spans(cfg, id, selected, relevant);
    // Truncate long values to the row width.
    let used = label_w + 4;
    let room = (width as usize).saturating_sub(used);
    let mut taken = 0;
    for s in values {
        let w = s.width();
        if taken + w > room {
            spans.push(Span::styled(truncate(&s.content, room.saturating_sub(taken)), s.style));
            break;
        }
        taken += w;
        spans.push(s);
    }
    ListItem::new(Line::from(spans))
}

fn highlight() -> Style {
    Style::new().bg(SURFACE).fg(ACCENT)
}

fn preview_lines(cfg: &Config, url: &str) -> Vec<Line<'static>> {
    let cmd = preview(cfg, url);
    let mut spans = Vec::new();
    let mut first = true;
    for tok in cmd.split(' ') {
        let style = if first {
            Style::new().fg(PINK).bold()
        } else if tok.starts_with('-') {
            Style::new().fg(ACCENT)
        } else if tok == "<URL>" || tok.contains("://") {
            Style::new().fg(TEAL)
        } else {
            Style::new().fg(SUBTEXT)
        };
        first = false;
        spans.push(Span::styled(tok.to_string(), style));
        spans.push(Span::raw(" "));
    }
    vec![Line::from(spans)]
}

fn preview_height(cfg: &Config, url: &str, width: u16) -> u16 {
    let len = preview(cfg, url).width() as u16;
    let inner = width.saturating_sub(4).max(10);
    // +1 slack: wrapping at spaces uses more lines than len / width
    (len / inner + 2 + 2).clamp(3, 10)
}

fn draw_preview(f: &mut Frame, area: Rect, cfg: &Config, url: &str) {
    let block = panel("Command", false).padding(Padding::horizontal(1));
    f.render_widget(
        Paragraph::new(preview_lines(cfg, url)).wrap(Wrap { trim: false }).block(block),
        area,
    );
}

// ---------- Download tab ----------

fn draw_download(f: &mut Frame, area: Rect, app: &App) {
    let first_url = app
        .url
        .value()
        .split_whitespace()
        .find(|w| crate::ytdlp::is_url(w))
        .map(crate::ytdlp::normalize_url)
        .unwrap_or_else(|| "<URL>".into());
    let ph = preview_height(&app.cfg, &first_url, area.width);
    // Show the activity panel only when there's room for it.
    let mid_h = crate::options::QUICK.len() as u16 + 2;
    let room = area.height.saturating_sub(3 + ph);
    let mid_h = if room >= mid_h + 5 { mid_h } else { room };
    let [url_area, mid, activity, prev] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(mid_h),
        Constraint::Min(0),
        Constraint::Length(ph),
    ])
    .areas(area);

    // URL input
    let focused = app.dl_focus == Focus::Input;
    let count = app.url.value().split_whitespace().filter(|w| crate::ytdlp::is_url(w)).count();
    let mut block = panel("Link", focused).padding(Padding::horizontal(1));
    if count > 1 {
        block = block.title(Line::from(format!(" {count} links ")).right_aligned().fg(TEAL));
    }
    let inner = block.inner(url_area);
    f.render_widget(block, url_area);
    let prompt = Span::styled("› ", Style::new().fg(if focused { PINK } else { DIM }).bold());
    let text_w = inner.width.saturating_sub(2);
    if app.url.value().is_empty() {
        let ph = "Paste a YouTube, SoundCloud, Bandcamp… link, or type to search";
        f.render_widget(
            Paragraph::new(Line::from(vec![
                prompt,
                Span::styled(truncate(ph, text_w as usize), Style::new().fg(DIM).italic()),
            ])),
            inner,
        );
        if focused {
            f.set_cursor_position((inner.x + 2, inner.y));
        }
    } else {
        let (visible, cx) = app.url.view(text_w);
        f.render_widget(
            Paragraph::new(Line::from(vec![prompt, Span::styled(visible, Style::new().fg(TEXT))])),
            inner,
        );
        if focused {
            f.set_cursor_position((inner.x + 2 + cx, inner.y));
        }
    }

    // Quick options + info
    let [opts_area, info_area] =
        Layout::horizontal([Constraint::Percentage(56), Constraint::Percentage(44)])
            .spacing(1)
            .areas(mid);
    let fields = app.quick_fields();
    let sel = app.quick_sel.min(fields.len().saturating_sub(1));
    let list_focus = !focused;
    let block = panel("Quick options", list_focus)
        .title_bottom(Line::from(" p presets · 4 all options ").right_aligned().fg(DIM));
    let inner_w = block.inner(opts_area).width.saturating_sub(2);
    let items: Vec<ListItem> = fields
        .iter()
        .enumerate()
        .map(|(i, id)| option_row(&app.cfg, *id, list_focus && i == sel, 16, inner_w))
        .collect();
    let mut state = ListState::default();
    if list_focus {
        state.select(Some(sel));
    }
    let list = List::new(items)
        .block(block)
        .highlight_style(highlight())
        .highlight_symbol("▌ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    f.render_stateful_widget(list, opts_area, &mut state);

    draw_download_info(f, info_area, app, list_focus.then(|| fields.get(sel).copied()).flatten());
    draw_activity(f, activity, app);
    draw_preview(f, prev, &app.cfg, &first_url);
}

/// Live view of the queue on the Download tab.
fn draw_activity(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 3 {
        return;
    }
    let block = panel("Activity", false)
        .title(Line::from(" 3 full queue ").right_aligned().fg(DIM));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if app.jobs.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                "Downloads you queue show up here.",
                Style::new().fg(DIM).italic(),
            ))
            .alignment(Alignment::Center),
            centered(inner, inner.width, 1),
        );
        return;
    }
    // Running first, then waiting, then the most recent finished ones.
    let mut order: Vec<&Job> = app.jobs.iter().filter(|j| j.is_active()).collect();
    order.extend(app.jobs.iter().filter(|j| j.status == Status::Pending));
    order.extend(app.jobs.iter().rev().filter(|j| j.is_finished()));
    let fit = (inner.height as usize / 3).max(1);
    let w = inner.width.saturating_sub(1);
    let items: Vec<ListItem> = order.iter().take(fit).map(|j| job_item(app, j, w)).collect();
    f.render_widget(List::new(items), inner.inner(Margin { horizontal: 1, vertical: 0 }));
}

fn draw_download_info(f: &mut Frame, area: Rect, app: &App, field_id: Option<F>) {
    let mut lines: Vec<Line> = Vec::new();
    let title;
    if let Some(id) = field_id {
        let def = field(id);
        title = def.label.to_string();
        lines.push(Line::from(Span::styled(def.help, Style::new().fg(TEXT))));
    } else {
        title = "Summary".into();
        let c = &app.cfg;
        let fmt = match c.mode {
            Mode::Audio => {
                if c.audio_format.takes_quality() {
                    format!("{} · {}", c.audio_format.label(), c.audio_quality.label())
                } else {
                    c.audio_format.label().to_string()
                }
            }
            Mode::Video => format!(
                "{} · {} · {}",
                c.video_container.label(),
                c.max_height.label(),
                c.video_codec.label()
            ),
        };
        let row = |k: &str, v: String, color: Color| {
            Line::from(vec![
                Span::styled(format!("{k:<10}"), Style::new().fg(DIM)),
                Span::styled(v, Style::new().fg(color)),
            ])
        };
        lines.push(row("Saves as", fmt, TEXT));
        lines.push(row("Into", crate::ytdlp::contract_home(&expand(&c.output_dir).display().to_string()), BLUE));
        let mut extras = Vec::new();
        if c.embed_metadata {
            extras.push("tags");
        }
        if c.embed_thumbnail {
            extras.push(if c.mode == Mode::Audio && c.square_cover { "square cover" } else { "cover" });
        }
        if c.embed_chapters {
            extras.push("chapters");
        }
        if c.split_chapters {
            extras.push("split chapters");
        }
        if c.sponsorblock != crate::config::SponsorBlock::Off {
            extras.push("sponsorblock");
        }
        if c.use_archive {
            extras.push("archive");
        }
        let extras = if extras.is_empty() { "none".to_string() } else { extras.join(", ") };
        lines.push(row("Extras", extras, SUBTEXT));
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Tip ", Style::new().fg(YELLOW).bold()),
            Span::styled(
                "paste several links at once, or type words to search.",
                Style::new().fg(SUBTEXT),
            ),
        ]));

    }
    let block = panel(&title, false).padding(Padding::horizontal(1));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }).block(block), area);
}

// ---------- Search tab ----------

fn draw_search(f: &mut Frame, area: Rect, app: &App) {
    let s = &app.search;
    let [input_area, results_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).areas(area);

    let focused = s.focus == Focus::Input;
    let block = panel(&format!("Search {}", app.cfg.search_source.label()), focused)
        .padding(Padding::horizontal(1))
        .title(
            Line::from(format!(" {} results · source in Options ", app.cfg.search_count))
                .right_aligned()
                .fg(DIM),
        );
    let inner = block.inner(input_area);
    f.render_widget(block, input_area);
    let prompt = Span::styled("⌕ ", Style::new().fg(if focused { PINK } else { DIM }).bold());
    let text_w = inner.width.saturating_sub(2);
    if s.input.value().is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                prompt,
                Span::styled("Artist, song, album…", Style::new().fg(DIM).italic()),
            ])),
            inner,
        );
        if focused {
            f.set_cursor_position((inner.x + 2, inner.y));
        }
    } else {
        let (visible, cx) = s.input.view(text_w);
        f.render_widget(
            Paragraph::new(Line::from(vec![prompt, Span::styled(visible, Style::new().fg(TEXT))])),
            inner,
        );
        if focused {
            f.set_cursor_position((inner.x + 2 + cx, inner.y));
        }
    }

    let list_focus = !focused;
    let title = if s.query.is_empty() {
        "Results".to_string()
    } else {
        format!("Results for “{}”", truncate(&s.query, 40))
    };
    let mut block = panel(&title, list_focus && !s.results.is_empty());
    if !s.marked.is_empty() {
        block = block.title(
            Line::from(format!(" {} selected · enter to queue ", s.marked.len()))
                .right_aligned()
                .fg(TEAL),
        );
    }
    let inner = block.inner(results_area);

    if s.loading {
        f.render_widget(block, results_area);
        let msg = Line::from(vec![
            Span::styled(format!("{} ", spinner(app)), Style::new().fg(ACCENT)),
            Span::styled("Searching…", Style::new().fg(SUBTEXT)),
        ]);
        f.render_widget(Paragraph::new(msg).alignment(Alignment::Center), centered(inner, inner.width, 1));
        return;
    }
    if s.results.is_empty() {
        f.render_widget(block, results_area);
        let lines = if let Some(e) = &s.error {
            vec![Line::from(Span::styled(e.clone(), Style::new().fg(RED)))]
        } else {
            vec![
                Line::from(Span::styled("Search, pick, download.", Style::new().fg(SUBTEXT).bold())),
                Line::default(),
                Line::from(Span::styled(
                    "Type a query and press Enter. Select results with Space,",
                    Style::new().fg(DIM),
                )),
                Line::from(Span::styled(
                    "then press Enter to queue them with your current options.",
                    Style::new().fg(DIM),
                )),
            ]
        };
        let h = lines.len() as u16;
        f.render_widget(
            Paragraph::new(lines).alignment(Alignment::Center).wrap(Wrap { trim: true }),
            centered(inner, inner.width, h),
        );
        return;
    }

    let w = inner.width.saturating_sub(2);
    let items: Vec<ListItem> = s
        .results
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let marked = s.marked.contains(&i);
            let mark = if marked {
                Span::styled("● ", Style::new().fg(PINK))
            } else {
                Span::styled("○ ", Style::new().fg(SURFACE))
            };
            let mut right = Vec::new();
            if !r.channel.is_empty() {
                right.push(Span::styled(truncate(&r.channel, 24), Style::new().fg(SUBTEXT)));
            }
            if let Some(v) = r.views {
                right.push(Span::styled(format!("  {:>6}", fmt_count(v)), Style::new().fg(DIM)));
            }
            let dur = r.duration.map(fmt_duration).unwrap_or_default();
            right.push(Span::styled(format!("  {dur:>7} "), Style::new().fg(TEAL)));
            let title_style = if marked { Style::new().fg(PINK) } else { Style::new().fg(TEXT) };
            ListItem::new(spread(
                vec![mark, Span::styled(r.title.clone(), title_style)],
                right,
                w,
            ))
        })
        .collect();
    let mut state = ListState::default();
    if list_focus {
        state.select(Some(s.sel));
    }
    let list = List::new(items)
        .block(block)
        .highlight_style(highlight())
        .highlight_symbol("▌")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    f.render_stateful_widget(list, results_area, &mut state);
    scrollbar(f, results_area, s.results.len(), s.sel);
}

fn scrollbar(f: &mut Frame, area: Rect, len: usize, pos: usize) {
    if len as u16 + 2 <= area.height {
        return;
    }
    let mut st = ScrollbarState::new(len).position(pos);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .track_style(Style::new().fg(SURFACE))
            .thumb_symbol("┃")
            .thumb_style(Style::new().fg(ACCENT)),
        area.inner(Margin { horizontal: 0, vertical: 1 }),
        &mut st,
    );
}

// ---------- Queue tab ----------

fn status_icon(app: &App, job: &Job) -> (&'static str, Color) {
    match &job.status {
        Status::Pending => ("◷", DIM),
        Status::Running => match job.phase {
            Phase::Processing(_) => ("⚙", YELLOW),
            _ => (spinner(app), ACCENT),
        },
        Status::Done if job.last_error.is_some() => ("✔", YELLOW),
        Status::Done => ("✔", GREEN),
        Status::Failed(_) => ("✖", RED),
        Status::Cancelled => ("⊘", DIM),
    }
}

fn pp_name(name: &str) -> String {
    match name {
        "ExtractAudio" => "converting audio".into(),
        "Merger" => "merging".into(),
        "EmbedThumbnail" => "embedding cover".into(),
        "ThumbnailsConvertor" => "converting cover".into(),
        "Metadata" | "FFmpegMetadata" => "writing tags".into(),
        "SponsorBlock" => "fetching sponsor segments".into(),
        "ModifyChapters" => "cutting segments".into(),
        "SplitChapters" => "splitting chapters".into(),
        "EmbedSubtitle" => "embedding subtitles".into(),
        "VideoRemuxer" | "VideoConvertor" => "remuxing".into(),
        "MoveFiles" => "moving files".into(),
        n if n.starts_with("Fixup") => "fixing up".into(),
        n => n.to_lowercase(),
    }
}

fn bar(frac: f64, width: usize, color: Color) -> Vec<Span<'static>> {
    let filled = ((frac.clamp(0.0, 1.0)) * width as f64).round() as usize;
    vec![
        Span::styled("━".repeat(filled), Style::new().fg(color)),
        Span::styled("─".repeat(width - filled), Style::new().fg(SURFACE)),
    ]
}

fn job_item(app: &App, job: &Job, width: u16) -> ListItem<'static> {
    let (icon, icon_color) = status_icon(app, job);
    let elapsed = job.elapsed().map(fmt_duration).unwrap_or_default();
    let (status_text, status_color) = match &job.status {
        Status::Pending => ("queued".to_string(), DIM),
        Status::Running => match &job.phase {
            Phase::Starting => ("starting…".into(), SUBTEXT),
            Phase::Downloading => (
                if job.cfg.mode == Mode::Video && job.progress.audio_stream {
                    "downloading audio".into()
                } else {
                    "downloading".into()
                },
                ACCENT,
            ),
            Phase::Processing(n) => (pp_name(n), YELLOW),
        },
        Status::Done => {
            let n = job.files.len();
            let files = match n {
                0 => "nothing new".to_string(),
                1 => "1 file".into(),
                n => format!("{n} files"),
            };
            (format!("{files} · {elapsed}"), if job.last_error.is_some() { YELLOW } else { GREEN })
        }
        Status::Failed(_) => ("failed".into(), RED),
        Status::Cancelled => ("cancelled".into(), DIM),
    };
    let title_style = match job.status {
        Status::Done | Status::Cancelled => Style::new().fg(SUBTEXT),
        _ => Style::new().fg(TEXT).bold(),
    };
    let line1 = spread(
        vec![
            Span::styled(format!("{icon} "), Style::new().fg(icon_color)),
            Span::styled(job.display_title().to_string(), title_style),
        ],
        vec![Span::styled(format!("{status_text} "), Style::new().fg(status_color))],
        width,
    );

    let bar_w = (width as usize).saturating_sub(48).clamp(10, 50);
    let indent = Span::raw("  ");
    let line2 = match &job.status {
        Status::Running => {
            let p = &job.progress;
            let mut spans = vec![indent];
            spans.extend(bar(job.fraction(), bar_w, ACCENT));
            spans.push(Span::styled(
                format!(" {:>3.0}%", job.fraction() * 100.0),
                Style::new().fg(TEXT).bold(),
            ));
            if let (Some(i), Some(n)) = (p.index, p.count)
                && n > 1 {
                    spans.push(Span::styled(format!("  {i}/{n}"), Style::new().fg(PINK)));
                }
            if matches!(job.phase, Phase::Downloading) {
                if let Some(t) = p.total {
                    spans.push(Span::styled(format!("  {}", fmt_bytes(t)), Style::new().fg(SUBTEXT)));
                }
                if let Some(s) = p.speed {
                    spans.push(Span::styled(format!("  {}/s", fmt_bytes(s)), Style::new().fg(TEAL)));
                }
                if let Some(e) = p.eta {
                    spans.push(Span::styled(format!("  eta {}", fmt_duration(e)), Style::new().fg(DIM)));
                }
            }
            Line::from(spans)
        }
        Status::Pending => Line::from(vec![
            indent,
            Span::styled(format!("waiting · {}", job_format(&job.cfg)), Style::new().fg(DIM)),
        ]),
        Status::Done => {
            let mut spans = vec![indent];
            spans.extend(bar(1.0, bar_w, if job.last_error.is_some() { YELLOW } else { GREEN }));
            if let Some(f) = job.files.last() {
                let name = std::path::Path::new(f)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                spans.push(Span::styled(
                    format!("  {}", truncate(&name, (width as usize).saturating_sub(bar_w + 6))),
                    Style::new().fg(DIM),
                ));
            }
            Line::from(spans)
        }
        Status::Failed(e) => Line::from(vec![
            indent,
            Span::styled(truncate(e, (width as usize).saturating_sub(4)), Style::new().fg(RED)),
        ]),
        Status::Cancelled => Line::from(vec![
            indent,
            Span::styled("stopped · partial .part files may remain", Style::new().fg(DIM)),
        ]),
    };
    ListItem::new(Text::from(vec![line1, line2, Line::default()]))
}

fn job_format(c: &Config) -> String {
    match c.mode {
        Mode::Audio => c.audio_format.label().to_string(),
        Mode::Video => format!("{} {}", c.video_container.label(), c.max_height.label()),
    }
}

fn draw_queue(f: &mut Frame, area: Rect, app: &App) {
    if app.jobs.is_empty() {
        let block = panel("Queue", false);
        let inner = block.inner(area);
        f.render_widget(block, area);
        let lines = vec![
            Line::from(Span::styled("Nothing queued yet", Style::new().fg(SUBTEXT).bold())),
            Line::default(),
            Line::from(vec![
                Span::styled("Paste a link in ", Style::new().fg(DIM)),
                Span::styled("1 Download", Style::new().fg(ACCENT)),
                Span::styled(" or find music in ", Style::new().fg(DIM)),
                Span::styled("2 Search", Style::new().fg(ACCENT)),
            ]),
        ];
        f.render_widget(
            Paragraph::new(lines).alignment(Alignment::Center),
            centered(inner, inner.width, 3),
        );
        return;
    }

    let wide = area.width >= 130;
    let [list_area, detail_area] = if wide {
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .spacing(1)
            .areas(area)
    } else {
        Layout::vertical([Constraint::Min(6), Constraint::Percentage(40)]).areas(area)
    };

    let done = app.jobs.iter().filter(|j| j.status == Status::Done).count();
    let failed = app.jobs.iter().filter(|j| matches!(j.status, Status::Failed(_))).count();
    let mut summary = format!(
        " {} active · {} waiting · {} done ",
        app.active_count(),
        app.pending_count(),
        done
    );
    if failed > 0 {
        summary.push_str(&format!("· {failed} failed "));
    }
    let title = if app.paused { "Queue (paused)" } else { "Queue" };
    let block = panel(title, true).title(Line::from(summary).right_aligned().fg(DIM));
    let inner_w = block.inner(list_area).width.saturating_sub(2);
    let sel = app.queue_sel.min(app.jobs.len() - 1);
    let items: Vec<ListItem> = app.jobs.iter().map(|j| job_item(app, j, inner_w)).collect();
    let mut state = ListState::default().with_selected(Some(sel));
    let list = List::new(items)
        .block(block)
        .highlight_style(highlight())
        .highlight_symbol("▌")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    f.render_stateful_widget(list, list_area, &mut state);

    draw_job_detail(f, detail_area, &app.jobs[sel]);
}

fn draw_job_detail(f: &mut Frame, area: Rect, job: &Job) {
    let block = panel("Details", false).padding(Padding::horizontal(1));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width as usize;
    let row = |k: &str, v: String, color: Color| {
        Line::from(vec![
            Span::styled(format!("{k:<10}"), Style::new().fg(DIM)),
            Span::styled(truncate(&v, w.saturating_sub(10)), Style::new().fg(color)),
        ])
    };
    let mut lines = vec![
        row("Link", job.url.clone(), TEAL),
        row("Format", job_format(&job.cfg), TEXT),
        row("Folder", crate::ytdlp::contract_home(&expand(&job.cfg.output_dir).display().to_string()), BLUE),
    ];
    if let Some(e) = &job.last_error {
        lines.push(row("Error", e.clone(), RED));
    }
    if job.warnings > 0 {
        lines.push(row("Warnings", job.warnings.to_string(), YELLOW));
    }
    lines.push(Line::from(Span::styled("─".repeat(w), Style::new().fg(SURFACE))));
    let room = (inner.height as usize).saturating_sub(lines.len());
    let skip = job.log.len().saturating_sub(room);
    for (line, err) in job.log.iter().skip(skip) {
        let color = if line.starts_with("ERROR") {
            RED
        } else if line.starts_with("WARNING") {
            YELLOW
        } else if line.starts_with("$ ") || line.starts_with("──") {
            DIM
        } else if line.starts_with("Saved: ") {
            GREEN
        } else if *err {
            SUBTEXT
        } else {
            TEXT
        };
        lines.push(Line::from(Span::styled(truncate(line, w), Style::new().fg(color))));
    }
    if job.log.is_empty() {
        lines.push(Line::from(Span::styled("No output yet", Style::new().fg(DIM).italic())));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

// ---------- Options tab ----------

fn draw_options(f: &mut Frame, area: Rect, app: &App) {
    let [list_area, side] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
            .spacing(1)
            .areas(area);

    let block = panel("Options", true)
        .title(Line::from(" saved automatically ").right_aligned().fg(DIM));
    let inner_w = block.inner(list_area).width.saturating_sub(2);
    let mut items = Vec::new();
    let mut sel_row = 0;
    let mut last_section = "";
    for (i, def) in FIELDS.iter().enumerate() {
        if def.section != last_section {
            if !last_section.is_empty() {
                items.push(ListItem::new(Line::default()));
            }
            items.push(ListItem::new(Line::from(Span::styled(
                def.section.to_uppercase(),
                Style::new().fg(PINK).bold(),
            ))));
            last_section = def.section;
        }
        if i == app.opt_sel {
            sel_row = items.len();
        }
        items.push(option_row(&app.cfg, def.id, i == app.opt_sel, 24, inner_w));
    }
    let total_rows = items.len();
    let mut state = ListState::default().with_selected(Some(sel_row));
    let list = List::new(items)
        .block(block)
        .highlight_style(highlight())
        .highlight_symbol("▌ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    f.render_stateful_widget(list, list_area, &mut state);
    scrollbar(f, list_area, total_rows, sel_row);

    // Side: help + preview
    let ph = preview_height(&app.cfg, "<URL>", side.width);
    let [help_area, prev_area] =
        Layout::vertical([Constraint::Min(6), Constraint::Length(ph)]).areas(side);
    let def = &FIELDS[app.opt_sel];
    let default_cfg = Config::default();
    let mut lines = vec![
        Line::from(Span::styled(def.help, Style::new().fg(TEXT))),
        Line::default(),
        Line::from(vec![
            Span::styled("Current  ", Style::new().fg(DIM)),
            Span::styled(
                {
                    let v = app.cfg.display(def.id);
                    if v.is_empty() { "(empty)".into() } else { v }
                },
                Style::new().fg(BLUE),
            ),
        ]),
        Line::from(vec![
            Span::styled("Default  ", Style::new().fg(DIM)),
            Span::styled(
                {
                    let v = default_cfg.display(def.id);
                    if v.is_empty() { "(empty)".into() } else { v }
                },
                Style::new().fg(SUBTEXT),
            ),
        ]),
    ];
    if !app.cfg.relevant(def.id) {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "▲ Not used with the current settings",
            Style::new().fg(YELLOW),
        )));
    }
    let block = panel(def.label, false).padding(Padding::horizontal(1));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }).block(block), help_area);
    draw_preview(f, prev_area, &app.cfg, "<URL>");
}

// ---------- popups ----------

fn popup_block(title: &str) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(PINK))
        .title(Span::styled(format!(" {title} "), Style::new().fg(PINK).bold()))
        .padding(Padding::new(2, 2, 1, 0))
}

fn draw_help(f: &mut Frame, _app: &App) {
    let sections: &[(&str, &[(&str, &str)])] = &[
        ("Everywhere", &[
            ("Tab / Shift+Tab", "next / previous tab"),
            ("1-4, F1-F4, Alt+1-4", "jump to tab"),
            ("Ctrl+V", "paste clipboard (or terminal paste)"),
            ("?", "this help"),
            ("q, Ctrl+C", "quit"),
        ]),
        ("Download", &[
            ("Enter", "queue the link(s); plain text searches"),
            ("↓ / Esc", "go to quick options"),
            ("←→ Space", "change option · Enter edits text"),
            ("p / r", "presets / reset option"),
        ]),
        ("Search", &[
            ("Enter", "search · queue selection"),
            ("Space / a", "select one / all"),
            ("o", "open in browser"),
        ]),
        ("Queue", &[
            ("c / r / d", "cancel / retry / remove"),
            ("R / C", "retry all failed / clear finished"),
            ("o, Enter", "open download folder"),
            ("p", "pause or resume the queue"),
            ("J / K", "move item down / up"),
        ]),
        ("Text fields", &[
            ("Ctrl+W, Ctrl+U", "delete word / to start"),
            ("Ctrl+←→, Home/End", "move by word / to ends"),
            ("Tab", "complete folder path"),
        ]),
    ];
    let render = |secs: &[(&str, &[(&str, &str)])]| {
        let mut lines = Vec::new();
        for (title, keys) in secs {
            lines.push(Line::from(Span::styled(title.to_string(), Style::new().fg(ACCENT).bold())));
            for (k, d) in *keys {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {k:<20}"), Style::new().fg(TEXT)),
                    Span::styled(d.to_string(), Style::new().fg(SUBTEXT)),
                ]));
            }
            lines.push(Line::default());
        }
        lines
    };
    if f.area().width >= 124 {
        let (a, b) = sections.split_at(3);
        let (left, mut right) = (render(a), render(b));
        right.push(Line::from(Span::styled("press any key to close", Style::new().fg(DIM))));
        let h = left.len().max(right.len()) as u16 + 3;
        let area = centered(f.area(), 120, h);
        f.render_widget(Clear, area);
        let block = popup_block("Keys");
        let inner = block.inner(area);
        f.render_widget(block, area);
        let [l, r] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
            .spacing(2)
            .areas(inner);
        f.render_widget(Paragraph::new(left), l);
        f.render_widget(Paragraph::new(right), r);
        return;
    }
    let mut lines = Vec::new();
    for (title, keys) in sections {
        lines.push(Line::from(Span::styled(*title, Style::new().fg(ACCENT).bold())));
        for (k, d) in *keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {k:<22}"), Style::new().fg(TEXT)),
                Span::styled(d.to_string(), Style::new().fg(SUBTEXT)),
            ]));
        }
        lines.push(Line::default());
    }
    lines.push(Line::from(Span::styled("press any key to close", Style::new().fg(DIM))));
    let h = lines.len() as u16 + 3;
    let area = centered(f.area(), 66, h);
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(popup_block("Keys")), area);
}

fn draw_edit(f: &mut Frame, app: &App) {
    let Some(Popup::Edit(e)) = &app.popup else { return };
    let def = field(e.field);
    let comp_rows = if e.completions.is_empty() { 0 } else { (e.completions.len().min(6) + 1) as u16 };
    let width = (f.area().width * 7 / 10).clamp(50, 100);
    let help_rows = (def.help.width() as u16 / width.saturating_sub(6).max(1)) + 1;
    let area = centered(f.area(), width, 6 + help_rows + comp_rows);
    f.render_widget(Clear, area);
    let block = popup_block(def.label).title_bottom(
        Line::from(if def.kind == Kind::Path {
            " enter save · esc cancel · tab complete "
        } else {
            " enter save · esc cancel "
        })
        .right_aligned()
        .fg(DIM),
    );
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [help, input, comps] = Layout::vertical([
        Constraint::Length(help_rows + 1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(inner);
    f.render_widget(
        Paragraph::new(Span::styled(def.help, Style::new().fg(SUBTEXT))).wrap(Wrap { trim: true }),
        help,
    );
    let (visible, cx) = e.input.view(input.width.saturating_sub(2));
    let line = if e.input.value().is_empty() {
        Line::from(vec![
            Span::styled("› ", Style::new().fg(PINK).bold()),
            Span::styled(def.placeholder, Style::new().fg(DIM).italic()),
        ])
    } else {
        Line::from(vec![
            Span::styled("› ", Style::new().fg(PINK).bold()),
            Span::styled(visible, Style::new().fg(TEXT)),
        ])
    };
    f.render_widget(Paragraph::new(line).bg(SURFACE), input);
    f.set_cursor_position((input.x + 2 + cx, input.y));
    if !e.completions.is_empty() {
        let mut lines = vec![Line::default()];
        for c in e.completions.iter().take(6) {
            lines.push(Line::from(Span::styled(format!("  {c}/"), Style::new().fg(BLUE))));
        }
        if e.completions.len() > 6
            && let Some(last) = lines.last_mut() {
                *last = Line::from(Span::styled(
                    format!("  … {} more", e.completions.len() - 5),
                    Style::new().fg(DIM),
                ));
            }
        f.render_widget(Paragraph::new(lines), comps);
    }
}

fn draw_presets(f: &mut Frame, sel: usize) {
    let h = PRESETS.len() as u16 * 2 + 4;
    let area = centered(f.area(), 64, h);
    f.render_widget(Clear, area);
    let block = popup_block("Presets")
        .title_bottom(Line::from(" enter apply · esc close ").right_aligned().fg(DIM))
        .padding(Padding::new(1, 1, 1, 0));
    let items: Vec<ListItem> = PRESETS
        .iter()
        .map(|p| {
            ListItem::new(Text::from(vec![
                Line::from(Span::styled(p.name, Style::new().fg(TEXT).bold())),
                Line::from(Span::styled(format!("  {}", p.desc), Style::new().fg(DIM))),
            ]))
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(sel));
    let list = List::new(items)
        .block(block)
        .highlight_style(highlight())
        .highlight_symbol("▌ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_confirm_quit(f: &mut Frame, app: &App) {
    let (a, p) = (app.active_count(), app.pending_count());
    let area = centered(f.area(), 52, 8);
    f.render_widget(Clear, area);
    let what = match (a, p) {
        (a, 0) => format!("{a} download(s) still running."),
        (0, p) => format!("{p} download(s) still waiting."),
        (a, p) => format!("{a} running and {p} waiting."),
    };
    let lines = vec![
        Line::from(Span::styled(what, Style::new().fg(TEXT))),
        Line::from(Span::styled("Quitting stops them.", Style::new().fg(SUBTEXT))),
        Line::default(),
        Line::from(vec![
            Span::styled(" y ", Style::new().fg(BASE).bg(RED).bold()),
            Span::styled(" quit   ", Style::new().fg(SUBTEXT)),
            Span::styled(" any key ", Style::new().fg(BASE).bg(SURFACE).bold()),
            Span::styled(" stay", Style::new().fg(SUBTEXT)),
        ]),
    ];
    f.render_widget(Paragraph::new(lines).block(popup_block("Quit?")), area);
}
