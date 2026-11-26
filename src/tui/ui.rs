//! Rendering ✧ everything pink ✧

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table,
        Wrap,
    },
};

use super::app::{App, Content, Focus, Section, fmt_ms};
use super::theme::*;

const LOGO: &str = "♡ f e m b o y   s p o t i f y ♡";

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Block::default().style(Style::default().bg(BG).fg(WHITE)), area);

    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(5),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(f, app, rows[0]);
    let cols = Layout::horizontal([Constraint::Length(26), Constraint::Min(20)]).split(rows[1]);
    draw_sidebar(f, app, cols[0]);
    draw_main(f, app, cols[1]);
    draw_player(f, app, rows[2]);
    draw_hints(f, rows[3]);

    if app.show_help {
        draw_help(f, area);
    }
}

fn block(title: &str, focused: bool) -> Block<'static> {
    let (border, text) = if focused { (HOT_PINK, PASTEL) } else { (DIM, DIM) };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(border))
        .title(Span::styled(format!(" {title} "), bold(text)))
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(PINK));
    let inner = b.inner(area);
    f.render_widget(b, area);

    // the logo shimmers through the femboy flag colors
    let shift = (app.tick / 2) as usize;
    let logo: Vec<Span> = LOGO
        .chars()
        .enumerate()
        .map(|(i, c)| Span::styled(c.to_string(), bold(FLAG[(i + shift) % FLAG.len()])))
        .collect();

    let sparkle = ["✧", "✦", "⋆", "✦"][(app.tick / 3) as usize % 4];
    let user = if app.user.is_empty() {
        String::new()
    } else {
        format!("{sparkle} hi, {} {sparkle} ", app.user)
    };

    let parts = Layout::horizontal([Constraint::Length(36), Constraint::Min(0)]).split(inner);
    f.render_widget(Paragraph::new(Line::from(logo)), parts[0]);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(user, fg(LAVENDER)))).alignment(Alignment::Right),
        parts[1],
    );
}

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let parts = Layout::vertical([Constraint::Length(5), Constraint::Min(3)]).split(area);

    let items: Vec<ListItem> = Section::ALL
        .iter()
        .map(|s| {
            let style = if *s == app.section { bold(HOT_PINK) } else { fg(WHITE) };
            ListItem::new(Span::styled(s.label(), style))
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(app.section_index()));
    let list = List::new(items)
        .block(block("menu", app.focus == Focus::Sidebar))
        .highlight_style(if app.focus == Focus::Sidebar { selected() } else { Style::default() })
        .highlight_symbol("❥ ");
    f.render_stateful_widget(list, parts[0], &mut state);

    // up next
    let mut lines: Vec<Line> = Vec::new();
    if let Some(pos) = app.queue_pos {
        let room = parts[1].height.saturating_sub(2) as usize;
        for (i, t) in app.queue.iter().enumerate().skip(pos + 1).take(room) {
            let style = if i % 2 == 0 { fg(PASTEL) } else { fg(PINK) };
            lines.push(Line::from(Span::styled(format!("· {}", t.name), style)));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("  (˶ᵔ ᵕ ᵔ˶)", fg(PINK))));
        lines.push(Line::from(Span::styled("  queue is empty", fg(DIM))));
    }
    let title = if app.shuffle { "up next (shuffle)" } else { "up next" };
    f.render_widget(Paragraph::new(lines).block(block(title, false)), parts[1]);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let mut area = area;
    if app.section == Section::Search {
        let parts = Layout::vertical([Constraint::Length(3), Constraint::Min(3)]).split(area);
        draw_search_box(f, app, parts[0]);
        area = parts[1];
    }

    let focused = app.focus == Focus::Main;
    let title = app.title.clone();

    let empty = match app.content {
        Content::Tracks => app.tracks.is_empty(),
        Content::Playlists => app.playlists.is_empty(),
    };
    if empty {
        let msg = if app.loading {
            vec![
                Line::from(""),
                Line::from(Span::styled("  loading... ₍ᐢ. .ᐢ₎", fg(PINK))),
            ]
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled("  nothing here yet~", fg(PINK))),
                Line::from(Span::styled("  press / to search, tab to open the menu ♡", fg(DIM))),
            ]
        };
        f.render_widget(Paragraph::new(msg).block(block(&title, focused)), area);
        return;
    }

    match app.content {
        Content::Tracks => draw_tracks(f, app, area, &title, focused),
        Content::Playlists => draw_playlists(f, app, area, &title, focused),
    }
}

