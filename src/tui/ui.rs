use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Gauge, List, ListItem, Paragraph},
    Frame,
};

use super::app::{App, DlState, Screen};

const BRAND: &str = " rustgram ";
const ACCENT: Color = Color::Cyan;
const DIM: Color = Color::DarkGray;
const ERR: Color = Color::Red;
const OK: Color = Color::Green;

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    // Outer 3-row layout: header | content | footer
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(3),
    ])
    .split(area);

    render_header(f, app, chunks[0]);
    render_content(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);
}

// ── Header ────────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let subtitle = match &app.screen {
        Screen::Connecting => "connecting…",
        Screen::LoginPhone => "login — phone",
        Screen::LoginCode => "login — verification code",
        Screen::LoginPassword => "login — 2FA password",
        Screen::Chats => "chats",
        Screen::DownloadConfig => "download — configure",
        Screen::Downloading => "download — in progress",
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let cols = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).split(inner);

    f.render_widget(
        Paragraph::new(BRAND).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        cols[0],
    );
    f.render_widget(
        Paragraph::new(subtitle)
            .alignment(Alignment::Right)
            .style(Style::default().fg(DIM)),
        cols[1],
    );
}

// ── Footer ────────────────────────────────────────────────────────────────────

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let help = match &app.screen {
        Screen::Connecting => "  ctrl+c: quit",
        Screen::LoginPhone | Screen::LoginCode | Screen::LoginPassword => {
            "  Enter: confirm   Esc: back   ctrl+c: quit"
        }
        Screen::Chats => {
            "  ↑↓: navigate   Enter: open   /: filter   Esc/q: quit"
        }
        Screen::DownloadConfig => {
            "  Enter/d: start download   t: cycle media type   Esc/q: back"
        }
        Screen::Downloading => {
            if app.dl_finished {
                "  ↑↓: scroll   Esc/q: back to chats"
            } else {
                "  ↑↓: scroll   (downloading…)"
            }
        }
    };

    // Show status / error message if any
    let status_line = if let Some((msg, is_err)) = &app.status {
        let color = if *is_err { ERR } else { OK };
        Line::from(vec![
            Span::raw("  "),
            Span::styled(msg, Style::default().fg(color)),
        ])
    } else {
        Line::from(Span::styled(help, Style::default().fg(DIM)))
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM));

    let inner = block.inner(area);
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(status_line), inner);
}

// ── Content dispatcher ────────────────────────────────────────────────────────

fn render_content(f: &mut Frame, app: &App, area: Rect) {
    match app.screen {
        Screen::Connecting => render_connecting(f, area),
        Screen::LoginPhone => render_login_input(f, app, area, "Phone number", false),
        Screen::LoginCode => render_login_input(f, app, area, "Verification code", false),
        Screen::LoginPassword => render_login_input(f, app, area, "2FA Password", true),
        Screen::Chats => render_chats(f, app, area),
        Screen::DownloadConfig => render_download_config(f, app, area),
        Screen::Downloading => render_downloading(f, app, area),
    }
}

// ── Connecting ────────────────────────────────────────────────────────────────

fn render_connecting(f: &mut Frame, area: Rect) {
    let p = Paragraph::new("Connecting to Telegram…")
        .alignment(Alignment::Center)
        .style(Style::default().fg(DIM));
    f.render_widget(p, center_vertical(1, area));
}

// ── Login input ───────────────────────────────────────────────────────────────

fn render_login_input(f: &mut Frame, app: &App, area: Rect, label: &str, masked: bool) {
    // Center a box in the screen
    let box_area = centered_rect(50, 5, area);

    let display = if masked {
        "•".repeat(app.input.len())
    } else {
        app.input.clone()
    };
    // Append a blinking cursor character
    let with_cursor = format!("{}_", display);

    let block = Block::default()
        .title(format!(" {label} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(box_area);
    f.render_widget(block, box_area);
    f.render_widget(
        Paragraph::new(with_cursor).style(Style::default().fg(Color::White)),
        inner,
    );
}

// ── Chat list ─────────────────────────────────────────────────────────────────

fn render_chats(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([
        Constraint::Length(3), // filter bar
        Constraint::Fill(1),   // list
    ])
    .split(area);

    // Filter bar
    let filter_text = if app.filter_active {
        format!(" /{}_", app.filter)
    } else if app.filter.is_empty() {
        "  / to filter…".into()
    } else {
        format!(" /{} (active)", app.filter)
    };

    let filter_style = if app.filter_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(DIM)
    };

    f.render_widget(
        Paragraph::new(filter_text)
            .style(filter_style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(DIM)),
            ),
        chunks[0],
    );

    // Chat list
    if app.chats_loading {
        f.render_widget(
            Paragraph::new("Loading chats…")
                .alignment(Alignment::Center)
                .style(Style::default().fg(DIM)),
            center_vertical(1, chunks[1]),
        );
        return;
    }

    let items: Vec<ListItem> = app
        .filtered_chats()
        .map(|c| {
            let kind_span = Span::styled(
                format!("{:<9}", c.kind.label()),
                Style::default().fg(kind_color(&c.kind)),
            );
            let name_span = Span::raw(&c.name);
            ListItem::new(Line::from(vec![kind_span, name_span]))
        })
        .collect();

    if items.is_empty() {
        f.render_widget(
            Paragraph::new("No chats found.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(DIM)),
            center_vertical(1, chunks[1]),
        );
        return;
    }

    let mut state = app.list_state.clone();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM)),
        )
        .highlight_style(
            Style::default()
                .bg(ACCENT)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, chunks[1], &mut state);
}

