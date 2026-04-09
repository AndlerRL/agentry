mod app;
mod ui;
mod event;
mod editor;

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
        Some(Commands::Sync { prompt, all, dry_run }) => cmd_sync(prompt, all, dry_run).await,
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

    let home = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default();
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
            println!("  {} {} → {}", icon, result.mapping.agent_id, result.message);
        }
    }

    Ok(())
}

async fn cmd_skills(action: Option<SkillsCommands>) -> Result<()> {
    match action {
        Some(SkillsCommands::List) => {
            println!("Skills list — not yet implemented (Phase 4)");
        }
        Some(SkillsCommands::Install { name }) => {
            println!("Installing skill '{}' — not yet implemented (Phase 4)", name);
        }
        Some(SkillsCommands::Update) => {
            println!("Updating skills — not yet implemented (Phase 4)");
        }
        Some(SkillsCommands::Remove { name }) => {
            println!("Removing skill '{}' — not yet implemented (Phase 4)", name);
        }
        None => {
            println!("Usage: agentry skills <list|install|update|remove>");
        }
    }
    Ok(())
}

async fn cmd_prompts(action: Option<PromptsCommands>) -> Result<()> {
    use agentry_core::discovery::discover_prompts;

    let home = std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let project_dirs = vec![home.join("Development")];

    match action {
        Some(PromptsCommands::List) => {
            let prompts = discover_prompts(&home, &project_dirs);
            println!("Discovered {} prompt(s):\n", prompts.len());
            for prompt in &prompts {
                let scope = match &prompt.scope {
                    agentry_core::models::PromptScope::Global => "global".to_string(),
                    agentry_core::models::PromptScope::Project { root } => {
                        format!("project:{}", root.file_name().unwrap_or_default().to_string_lossy())
                    }
                };
                println!("  {:<30} {:<12} {}", prompt.name, prompt.source_format, scope);
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
    match action {
        Some(OpenclawCommands::Workspaces) => {
            println!("OpenClaw workspaces — not yet implemented (Phase 5)");
        }
        None => {
            println!("Usage: agentry openclaw <workspaces>");
        }
    }
    Ok(())
}