fn draw_search_box(f: &mut Frame, app: &App, area: Rect) {
    let typing = app.focus == Focus::Input;
    let cursor = if typing && app.tick % 4 < 2 { "▏" } else { " " };
    let text = if app.input.is_empty() && !typing {
        Line::from(Span::styled("press / and type a song, artist or album...", fg(DIM)))
    } else {
        Line::from(vec![
            Span::styled("✿ ", fg(HOT_PINK)),
            Span::styled(app.input.clone(), bold(WHITE)),
            Span::styled(cursor, fg(HOT_PINK)),
        ])
    };
    f.render_widget(Paragraph::new(text).block(block("search", typing)), area);
}

fn draw_tracks(f: &mut Frame, app: &mut App, area: Rect, title: &str, focused: bool) {
    let playing_uri = app.now_playing().map(|t| t.uri.clone());
    let rows: Vec<Row> = app
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_playing = playing_uri.as_deref() == Some(t.uri.as_str());
            let (num, style) = if is_playing {
                ("♪".to_string(), bold(HOT_PINK))
            } else {
                ((i + 1).to_string(), fg(WHITE))
            };
            Row::new(vec![
                Cell::from(Span::styled(num, fg(DIM))),
                Cell::from(t.name.clone()),
                Cell::from(Span::styled(t.artists.clone(), fg(PASTEL))),
                Cell::from(Span::styled(t.album.clone(), fg(LAVENDER))),
                Cell::from(Span::styled(fmt_ms(t.duration_ms), fg(DIM))),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
            Constraint::Percentage(30),
            Constraint::Length(6),
        ],
    )
    .header(
        Row::new(vec!["#", "title", "artist", "album", "time"])
            .style(bold(HOT_PINK))
            .bottom_margin(1),
    )
    .block(block(title, focused))
    .row_highlight_style(selected())
    .highlight_symbol("❥ ");
    f.render_stateful_widget(table, area, &mut app.track_state);
}

fn draw_playlists(f: &mut Frame, app: &mut App, area: Rect, title: &str, focused: bool) {
    let items: Vec<ListItem> = app
        .playlists
        .iter()
        .map(|p| {
            ListItem::new(Line::from(vec![
                Span::styled(p.name.clone(), bold(WHITE)),
                Span::styled(format!("  · {} · {} tracks", p.owner, p.total), fg(DIM)),
            ]))
        })
        .collect();
    let list = List::new(items)
        .block(block(title, focused))
        .highlight_style(selected())
        .highlight_symbol("❥ ");
    f.render_stateful_widget(list, area, &mut app.playlist_state);
}

fn draw_player(f: &mut Frame, app: &App, area: Rect) {
    let b = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(fg(PINK))
        .title(Span::styled(" now playing ", bold(PASTEL)));
    let inner = b.inner(area);
    f.render_widget(b, area);
    let rows = Layout::vertical([Constraint::Length(1); 3]).split(inner);

    let Some(track) = app.now_playing() else {
        f.render_widget(
            Paragraph::new(Span::styled("silence... pick a song and press enter ♡", fg(DIM))),
            rows[0],
        );
        draw_volume(f, app, rows[2]);
        return;
    };

    let icon = if app.buffering {
        "◌"
    } else if app.playing {
        "▶"
    } else {
        "❚❚"
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{icon} "), bold(HOT_PINK)),
            Span::styled(track.name.clone(), bold(WHITE)),
            Span::styled(" — ", fg(DIM)),
            Span::styled(track.artists.clone(), fg(PASTEL)),
        ])),
        rows[0],
    );

    let pos = app.position_ms();
    let ratio = if track.duration_ms > 0 {
        (pos as f64 / track.duration_ms as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(HOT_PINK).bg(BG_SEL))
        .ratio(ratio)
        .label(Span::styled(
            format!("{} / {}", fmt_ms(pos), fmt_ms(track.duration_ms)),
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ));
    f.render_widget(gauge, rows[1]);
    draw_volume(f, app, rows[2]);
}

