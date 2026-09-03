mod app;
mod event;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agentry",
    version,
    disable_version_flag = true,
    about = "Multi-Agent Prompt Manager"
)]
#[command(long_about = "agentry — The Multi-Agent Prompt Manager\n\n\
Unified prompt management for Claude Code, Continue, OpenClaw, Codex, Gemini CLI, \
Amp, OpenCode, Firebender, DeepAgents, Antigravity, and Warp.\n\n\
First run? Try `agentry setup` for guided onboarding.")]
struct Cli {
    #[arg(short = 'v', long = "version", visible_short_alias = 'V', action = clap::ArgAction::Version)]
    version_flag: bool,
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
    /// Audit agent health and configuration
    Audit {
        /// Filter audit to a single agent id (e.g. claude-code)
        #[arg(long)]
        agent: Option<String>,
        /// Output the full AuditReport as JSON
        #[arg(long)]
        json: bool,
        /// Only show findings at or above this severity (critical|warning|info|suggestion)
        #[arg(long)]
        severity: Option<String>,
        /// Interactively apply auto-fixes to fixable findings
        #[arg(long)]
        fix: bool,
        /// Apply all auto-fixes without confirmation (requires --fix)
        #[arg(long, requires = "fix")]
        yes: bool,
    },
    /// Set up or run the agent auditor
    Auditor {
        #[command(subcommand)]
        action: AuditorCommands,
    },
}

#[derive(Subcommand)]
enum AuditorCommands {
    /// Write [auditor] config, canonical prompt if absent, adopt orphaned skills
    Setup,
    /// Run the audit, review findings with the host LLM, and print the merged report
    Review {
        /// Output the merged AuditReport as JSON
        #[arg(long)]
        json: bool,
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

/// Resolve the user's home directory, preferring $HOME, falling back to
/// `dirs::home_dir()`, and finally `/tmp` as a last resort. Emits a warning
/// to stderr when neither $HOME nor a platform home directory can be found.
fn resolve_home() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .ok()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| {
            eprintln!("warning: HOME environment variable not set, falling back to /tmp");
            std::path::PathBuf::from("/tmp")
        })
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
        Some(Commands::Audit {
            agent,
            json,
            severity,
            fix,
            yes,
        }) => cmd_audit(agent, json, severity, fix, yes).await,
        Some(Commands::Auditor { action }) => cmd_auditor(action).await,
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

    if let Some(msg) = app.panic_message {
        eprintln!("TUI panic: {msg}");
        return Err(anyhow::anyhow!("TUI panic: {msg}"));
    }

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

        if agent.installed {
            let detected_labels: Vec<&str> =
                agent.detected_methods.iter().map(|m| m.label()).collect();
            if detected_labels.is_empty() {
                println!("  {} {:<18} v{:<8}", status, agent.spec.name, version);
            } else {
                println!(
                    "  {} {:<18} v{:<8} via {}",
                    status,
                    agent.spec.name,
                    version,
                    detected_labels.join(", ")
                );
            }
        } else {
            let available: Vec<&str> = agent
                .spec
                .install_methods
                .iter()
                .filter(|m| m.available_on_os())
                .map(|m| m.method_key())
                .collect();
            if available.is_empty() {
                println!("  {} {:<18} {:<8}", status, agent.spec.name, "---");
            } else {
                println!(
                    "  {} {:<18} {:<8} [available: {}]",
                    status,
                    agent.spec.name,
                    "---",
                    available.join(", ")
                );
            }
        }
    }
    Ok(())
}

