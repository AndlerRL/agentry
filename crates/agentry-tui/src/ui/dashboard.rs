use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

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
        Some(Tab::OpenClaw) => {
            draw_openclaw_list(f, app, main[0]);
            draw_openclaw_detail(f, app, main[1]);
        }
        None => {}
    }

    let status = if let Some(ref err) = app.error_message {
        err.as_str()
    } else {
        app.status_message
            .as_deref()
            .unwrap_or("j/k:navigate  Tab:next-tab  s:sync  q:quit  ?:help")
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
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
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
                        Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
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
            let detected_methods: Vec<&str> = agent
                .detected_methods
                .iter()
                .map(|m| m.label())
                .collect();
            if detected_methods.is_empty() {
                agent_lines.push(Line::from(vec![
                    Span::styled("  Status:   ", Style::default().fg(Color::Yellow)),
                    Span::styled("Installed", Style::default().fg(Color::Green)),
                ]));
            } else {
                agent_lines.push(Line::from(vec![
                    Span::styled("  Detected: ", Style::default().fg(Color::Yellow)),
                    Span::styled(detected_methods.join(", "), Style::default().fg(Color::Green)),
                ]));
            }
        } else {
            agent_lines.push(Line::from(vec![
                Span::styled("  Status:   ", Style::default().fg(Color::Yellow)),
                Span::styled("Not installed", Style::default().fg(Color::Red)),
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
                    Span::styled(
                        format!("[{}] ", check),
                        Style::default().fg(check_color),
                    ),
                    Span::styled(
                        format!("{:<20}", method.label()),
                        Style::default().fg(Color::White),
                    ),
                ];
                // Show install command hint
                let hint = method.install_command(None);
                let hint_short = truncate_to_width(&hint, detail_width.saturating_sub(28));
                spans.push(Span::styled(hint_short, Style::default().fg(Color::DarkGray)));

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
                    root.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
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
            .filter(|(_, p)| {
                matches!(p.scope, agentry_core::models::PromptScope::Project { .. })
            })
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
                    format!(" {}", truncate_to_width(line, detail_width.saturating_sub(1))),
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
                format!(
                    " ── {} ({}/{}) ──",
                    source,
                    installed_count,
                    skills.len()
                ),
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
                        if skill.source.is_empty() { "—" } else { &skill.source },
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
        let mut prompt_groups: std::collections::BTreeMap<
            &str,
            Vec<&crate::app::SyncResultEntry>,
        > = std::collections::BTreeMap::new();
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

// ── OpenClaw Tab ─────────────────────────────────────────────────────────

fn draw_openclaw_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if let Some(ref oc_state) = app.openclaw_state {
        if oc_state.workspaces.is_empty() {
            let status = if oc_state.installed {
                "OpenClaw installed — no workspaces found"
            } else {
                "OpenClaw not installed"
            };
            vec![
                ListItem::new(Line::from(Span::styled(
                    format!("  {}", status),
                    Style::default().fg(Color::DarkGray),
                ))),
                ListItem::new(Line::from("")),
                ListItem::new(Line::from(Span::styled(
                    "  c: Create workspace (via openclaw CLI)",
                    Style::default().fg(Color::Yellow),
                ))),
                ListItem::new(Line::from(Span::styled(
                    "  a: Add sub-agent",
                    Style::default().fg(Color::Yellow),
                ))),
            ]
        } else {
            let mut items = Vec::new();

            let status_icon = if oc_state.installed { "✓" } else { "✗" };
            let status_color = if oc_state.installed {
                Color::Green
            } else {
                Color::Red
            };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {} OpenClaw ", status_icon),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    format!(
                        "({} workspace{})",
                        oc_state.workspaces.len(),
                        if oc_state.workspaces.len() == 1 {
                            ""
                        } else {
                            "s"
                        }
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ])));
            items.push(ListItem::new(Line::from("")));

            for ws in &oc_state.workspaces {
                let default_marker = if ws.is_default { " ★" } else { "" };
                let model_info = ws.model.as_deref().unwrap_or("default");

                // Doc status as compact badges
                let doc_badges = format!(
                    " {}{}{}{}{}{}",
                    if ws.has_soul_md { "S" } else { "·" },
                    if ws.has_agents_md { "A" } else { "·" },
                    if ws.has_tools_md { "T" } else { "·" },
                    if ws.has_identity_md { "I" } else { "·" },
                    if ws.has_memory_md { "M" } else { "·" },
                    if ws.has_user_md { "U" } else { "·" },
                );

                items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {:<20}", ws.name),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!("[{}]", model_info),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(default_marker, Style::default().fg(Color::Yellow)),
                    Span::styled(doc_badges, Style::default().fg(Color::DarkGray)),
                ])));
            }

            items
        }
    } else {
        vec![ListItem::new(Line::from(Span::styled(
            "  Not loaded",
            Style::default().fg(Color::DarkGray),
        )))]
    };

    let ws_count = app
        .openclaw_state
        .as_ref()
        .map(|s| s.workspaces.len())
        .unwrap_or(0);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" OpenClaw ({}) ", ws_count))
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

