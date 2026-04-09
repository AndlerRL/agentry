use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};

use super::Tab;
use crate::app::App;

pub fn draw_dashboard(f: &mut Frame, app: &App) {
    let size = f.area();

    // Layout: top tabs | main content (left + right) | bottom status
    let chunks = Layout::vertical([
        Constraint::Length(3), // tabs
        Constraint::Min(10),   // main content
        Constraint::Length(1), // status bar
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

    // Status bar
    let status = app
        .status_message
        .as_deref()
        .unwrap_or("j/k:navigate  Tab:next-tab  s:sync  q:quit  ?:help");
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
            let status_color = if agent.installed {
                Color::Green
            } else {
                Color::Red
            };
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
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
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
                Span::styled(
                    format!("{}", agent.spec.prompt_format),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        if let Some(ref skills_dir) = agent.skills_dir {
            lines.push(Line::from(vec![
                Span::styled("  Skills:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!(
                        "{} ({} installed)",
                        skills_dir.display(),
                        agent.installed_skills.len()
                    ),
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

fn draw_prompts_list(f: &mut Frame, app: &App, area: Rect) {
    let mut items: Vec<ListItem> = Vec::new();

    // Global prompts
    let global_prompts: Vec<_> = app
        .prompts
        .iter()
        .filter(|p| matches!(p.scope, agentry_core::models::PromptScope::Global))
        .collect();

    if !global_prompts.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            " Global Prompts",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))));
        for prompt in &global_prompts {
            let selected = app.list_selected < app.prompts.len()
                && app.prompts[app.list_selected].id == prompt.id;
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  {} {}", prompt.name, if selected { "◄" } else { "" }),
                Style::default().fg(Color::White),
            ))));
        }
    }

    // Project prompts
    let project_prompts: Vec<_> = app
        .prompts
        .iter()
        .filter(|p| matches!(p.scope, agentry_core::models::PromptScope::Project { .. }))
        .collect();

    if !project_prompts.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            " Project Prompts",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ))));
        for prompt in &project_prompts {
            let scope_label = match &prompt.scope {
                agentry_core::models::PromptScope::Project { root } => {
                    root.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                }
                _ => "",
            };
            items.push(ListItem::new(Line::from(Span::styled(
                format!("  {}/{}", scope_label, prompt.name),
                Style::default().fg(Color::White),
            ))));
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
    let prompt = app.prompts.get(app.list_selected);

    let lines = if let Some(prompt) = prompt {
        let scope_label = match &prompt.scope {
            agentry_core::models::PromptScope::Global => "Global".to_string(),
            agentry_core::models::PromptScope::Project { root } => {
                format!("Project ({})", root.display())
            }
        };

        let mut lines = vec![
            Line::from(Span::styled(
                format!(" {} ", prompt.name),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Scope:    ", Style::default().fg(Color::Yellow)),
                Span::styled(scope_label, Style::default().fg(Color::White)),
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
                    prompt
                        .source_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ];

        if !prompt.description.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("  Desc:     ", Style::default().fg(Color::Yellow)),
                Span::styled(&prompt.description, Style::default().fg(Color::White)),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " ── Preview ──────────────────────────",
            Style::default().fg(Color::DarkGray),
        )));

        // Show first 20 lines of the prompt body
        for line in prompt.body.lines().take(20) {
            lines.push(Line::from(Span::styled(
                format!(" {}", line),
                Style::default().fg(Color::White),
            )));
        }

        lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Select a prompt to view details",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  n: New prompt",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(Span::styled(
                "  e: Edit prompt",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(Span::styled(
                "  d: Delete prompt",
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

fn draw_skills_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if let Some(ref hub) = app.skill_hub {
        let mut items: Vec<ListItem> = Vec::new();

        // Group by source
        let mut source_groups: std::collections::BTreeMap<String, Vec<&agentry_skills::hub::AvailableSkill>> = std::collections::BTreeMap::new();
        for skill in hub.skills.values() {
            let source_key = if skill.source.is_empty() {
                "unknown".to_string()
            } else {
                skill.source.clone()
            };
            source_groups.entry(source_key).or_default().push(skill);
        }

        for (source, skills) in &source_groups {
            let installed_count = skills.iter().filter(|s| s.installed).count();
            items.push(ListItem::new(Line::from(Span::styled(
                format!(" {} ({} installed)", source, installed_count),
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
                    Span::styled(
                        format!("  {} ", status),
                        Style::default().fg(status_color),
                    ),
                    Span::styled(
                        skill.name.clone(),
                        Style::default().fg(Color::White),
                    ),
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
    let total = app
        .skill_hub
        .as_ref()
        .map(|h| h.total_count())
        .unwrap_or(0);

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
    let lines = if let Some(ref hub) = app.skill_hub {
        let skills: Vec<_> = hub.skills.values().collect();
        if app.list_selected < skills.len() {
            let skill = skills[app.list_selected];

            let mut lines = vec![
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
                        if skill.installed { "Installed ✓" } else { "Not installed" },
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
                        if skill.source.is_empty() {
                            "—".to_string()
                        } else {
                            skill.source.clone()
                        },
                        Style::default().fg(Color::White),
                    ),
                ]),
            ];

            if !skill.description.is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("  Desc:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(&skill.description, Style::default().fg(Color::White)),
                ]));
            }

            if let Some(ref hash) = skill.installed_hash {
                lines.push(Line::from(vec![
                    Span::styled("  Hash:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(hash.clone(), Style::default().fg(Color::DarkGray)),
                ]));
            }

            if let Some(ref path) = skill.install_path {
                lines.push(Line::from(vec![
                    Span::styled("  Path:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        path.display().to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " ── Actions ──────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));

            if skill.installed {
                lines.push(Line::from(Span::styled(
                    "  u: Update  r: Remove  g: Open GitHub",
                    Style::default().fg(Color::Yellow),
                )));
            } else if !skill.source.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  i: Install  g: Open GitHub",
                    Style::default().fg(Color::Yellow),
                )));
            }

            lines
        } else {
            vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  Select a skill to view details",
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
    } else {
        vec![Line::from(Span::styled(
            "  No skill data available",
            Style::default().fg(Color::DarkGray),
        ))]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Skill Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn draw_sync_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.sync_results.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  Press 's' to load sync plan",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        // Group by prompt
        let mut prompt_groups: std::collections::BTreeMap<String, Vec<&crate::app::SyncResultEntry>> =
            std::collections::BTreeMap::new();
        for entry in &app.sync_results {
            prompt_groups
                .entry(entry.prompt_name.clone())
                .or_default()
                .push(entry);
        }

        let mut items = Vec::new();
        for (prompt_name, mappings) in &prompt_groups {
            items.push(ListItem::new(Line::from(Span::styled(
                format!(" {}", prompt_name),
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
            app.sync_results.iter().filter(|r| r.status == agentry_core::models::SyncStatus::Missing || r.status == agentry_core::models::SyncStatus::Outdated).count()
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
    let lines = if !app.sync_results.is_empty() && app.list_selected < app.sync_results.len() {
        let entry = &app.sync_results[app.list_selected];

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
                Span::styled(entry.destination.clone(), Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " ── Actions ──────────────────────────",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "  s: Execute sync  Tab: other tabs",
                Style::default().fg(Color::Yellow),
            )),
        ]
    } else if app.sync_results.is_empty() {
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
    } else {
        vec![Line::from("")]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Sync Details ")
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn draw_openclaw_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if let Some(ref oc_state) = app.openclaw_state {
        if oc_state.workspaces.is_empty() {
            let status = if oc_state.installed {
                "OpenClaw installed — no workspaces found"
            } else {
                "OpenClaw not installed"
            };
            vec![ListItem::new(Line::from(Span::styled(
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
            )))]
        } else {
            let mut items = Vec::new();

            // Header showing install status
            let status_icon = if oc_state.installed { "✓" } else { "✗" };
            let status_color = if oc_state.installed { Color::Green } else { Color::Red };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!(" {} OpenClaw ", status_icon), Style::default().fg(status_color)),
                Span::styled(
                    format!("({} workspace{})", oc_state.workspaces.len(), if oc_state.workspaces.len() == 1 { "" } else { "s" }),
                    Style::default().fg(Color::DarkGray),
                ),
            ])));
            items.push(ListItem::new(Line::from("")));

            for ws in &oc_state.workspaces {
                let default_marker = if ws.is_default { " (default)" } else { "" };
                let model_info = ws.model.as_deref().unwrap_or("default");
                items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {} ", if ws.is_default { "★" } else { "○" }),
                        Style::default().fg(if ws.is_default { Color::Yellow } else { Color::DarkGray }),
                    ),
                    Span::styled(
                        format!("{}{}", ws.name, default_marker),
                        Style::default().fg(Color::White),
                    ),
                    Span::styled(
                        format!(" [{}]", model_info),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])));

                // Show doc status
                let doc_icons = format!(
                    "    {}{}{}{}{}{}",
                    if ws.has_soul_md { "S" } else { "·" },
                    if ws.has_agents_md { "A" } else { "·" },
                    if ws.has_tools_md { "T" } else { "·" },
                    if ws.has_identity_md { "I" } else { "·" },
                    if ws.has_memory_md { "M" } else { "·" },
                    if ws.has_user_md { "U" } else { "·" },
                );
                items.push(ListItem::new(Line::from(Span::styled(
                    doc_icons,
                    Style::default().fg(Color::DarkGray),
                ))));
            }

            items
        }
    } else {
        vec![ListItem::new(Line::from(Span::styled(
            "  Not loaded",
            Style::default().fg(Color::DarkGray),
        )))]
    };

    let ws_count = app.openclaw_state.as_ref().map(|s| s.workspaces.len()).unwrap_or(0);
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
        } else if app.list_selected < oc_state.workspaces.len() {
            let ws = &oc_state.workspaces[app.list_selected];

            let mut lines = vec![
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
                        ws.workspace_path.display().to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
            ];

            if let Some(ref model) = ws.model {
                lines.push(Line::from(vec![
                    Span::styled("  Model:     ", Style::default().fg(Color::Yellow)),
                    Span::styled(model.clone(), Style::default().fg(Color::White)),
                ]));
            }

            if ws.is_default {
                lines.push(Line::from(vec![
                    Span::styled("  Default:   ", Style::default().fg(Color::Yellow)),
                    Span::styled("Yes ★", Style::default().fg(Color::Green)),
                ]));
            }

            // Document status
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " ── Workspace Docs ─────────────────────",
                Style::default().fg(Color::DarkGray),
            )));

            for doc in &ws.docs {
                let size_kb = doc.size_bytes as f64 / 1024.0;
                lines.push(Line::from(vec![
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
                lines.push(Line::from(Span::styled(
                    "  No docs found",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            // Lobster workflows
            if !ws.lobster_workflows.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " ── Lobster Workflows ───────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
                for wf in &ws.lobster_workflows {
                    lines.push(Line::from(Span::styled(
                        format!("  {} {}", "⚡", wf.name),
                        Style::default().fg(Color::White),
                    )));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                " ── Actions ──────────────────────────",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "  Enter: Edit doc  c: Create workspace",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(Span::styled(
                "  a: Add sub-agent  g: Open in shell",
                Style::default().fg(Color::Yellow),
            )));

            lines
        } else {
            vec![Line::from("")]
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
            Span::styled("  1-6        ", Style::default().fg(Color::Yellow)),
            Span::raw("Jump to tab"),
        ]),
        Line::from(vec![
            Span::styled("  Enter      ", Style::default().fg(Color::Yellow)),
            Span::raw("Open/Edit selected"),
        ]),
        Line::from(Span::styled(
            " ── Prompts ─────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
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
            Span::raw("Edit prompt"),
        ]),
        Line::from(vec![
            Span::styled("  s          ", Style::default().fg(Color::Yellow)),
            Span::raw("Sync to agents"),
        ]),
        Line::from(Span::styled(
            " ── Skills ──────────────────────",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("  i          ", Style::default().fg(Color::Yellow)),
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

    let width = 50.min(area.width);
    let height = 16.min(area.height);
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}