async fn cmd_sync(prompt_name: Option<String>, all: bool, dry_run: bool) -> Result<()> {
    use agentry_core::discovery::discover_prompts;

    let home = resolve_home();
    let project_dirs = vec![home.join(agentry_core::models::DEFAULT_PROJECT_DIR)];

    let agents = agentry_agents::detect_all_agents().await;
    let prompts = discover_prompts(&home, &project_dirs);

    let prompts_to_sync: Vec<_> = if let Some(name) = prompt_name {
        prompts.iter().filter(|p| p.name == name).cloned().collect()
    } else if all {
        prompts.clone()
    } else {
        println!("Specify --prompt <name> or --all to sync");
        return Ok(());
    };

    if dry_run {
        println!("DRY RUN — no changes will be made\n");
        for prompt in &prompts_to_sync {
            println!("Syncing: {}", prompt.name);
            let plan = agentry_sync::planner::plan_sync(prompt, &agents, &home);
            let results = agentry_sync::executor::execute_sync(prompt, &plan.mappings, true);
            for result in &results {
                let icon = if result.success { "✓" } else { "✗" };
                println!(
                    "  {} {} → {}",
                    icon, result.mapping.agent_id, result.message
                );
            }
        }
        return Ok(());
    }

    let mut registry = agentry_harness::HarnessRegistry::with_default_actions();
    registry.register(Box::new(agentry_auditor::action::AuditorReviewAction));
    for prompt in &prompts_to_sync {
        println!("Syncing: {}", prompt.name);
        let ctx =
            agentry_harness::HarnessContext::new(home.clone(), agents.clone(), prompts.clone());
        let input = agentry_harness::ActionInput::SyncExecute {
            prompt_id: Some(prompt.id.clone()),
            mappings: Vec::new(),
        };
        let result = registry.invoke_confirmed(&ctx, "sync.execute", input).await;
        match result {
            Ok(agentry_harness::ActionOutput::SyncExecuted { applied, skipped }) => {
                println!("  ✓ {applied} applied, {skipped} skipped");
            }
            Ok(_) => unreachable!("sync.execute returns SyncExecuted"),
            Err(err) => {
                println!("  ✗ {err}");
            }
        }
    }

    Ok(())
}

