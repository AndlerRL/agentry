mod app;
mod editor;
mod event;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agentry", version, about = "Multi-Agent Prompt Manager")]
#[command(long_about = "agentry — The Multi-Agent Prompt Manager\n\n\
Unified prompt management for Claude Code, Continue, OpenClaw, Codex, Gemini CLI, \
Amp, OpenCode, Firebender, DeepAgents, Antigravity, and Warp.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Detect installed agents on the system
    Detect,
    /// Sync prompts to agents
    Sync {
        /// Sync a specific prompt by name
        #[arg(short, long)]
        prompt: Option<String>,
        /// Sync all prompts
        #[arg(short, long)]
        all: bool,
        /// Dry run mode (show what would be done)
        #[arg(short, long)]
        dry_run: bool,
    },
    /// List, install, or update skills
    Skills {
        #[command(subcommand)]
        action: Option<SkillsCommands>,
    },
    /// List or manage prompts
    Prompts {
        #[command(subcommand)]
        action: Option<PromptsCommands>,
    },
    /// Browse OpenClaw workspaces
    Openclaw {
        #[command(subcommand)]
        action: Option<OpenclawCommands>,
    },
}

#[derive(Subcommand)]
enum SkillsCommands {
    /// List installed skills
    List,
    /// Install a skill from the hub
    Install {
        /// Skill name to install
        name: String,
    },
    /// Update all installed skills
    Update,
    /// Remove an installed skill
    Remove {
        /// Skill name to remove
        name: String,
    },
}

#[derive(Subcommand)]
enum PromptsCommands {
    /// List all discovered prompts
    List,
    /// Create a new global prompt
    New {
        /// Name for the new prompt
        name: String,
    },
}

#[derive(Subcommand)]
enum OpenclawCommands {
    /// List OpenClaw workspaces
    Workspaces,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Detect) => cmd_detect().await,
        Some(Commands::Sync {
            prompt,
            all,
            dry_run,
        }) => cmd_sync(prompt, all, dry_run).await,
        Some(Commands::Skills { action }) => cmd_skills(action).await,
        Some(Commands::Prompts { action }) => cmd_prompts(action).await,
        Some(Commands::Openclaw { action }) => cmd_openclaw(action).await,
        None => run_tui().await,
    }
}