fn kind_color(kind: &super::app::ChatKind) -> Color {
    match kind {
        super::app::ChatKind::Private => Color::Blue,
        super::app::ChatKind::Group => Color::Green,
        super::app::ChatKind::Channel => Color::Magenta,
    }
}

// ── Download config ───────────────────────────────────────────────────────────

fn render_download_config(f: &mut Frame, app: &App, area: Rect) {
    let chat = match &app.selected_chat {
        Some(c) => c,
        None => return,
    };

    let output_path = app.output_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| {
        let base = dirs::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        base.join("rustgram")
            .join(crate::media::sanitize_name(&chat.name))
            .display()
            .to_string()
    });

    let box_area = centered_rect(60, 10, area);

    let block = Block::default()
        .title(format!(" {} ", chat.name))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT));

    let inner = block.inner(box_area);
    f.render_widget(block, box_area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Media type : ", Style::default().fg(DIM)),
            Span::styled(
                app.media_filter.label(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled("  (press t to cycle)", Style::default().fg(DIM)),
        ])),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Output dir : ", Style::default().fg(DIM)),
            Span::raw(&output_path),
        ])),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(
            Line::from(Span::styled(
                "Press Enter or d to start downloading",
                Style::default().fg(OK),
            ))
        ),
        rows[5],
    );
}

// ── Downloading ───────────────────────────────────────────────────────────────

fn render_downloading(f: &mut Frame, app: &App, area: Rect) {
    let chat_name = app
        .selected_chat
        .as_ref()
        .map(|c| c.name.as_str())
        .unwrap_or("Download");

    let chunks = Layout::vertical([
        Constraint::Length(3), // stats bar
        Constraint::Fill(1),   // file list
    ])
    .split(area);

    // Stats bar
    let (done, active, failed, bytes_done, bytes_total) = app.dl_stats();
    let stats = format!(
        "  {} done  {} active  {} failed  |  {}/{}",
        done,
        active,
        failed,
        fmt_bytes(bytes_done),
        if bytes_total > 0 { fmt_bytes(bytes_total) } else { "?".into() },
    );
    f.render_widget(
        Paragraph::new(stats).style(Style::default().fg(DIM)).block(
            Block::default()
                .title(format!(" {} ", chat_name))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT)),
        ),
        chunks[0],
    );

    if app.downloads.is_empty() {
        f.render_widget(
            Paragraph::new("Scanning messages…")
                .alignment(Alignment::Center)
                .style(Style::default().fg(DIM)),
            center_vertical(1, chunks[1]),
        );
        return;
    }

    // Each entry occupies 2 rows: filename + gauge
    let list_area = chunks[1];
    let visible_items = (list_area.height as usize) / 2;
    let start = app.dl_scroll.min(app.downloads.len().saturating_sub(1));
    let visible = &app.downloads[start..app.downloads.len().min(start + visible_items)];

    let row_constraints: Vec<Constraint> = visible
        .iter()
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
        .collect();

    let rows = Layout::vertical(row_constraints).split(list_area);
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM));
    f.render_widget(outer_block, list_area);

    for (i, entry) in visible.iter().enumerate() {
        if rows.len() < (i + 1) * 2 { break; }
        let name_row = rows[i * 2];
        let bar_row = rows[i * 2 + 1];

        // Filename + state indicator
        let (state_char, state_color) = match &entry.state {
            DlState::Active => ("⟳", ACCENT),
            DlState::Done => ("✓", OK),
            DlState::Failed(_) => ("✗", ERR),
            DlState::Pending => ("○", DIM),
        };

        // Trim filename to fit
        let max_len = name_row.width.saturating_sub(6) as usize;
        let name = if entry.filename.len() > max_len {
            format!("…{}", &entry.filename[entry.filename.len().saturating_sub(max_len)..])
        } else {
            entry.filename.clone()
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(state_char, Style::default().fg(state_color)),
                Span::raw(" "),
                Span::raw(name),
            ])),
            name_row,
        );

        // Progress gauge
        let ratio = entry.ratio();
        let label = if entry.size > 0 {
            format!("{:>3}%  {}", (ratio * 100.0) as u8, fmt_bytes(entry.downloaded))
        } else {
            format!("{}", fmt_bytes(entry.downloaded))
        };

        let gauge_color = match &entry.state {
            DlState::Done => OK,
            DlState::Failed(_) => ERR,
            _ => ACCENT,
        };

        let gauge = Gauge::default()
            .ratio(ratio)
            .label(label)
            .gauge_style(Style::default().fg(gauge_color).bg(Color::Black));

        // Indent gauge slightly
        let gauge_area = Rect {
            x: bar_row.x + 4,
            y: bar_row.y,
            width: bar_row.width.saturating_sub(4),
            height: bar_row.height,
        };
        f.render_widget(gauge, gauge_area);
    }
}

// ── Layout helpers ────────────────────────────────────────────────────────────

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .split(v[1])[1]
}

fn center_vertical(height: u16, area: Rect) -> Rect {
    Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .split(area)[1]
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