async fn cmd_skills(action: Option<SkillsCommands>) -> Result<()> {
    let home = resolve_home();

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

    let home = resolve_home();
    let project_dirs = vec![home.join(agentry_core::models::DEFAULT_PROJECT_DIR)];

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
    let home = resolve_home();

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

fn audit_version_lookup() -> impl Fn(&str, &str) -> Option<Vec<String>> {
    use agentry_agents::detector::{
        list_brew_versions, list_cargo_versions, list_npm_versions, list_pip_versions,
    };
    use agentry_core::models::InstallMethod;

    move |agent_id: &str, method_key: &str| -> Option<Vec<String>> {
        let spec = agentry_agents::all_agent_specs()
            .into_iter()
            .find(|spec| spec.id == agent_id)?;
        let method = spec
            .install_methods
            .iter()
            .find(|m| m.method_key() == method_key)?;
        match method {
            InstallMethod::Brew { formula, .. } => list_brew_versions(formula).ok(),
            InstallMethod::Npm { package } => list_npm_versions(package).ok(),
            InstallMethod::Cargo { crate_name } => list_cargo_versions(crate_name).ok(),
            InstallMethod::Pip { package } => list_pip_versions(package).ok(),
            _ => None,
        }
    }
}

fn audit_severity_filter(raw: &str) -> Option<agentry_audit::report::Severity> {
    match raw.to_ascii_lowercase().as_str() {
        "critical" => Some(agentry_audit::report::Severity::Critical),
        "warning" => Some(agentry_audit::report::Severity::Warning),
        "info" => Some(agentry_audit::report::Severity::Info),
        "suggestion" => Some(agentry_audit::report::Severity::Suggestion),
        _ => None,
    }
}

fn audit_finding_lines(finding: &agentry_audit::report::AuditFinding) -> Vec<String> {
    let mut lines = vec![format!("    [{}] {}", finding.check_id, finding.message)];
    lines.push(format!("      remediation: {}", finding.remediation));
    if finding.auto_fixable {
        lines.push("      auto-fixable".to_string());
    }
    lines
}

fn audit_print_findings(
    findings: &[agentry_audit::report::AuditFinding],
    min_severity: Option<agentry_audit::report::Severity>,
) {
    use agentry_audit::report::Severity;

    let mut by_severity: std::collections::BTreeMap<
        Severity,
        Vec<&agentry_audit::report::AuditFinding>,
    > = std::collections::BTreeMap::new();
    for finding in findings {
        if let Some(min) = min_severity {
            if finding.severity > min {
                continue;
            }
        }
        by_severity
            .entry(finding.severity)
            .or_default()
            .push(finding);
    }
    for (severity, group) in &by_severity {
        let label = match severity {
            Severity::Critical => "Critical",
            Severity::Warning => "Warning",
            Severity::Info => "Info",
            Severity::Suggestion => "Suggestion",
        };
        println!("  {}:", label);
        for finding in group {
            for line in audit_finding_lines(finding) {
                println!("{}", line);
            }
        }
    }
}

fn count_findings(report: &agentry_audit::report::AuditReport) -> usize {
    report
        .agents
        .iter()
        .map(|a| a.findings.len())
        .sum::<usize>()
        + report.global_findings.len()
}

fn fix_summary_line(applied: usize, attempted: usize, after: usize, before: usize) -> String {
    format!("{applied} of {attempted} fixes applied; {after} findings remain (was {before})")
}

async fn run_audit_with_detection(
    home: &std::path::Path,
    project_dirs: Vec<std::path::PathBuf>,
) -> agentry_audit::report::AuditReport {
    use agentry_audit::engine::{build_context, run_audit};

    let prompts = agentry_core::discovery::discover_prompts(home, &project_dirs);
    let mut ctx = build_context(home, prompts);
    ctx.version_lookup = Some(Box::new(audit_version_lookup()));
    let detected = agentry_agents::detect_all_agents().await;
    for agent in &mut ctx.agents {
        if let Some(found) = detected.iter().find(|d| d.spec.id == agent.spec.id) {
            agent.version = found.version.clone();
            agent.detected_methods = found.detected_methods.clone();
            agent.installed = found.installed;
        }
    }
    run_audit(&ctx)
}

async fn cmd_audit(
    agent: Option<String>,
    json: bool,
    severity: Option<String>,
    fix: bool,
    yes: bool,
) -> Result<()> {
    use agentry_audit::report::Severity;

    if fix && json {
        eprintln!("--fix and --json are mutually exclusive");
        std::process::exit(2);
    }

    let min_severity = match severity.as_deref() {
        Some(raw) => match audit_severity_filter(raw) {
            Some(parsed) => Some(parsed),
            None => {
                eprintln!(
                    "Invalid --severity '{}'. Use critical|warning|info|suggestion",
                    raw
                );
                std::process::exit(2);
            }
        },
        None => None,
    };

    let home = resolve_home();
    let project_dirs = vec![home.join(agentry_core::models::DEFAULT_PROJECT_DIR)];

    let mut report = run_audit_with_detection(&home, project_dirs.clone()).await;

    let history = match agentry_audit::history::load_history(&home) {
        Ok(history) => history,
        Err(err) => {
            eprintln!("warning: failed to load audit history: {}", err);
            Vec::new()
        }
    };
    agentry_audit::history::apply_feedback(&mut report, &history);

    if let Some(agent_id) = &agent {
        if !report.agents.iter().any(|a| &a.agent_id == agent_id) {
            eprintln!("Unknown agent id '{}'", agent_id);
            std::process::exit(2);
        }
        report.agents.retain(|a| &a.agent_id == agent_id);
        report.global_findings.clear();
        report.summary = agentry_audit::report::AuditSummary {
            total_findings: report.agents[0].findings.len(),
            by_severity: report.agents[0].findings.iter().fold(
                std::collections::BTreeMap::new(),
                |mut acc, f| {
                    *acc.entry(f.severity).or_insert(0) += 1;
                    acc
                },
            ),
            by_category: report.agents[0].findings.iter().fold(
                std::collections::BTreeMap::new(),
                |mut acc, f| {
                    *acc.entry(f.category).or_insert(0) += 1;
                    acc
                },
            ),
            auto_fixable_count: report.agents[0]
                .findings
                .iter()
                .filter(|f| f.auto_fixable)
                .count(),
            healthy_agents: usize::from(
                report.agents[0].grade == agentry_audit::report::HealthGrade::Healthy,
            ),
            degraded_agents: usize::from(
                report.agents[0].grade == agentry_audit::report::HealthGrade::Degraded,
            ),
        };
    }

    let has_critical = |report: &agentry_audit::report::AuditReport| {
        report
            .agents
            .iter()
            .any(|a| a.findings.iter().any(|f| f.severity == Severity::Critical))
            || report
                .global_findings
                .iter()
                .any(|f| f.severity == Severity::Critical)
    };

    if fix {
        let findings = agentry_audit::fix::fixable_findings(&report);
        if findings.is_empty() {
            println!("No auto-fixable findings.");
            if has_critical(&report) {
                std::process::exit(1);
            }
            return Ok(());
        }
        let before_count = report.summary.total_findings;
        let outcomes = if yes {
            let mut registry = agentry_harness::HarnessRegistry::with_default_actions();
            registry.register(Box::new(agentry_auditor::action::AuditorReviewAction));
            let ctx = agentry_harness::HarnessContext::new(
                home.clone(),
                agentry_agents::detect_all_agents().await,
                agentry_core::discovery::discover_prompts(&home, &project_dirs),
            )
            .with_report(Some(report.clone()));
            match registry
                .invoke_confirmed(
                    &ctx,
                    "fix.apply_all",
                    agentry_harness::ActionInput::FixApplyAll,
                )
                .await
            {
                Ok(agentry_harness::ActionOutput::FixAppliedAll { outcomes }) => outcomes,
                Ok(_) => unreachable!("fix.apply_all returns FixAppliedAll"),
                Err(err) => {
                    eprintln!("fix.apply_all failed: {}", err);
                    std::process::exit(1);
                }
            }
        } else {
            agentry_audit::fix::apply_fixes(&findings, &home, false)
        };
        for outcome in &outcomes {
            let icon = if outcome.success {
                "✓"
            } else if outcome.message == "skipped by user" {
                "○"
            } else {
                "✗"
            };
            println!("{} {} — {}", icon, outcome.check_id, outcome.message);
        }
        let succeeded_keys: Vec<(String, Option<String>)> = outcomes
            .iter()
            .filter(|o| o.success)
            .map(|o| (o.check_id.clone(), o.agent_id.clone()))
            .collect();

        let mut after_report = run_audit_with_detection(&home, project_dirs).await;
        agentry_audit::history::apply_feedback(&mut after_report, &history);
        if let Some(agent_id) = &agent {
            after_report.agents.retain(|a| &a.agent_id == agent_id);
            after_report.global_findings.clear();
        }
        let applied = succeeded_keys.len();
        let attempted = outcomes.len();
        if let Err(err) = agentry_audit::history::append_history(&home, &report, &succeeded_keys) {
            eprintln!("warning: failed to append audit history: {}", err);
        }

        let after_count = count_findings(&after_report);
        println!(
            "{}",
            fix_summary_line(applied, attempted, after_count, before_count)
        );

        if has_critical(&after_report) {
            std::process::exit(1);
        }
        return Ok(());
    }

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(output) => println!("{}", output),
            Err(err) => {
                eprintln!("Failed to serialize audit report: {}", err);
                std::process::exit(2);
            }
        }
    } else {
        println!(
            "Agent Audit — {}",
            report.generated_at.format("%Y-%m-%d %H:%M UTC")
        );
        println!();
        for agent_audit in &report.agents {
            println!(
                "  {} — {}/100 ({:?})",
                agent_audit.detected.spec.name, agent_audit.health_score, agent_audit.grade
            );
            audit_print_findings(&agent_audit.findings, min_severity);
            println!();
        }
        if !report.global_findings.is_empty() {
            println!("Global:");
            audit_print_findings(&report.global_findings, min_severity);
            println!();
        }
        let (displayed, auto_fixable) = match min_severity {
            Some(min) => {
                let findings = report
                    .agents
                    .iter()
                    .flat_map(|a| a.findings.iter())
                    .chain(report.global_findings.iter())
                    .filter(|f| f.severity <= min);
                let displayed = findings.clone().count();
                (displayed, findings.filter(|f| f.auto_fixable).count())
            }
            None => (
                report.summary.total_findings,
                report.summary.auto_fixable_count,
            ),
        };
        println!(
            "Summary: {} finding(s), {} auto-fixable",
            displayed, auto_fixable
        );
    }

    if let Err(err) = agentry_audit::history::append_history(&home, &report, &[]) {
        eprintln!("warning: failed to append audit history: {}", err);
    }

    if has_critical(&report) {
        std::process::exit(1);
    }

    Ok(())
}

