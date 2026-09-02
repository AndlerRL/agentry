use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use agentry_audit::report::{AuditReport, HealthGrade, Severity};

use super::Tab;
use crate::app::App;

/// Truncate a string to fit within a given display width (accounting for Unicode width).
fn truncate_to_width(line: &str, width: u16) -> String {
    let width = width as usize;
    if width == 0 {
        return String::new();
    }
    let display_width = unicode_width::UnicodeWidthStr::width(line);
    if display_width <= width {
        return line.to_string();
    }
    let mut result = String::with_capacity(line.len());
    let mut current_width = 0usize;
    for c in line.chars() {
        let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if current_width + cw > width {
            break;
        }
        result.push(c);
        current_width += cw;
    }
    result
}

pub fn draw_dashboard(f: &mut Frame, app: &App) {
    let size = f.area();

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(10),
        Constraint::Length(1),
        Constraint::Length(2),
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
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    let main = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    match Tab::from_index(app.tab_index) {
        Some(Tab::Agents) => {
            draw_agents_list_enhanced(f, app, main[0]);
            draw_agent_detail_enhanced(f, app, main[1]);
        }
        Some(Tab::Prompts) => {
            draw_prompts_list(f, app, main[0]);
            draw_prompt_detail(f, app, main[1]);
        }
        Some(Tab::Skills) => {
            draw_skills_list(f, app, main[0]);
            draw_skill_detail(f, app, main[1]);
        }
        Some(Tab::Sync) => {
            draw_sync_list(f, app, main[0]);
            draw_sync_detail(f, app, main[1]);
        }
        Some(Tab::Audit) => {
            draw_audit_list(f, app, main[0]);
            draw_audit_detail(f, app, main[1]);
        }
        None => {}
    }

    let status = if let Some(ref err) = app.error_message {
        err.as_str()
    } else {
        app.status_message.as_deref().unwrap_or("")
    };
    let status_color = if app.error_message.is_some() {
        Color::Red
    } else {
        Color::DarkGray
    };
    let status_bar = Paragraph::new(Line::from(Span::styled(
        format!(" {}", status),
        Style::default().fg(status_color),
    )));
    f.render_widget(status_bar, chunks[2]);

    f.render_widget(ratatui::widgets::Clear, chunks[3]);
    let keymap_lines = crate::ui::keymap::bar_lines(app.tab_index, app, chunks[3].width as usize);
    if !keymap_lines.is_empty() {
        let keymap_bar = Paragraph::new(keymap_lines);
        f.render_widget(keymap_bar, chunks[3]);
    }

    if app.show_help {
        draw_help(f, size);
    }
}

