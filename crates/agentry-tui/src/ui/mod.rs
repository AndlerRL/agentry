mod dashboard;
mod intro;

use ratatui::layout::Rect;

use crate::editor::Editor;

pub use dashboard::draw_dashboard;
pub use intro::draw_intro;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Agents,
    Prompts,
    Skills,
    Sync,
    OpenClaw,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Dashboard,
        Tab::Agents,
        Tab::Prompts,
        Tab::Skills,
        Tab::Sync,
        Tab::OpenClaw,
    ];

    pub fn _index(self) -> usize {
        match self {
            Tab::Dashboard => 0,
            Tab::Agents => 1,
            Tab::Prompts => 2,
            Tab::Skills => 3,
            Tab::Sync => 4,
            Tab::OpenClaw => 5,
        }
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        Tab::ALL.get(i).copied()
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Agents => "Agents",
            Tab::Prompts => "Prompts",
            Tab::Skills => "Skills",
            Tab::Sync => "Sync",
            Tab::OpenClaw => "OpenClaw",
        }
    }
}

fn _centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

/// Draw the vim-like editor as a full-screen view.
pub fn draw_editor(f: &mut ratatui::Frame, editor: &Editor) {
    use ratatui::{
        layout::{Constraint, Layout},
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph, Wrap},
    };

    let size = f.area();

    let chunks = Layout::vertical([
        Constraint::Min(1),    // editor content
        Constraint::Length(1), // status line
        Constraint::Length(1), // command/message line
    ])
    .split(size);

    // Editor content with line numbers
    let viewport_height = chunks[0].height as usize;
    let viewport_start = if editor.cursor.row >= viewport_height {
        editor.cursor.row - viewport_height + 3
    } else {
        0
    };

    let lines = editor.render_lines(viewport_start, viewport_height);
    let line_count = editor.buffer.line_count();
    let _line_num_width = line_count.to_string().len().max(3);

    let content: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let row = viewport_start + i;
            if row == editor.cursor.row && editor.mode == crate::editor::EditorMode::Normal {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().add_modifier(Modifier::REVERSED),
                ))
            } else if row == editor.cursor.row {
                Line::from(line.to_string())
            } else {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(Color::White),
                ))
            }
        })
        .collect();

    let mode_indicator = match editor.mode {
        crate::editor::EditorMode::Normal => "-- NORMAL --",
        crate::editor::EditorMode::Insert => "-- INSERT --",
        crate::editor::EditorMode::Visual => "-- VISUAL --",
        crate::editor::EditorMode::Command => "-- COMMAND --",
    };

    let block = Block::default()
        .borders(Borders::NONE)
        .title(format!(
            " {} │ {}",
            editor.filename.as_deref().unwrap_or("[No Name]"),
            mode_indicator
        ))
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, chunks[0]);

    // Status line
    let status = editor.status_line();
    let status_bar = Paragraph::new(Line::from(Span::styled(
        format!(" {}", status),
        Style::default().fg(Color::Black).bg(Color::Cyan),
    )));
    f.render_widget(status_bar, chunks[1]);

    // Command/message line
    let msg = if editor.mode == crate::editor::EditorMode::Command {
        format!(":{}", editor.command_buf)
    } else if let Some(ref m) = editor.message {
        m.clone()
    } else {
        String::new()
    };
    let msg_line = Paragraph::new(Line::from(Span::styled(
        msg,
        Style::default().fg(Color::Yellow),
    )));
    f.render_widget(msg_line, chunks[2]);
}