fn draw_openclaw_detail(f: &mut Frame, app: &App, area: Rect) {
    let detail_width = area.width.saturating_sub(4);

    let lines = if let Some(ref oc_state) = app.openclaw_state {
        if oc_state.workspaces.is_empty() {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  No OpenClaw workspaces found",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press 'c' to create a workspace via openclaw CLI",
                    Style::default().fg(Color::Yellow),
                )),
                Line::from(Span::styled(
                    "  Press 'a' to add a sub-agent",
                    Style::default().fg(Color::Yellow),
                )),
            ]
        } else {
            // Resolve workspace index (row 0 = status, row 1 = spacer, row 2+ = workspaces)
            let ws_row = app.list_selected.saturating_sub(2);
            if let Some(ws) = oc_state.workspaces.get(ws_row) {
                let mut detail_lines = vec![
                    Line::from(Span::styled(
                        format!(" {} ", ws.name),
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  ID:        ", Style::default().fg(Color::Yellow)),
                        Span::styled(&ws.id, Style::default().fg(Color::White)),
                    ]),
                    Line::from(vec![
                        Span::styled("  Path:      ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            truncate_to_width(
                                &ws.workspace_path.display().to_string(),
                                detail_width,
                            ),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]),
                ];

                if let Some(ref model) = ws.model {
                    detail_lines.push(Line::from(vec![
                        Span::styled("  Model:     ", Style::default().fg(Color::Yellow)),
                        Span::styled(model.clone(), Style::default().fg(Color::White)),
                    ]));
                }

                if ws.is_default {
                    detail_lines.push(Line::from(vec![
                        Span::styled("  Default:   ", Style::default().fg(Color::Yellow)),
                        Span::styled("Yes ★", Style::default().fg(Color::Green)),
                    ]));
                }

                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from(Span::styled(
                    " ── Workspace Docs ─────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));

                for doc in &ws.docs {
                    let size_kb = doc.size_bytes as f64 / 1024.0;
                    detail_lines.push(Line::from(vec![
                        Span::styled(
                            format!("  {:<14}", doc.doc_type.to_string()),
                            Style::default().fg(Color::White),
                        ),
                        Span::styled(
                            format!("{:>6.1} KB", size_kb),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                }

                if ws.docs.is_empty() {
                    detail_lines.push(Line::from(Span::styled(
                        "  No docs found",
                        Style::default().fg(Color::DarkGray),
                    )));
                }

                if !ws.lobster_workflows.is_empty() {
                    detail_lines.push(Line::from(""));
                    detail_lines.push(Line::from(Span::styled(
                        " ── Lobster Workflows ───────────────────",
                        Style::default().fg(Color::DarkGray),
                    )));
                    for wf in &ws.lobster_workflows {
                        detail_lines.push(Line::from(Span::styled(
                            format!("  {} {}", "⚡", wf.name),
                            Style::default().fg(Color::White),
                        )));
                    }
                }

                detail_lines.push(Line::from(""));
                detail_lines.push(Line::from(Span::styled(
                    " ── Actions ──────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
                detail_lines.push(Line::from(Span::styled(
                    "  Enter: Edit doc  n: Create workspace",
                    Style::default().fg(Color::Yellow),
                )));
                detail_lines.push(Line::from(Span::styled(
                    "  a: Add sub-agent  g: Open in shell",
                    Style::default().fg(Color::Yellow),
                )));

                detail_lines
            } else {
                vec![Line::from("")]
            }
        }
    } else {
        vec![Line::from(Span::styled(
            "  OpenClaw not loaded",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Workspace Detail ")
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
    let height = 23.min(area.height);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}
