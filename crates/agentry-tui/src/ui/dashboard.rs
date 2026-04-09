use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use crate::app::App;
use super::Tab;

pub fn draw_dashboard(f: &mut Frame, app: &App) {
    let size = f.area();

    // Layout: top tabs | main content (left + right) | bottom status
    let chunks = Layout::vertical([
        Constraint::Length(3),   // tabs
        Constraint::Min(10),     // main content
        Constraint::Length(1),   // status bar
    ])
    .split(size);

    // Draw tabs
    let tab_titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let title = format!("{}:{}", i + 1, t.title());
            Line::from(Span::styled(title, Style::default()))
        })
        .collect();

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(app.tab_index)
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    // Main content area split into left (list) and right (detail)
    let main = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    // Left panel: depends on active tab
    match Tab::from_index(app.tab_index) {
        Some(Tab::Dashboard) | Some(Tab::Agents) => {
            draw_agents_list(f, app, main[0]);
            draw_agent_detail(f, app, main[1]);
        }
        Some(Tab::Prompts) => {
            draw_prompts_placeholder(f, main[0], "Prompts");
            draw_prompts_detail_placeholder(f, main[1]);
        }
        Some(Tab::Skills) => {
            draw_skills_placeholder(f, main[0]);
            draw_skills_detail_placeholder(f, main[1]);
        }
        Some(Tab::Sync) => {
            draw_sync_placeholder(f, main[0]);
            draw_sync_detail_placeholder(f, main[1]);
        }
        Some(Tab::OpenClaw) => {
            draw_openclaw_placeholder(f, main[0]);
            draw_openclaw_detail_placeholder(f, main[1]);
        }
        None => {}
    }

    // Status bar
    let status = app.status_message.as_deref().unwrap_or("j/k:navigate  Tab:next-tab  s:sync  q:quit  ?:help");
    let status_bar = Paragraph::new(Line::from(Span::styled(
        format!(" {}", status),
        Style::default().fg(Color::DarkGray),
    )));
    f.render_widget(status_bar, chunks[2]);

    // Help overlay
    if app.show_help {
        draw_help(f, size);
    }
}

fn draw_agents_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .detected_agents
        .iter()
        .map(|agent| {
            let status_color = if agent.installed { Color::Green } else { Color::Red };
            let version = agent.version.as_deref().unwrap_or("---");
            let line = Line::from(vec![
                Span::styled(
                    format!("  {:<16}", agent.spec.name),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    format!(" v{:<6}", version),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{}]", agent.status_label()),
                    Style::default().fg(status_color),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let count = app.detected_agents.iter().filter(|a| a.installed).count();
    let total = app.detected_agents.len();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Detected Agents ({}/{}) ", count, total))
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    if app.list_selected < app.detected_agents.len() {
        state.select(Some(app.list_selected));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_agent_detail(f: &mut Frame, app: &App, area: Rect) {
    let agent = app.detected_agents.get(app.list_selected);

    let lines = if let Some(agent) = agent {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let config_path = format!("{}/{}", home, agent.spec.config_dir);
        let prompt_info = agent.spec.prompt_filename.to_string();

        let mut lines = vec![
            Line::from(Span::styled(
                format!(" {} ", agent.spec.name),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Config:   ", Style::default().fg(Color::Yellow)),
                Span::styled(config_path, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  Prompts:  ", Style::default().fg(Color::Yellow)),
                Span::styled(prompt_info, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("  Format:   ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{}", agent.spec.prompt_format), Style::default().fg(Color::White)),
            ]),
        ];

        if let Some(ref skills_dir) = agent.skills_dir {
            lines.push(Line::from(vec![
                Span::styled("  Skills:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{} ({} installed)", skills_dir.display(), agent.installed_skills.len()),
                    Style::default().fg(Color::White),
                ),
            ]));
        }

        if let Some(ref pattern) = agent.skills_symlink_pattern {
            lines.push(Line::from(vec![
                Span::styled("  Symlinks: ", Style::default().fg(Color::Yellow)),
                Span::styled(pattern.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        if let Some(ref version) = agent.version {
            lines.push(Line::from(vec![
                Span::styled("  Version:  ", Style::default().fg(Color::Yellow)),
                Span::styled(version.clone(), Style::default().fg(Color::White)),
            ]));
        }

        if !agent.installed_skills.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " ── Installed Skills ──────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            let skills_display = if agent.installed_skills.len() > 10 {
                let mut s = agent.installed_skills[..10].join(", ");
                s.push_str(", ...");
                s
            } else {
                agent.installed_skills.join(", ")
            };
            lines.push(Line::from(Span::styled(
                format!("  {}", skills_display),
                Style::default().fg(Color::White),
            )));
        }

        lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No agent selected",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Agent Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn draw_prompts_placeholder(f: &mut Frame, area: Rect, label: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} (coming in Phase 2) ", label))
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new(Line::from(Span::styled(
        "  Not yet implemented",
        Style::default().fg(Color::DarkGray),
    )))
    .block(block);
    f.render_widget(text, area);
}

fn draw_prompts_detail_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Prompt Editor (Phase 2) ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new("").block(block);
    f.render_widget(text, area);
}

fn draw_skills_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Skills (Phase 4) ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new(Line::from(Span::styled(
        "  Not yet implemented",
        Style::default().fg(Color::DarkGray),
    )))
    .block(block);
    f.render_widget(text, area);
}

fn draw_skills_detail_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Skill Details ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new("").block(block);
    f.render_widget(text, area);
}

fn draw_sync_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Sync (Phase 3) ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new(Line::from(Span::styled(
        "  Not yet implemented",
        Style::default().fg(Color::DarkGray),
    )))
    .block(block);
    f.render_widget(text, area);
}

fn draw_sync_detail_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Sync Details ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new("").block(block);
    f.render_widget(text, area);
}

fn draw_openclaw_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" OpenClaw (Phase 5) ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new(Line::from(Span::styled(
        "  Not yet implemented",
        Style::default().fg(Color::DarkGray),
    )))
    .block(block);
    f.render_widget(text, area);
}

fn draw_openclaw_detail_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" OpenClaw Details ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new("").block(block);
    f.render_widget(text, area);
}

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(" agentry — Keybindings ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  j/k, ↑/↓  ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate list"),
        ]),
        Line::from(vec![
            Span::styled("  Tab/S-Tab  ", Style::default().fg(Color::Yellow)),
            Span::raw("Switch tabs"),
        ]),
        Line::from(vec![
            Span::styled("  1-6        ", Style::default().fg(Color::Yellow)),
            Span::raw("Jump to tab"),
        ]),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
            Span::raw("Open/Edit selected"),
        ]),
        Line::from(vec![
            Span::styled("  n          ", Style::default().fg(Color::Yellow)),
            Span::raw("New prompt"),
        ]),
        Line::from(vec![
            Span::styled("  d          ", Style::default().fg(Color::Yellow)),
            Span::raw("Delete prompt"),
        ]),
        Line::from(vec![
            Span::styled("  s          ", Style::default().fg(Color::Yellow)),
            Span::raw("Sync to agents"),
        ]),
        Line::from(vec![
            Span::styled("  e          ", Style::default().fg(Color::Yellow)),
            Span::raw("Edit prompt"),
        ]),
        Line::from(vec![
            Span::styled("  u          ", Style::default().fg(Color::Yellow)),
            Span::raw("Update skills"),
        ]),
        Line::from(vec![
            Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q          ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Press ? or Esc to close", Style::default().fg(Color::DarkGray))),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let width = 50.min(area.width);
    let height = 16.min(area.height);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}