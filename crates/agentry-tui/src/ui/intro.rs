use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

const ASCII_ART: &[&str] = &[
    "          █████╗ ███████╗███████╗██████╗         ",
    "         ██╔══██╗██╔════╝██╔════╝██╔══██╗        ",
    "         ███████║███████╗███████╗██████╔╝        ",
    "         ██╔══██║╚════██║╚════██║██╔═══╗         ",
    "         ██║  ██║███████║███████║██║  ██╗        ",
    "         ╚═╝  ╚═╝╚══════╝╚══════╝╚═╝  ╚═╝      ",
];

pub fn draw_intro(f: &mut Frame, app: &App) {
    let size = f.area();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" agentry ");

    let inner = block.inner(size);
    f.render_widget(block, size);

    let lines_to_show = ((ASCII_ART.len() as f32) * app.intro_progress).ceil() as usize;

    let mut lines: Vec<Line> = Vec::new();

    // Top spacing
    let top_pad = inner.height.saturating_sub(20) / 2;
    for _ in 0..top_pad {
        lines.push(Line::from(""));
    }

    // ASCII art (progressive reveal)
    let art_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    for (i, line) in ASCII_ART.iter().enumerate() {
        if i < lines_to_show {
            lines.push(Line::from(Span::styled(*line, art_style)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "     The Multi-Agent Prompt Manager",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Progress bar
    let bar_width = 40usize;
    let filled = (bar_width as f32 * app.intro_progress).round() as usize;
    let bar_str = format!(
        "[{}{}]",
        "▓".repeat(filled),
        "░".repeat(bar_width - filled),
    );

    let detected = app.detected_agents.len();
    let total = 11;
    let status = if app.intro_progress < 1.0 {
        format!("  {} Loading agents... ({}/{})", app.spinner_char(), detected, total)
    } else {
        format!("  {} {} agents detected", app.spinner_char(), detected)
    };

    lines.push(Line::from(Span::styled(
        format!("  {}", bar_str),
        Style::default().fg(Color::Green),
    )));
    lines.push(Line::from(Span::styled(status, Style::default().fg(Color::Yellow))));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "        v0.1.0  │  Press any key to continue",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
}