fn draw_volume(f: &mut Frame, app: &App, area: Rect) {
    let filled = (app.volume as usize + 5) / 10;
    let mut spans = vec![Span::styled("vol ", fg(DIM))];
    spans.push(Span::styled("♥".repeat(filled), fg(HOT_PINK)));
    spans.push(Span::styled("♡".repeat(10 - filled.min(10)), fg(DIM)));
    spans.push(Span::styled(format!(" {}%", app.volume), fg(DIM)));
    if app.shuffle {
        spans.push(Span::styled("   ⤨ shuffle", fg(LAVENDER)));
    }
    spans.push(Span::styled(format!("   {}", app.status), fg(PINK)));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_hints(f: &mut Frame, area: Rect) {
    let keys = [
        ("/", "search"),
        ("⏎", "play"),
        ("space", "pause"),
        ("n/p", "next/prev"),
        ("←/→", "seek"),
        ("+/-", "vol"),
        ("s", "shuffle"),
        ("tab", "menu"),
        ("?", "help"),
        ("q", "quit"),
    ];
    let mut spans = Vec::new();
    for (k, v) in keys {
        spans.push(Span::styled(format!(" {k}"), bold(HOT_PINK)));
        spans.push(Span::styled(format!(" {v} "), fg(DIM)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let w = 52.min(area.width);
    let h = 20.min(area.height);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    let rows = [
        ("/", "search for tracks"),
        ("enter", "play track / open playlist"),
        ("space", "pause / resume"),
        ("n / p", "next / previous track"),
        ("← / →", "seek 10 seconds"),
        ("+ / -", "volume up / down"),
        ("s", "toggle shuffle"),
        ("tab", "switch between menu and list"),
        ("↑↓ / j k", "move"),
        ("esc", "back to playlists"),
        ("q / ctrl+c", "quit"),
    ];
    let mut lines = vec![
        Line::from(Span::styled("  ♡ how to use me ♡", bold(HOT_PINK))),
        Line::from(""),
    ];
    for (k, v) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:<12}"), bold(PINK)),
            Span::styled(v, fg(WHITE)),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("  press any key to close~", fg(DIM))));

    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(BG))
            .block(block("help", true)),
        rect,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::Track;
    use ratatui::{Terminal, backend::TestBackend};

    fn fake_app() -> App {
        let mut app = App::new(70);
        app.user = "cutie".into();
        app.on_msg(crate::tui::app::Msg::Tracks {
            title: "♥ liked songs".into(),
            tracks: (1..=5)
                .map(|i| Track {
                    uri: format!("spotify:track:{i}"),
                    name: format!("song number {i}"),
                    artists: "some artist".into(),
                    album: "pink album".into(),
                    duration_ms: 180_000,
                    ..Default::default()
                })
                .collect(),
        });
        app
    }

    #[test]
    fn renders_without_panicking() {
        for (w, h) in [(120, 30), (60, 16), (20, 8)] {
            let mut app = fake_app();
            let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
            term.draw(|f| draw(f, &mut app)).unwrap();
            app.show_help = true;
            term.draw(|f| draw(f, &mut app)).unwrap();
        }
    }

    #[test]
    fn dump_screen() {
        let mut app = fake_app();
        app.queue = app.tracks.clone();
        app.queue_pos = Some(1);
        app.playing = true;
        let mut term = Terminal::new(TestBackend::new(110, 24)).unwrap();
        term.draw(|f| draw(f, &mut app)).unwrap();
        let buf = term.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        println!("{out}");
    }
}
