use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

const ASCII_ART: &[&str] = &[
    "████████████████████████████████████████████████████████████████████████████████████████████",
    "█▌                                                                                        ▐█",
    "█▌                                                                                        ▐█",
    "█▌                                                                                        ▐█",
    "█▌                                                     I8                                 ▐█",
    "█▌                                                     I8                                 ▐█",
    "█▌                                                   88888888                             ▐█",
    "█▌                                                     I8                                 ▐█",
    "█▌       ,gggg,gg    ,gggg,gg   ,ggg,    ,ggg,,ggg,    I8    ,gggggg,  gg     gg          ▐█",
    "█▌      dP\"  \"Y8I   dP\"  \"Y8I  i8\" \"8i  ,8\" \"8P\" \"8,   I8    dP\"\"\"\"8I  I8     8I          ▐█",
    "█▌     i8'    ,8I  i8'    ,8I  I8, ,8I  I8   8I   8I  ,I8,  ,8'    8I  I8,   ,8I          ▐█",
    "█▌    ,d8,   ,d8b,,d8,   ,d8I  'YbadP' ,dP   8I   Yb,,d88b,,dP     Y8,,d8b, ,d8I          ▐█",
    "█▌    P\"Y8888P\"'Y8P\"Y8888P\"888888P\"Y8888P'   8I   'Y88P\"\"Y88P      'Y8P\"\"Y88P\"888         ▐█",
    "█▌                       ,d8I'                                              ,d8I'         ▐█",
    "█▌                     ,dP'8I                                             ,dP'8I          ▐█",
    "█▌                    ,8\"  8I                                            ,8\"  8I          ▐█",
    "█▌                    I8   8I                                            I8   8I          ▐█",
    "█▌                    '8, ,8I                                            '8, ,8I          ▐█",
    "█▌                     'Y8P\"                                              'Y8P\"           ▐█",
    "█▌                                                                                        ▐█",
    "█▌                                                                                        ▐█",
    "█▌                                                                                        ▐█",
    "████████████████████████████████████████████████████████████████████████████████████████████"
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
    let top_pad = inner.height.saturating_sub(32) / 2;
    for _ in 0..top_pad {
        lines.push(Line::from(""));
    }

    // ASCII art (progressive reveal)
    let art_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    for (i, line) in ASCII_ART.iter().enumerate() {
        if i < lines_to_show {
            lines.push(Line::from(Span::styled(*line, art_style)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "     The Multi-Agent Prompt Manager",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Progress bar
    let bar_width = 40usize;
    let filled = (bar_width as f32 * app.intro_progress).round() as usize;
    let bar_str = format!("[{}{}]", "▓".repeat(filled), "░".repeat(bar_width - filled),);

    let detected = app.detected_agents.len();
    let total = 11;
    let status = if app.intro_progress < 1.0 {
        format!(
            "  {} Loading agents... ({}/{})",
            app.spinner_char(),
            detected,
            total
        )
    } else {
        format!("  {} {} agents detected", app.spinner_char(), detected)
    };

    lines.push(Line::from(Span::styled(
        format!("  {}", bar_str),
        Style::default().fg(Color::Green),
    )));
    lines.push(Line::from(Span::styled(
        status,
        Style::default().fg(Color::Yellow),
    )));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "        v0.1.0  │  Press any key to continue",
        Style::default().fg(Color::DarkGray),
    )));

    // Help hint
    lines.push(Line::from(Span::styled(
        "        j/k: navigate  Tab: switch  ?: help",
        Style::default().fg(Color::DarkGray),
    )));

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
}