fn draw_agents_list_enhanced(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .detected_agents
        .iter()
        .map(|agent| {
            let status_icon = if agent.installed { "[ON]" } else { "[--]" };
            let status_color = if agent.installed {
                Color::Green
            } else {
                Color::DarkGray
            };
            let version = agent.version.as_deref().unwrap_or("--");

            // Build spans: icon, name, badges, version
            let mut spans = vec![
                Span::styled(
                    format!("{} ", status_icon),
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<18}", agent.spec.name),
                    Style::default().fg(Color::White),
                ),
            ];

            // Per-method badges with color (immediately after name)
            for method in &agent.spec.install_methods {
                if method.available_on_os() {
                    let is_detected = agent.detected_methods.contains(method);
                    let badge_color = if is_detected {
                        Color::Green
                    } else {
                        Color::DarkGray
                    };
                    let key = method.method_key();
                    spans.push(Span::styled(
                        format!(" {}", key),
                        Style::default()
                            .fg(badge_color)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
            }

            // Version at the end
            spans.push(Span::styled(
                format!("  v{}", version),
                Style::default().fg(Color::DarkGray),
            ));

            let line = Line::from(spans);
            ListItem::new(line)
        })
        .collect();

    let count = app.detected_agents.iter().filter(|a| a.installed).count();
    let total = app.detected_agents.len();
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Agents ({}/{}) ", count, total))
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    if app.list_selected < items.len() {
        state.select(Some(app.list_selected));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_agent_detail_enhanced(f: &mut Frame, app: &App, area: Rect) {
    let detail_width = area.width.saturating_sub(4);
    let agent = app.detected_agents.get(app.list_selected);

    let lines = if let Some(agent) = agent {
        let config_path = format!("{}/{}", app.home_dir.display(), agent.spec.config_dir);
        let config_path = truncate_to_width(&config_path, detail_width);
        let prompt_info = agent.spec.prompt_filename.to_string();

        let mut agent_lines = vec![
            Line::from(Span::styled(
                format!(" {} ", agent.spec.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Detection summary
        if agent.installed {
            let detected_methods: Vec<&str> =
                agent.detected_methods.iter().map(|m| m.label()).collect();
            if detected_methods.is_empty() {
                agent_lines.push(Line::from(vec![
                    Span::styled("  Status:   ", Style::default().fg(Color::Yellow)),
                    Span::styled("Installed", Style::default().fg(Color::Green)),
                ]));
            } else {
                agent_lines.push(Line::from(vec![
                    Span::styled("  Detected: ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        detected_methods.join(", "),
                        Style::default().fg(Color::Green),
                    ),
                ]));
            }
        } else {
            agent_lines.push(Line::from(vec![
                Span::styled("  Status:   ", Style::default().fg(Color::Yellow)),
                Span::styled("Not installed", Style::default().fg(Color::Red)),
            ]));
        }

        if let Some(ref report) = app.audit_report {
            if let Some(agent_audit) = report.agents.iter().find(|a| a.agent_id == agent.spec.id) {
                let critical = agent_audit
                    .findings
                    .iter()
                    .filter(|f| f.severity == Severity::Critical)
                    .count();
                let warning = agent_audit
                    .findings
                    .iter()
                    .filter(|f| f.severity == Severity::Warning)
                    .count();
                let health = format!(
                    "{}/100 ({}) · {} critical, {} warning",
                    agent_audit.health_score,
                    grade_label(agent_audit.grade),
                    critical,
                    warning
                );
                agent_lines.push(Line::from(vec![
                    Span::styled("  Health:   ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        truncate_to_width(&health, detail_width),
                        Style::default().fg(grade_color(agent_audit.grade)),
                    ),
                ]));
            } else {
                agent_lines.push(Line::from(vec![
                    Span::styled("  Health:   ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        "not audited (press r in Audit tab)",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        } else {
            agent_lines.push(Line::from(vec![
                Span::styled("  Health:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    "not audited (press r in Audit tab)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        agent_lines.push(Line::from(""));
        agent_lines.push(Line::from(Span::styled(
            " ── Config ───────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        agent_lines.push(Line::from(vec![
            Span::styled("  Dir:      ", Style::default().fg(Color::Yellow)),
            Span::styled(config_path, Style::default().fg(Color::White)),
        ]));
        agent_lines.push(Line::from(vec![
            Span::styled("  Prompts:  ", Style::default().fg(Color::Yellow)),
            Span::styled(prompt_info, Style::default().fg(Color::White)),
        ]));
        agent_lines.push(Line::from(vec![
            Span::styled("  Format:   ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{}", agent.spec.prompt_format),
                Style::default().fg(Color::White),
            ),
        ]));

        if let Some(ref skills_dir) = agent.skills_dir {
            let skills_str = format!(
                "{} ({} installed)",
                skills_dir.display(),
                agent.installed_skills.len()
            );
            agent_lines.push(Line::from(vec![
                Span::styled("  Skills:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    truncate_to_width(&skills_str, detail_width),
                    Style::default().fg(Color::White),
                ),
            ]));
        }
        if let Some(ref version) = agent.version {
            agent_lines.push(Line::from(vec![
                Span::styled("  Version:  ", Style::default().fg(Color::Yellow)),
                Span::styled(version.clone(), Style::default().fg(Color::White)),
            ]));
        }

        // Install Methods section
        agent_lines.push(Line::from(""));
        agent_lines.push(Line::from(Span::styled(
            " ── Install Methods ──────────────────",
            Style::default().fg(Color::DarkGray),
        )));

        let os_methods: Vec<(usize, &agentry_core::models::InstallMethod)> = agent
            .spec
            .install_methods
            .iter()
            .enumerate()
            .filter(|(_, m)| m.available_on_os())
            .collect();

        if os_methods.is_empty() {
            agent_lines.push(Line::from(Span::styled(
                "  No methods available for this OS",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (i, method) in &os_methods {
                let is_detected = agent.detected_methods.contains(method);
                let is_selected = *i == app.method_selected;
                let cursor = if is_selected { ">" } else { " " };
                let check = if is_detected { "✓" } else { "○" };
                let check_color = if is_detected {
                    Color::Green
                } else {
                    Color::DarkGray
                };

                let mut spans = vec![
                    Span::styled(
                        format!(" {} ", cursor),
                        Style::default().fg(if is_selected {
                            Color::Yellow
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(format!("[{}] ", check), Style::default().fg(check_color)),
                    Span::styled(
                        format!("{:<20}", method.label()),
                        Style::default().fg(Color::White),
                    ),
                ];
                // Show install command hint
                let hint = method.install_command(None);
                let hint_short = truncate_to_width(&hint, detail_width.saturating_sub(28));
                spans.push(Span::styled(
                    hint_short,
                    Style::default().fg(Color::DarkGray),
                ));

                agent_lines.push(Line::from(spans));
            }
        }

        // Version info
        if let Some(ref versions) = app.version_list {
            agent_lines.push(Line::from(""));
            agent_lines.push(Line::from(Span::styled(
                format!(" ── Versions ({}) ────────────────────", versions.len()),
                Style::default().fg(Color::DarkGray),
            )));
            let show = versions.iter().take(15).cloned().collect::<Vec<_>>();
            for v in &show {
                agent_lines.push(Line::from(Span::styled(
                    format!("  {}", v),
                    Style::default().fg(Color::White),
                )));
            }
            if versions.len() > 15 {
                agent_lines.push(Line::from(Span::styled(
                    format!("  ... and {} more", versions.len() - 15),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        } else if let Some(ref err) = app.version_list_error {
            agent_lines.push(Line::from(""));
            agent_lines.push(Line::from(Span::styled(
                format!("  {}", err),
                Style::default().fg(Color::Red),
            )));
        }

        // Actions section
        agent_lines.push(Line::from(""));
        agent_lines.push(Line::from(Span::styled(
            " ── Actions ──────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));

        if let Some(method) = agent.spec.install_methods.get(app.method_selected) {
            let is_detected = agent.detected_methods.contains(method);
            if is_detected {
                agent_lines.push(Line::from(Span::styled(
                    "  u: Update  r: Remove",
                    Style::default().fg(Color::Yellow),
                )));
            } else {
                agent_lines.push(Line::from(Span::styled(
                    "  Enter: Install  v: List versions",
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        // Show confirm prompt if active
        if app.agent_confirm.is_some() {
            agent_lines.push(Line::from(""));
            agent_lines.push(Line::from(Span::styled(
                format!("  {}", app.status_message.as_deref().unwrap_or("Confirm?")),
                Style::default().fg(Color::Yellow),
            )));
        }

        if agent.spec.id == "openclaw" {
            if let Some(ref oc_state) = app.openclaw_state {
                agent_lines.push(Line::from(""));
                agent_lines.push(Line::from(Span::styled(
                    format!(
                        " ── Workspaces ({}) ──────────────────",
                        oc_state.workspaces.len()
                    ),
                    Style::default().fg(Color::DarkGray),
                )));
                if oc_state.workspaces.is_empty() {
                    let status = if oc_state.installed {
                        "  No workspaces found"
                    } else {
                        "  OpenClaw not installed"
                    };
                    agent_lines.push(Line::from(Span::styled(
                        status,
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    for ws in &oc_state.workspaces {
                        let default_marker = if ws.is_default { " ★" } else { "" };
                        let model_info = ws.model.as_deref().unwrap_or("default");
                        let doc_badges = format!(
                            " {}{}{}{}{}{}",
                            if ws.has_soul_md { "S" } else { "·" },
                            if ws.has_agents_md { "A" } else { "·" },
                            if ws.has_tools_md { "T" } else { "·" },
                            if ws.has_identity_md { "I" } else { "·" },
                            if ws.has_memory_md { "M" } else { "·" },
                            if ws.has_user_md { "U" } else { "·" },
                        );
                        let wf_info = if ws.lobster_workflows.is_empty() {
                            String::new()
                        } else {
                            format!(" ⚡{}", ws.lobster_workflows.len())
                        };
                        let row = format!(
                            "  {:<16}[{}]{} {}{}",
                            ws.name, model_info, default_marker, doc_badges, wf_info
                        );
                        agent_lines.push(Line::from(Span::styled(
                            truncate_to_width(&row, detail_width),
                            Style::default().fg(if ws.is_default {
                                Color::White
                            } else {
                                Color::DarkGray
                            }),
                        )));
                        for doc in &ws.docs {
                            let size_kb = doc.size_bytes as f64 / 1024.0;
                            let doc_row = format!(
                                "      {:<14}{:>7.1} KB  {}",
                                doc.doc_type.to_string(),
                                size_kb,
                                doc.path.display()
                            );
                            agent_lines.push(Line::from(Span::styled(
                                truncate_to_width(&doc_row, detail_width),
                                Style::default().fg(Color::DarkGray),
                            )));
                        }
                    }
                }
                agent_lines.push(Line::from(Span::styled(
                    "  Enter: edit first doc  n: New workspace  c: openclaw setup  a: Add agent",
                    Style::default().fg(Color::Yellow),
                )));
            }
        }

        agent_lines
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

// ── Prompts Tab ──────────────────────────────────────────────────────────

fn draw_prompts_list(f: &mut Frame, app: &App, area: Rect) {
    let global_prompts: Vec<(usize, &agentry_core::models::UnifiedPrompt)> = app
        .prompts
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(p.scope, agentry_core::models::PromptScope::Global))
        .collect();

    let project_prompts: Vec<(usize, &agentry_core::models::UnifiedPrompt)> = app
        .prompts
        .iter()
        .enumerate()
        .filter(|(_, p)| matches!(p.scope, agentry_core::models::PromptScope::Project { .. }))
        .collect();

    let mut items: Vec<ListItem> = Vec::new();

    if !global_prompts.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            " ── Global Prompts ──",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))));
        for (_orig_idx, prompt) in &global_prompts {
            items.push(ListItem::new(Line::from(Span::styled(
                format!("   {}", prompt.name),
                Style::default().fg(Color::White),
            ))));
        }
    }

    if !project_prompts.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            " ── Project Prompts ──",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))));
        for (_orig_idx, prompt) in &project_prompts {
            let scope_label = match &prompt.scope {
                agentry_core::models::PromptScope::Project { root } => {
                    root.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                }
                _ => "",
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!("   [{}] ", scope_label),
                    Style::default().fg(Color::Magenta),
                ),
                Span::styled(&prompt.name, Style::default().fg(Color::White)),
            ])));
        }
    }

    items.push(ListItem::new(Line::from(Span::styled(
        " + [New Global Prompt]",
        Style::default().fg(Color::Green),
    ))));

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Prompts ({}) ", app.prompts.len()))
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    if app.list_selected < items.len() {
        state.select(Some(app.list_selected));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_prompt_detail(f: &mut Frame, app: &App, area: Rect) {
    let detail_width = area.width.saturating_sub(4);

    // Use selected_prompt_index via a manual reimplementation to avoid borrow issues
    let prompt_idx = {
        let global_prompts: Vec<(usize, &agentry_core::models::UnifiedPrompt)> = app
            .prompts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.scope, agentry_core::models::PromptScope::Global))
            .collect();
        let project_prompts: Vec<(usize, &agentry_core::models::UnifiedPrompt)> = app
            .prompts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.scope, agentry_core::models::PromptScope::Project { .. }))
            .collect();

        let mut list_row = 0;
        let mut found = None;

        if !global_prompts.is_empty() {
            if app.list_selected == list_row {
                found = None;
            } else {
                list_row += 1;
                for (orig_idx, _) in &global_prompts {
                    if app.list_selected == list_row {
                        found = Some(*orig_idx);
                        break;
                    }
                    list_row += 1;
                }
            }
        }

        if found.is_none() && !project_prompts.is_empty() {
            if app.list_selected == list_row {
                found = None;
            } else {
                list_row += 1;
                for (orig_idx, _) in &project_prompts {
                    if app.list_selected == list_row {
                        found = Some(*orig_idx);
                        break;
                    }
                    list_row += 1;
                }
            }
        }

        found
    };

    let lines = if let Some(idx) = prompt_idx {
        if let Some(prompt) = app.prompts.get(idx) {
            let scope_label = match &prompt.scope {
                agentry_core::models::PromptScope::Global => "Global".to_string(),
                agentry_core::models::PromptScope::Project { root } => {
                    format!("Project ({})", root.display())
                }
            };

            let mut detail_lines = vec![
                Line::from(Span::styled(
                    format!(" {} ", prompt.name),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Scope:    ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        truncate_to_width(&scope_label, detail_width),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  Format:   ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        format!("{}", prompt.source_format),
                        Style::default().fg(Color::White),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("  File:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        truncate_to_width(
                            &prompt
                                .source_path
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default(),
                            detail_width,
                        ),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ];

            if !prompt.description.is_empty() {
                detail_lines.push(Line::from(vec![
                    Span::styled("  Desc:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        truncate_to_width(&prompt.description, detail_width),
                        Style::default().fg(Color::White),
                    ),
                ]));
            }

            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled(
                " ── Preview ──────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));

            for line in prompt.body.lines().take(20) {
                detail_lines.push(Line::from(Span::styled(
                    format!(
                        " {}",
                        truncate_to_width(line, detail_width.saturating_sub(1))
                    ),
                    Style::default().fg(Color::White),
                )));
            }

            detail_lines
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Select a prompt to view details",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
    } else if app.list_is_new_prompt_action() {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  New Global Prompt",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press Enter, then type the name",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  and press Enter again to create.",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a prompt to view details",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Enter: Edit  n: New  d: Delete",
                Style::default().fg(Color::Yellow),
            )),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Prompt Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

#[allow(dead_code)]
fn draw_prompts_detail_placeholder(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Prompt Details ")
        .border_style(Style::default().fg(Color::DarkGray));
    let text = Paragraph::new("").block(block);
    f.render_widget(text, area);
}

// ── Skills Tab ───────────────────────────────────────────────────────────

fn draw_skills_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if let Some(ref hub) = app.skill_hub {
        let mut items: Vec<ListItem> = Vec::new();

        let mut source_groups: std::collections::BTreeMap<
            &str,
            Vec<&agentry_skills::hub::AvailableSkill>,
        > = std::collections::BTreeMap::new();
        for skill in hub.skills.values() {
            let key = if skill.source.is_empty() {
                "unknown"
            } else {
                skill.source.as_str()
            };
            source_groups.entry(key).or_default().push(skill);
        }

        for (source, skills) in &source_groups {
            let installed_count = skills.iter().filter(|s| s.installed).count();
            items.push(ListItem::new(Line::from(Span::styled(
                format!(" ── {} ({}/{}) ──", source, installed_count, skills.len()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))));
            for skill in skills {
                let status = if skill.installed { "✓" } else { "○" };
                let status_color = if skill.installed {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(format!("  {} ", status), Style::default().fg(status_color)),
                    Span::styled(&skill.name, Style::default().fg(Color::White)),
                ])));
            }
        }

        items
    } else {
        vec![ListItem::new(Line::from(Span::styled(
            "  Failed to load skills",
            Style::default().fg(Color::Red),
        )))]
    };

    let installed = app
        .skill_hub
        .as_ref()
        .map(|h| h.installed_count())
        .unwrap_or(0);
    let total = app.skill_hub.as_ref().map(|h| h.total_count()).unwrap_or(0);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Skills ({}/{}) ", installed, total))
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    if app.list_selected < items.len() {
        state.select(Some(app.list_selected));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_skill_detail(f: &mut Frame, app: &App, area: Rect) {
    let detail_width = area.width.saturating_sub(4);

    // Resolve selected skill by walking group structure
    let selected = if let Some(ref hub) = app.skill_hub {
        let skills: Vec<_> = hub.skills.values().collect();

        let mut source_groups: std::collections::BTreeMap<
            &str,
            Vec<(usize, &agentry_skills::hub::AvailableSkill)>,
        > = std::collections::BTreeMap::new();
        for (i, skill) in skills.iter().enumerate() {
            let key = if skill.source.is_empty() {
                "unknown"
            } else {
                skill.source.as_str()
            };
            source_groups.entry(key).or_default().push((i, skill));
        }

        let mut list_row = 0;
        let mut found: Option<&agentry_skills::hub::AvailableSkill> = None;
        for group_skills in source_groups.values() {
            if app.list_selected == list_row {
                break;
            }
            list_row += 1;
            for (_orig_idx, skill) in group_skills {
                if app.list_selected == list_row {
                    found = Some(*skill);
                    break;
                }
                list_row += 1;
            }
            if found.is_some() {
                break;
            }
        }
        found
    } else {
        None
    };

    let lines = if let Some(skill) = selected {
        let mut detail_lines = vec![
            Line::from(Span::styled(
                format!(" {} ", skill.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Status:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    if skill.installed {
                        "Installed ✓"
                    } else {
                        "Not installed"
                    },
                    Style::default().fg(if skill.installed {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Source:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    truncate_to_width(
                        if skill.source.is_empty() {
                            "—"
                        } else {
                            &skill.source
                        },
                        detail_width,
                    ),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        if !skill.description.is_empty() {
            detail_lines.push(Line::from(vec![
                Span::styled("  Desc:     ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    truncate_to_width(&skill.description, detail_width),
                    Style::default().fg(Color::White),
                ),
            ]));
        }

        if let Some(ref hash) = skill.installed_hash {
            detail_lines.push(Line::from(vec![
                Span::styled("  Hash:     ", Style::default().fg(Color::Yellow)),
                Span::styled(hash.clone(), Style::default().fg(Color::DarkGray)),
            ]));
        }

        if let Some(ref path) = skill.install_path {
            detail_lines.push(Line::from(vec![
                Span::styled("  Path:     ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    truncate_to_width(&path.display().to_string(), detail_width),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }

        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled(
            " ── Actions ──────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));

        if skill.installed {
            detail_lines.push(Line::from(Span::styled(
                "  u: Update  r: Remove  g: Open GitHub",
                Style::default().fg(Color::Yellow),
            )));
        } else if !skill.source.is_empty() {
            detail_lines.push(Line::from(Span::styled(
                "  Enter/i: Install  g: Open GitHub",
                Style::default().fg(Color::Yellow),
            )));
        }

        detail_lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a skill to view details",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Skill Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

// ── Sync Tab ─────────────────────────────────────────────────────────────

fn draw_sync_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.sync_results.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  Press 's' to load sync plan",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        let mut prompt_groups: std::collections::BTreeMap<&str, Vec<&crate::app::SyncResultEntry>> =
            std::collections::BTreeMap::new();
        for entry in &app.sync_results {
            prompt_groups
                .entry(&entry.prompt_name)
                .or_default()
                .push(entry);
        }

        let mut items = Vec::new();
        for (prompt_name, mappings) in &prompt_groups {
            items.push(ListItem::new(Line::from(Span::styled(
                format!(" ── {} ──", prompt_name),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))));
            for mapping in mappings {
                let (status_icon, status_color) = match mapping.status {
                    agentry_core::models::SyncStatus::UpToDate => ("✓", Color::Green),
                    agentry_core::models::SyncStatus::Missing => ("?", Color::Yellow),
                    agentry_core::models::SyncStatus::Outdated => ("↑", Color::Yellow),
                    agentry_core::models::SyncStatus::Conflict => ("!", Color::Red),
                };
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {} ", status_icon),
                        Style::default().fg(status_color),
                    ),
                    Span::styled(
                        format!("{:<16}", mapping.agent_id),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!(" {}", mapping.status),
                        Style::default().fg(status_color),
                    ),
                ])));
            }
        }
        items
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(
            " Sync ({}) ",
            app.sync_results
                .iter()
                .filter(|r| r.status == agentry_core::models::SyncStatus::Missing
                    || r.status == agentry_core::models::SyncStatus::Outdated)
                .count()
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let mut state = ListState::default();
    if app.list_selected < items.len() {
        state.select(Some(app.list_selected));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_sync_detail(f: &mut Frame, app: &App, area: Rect) {
    let detail_width = area.width.saturating_sub(4);

    // Resolve selected sync entry through grouped structure
    let lines = if !app.sync_results.is_empty() {
        let mut prompt_groups: std::collections::BTreeMap<
            &str,
            Vec<(usize, &crate::app::SyncResultEntry)>,
        > = std::collections::BTreeMap::new();
        for (i, entry) in app.sync_results.iter().enumerate() {
            prompt_groups
                .entry(&entry.prompt_name)
                .or_default()
                .push((i, entry));
        }

        let mut list_row = 0;
        let mut found: Option<&crate::app::SyncResultEntry> = None;
        for entries in prompt_groups.values() {
            if app.list_selected == list_row {
                break;
            }
            list_row += 1;
            for (_orig_idx, entry) in entries {
                if app.list_selected == list_row {
                    found = Some(*entry);
                    break;
                }
                list_row += 1;
            }
            if found.is_some() {
                break;
            }
        }

        if let Some(entry) = found {
            let (status_icon, status_color) = match entry.status {
                agentry_core::models::SyncStatus::UpToDate => ("Up to date ✓", Color::Green),
                agentry_core::models::SyncStatus::Missing => ("Missing ?", Color::Yellow),
                agentry_core::models::SyncStatus::Outdated => ("Outdated ↑", Color::Yellow),
                agentry_core::models::SyncStatus::Conflict => ("Conflict !", Color::Red),
            };

            let action_label = match entry.action {
                agentry_core::models::SyncAction::Copy => "Copy (format-convert)",
                agentry_core::models::SyncAction::Symlink => "Symlink (relative)",
                agentry_core::models::SyncAction::Source => "Source (skip)",
                agentry_core::models::SyncAction::Skip => "Skip",
            };

            vec![
                Line::from(Span::styled(
                    format!(" {} → {} ", entry.prompt_name, entry.agent_id),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled("  Status:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(status_icon.to_string(), Style::default().fg(status_color)),
                ]),
                Line::from(vec![
                    Span::styled("  Action:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(action_label, Style::default().fg(Color::White)),
                ]),
                Line::from(vec![
                    Span::styled("  Target:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        truncate_to_width(&entry.destination, detail_width),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    " ── Actions ──────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(Span::styled(
                    "  s: Execute sync  w: Generate workflow",
                    Style::default().fg(Color::Yellow),
                )),
            ]
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Select a sync entry to view details",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Press 's' to load sync plan",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Shows where each prompt will be synced",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  across all detected agents.",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Sync Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical => "Critical",
        Severity::Warning => "Warning",
        Severity::Info => "Info",
        Severity::Suggestion => "Suggestion",
    }
}

fn severity_color(severity: Severity) -> Color {
    match severity {
        Severity::Critical => Color::Red,
        Severity::Warning => Color::Yellow,
        Severity::Info => Color::Cyan,
        Severity::Suggestion => Color::DarkGray,
    }
}

fn grade_label(grade: HealthGrade) -> &'static str {
    match grade {
        HealthGrade::Healthy => "Healthy",
        HealthGrade::Degraded => "Degraded",
        HealthGrade::Unhealthy => "Unhealthy",
        HealthGrade::Critical => "Critical",
    }
}

fn grade_color(grade: HealthGrade) -> Color {
    match grade {
        HealthGrade::Healthy => Color::Green,
        HealthGrade::Degraded => Color::Yellow,
        HealthGrade::Unhealthy => Color::Red,
        HealthGrade::Critical => Color::Red,
    }
}

fn health_bar(score: u8) -> String {
    let filled = (score.min(100) / 10) as usize;
    let mut bar = String::with_capacity(10);
    for _ in 0..filled {
        bar.push('█');
    }
    for _ in filled..10 {
        bar.push('░');
    }
    bar
}

fn audit_summary_lines(report: &AuditReport, width: u16) -> Vec<Line<'static>> {
    let summary = &report.summary;
    let count = |severity: Severity| summary.by_severity.get(&severity).copied().unwrap_or(0);

    let mut lines = vec![
        Line::from(Span::styled(
            format!(" {} findings", summary.total_findings),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                format!("  {} critical", count(Severity::Critical)),
                Style::default().fg(severity_color(Severity::Critical)),
            ),
            Span::raw(" · "),
            Span::styled(
                format!("{} warning", count(Severity::Warning)),
                Style::default().fg(severity_color(Severity::Warning)),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("  {} info", count(Severity::Info)),
                Style::default().fg(severity_color(Severity::Info)),
            ),
            Span::raw(" · "),
            Span::styled(
                format!("{} suggestion", count(Severity::Suggestion)),
                Style::default().fg(severity_color(Severity::Suggestion)),
            ),
        ]),
        Line::from(Span::styled(
            format!("  {} auto-fixable", summary.auto_fixable_count),
            Style::default().fg(Color::Green),
        )),
        Line::from(""),
    ];

    for agent in &report.agents {
        let bar = health_bar(agent.health_score);
        let grade = grade_label(agent.grade);
        let row = format!(
            " {:<12} {} {:>3} {}",
            agent.detected.spec.name, bar, agent.health_score, grade
        );
        lines.push(Line::from(Span::styled(
            truncate_to_width(&row, width),
            Style::default().fg(grade_color(agent.grade)),
        )));
    }

    lines
}

fn draw_audit_list(f: &mut Frame, app: &App, area: Rect) {
    let list_width = area.width.saturating_sub(2);

    let Some(report) = app.audit_report.as_ref() else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Findings (All) ")
            .border_style(Style::default().fg(Color::Cyan));
        let paragraph = Paragraph::new(Line::from(Span::styled(
            "  Press r to run the audit",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(paragraph, area);
        return;
    };

    let filter_label = match app.audit_filter {
        None => "All".to_string(),
        Some(min) => format!("{}+", severity_label(min)),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Findings ({}) ", filter_label))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let summary_lines = audit_summary_lines(report, list_width);
    let summary_height = summary_lines.len() as u16;
    let chunks =
        Layout::vertical([Constraint::Length(summary_height), Constraint::Min(0)]).split(inner);
    let summary = Paragraph::new(summary_lines);
    f.render_widget(summary, chunks[0]);

    let mut items: Vec<ListItem> = Vec::new();
    let groups = app.audit_groups(report);
    for (severity, findings) in &groups {
        if findings.is_empty() {
            continue;
        }
        items.push(ListItem::new(Line::from(Span::styled(
            format!(
                " ▼ {} ({})",
                severity_label(*severity).to_uppercase(),
                findings.len()
            ),
            Style::default()
                .fg(severity_color(*severity))
                .add_modifier(Modifier::BOLD),
        ))));
        for finding in findings {
            let agent = finding.agent_id.as_deref().unwrap_or("-");
            let row = format!("   [{}] {} — {}", agent, finding.check_id, finding.message);
            items.push(ListItem::new(Line::from(Span::styled(
                truncate_to_width(&row, list_width),
                Style::default().fg(Color::White),
            ))));
        }
    }

    let mut state = ListState::default();
    if app.list_selected < items.len() {
        state.select(Some(app.list_selected));
    }

    let list = List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, chunks[1], &mut state);
}

fn draw_audit_detail(f: &mut Frame, app: &App, area: Rect) {
    let detail_width = area.width.saturating_sub(4);

    let lines = if let Some(finding) = app.selected_finding() {
        let mut detail_lines = vec![
            Line::from(Span::styled(
                format!(" {} ", finding.check_id),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Severity:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    severity_label(finding.severity),
                    Style::default()
                        .fg(severity_color(finding.severity))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Category:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:?}", finding.category),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Agent:     ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    finding.agent_id.clone().unwrap_or_else(|| "-".to_string()),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        if finding.auto_fixable {
            detail_lines.push(Line::from(vec![
                Span::styled("  Auto-fix:  ", Style::default().fg(Color::Yellow)),
                Span::styled("yes", Style::default().fg(Color::Green)),
            ]));
        }

        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled(
            " ── Message ──────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        for line in finding.message.lines().take(10) {
            detail_lines.push(Line::from(Span::styled(
                format!(
                    " {}",
                    truncate_to_width(line, detail_width.saturating_sub(1))
                ),
                Style::default().fg(Color::White),
            )));
        }

        if let Some(ref evidence) = finding.evidence {
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled(
                " ── Evidence ─────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            for line in evidence.lines().take(10) {
                detail_lines.push(Line::from(Span::styled(
                    format!(
                        " {}",
                        truncate_to_width(line, detail_width.saturating_sub(1))
                    ),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }

        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled(
            " ── Remediation ──────────────────────",
            Style::default().fg(Color::DarkGray),
        )));
        for line in finding.remediation.lines().take(10) {
            detail_lines.push(Line::from(Span::styled(
                format!(
                    " {}",
                    truncate_to_width(line, detail_width.saturating_sub(1))
                ),
                Style::default().fg(Color::Green),
            )));
        }

        detail_lines
    } else if let Some(ref report) = app.audit_report {
        let mut detail_lines = audit_summary_lines(report, detail_width);
        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled(
            "  Select a finding to view details",
            Style::default().fg(Color::DarkGray),
        )));
        detail_lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a finding to view details",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  r: Run audit  f: Cycle severity filter",
                Style::default().fg(Color::Yellow),
            )),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Finding Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

// ── Help ─────────────────────────────────────────────────────────────────

fn draw_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(
            " agentry — Keybindings ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
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
            Span::styled("  1-5        ", Style::default().fg(Color::Yellow)),
            Span::raw("Jump to tab (1=Agents, 2=Prompts, ...)"),
        ]),
        Line::from(Span::styled(
            " ── Agents ──────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  ←/→       ", Style::default().fg(Color::Yellow)),
            Span::raw("Select install method"),
        ]),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
            Span::raw("Install via selected method"),
        ]),
        Line::from(vec![
            Span::styled("  u          ", Style::default().fg(Color::Yellow)),
            Span::raw("Update via selected method"),
        ]),
        Line::from(vec![
            Span::styled("  r          ", Style::default().fg(Color::Yellow)),
            Span::raw("Remove via selected method"),
        ]),
        Line::from(vec![
            Span::styled("  v          ", Style::default().fg(Color::Yellow)),
            Span::raw("List available versions"),
        ]),
        Line::from(Span::styled(
            " ── Prompts ─────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
            Span::raw("Edit prompt via $EDITOR"),
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
            Span::styled("  e          ", Style::default().fg(Color::Yellow)),
            Span::raw("Edit prompt (alias)"),
        ]),
        Line::from(Span::styled(
            " ── Skills ──────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  i/Enter    ", Style::default().fg(Color::Yellow)),
            Span::raw("Install skill"),
        ]),
        Line::from(vec![
            Span::styled("  u          ", Style::default().fg(Color::Yellow)),
            Span::raw("Update skill"),
        ]),
        Line::from(vec![
            Span::styled("  r          ", Style::default().fg(Color::Yellow)),
            Span::raw("Remove skill"),
        ]),
        Line::from(vec![
            Span::styled("  g          ", Style::default().fg(Color::Yellow)),
            Span::raw("Open GitHub source"),
        ]),
        Line::from(Span::styled(
            " ── Sync ────────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  s          ", Style::default().fg(Color::Yellow)),
            Span::raw("Load/execute sync plan"),
        ]),
        Line::from(vec![
            Span::styled("  w          ", Style::default().fg(Color::Yellow)),
            Span::raw("Generate workflow"),
        ]),
        Line::from(Span::styled(
            " ── Audit ───────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  r          ", Style::default().fg(Color::Yellow)),
            Span::raw("Re-run audit"),
        ]),
        Line::from(vec![
            Span::styled("  f          ", Style::default().fg(Color::Yellow)),
            Span::raw("Cycle severity filter"),
        ]),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
            Span::raw("Open finding file / show remediation"),
        ]),
        Line::from(vec![
            Span::styled("  j/k        ", Style::default().fg(Color::Yellow)),
            Span::raw("Navigate findings"),
        ]),
        Line::from(Span::styled(
            " ── General ─────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  ?          ", Style::default().fg(Color::Yellow)),
            Span::raw("Toggle this help"),
        ]),
        Line::from(vec![
            Span::styled("  q          ", Style::default().fg(Color::Yellow)),
            Span::raw("Quit"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  Press ? or Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let width = 52.min(area.width);
    let height = 36.min(area.height);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_bar_at_bounds_and_middle() {
        assert_eq!(health_bar(0), "░░░░░░░░░░");
        assert_eq!(health_bar(50), "█████░░░░░");
        assert_eq!(health_bar(100), "██████████");
    }

    #[test]
    fn health_bar_clamps_out_of_range_scores() {
        assert_eq!(health_bar(150), "██████████");
        assert_eq!(health_bar(82), "████████░░");
    }

    #[test]
    fn agent_detail_shows_not_audited_without_report() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![agentry_core::models::DetectedAgent {
            spec: agentry_core::models::AgentSpec {
                id: "codex".to_string(),
                name: "codex".to_string(),
                cli_binary: "codex".to_string(),
                config_dir: ".codex".to_string(),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: agentry_core::models::PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: Vec::new(),
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: true,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }];

        let backend = ratatui::backend::TestBackend::new(60, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_agent_detail_enhanced(f, &app, f.area()))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let rendered: String = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(rendered.contains("not audited (press r in Audit tab)"));
    }

    fn render_dashboard_to_string(app: &App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_dashboard(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn dashboard_renders_keymap_bar_on_agents_tab() {
        let mut app = App::new();
        app.tab_index = 0;
        let rendered = render_dashboard_to_string(&app, 200, 24);
        assert!(rendered.contains("Enter Install"));
        assert!(rendered.contains("j Next item"));
        assert!(!rendered.contains("j/k:navigate"));
    }

    #[test]
    fn dashboard_renders_keymap_bar_on_audit_tab() {
        let mut app = App::new();
        app.tab_index = 4;
        let rendered = render_dashboard_to_string(&app, 200, 24);
        assert!(rendered.contains("r Run audit"));
    }

    #[test]
    fn dashboard_keymap_bar_occupies_bottom_two_rows() {
        let app = App::new();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_dashboard(f, &app)).unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row_text = |y: u16| -> String {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<Vec<_>>()
                .join("")
        };
        assert!(row_text(22).contains("·") || row_text(22).trim().is_empty());
        assert!(row_text(23).contains("·"));
        assert!(row_text(23).contains("Quit"));
    }
}