async fn cmd_auditor(action: AuditorCommands) -> Result<()> {
    let home = resolve_home();
    match action {
        AuditorCommands::Setup => {
            let config = agentry_auditor::config::load_config(&home);
            let mut written = false;
            if config.host_cli.is_none() {
                agentry_auditor::config::write_config(&home, &config)
                    .map_err(|err| anyhow::anyhow!(err))?;
                written = true;
            }
            let prompt_written = agentry_auditor::config::write_canonical_prompt_if_absent(&home)
                .map_err(|err| anyhow::anyhow!(err))?;
            let adopted = agentry_auditor::config::adopt_orphaned_collection(&home)
                .map_err(|err| anyhow::anyhow!(err))?;
            println!("auditor setup complete");
            if written {
                println!("  wrote [auditor] defaults to ~/.agents/agentry.toml");
            }
            if prompt_written {
                println!("  wrote canonical prompt to ~/.agents/prompts/agentry-auditor.md");
            }
            if adopted {
                println!("  adopted context-engineering-collection into the skill lockfile");
            }
            Ok(())
        }
        AuditorCommands::Review { json } => {
            let project_dirs = vec![home.join(agentry_core::models::DEFAULT_PROJECT_DIR)];
            let report = run_audit_with_detection(&home, project_dirs.clone()).await;
            let agents = agentry_agents::detect_all_agents().await;
            let prompts = agentry_core::discovery::discover_prompts(&home, &project_dirs);
            let ctx = agentry_harness::HarnessContext::new(home.clone(), agents, prompts)
                .with_report(Some(report));
            let mut registry = agentry_harness::HarnessRegistry::with_default_actions();
            registry.register(Box::new(agentry_auditor::action::AuditorReviewAction));
            let output = registry
                .invoke_confirmed(
                    &ctx,
                    "auditor.review",
                    agentry_harness::ActionInput::AuditorReview {
                        focus_check_id: None,
                    },
                )
                .await
                .map_err(|err| anyhow::anyhow!(err))?;
            match output {
                agentry_harness::ActionOutput::AuditorMerged { added, report } => {
                    if json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        println!("Auditor review complete: {added} findings added");
                        let all: Vec<agentry_audit::report::AuditFinding> = report
                            .agents
                            .iter()
                            .flat_map(|agent| agent.findings.iter().cloned())
                            .chain(report.global_findings.iter().cloned())
                            .collect();
                        audit_print_findings(&all, None);
                    }
                }
                other => {
                    anyhow::bail!("auditor.review returned unexpected output: {other:?}")
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_severity_filter_parses_all_levels() {
        assert_eq!(
            audit_severity_filter("critical"),
            Some(agentry_audit::report::Severity::Critical)
        );
        assert_eq!(
            audit_severity_filter("WARNING"),
            Some(agentry_audit::report::Severity::Warning)
        );
        assert_eq!(
            audit_severity_filter("info"),
            Some(agentry_audit::report::Severity::Info)
        );
        assert_eq!(
            audit_severity_filter("suggestion"),
            Some(agentry_audit::report::Severity::Suggestion)
        );
        assert_eq!(audit_severity_filter("bogus"), None);
    }

    #[test]
    fn audit_version_lookup_resolves_known_and_rejects_unknown() {
        let lookup = audit_version_lookup();
        assert!(lookup("nonexistent-id", "npm").is_none());
        assert!(lookup("claude-code", "not-a-method").is_none());
    }

    #[test]
    fn fix_summary_line_formats_counts() {
        assert_eq!(
            fix_summary_line(3, 5, 7, 12),
            "3 of 5 fixes applied; 7 findings remain (was 12)"
        );
        assert_eq!(
            fix_summary_line(0, 0, 0, 0),
            "0 of 0 fixes applied; 0 findings remain (was 0)"
        );
    }
}