async fn run_tui() -> Result<()> {
    use crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let mut app = app::App::new();
    let result = app.run(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn cmd_detect() -> Result<()> {
    println!("Detecting agents...\n");
    let agents = agentry_agents::detect_all_agents().await;
    let installed = agents.iter().filter(|a| a.installed).count();
    println!("Detected {}/{} agents:\n", installed, agents.len());
    for agent in &agents {
        let status = if agent.installed { "✓" } else { "✗" };
        let version = agent.version.as_deref().unwrap_or("---");
        println!("  {} {:<16} v{}", status, agent.spec.name, version);
    }
    Ok(())
}

async fn cmd_sync(prompt_name: Option<String>, all: bool, dry_run: bool) -> Result<()> {
    use agentry_core::discovery::discover_prompts;
    use agentry_sync::executor::execute_sync;
    use agentry_sync::planner::plan_sync;

    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let project_dirs = vec![home.join("Development")];

    let agents = agentry_agents::detect_all_agents().await;
    let prompts = discover_prompts(&home, &project_dirs);

    let prompts_to_sync: Vec<_> = if let Some(name) = prompt_name {
        prompts.into_iter().filter(|p| p.name == name).collect()
    } else if all {
        prompts
    } else {
        println!("Specify --prompt <name> or --all to sync");
        return Ok(());
    };

    if dry_run {
        println!("DRY RUN — no changes will be made\n");
    }

    for prompt in &prompts_to_sync {
        println!("Syncing: {}", prompt.name);
        let plan = plan_sync(prompt, &agents, &home);
        let results = execute_sync(prompt, &plan.mappings, dry_run);
        for result in &results {
            let icon = if result.success { "✓" } else { "✗" };
            println!(
                "  {} {} → {}",
                icon, result.mapping.agent_id, result.message
            );
        }
    }

    Ok(())
}

async fn cmd_skills(action: Option<SkillsCommands>) -> Result<()> {
    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();

    match action {
        Some(SkillsCommands::List) => {
            let hub = agentry_skills::hub::SkillHub::load(&home, &[])?;
            let installed = hub.installed_count();
            let total = hub.total_count();
            println!("Skills ({}/{} installed):\n", installed, total);

            // Group by source
            let mut source_groups: std::collections::BTreeMap<
                String,
                Vec<&agentry_skills::hub::AvailableSkill>,
            > = std::collections::BTreeMap::new();
            for skill in hub.skills.values() {
                let key = if skill.source.is_empty() {
                    "unknown".to_string()
                } else {
                    skill.source.clone()
                };
                source_groups.entry(key).or_default().push(skill);
            }

            for (source, skills) in &source_groups {
                let inst = skills.iter().filter(|s| s.installed).count();
                println!("  {} ({}/{} installed)", source, inst, skills.len());
                for skill in skills {
                    let status = if skill.installed { "✓" } else { "○" };
                    let desc = if skill.description.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", skill.description)
                    };
                    println!("    {} {}{}", status, skill.name, desc);
                }
                println!();
            }
        }
        Some(SkillsCommands::Install { name }) => {
            println!("Installing skill '{}'...", name);

            // Detect agents to find skills dirs
            let agents = agentry_agents::detect_all_agents().await;
            let skills_dirs: Vec<std::path::PathBuf> = agents
                .iter()
                .filter(|a| a.installed)
                .filter_map(|a| a.skills_dir.clone())
                .collect();

            // Find the skill in the hub or look up by source
            let hub = agentry_skills::hub::SkillHub::load(&home, &[])?;

            // Try to find skill by name in existing entries
            if let Some(skill) = hub.skills.get(&name) {
                if skill.installed {
                    println!("  Skill '{}' is already installed", name);
                    return Ok(());
                }
                let source = if skill.source.is_empty() {
                    // Try to find in known sources
                    println!("  No source found for '{}'. Try: agentry skills install <source>/<skill-path>", name);
                    return Ok(());
                } else {
                    skill.source.clone()
                };
                let result = agentry_skills::install::install_skill(
                    &home,
                    &source,
                    &skill.skill_path,
                    &skills_dirs,
                )?;
                let icon = if result.success { "✓" } else { "✗" };
                println!("  {} {}", icon, result.message);
            } else {
                println!(
                    "  Skill '{}' not found. Use skills.sh to browse available skills.",
                    name
                );
            }
        }
        Some(SkillsCommands::Update) => {
            println!("Updating all skills...");

            let agents = agentry_agents::detect_all_agents().await;
            let skills_dirs: Vec<std::path::PathBuf> = agents
                .iter()
                .filter(|a| a.installed)
                .filter_map(|a| a.skills_dir.clone())
                .collect();

            let results = agentry_skills::install::update_all_skills(&home, &skills_dirs);
            let ok = results.iter().filter(|r| r.success).count();
            for result in &results {
                let icon = if result.success { "✓" } else { "✗" };
                println!("  {} {} — {}", icon, result.skill_name, result.message);
            }
            println!("\nUpdated {}/{} skills", ok, results.len());
        }
        Some(SkillsCommands::Remove { name }) => {
            println!("Removing skill '{}'...", name);

            let agents = agentry_agents::detect_all_agents().await;
            let skills_dirs: Vec<std::path::PathBuf> = agents
                .iter()
                .filter(|a| a.installed)
                .filter_map(|a| a.skills_dir.clone())
                .collect();

            let result = agentry_skills::install::remove_skill(&home, &name, &skills_dirs)?;
            let icon = if result.success { "✓" } else { "✗" };
            println!("  {} {}", icon, result.message);
        }
        None => {
            println!("Usage: agentry skills <list|install|update|remove>");
        }
    }
    Ok(())
}

async fn cmd_prompts(action: Option<PromptsCommands>) -> Result<()> {
    use agentry_core::discovery::discover_prompts;

    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let project_dirs = vec![home.join("Development")];

    match action {
        Some(PromptsCommands::List) => {
            let prompts = discover_prompts(&home, &project_dirs);
            println!("Discovered {} prompt(s):\n", prompts.len());
            for prompt in &prompts {
                let scope = match &prompt.scope {
                    agentry_core::models::PromptScope::Global => "global".to_string(),
                    agentry_core::models::PromptScope::Project { root } => {
                        format!(
                            "project:{}",
                            root.file_name().unwrap_or_default().to_string_lossy()
                        )
                    }
                };
                println!(
                    "  {:<30} {:<12} {}",
                    prompt.name, prompt.source_format, scope
                );
            }
        }
        Some(PromptsCommands::New { name }) => {
            println!("Creating prompt '{}' — not yet implemented (Phase 2)", name);
        }
        None => {
            println!("Usage: agentry prompts <list|new>");
        }
    }
    Ok(())
}

async fn cmd_openclaw(action: Option<OpenclawCommands>) -> Result<()> {
    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default();

    match action {
        Some(OpenclawCommands::Workspaces) => {
            let installed = agentry_openclaw::discovery::is_openclaw_installed();
            if !installed {
                println!("OpenClaw CLI is not installed.");
                println!("Install from https://openclaw.dev\n");
            }

            let workspaces = agentry_openclaw::discovery::discover_workspaces(&home)?;
            if workspaces.is_empty() {
                println!("No OpenClaw workspaces found.");
                if installed {
                    println!("\nRun 'openclaw setup' to create a default workspace.");
                }
            } else {
                println!("OpenClaw Workspaces ({}):\n", workspaces.len());
                for ws in &workspaces {
                    let default_marker = if ws.is_default { " (default)" } else { "" };
                    let model_info = ws.model.as_deref().unwrap_or("default");
                    println!("  {}{} [{}]", ws.name, default_marker, model_info);
                    println!("    Path: {}", ws.workspace_path.display());

                    // Doc status
                    let docs: Vec<String> = ws.docs.iter().map(|d| d.name.clone()).collect();
                    if !docs.is_empty() {
                        println!("    Docs: {}", docs.join(", "));
                    }

                    // Lobster workflows
                    if !ws.lobster_workflows.is_empty() {
                        let wfs: Vec<String> = ws
                            .lobster_workflows
                            .iter()
                            .map(|w| w.name.clone())
                            .collect();
                        println!("    Workflows: {}", wfs.join(", "));
                    }
                    println!();
                }
            }
        }
        None => {
            println!("Usage: agentry openclaw <workspaces>");
        }
    }
    Ok(())
}
