use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{backend::Backend, Frame, Terminal};

use crate::ui;
use crate::ui::keymap::{resolve, TuiAction};
/// A single sync result for display in the Sync tab.
#[derive(Debug, Clone)]
pub struct SyncResultEntry {
    pub prompt_name: String,
    pub agent_id: String,
    pub destination: String,
    pub status: agentry_core::models::SyncStatus,
    pub action: agentry_core::models::SyncAction,
    pub mapping: agentry_core::models::SyncMapping,
}

/// OpenClaw workspace data loaded on startup.
pub struct OpenClawState {
    pub workspaces: Vec<agentry_openclaw::discovery::OpenClawWorkspace>,
    pub installed: bool,
}

/// Application state machine: Intro → Dashboard → (various tabs) → Quit
pub struct App {
    /// Current mode
    pub mode: AppMode,
    /// Current tab index (0-4)
    pub tab_index: usize,
    /// Currently selected item in list panels
    pub list_selected: usize,
    /// Agent detection results
    pub detected_agents: Vec<agentry_core::models::DetectedAgent>,
    /// Discovered prompts
    pub prompts: Vec<agentry_core::models::UnifiedPrompt>,
    /// Skill hub data
    pub skill_hub: Option<agentry_skills::hub::SkillHub>,
    /// Agent skills directories (for symlink creation)
    pub agent_skills_dirs: Vec<PathBuf>,
    /// Sync plan entries (populated when the Sync tab is entered)
    pub sync_results: Vec<SyncResultEntry>,
    /// True once the sync plan has been loaded for this session
    pub sync_loaded: bool,
    /// True once the audit has run for this session
    pub audit_loaded: bool,
    /// OpenClaw workspace data
    pub openclaw_state: Option<OpenClawState>,
    /// New prompt name (when creating a new prompt)
    pub new_prompt_name: Option<String>,
    /// Delete confirmation pending
    pub delete_confirm: Option<usize>,
    /// Home directory
    pub home_dir: PathBuf,
    /// Intro animation progress (0.0 → 1.0)
    pub intro_progress: f32,
    /// Intro start timestamp (for rotating brand phrases)
    pub intro_started_at: std::time::Instant,
    /// Intro animation: show "press any key"
    pub intro_ready: bool,
    /// Spinner state
    pub spinner_frame: usize,
    /// Should we quit?
    pub should_quit: bool,
    /// Status message
    pub status_message: Option<String>,
    /// Help visible
    pub show_help: bool,
    /// Skill action pending confirmation
    pub skill_confirm: Option<SkillConfirmAction>,
    /// Error message to display in the status bar (cleared on next key press)
    pub error_message: Option<String>,
    /// Set to true after external editor exits — main loop calls terminal.clear()
    pub needs_terminal_clear: bool,
    /// For Agents tab: which install method is highlighted in the detail panel.
    pub method_selected: usize,
    /// Agent install/update/remove confirmation pending.
    pub agent_confirm: Option<AgentConfirmAction>,
    /// Cached version list for the selected install method.
    pub version_list: Option<Vec<String>>,
    /// Selected index into version_list while choosing a version.
    pub version_selected: usize,
    /// Error fetching versions.
    pub version_list_error: Option<String>,
    pub audit_report: Option<agentry_audit::report::AuditReport>,
    pub audit_filter: Option<agentry_audit::report::Severity>,
    pub harness: agentry_harness::HarnessRegistry,
    pub harness_confirm: Option<HarnessPendingInvocation>,
}

pub struct HarnessPendingInvocation {
    pub action_id: String,
    pub input: agentry_harness::ActionInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Intro,
    Dashboard,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillConfirmAction {
    Install(String),
    Remove(String),
    Update(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentConfirmAction {
    Install {
        agent_id: String,
        method: agentry_core::models::InstallMethod,
        version: Option<String>,
    },
    Update {
        agent_id: String,
        method: agentry_core::models::InstallMethod,
    },
    Remove {
        agent_id: String,
        method: agentry_core::models::InstallMethod,
    },
}

impl AgentConfirmAction {
    pub fn agent_id(&self) -> &str {
        match self {
            AgentConfirmAction::Install { agent_id, .. }
            | AgentConfirmAction::Update { agent_id, .. }
            | AgentConfirmAction::Remove { agent_id, .. } => agent_id,
        }
    }
}

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

fn is_safe_version(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
}

fn parse_versions(stdout: &str, method: &agentry_core::models::InstallMethod) -> Vec<String> {
    match method {
        agentry_core::models::InstallMethod::Brew { .. } => {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(stdout) {
                let v = val["versions"]["stable"].as_str().unwrap_or("unknown");
                vec![v.to_string()]
            } else {
                vec!["parse error".into()]
            }
        }
        agentry_core::models::InstallMethod::Npm { .. } => {
            serde_json::from_str::<Vec<String>>(stdout).unwrap_or_default()
        }
        _ => stdout
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
    }
    .into_iter()
    .filter(|v| is_safe_version(v))
    .collect()
}

impl App {
    pub fn new() -> Self {
        let home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| {
                eprintln!("warning: HOME environment variable not set, falling back to /tmp");
                PathBuf::from("/tmp")
            });

        Self {
            mode: AppMode::Intro,
            tab_index: 0,
            list_selected: 0,
            detected_agents: Vec::new(),
            prompts: Vec::new(),
            skill_hub: None,
            agent_skills_dirs: Vec::new(),
            sync_results: Vec::new(),
            sync_loaded: false,
            audit_loaded: false,
            openclaw_state: None,
            new_prompt_name: None,
            delete_confirm: None,
            home_dir,
            intro_progress: 0.0,
            intro_started_at: std::time::Instant::now(),
            intro_ready: false,
            spinner_frame: 0,
            should_quit: false,
            status_message: None,
            show_help: false,
            skill_confirm: None,
            error_message: None,
            needs_terminal_clear: false,
            method_selected: 0,
            agent_confirm: None,
            version_list: None,
            version_selected: 0,
            version_list_error: None,
            audit_report: None,
            audit_filter: None,
            harness: {
                let mut registry = agentry_harness::HarnessRegistry::with_default_actions();
                registry.register(Box::new(agentry_auditor::action::AuditorReviewAction));
                registry
            },
            harness_confirm: None,
        }
    }

    pub async fn run<B: Backend + std::io::Write>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        // Run intro animation with agent detection
        self.run_intro(terminal).await?;

        if self.should_quit {
            return Ok(());
        }

        // Discover prompts
        self.discover_prompts();

        // Discover skills
        self.discover_skills();

        // Discover OpenClaw workspaces
        self.discover_openclaw();

        // Main event loop
        while !self.should_quit {
            if self.needs_terminal_clear {
                terminal.clear()?;
                self.needs_terminal_clear = false;
            }

            terminal.draw(|f| self.draw(f))?;

            // Poll for events with timeout
            if crossterm::event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key)?;
                }
            }

            // Advance spinner
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
        }

        Ok(())
    }

    fn discover_prompts(&mut self) {
        let project_dirs = vec![self.home_dir.join("Development")];
        self.prompts = agentry_core::discover_prompts(&self.home_dir, &project_dirs);
    }

    fn discover_skills(&mut self) {
        let extra_sources: Vec<String> = Vec::new();
        match agentry_skills::hub::SkillHub::load(&self.home_dir, &extra_sources) {
            Ok(hub) => {
                // Collect agent skills directories for symlink management
                let dirs: Vec<PathBuf> = self
                    .detected_agents
                    .iter()
                    .filter(|a| a.installed)
                    .filter_map(|a| a.skills_dir.clone())
                    .collect();
                self.agent_skills_dirs = dirs;
                self.skill_hub = Some(hub);
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to load skills: {}", e));
            }
        }
    }

    fn discover_openclaw(&mut self) {
        let installed = agentry_openclaw::discovery::is_openclaw_installed();
        match agentry_openclaw::discovery::discover_workspaces(&self.home_dir) {
            Ok(workspaces) => {
                self.openclaw_state = Some(OpenClawState {
                    workspaces,
                    installed,
                });
            }
            Err(e) => {
                self.openclaw_state = Some(OpenClawState {
                    workspaces: Vec::new(),
                    installed,
                });
                self.status_message = Some(format!("OpenClaw: {}", e));
            }
        }
    }

    async fn run_intro<B: Backend + std::io::Write>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        use tokio::sync::mpsc;

        // Spawn agent detection in background so we can animate immediately
        let (tx, mut rx) = mpsc::channel::<Vec<agentry_core::models::DetectedAgent>>(1);
        tokio::spawn(async move {
            let agents = agentry_agents::detect_all_agents().await;
            let _ = tx.send(agents).await;
        });

        // Animate progress while detection runs or until user skips
        let mut detection_done = false;
        loop {
            // Check if detection finished
            if !detection_done {
                if let Ok(agents) = rx.try_recv() {
                    self.detected_agents = agents;
                    self.intro_progress = 1.0;
                    detection_done = true;
                } else {
                    // Animate progress upward but never reach 1.0 until done
                    let target = 0.85;
                    self.intro_progress += (target - self.intro_progress) * 0.15;
                    self.intro_progress = self.intro_progress.min(target);
                }
            }

            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
            terminal.draw(|f| ui::draw_intro(f, self))?;

            // Check if user pressed a key to skip
            if crossterm::event::poll(std::time::Duration::from_millis(60))? {
                if let Event::Key(_) = event::read()? {
                    if !detection_done {
                        // Still waiting — drain the channel when it arrives
                        if let Some(agents) = rx.recv().await {
                            self.detected_agents = agents;
                        }
                    }
                    break;
                }
            }

            if detection_done {
                // Show the finished screen briefly, then wait for key press
                self.intro_ready = true;
                terminal.draw(|f| ui::draw_intro(f, self))?;
                break;
            }
        }

        // Wait for key press to continue
        loop {
            if crossterm::event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        self.mode = AppMode::Dashboard;
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear any previous error message on next key press
        if self.error_message.is_some() {
            self.error_message = None;
        }

        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                _ => {}
            }
            return Ok(());
        }

        // Handle delete confirmation
        if self.delete_confirm.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    if let Some(idx) = self.delete_confirm.take() {
                        if idx < self.prompts.len() {
                            let prompt = &self.prompts[idx];
                            let name = prompt.name.clone();
                            if let Err(e) = agentry_core::delete_prompt(&self.home_dir, &name) {
                                self.status_message = Some(format!("Error deleting: {}", e));
                            } else {
                                self.prompts.remove(idx);
                                self.status_message = Some(format!("Deleted prompt: {}", name));
                                self.discover_prompts();
                            }
                        }
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.delete_confirm = None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle agent confirm action
        if self.agent_confirm.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let action = self.agent_confirm.take();
                    if let Some(action) = action {
                        self.execute_agent_action(action);
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.agent_confirm = None;
                    self.status_message = Some("Cancelled".into());
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle skill confirm action
        if self.skill_confirm.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let action = self.skill_confirm.take();
                    if let Some(action) = action {
                        self.execute_skill_action(action);
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.skill_confirm = None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle harness confirm action
        if self.harness_confirm.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let pending = self.harness_confirm.take();
                    if let Some(pending) = pending {
                        self.invoke_harness(pending);
                    }
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.harness_confirm = None;
                    self.status_message = Some("Cancelled".into());
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle new prompt name input
        if self.new_prompt_name.is_some() {
            match key.code {
                KeyCode::Enter => {
                    if let Some(name) = self.new_prompt_name.take() {
                        if !name.is_empty() {
                            // Create a new empty global prompt
                            let prompt_path = self
                                .home_dir
                                .join(".agents")
                                .join("prompts")
                                .join(format!("{}.md", name));
                            if let Some(parent) = prompt_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            // Write an empty template
                            let template =
                                format!("# {}\n\n<!-- Write your prompt content here -->\n", name);
                            if let Err(e) = std::fs::write(&prompt_path, &template) {
                                self.error_message =
                                    Some(format!("Failed to create prompt: {}", e));
                            } else {
                                self.status_message = Some(format!("Created prompt: {}", name));
                                // Reload prompts
                                self.discover_prompts();
                                // Open it in external editor
                                self.edit_file_externally(&prompt_path);
                                // Reload the content
                                if let Ok(content) = std::fs::read_to_string(&prompt_path) {
                                    // Find the new prompt and update it
                                    if let Some(p) =
                                        self.prompts.iter_mut().find(|p| p.name == name)
                                    {
                                        p.body = content;
                                    }
                                }
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    self.new_prompt_name = None;
                }
                KeyCode::Char(c) => {
                    if let Some(ref mut name) = self.new_prompt_name {
                        name.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(ref mut name) = self.new_prompt_name {
                        name.pop();
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        let key_string = match key.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "BackTab".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            _ => return Ok(()),
        };

        match resolve(self.tab_index, self, &key_string) {
            Some(TuiAction::Quit) => {
                self.mode = AppMode::Quit;
                self.should_quit = true;
            }
            Some(TuiAction::Help) => self.show_help = !self.show_help,
            Some(TuiAction::NextTab) => self.next_tab(),
            Some(TuiAction::PrevTab) => self.prev_tab(),
            Some(TuiAction::JumpTab(i)) => {
                self.tab_index = i;
                self.reset_tab_selection();
                self.maybe_autoload_sync();
                self.maybe_autoload_audit();
            }
            Some(TuiAction::ListNext) => self.list_next(),
            Some(TuiAction::ListPrev) => self.list_prev(),
            Some(TuiAction::Enter) => self.on_enter(),
            Some(TuiAction::New) => self.on_new(),
            Some(TuiAction::Delete) => self.on_delete(),
            Some(TuiAction::SyncExecuteSelected) => self.execute_selected_sync(),
            Some(TuiAction::SyncExecuteAll) => self.execute_all_sync(),
            Some(TuiAction::Edit) => self.on_edit(),
            Some(TuiAction::Insert) => self.on_insert(),
            Some(TuiAction::Update) => self.on_update(),
            Some(TuiAction::Remove) => self.on_remove(),
            Some(TuiAction::RunAudit) => self.on_run_audit(),
            Some(TuiAction::CycleAuditFilter) => self.on_cycle_audit_filter(),
            Some(TuiAction::Github) => self.on_github(),
            Some(TuiAction::CreateWorkspace) => self.on_create_workspace(),
            Some(TuiAction::AddAgent) => self.on_add_agent(),
            Some(TuiAction::MethodPrev) => self.method_prev(),
            Some(TuiAction::MethodNext) => self.method_next(),
            Some(TuiAction::ListVersions) => self.on_list_versions(),
            Some(TuiAction::CancelVersion) => {
                self.version_list = None;
                self.version_selected = 0;
                self.status_message = Some("Version selection cancelled".into());
            }
            Some(TuiAction::Harness(invocation)) => {
                let input = self.resolve_harness_input(&invocation);
                match input {
                    Ok(input) => self.prepare_harness(&invocation.action_id, input),
                    Err(err) => {
                        self.error_message = Some(err);
                    }
                }
            }
            None => {}
        }
        Ok(())
    }

    fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % 5;
        self.reset_tab_selection();
        self.maybe_autoload_sync();
        self.maybe_autoload_audit();
    }

    fn prev_tab(&mut self) {
        self.tab_index = if self.tab_index == 0 {
            4
        } else {
            self.tab_index - 1
        };
        self.reset_tab_selection();
        self.maybe_autoload_sync();
        self.maybe_autoload_audit();
    }

    fn reset_tab_selection(&mut self) {
        self.list_selected = 0;
        self.method_selected = 0;
        self.version_list = None;
        self.version_selected = 0;
    }

    fn maybe_autoload_sync(&mut self) {
        if self.tab_index == 3 && !self.sync_loaded {
            self.load_sync_plan();
            self.sync_loaded = true;
        }
    }

    fn maybe_autoload_audit(&mut self) {
        if self.tab_index == 4 && !self.audit_loaded {
            self.on_run_audit();
            self.audit_loaded = true;
        }
    }

    fn list_next(&mut self) {
        if self.version_list.is_some() {
            let max = self.version_list.as_ref().map(|v| v.len()).unwrap_or(0);
            if max > 0 && self.version_selected < max - 1 {
                self.version_selected += 1;
            }
            return;
        }
        let max = self.list_max();
        if max > 0 && self.list_selected < max - 1 {
            self.list_selected += 1;
        }
    }

    fn list_prev(&mut self) {
        if self.version_list.is_some() {
            self.version_selected = self.version_selected.saturating_sub(1);
            return;
        }
        if self.list_selected > 0 {
            self.list_selected -= 1;
        }
    }

    /// Build the confirm action installing the currently selected version,
    /// then close the version picker.
    fn confirm_selected_version(&mut self) {
        let Some(agent) = self.detected_agents.get(self.list_selected) else {
            self.version_list = None;
            self.version_selected = 0;
            return;
        };
        let Some(method) = agent.spec.install_methods.get(self.method_selected) else {
            self.version_list = None;
            self.version_selected = 0;
            return;
        };
        let version = self
            .version_list
            .as_ref()
            .and_then(|v| v.get(self.version_selected))
            .cloned();
        match version {
            Some(v) if is_safe_version(&v) => {
                self.agent_confirm = Some(AgentConfirmAction::Install {
                    agent_id: agent.spec.id.clone(),
                    method: method.clone(),
                    version: Some(v.clone()),
                });
                self.status_message = Some(format!("Install {} {}? (y/n)", agent.spec.name, v));
            }
            Some(_) => {
                self.error_message = Some("Invalid version string".into());
            }
            None => {
                self.status_message = Some("No version selected".into());
            }
        }
        self.version_list = None;
        self.version_selected = 0;
    }

    /// Total number of items in the current tab's list (including headers/actions).
    fn list_max(&self) -> usize {
        match self.tab_index {
            0 => self.detected_agents.len().max(1), // Agents tab
            1 => {
                // Prompts tab
                // Prompts: global header + global prompts + project header + project prompts + new action
                let has_global = self
                    .prompts
                    .iter()
                    .any(|p| matches!(p.scope, agentry_core::models::PromptScope::Global));
                let has_project = self
                    .prompts
                    .iter()
                    .any(|p| matches!(p.scope, agentry_core::models::PromptScope::Project { .. }));
                let mut count = 0;
                if has_global {
                    count += 1; // header
                    count += self
                        .prompts
                        .iter()
                        .filter(|p| matches!(p.scope, agentry_core::models::PromptScope::Global))
                        .count();
                }
                if has_project {
                    count += 1; // header
                    count += self
                        .prompts
                        .iter()
                        .filter(|p| {
                            matches!(p.scope, agentry_core::models::PromptScope::Project { .. })
                        })
                        .count();
                }
                count += 1; // "New Global Prompt" action
                count.max(1)
            }
            2 => {
                // Skills grouped by source: each source has a header + its skills
                if let Some(ref hub) = self.skill_hub {
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
                    let mut count = 0;
                    for skills in source_groups.values() {
                        count += 1; // source header
                        count += skills.len(); // skills under this source
                    }
                    count.max(1)
                } else {
                    1
                }
            }
            3 => {
                // Sync: grouped by prompt name — each group has header + entries
                if self.sync_results.is_empty() {
                    1 // placeholder
                } else {
                    let mut groups: std::collections::BTreeMap<
                        &str,
                        Vec<&crate::app::SyncResultEntry>,
                    > = std::collections::BTreeMap::new();
                    for entry in &self.sync_results {
                        groups.entry(&entry.prompt_name).or_default().push(entry);
                    }
                    let mut count = 0;
                    for entries in groups.values() {
                        count += 1; // prompt name header
                        count += entries.len(); // mappings
                    }
                    count.max(1)
                }
            }
            4 => {
                let report = match self.audit_report.as_ref() {
                    Some(r) => r,
                    None => return 0,
                };
                let mut count = 0;
                for findings in self.audit_groups(report).values() {
                    if findings.is_empty() {
                        continue;
                    }
                    count += 1;
                    count += findings.len();
                }
                count
            }
            _ => 0,
        }
    }

    /// Resolve the selected prompt index (Prompts tab). Returns None if a header or action is selected.
    fn selected_prompt_index(&self) -> Option<usize> {
        if self.tab_index != 1 {
            return None;
        }
        let global_prompts: Vec<(usize, &agentry_core::models::UnifiedPrompt)> = self
            .prompts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.scope, agentry_core::models::PromptScope::Global))
            .collect();
        let project_prompts: Vec<(usize, &agentry_core::models::UnifiedPrompt)> = self
            .prompts
            .iter()
            .enumerate()
            .filter(|(_, p)| matches!(p.scope, agentry_core::models::PromptScope::Project { .. }))
            .collect();

        let mut list_row = 0;

        // Global header
        if !global_prompts.is_empty() {
            if self.list_selected == list_row {
                return None;
            }
            list_row += 1;
            // Global prompts
            for (orig_idx, _) in &global_prompts {
                if self.list_selected == list_row {
                    return Some(*orig_idx);
                }
                list_row += 1;
            }
        }

        // Project header
        if !project_prompts.is_empty() {
            if self.list_selected == list_row {
                return None;
            }
            list_row += 1;
            // Project prompts
            for (orig_idx, _) in &project_prompts {
                if self.list_selected == list_row {
                    return Some(*orig_idx);
                }
                list_row += 1;
            }
        }

        // "New Global Prompt" action
        None
    }

    /// True if list_selected points to the "New Global Prompt" action row.
    pub fn list_is_new_prompt_action(&self) -> bool {
        if self.tab_index != 1 {
            return false;
        }
        let total = self.list_max();
        self.list_selected == total.saturating_sub(1) && total > 0
    }

    /// Resolve the selected skill (Skills tab). Returns None if a header is selected.
    fn selected_skill(&self) -> Option<&agentry_skills::hub::AvailableSkill> {
        if self.tab_index != 2 {
            return None;
        }
        let hub = self.skill_hub.as_ref()?;

        // Build the same grouped structure as draw_skills_list
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

        let mut list_row = 0;
        for skills in source_groups.values() {
            // Source header
            if self.list_selected == list_row {
                return None;
            }
            list_row += 1;
            // Skills in this group
            for skill in skills {
                if self.list_selected == list_row {
                    return Some(skill);
                }
                list_row += 1;
            }
        }
        None
    }

    /// Collect flat list of skills with their original indices (same order as draw_skills_list).
    fn selected_skill_index(&self) -> Option<usize> {
        if self.tab_index != 2 {
            return None;
        }
        let hub = self.skill_hub.as_ref()?;
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
        for group_skills in source_groups.values() {
            if self.list_selected == list_row {
                return None; // header
            }
            list_row += 1;
            for (orig_idx, _) in group_skills {
                if self.list_selected == list_row {
                    return Some(*orig_idx);
                }
                list_row += 1;
            }
        }
        None
    }

    /// Resolve the selected sync entry (Sync tab). Returns None if a header is selected.
    fn selected_sync_entry(&self) -> Option<&crate::app::SyncResultEntry> {
        if self.tab_index != 3 || self.sync_results.is_empty() {
            return None;
        }
        let mut prompt_groups: std::collections::BTreeMap<
            &str,
            Vec<(usize, &crate::app::SyncResultEntry)>,
        > = std::collections::BTreeMap::new();
        for (i, entry) in self.sync_results.iter().enumerate() {
            prompt_groups
                .entry(&entry.prompt_name)
                .or_default()
                .push((i, entry));
        }

        let mut list_row = 0;
        for entries in prompt_groups.values() {
            if self.list_selected == list_row {
                return None; // header
            }
            list_row += 1;
            for (_orig_idx, entry) in entries {
                if self.list_selected == list_row {
                    return Some(entry);
                }
                list_row += 1;
            }
        }
        None
    }

    fn selected_agent(&self) -> Option<&agentry_core::models::DetectedAgent> {
        self.detected_agents.get(self.list_selected)
    }

    pub(crate) fn selected_agent_is_openclaw(&self) -> bool {
        self.selected_agent()
            .is_some_and(|a| a.spec.id == "openclaw")
    }

    pub(crate) fn openclaw_default_doc_path(&self) -> Option<std::path::PathBuf> {
        let oc_state = self.openclaw_state.as_ref()?;
        let ws = oc_state
            .workspaces
            .iter()
            .find(|ws| ws.is_default)
            .or_else(|| oc_state.workspaces.first())?;
        ws.docs.first().map(|d| d.path.clone())
    }

    pub(crate) fn audit_groups<'a>(
        &self,
        report: &'a agentry_audit::report::AuditReport,
    ) -> std::collections::BTreeMap<
        agentry_audit::report::Severity,
        Vec<&'a agentry_audit::report::AuditFinding>,
    > {
        let mut groups: std::collections::BTreeMap<
            agentry_audit::report::Severity,
            Vec<&agentry_audit::report::AuditFinding>,
        > = std::collections::BTreeMap::new();
        for finding in report
            .agents
            .iter()
            .flat_map(|a| a.findings.iter())
            .chain(report.global_findings.iter())
        {
            if let Some(min) = self.audit_filter {
                if finding.severity > min {
                    continue;
                }
            }
            groups.entry(finding.severity).or_default().push(finding);
        }
        groups
    }

    pub fn selected_finding(&self) -> Option<&agentry_audit::report::AuditFinding> {
        if self.tab_index != 4 {
            return None;
        }
        let report = self.audit_report.as_ref()?;
        let groups = self.audit_groups(report);

        let mut list_row = 0;
        for findings in groups.values() {
            if findings.is_empty() {
                continue;
            }
            if self.list_selected == list_row {
                return None;
            }
            list_row += 1;
            for finding in findings {
                if self.list_selected == list_row {
                    return Some(finding);
                }
                list_row += 1;
            }
        }
        None
    }

    fn on_cycle_audit_filter(&mut self) {
        if self.tab_index != 4 {
            return;
        }
        use agentry_audit::report::Severity;
        self.audit_filter = match self.audit_filter {
            None => Some(Severity::Critical),
            Some(Severity::Critical) => Some(Severity::Warning),
            Some(Severity::Warning) => Some(Severity::Info),
            Some(Severity::Info) => Some(Severity::Suggestion),
            Some(Severity::Suggestion) => None,
        };
        self.list_selected = 0;
    }

    /// Shell out to $EDITOR (or nvim/vim/vi) to edit a prompt by its index.
    fn edit_with_external_editor(&mut self, prompt_idx: usize) {
        if prompt_idx >= self.prompts.len() {
            return;
        }

        let prompt = &self.prompts[prompt_idx];
        let file_path = if let Some(ref path) = prompt.source_path {
            path.clone()
        } else {
            let store_path = self
                .home_dir
                .join(".agents")
                .join("prompts")
                .join(prompt.canonical_filename());
            if let Some(parent) = store_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if !store_path.exists() {
                let _ = std::fs::write(&store_path, &prompt.body);
            }
            store_path
        };

        self.edit_file_externally(&file_path);

        // Reload the prompt content from disk
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            self.prompts[prompt_idx].body = content;
            self.status_message = Some(format!("Edited: {}", self.prompts[prompt_idx].name));
        }
    }

    /// Shell out to $EDITOR for an arbitrary file path.
    fn edit_file_externally(&mut self, file_path: &std::path::Path) {
        let editor = std::env::var("EDITOR").ok().unwrap_or_else(|| {
            for cmd in &["nvim", "vim", "vi"] {
                if std::process::Command::new("which")
                    .arg(cmd)
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return cmd.to_string();
                }
            }
            "vi".to_string()
        });

        // Suspend TUI
        use crossterm::{
            execute,
            terminal::{disable_raw_mode, LeaveAlternateScreen},
        };
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

        // Run the editor
        let result = std::process::Command::new(&editor).arg(file_path).status();

        // Restore TUI
        use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
        let _ = execute!(std::io::stdout(), EnterAlternateScreen);
        let _ = enable_raw_mode();

        // Signal main loop to clear terminal before next draw
        self.needs_terminal_clear = true;

        match result {
            Ok(status) if !status.success() => {
                self.status_message = Some("Editor exited with error".into());
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to launch {}: {}", editor, e));
            }
            _ => {}
        }
    }

    fn finding_edit_path(finding: &agentry_audit::report::AuditFinding) -> Option<PathBuf> {
        match finding.fix {
            Some(agentry_audit::report::FixAction::SymlinkRecreate { ref path, .. }) => {
                Some(path.clone())
            }
            _ => None,
        }
    }

    fn on_enter(&mut self) {
        match self.tab_index {
            0 => {
                if self.version_list.is_some() {
                    self.confirm_selected_version();
                    return;
                }
                if self.selected_agent_is_openclaw() {
                    let doc_path = self.openclaw_default_doc_path();
                    match doc_path {
                        Some(path) => self.edit_file_externally(&path),
                        None => {
                            self.status_message = Some("No docs in the default workspace".into())
                        }
                    }
                    return;
                }
                // Agents tab - install via selected method
                if let Some(agent) = self.detected_agents.get(self.list_selected) {
                    if let Some(method) = agent.spec.install_methods.get(self.method_selected) {
                        if method.available_on_os() {
                            let is_detected = agent.detected_methods.contains(method);
                            if !is_detected {
                                self.agent_confirm = Some(AgentConfirmAction::Install {
                                    agent_id: agent.spec.id.clone(),
                                    method: method.clone(),
                                    version: None,
                                });
                                self.status_message = Some(format!(
                                    "Install {} via {}? (y/n)",
                                    agent.spec.name,
                                    method.label()
                                ));
                            } else {
                                self.status_message = Some(format!(
                                    "{} already installed via {}",
                                    agent.spec.name,
                                    method.label()
                                ));
                            }
                        }
                    }
                }
            }
            1 => {
                // Prompts tab - edit selected prompt via $EDITOR
                if let Some(idx) = self.selected_prompt_index() {
                    self.edit_with_external_editor(idx);
                } else if self.list_is_new_prompt_action() {
                    // "New Global Prompt" action row
                    self.new_prompt_name = Some(String::new());
                    self.status_message = Some("Enter prompt name, then press Enter".into());
                }
            }
            2 => {
                // Skills tab - install selected skill (clone data to avoid borrow conflicts)
                let skill_info = self
                    .selected_skill()
                    .map(|s| (s.name.clone(), s.installed, s.source.clone()));
                if let Some((name, installed, source)) = skill_info {
                    if !installed && !source.is_empty() {
                        self.skill_confirm = Some(SkillConfirmAction::Install(name.clone()));
                        self.status_message = Some(format!("Install '{}'? (y/n)", name));
                    } else if installed {
                        self.status_message = Some(format!("'{}' is already installed", name));
                    }
                }
            }
            4 => {
                let finding = self.selected_finding().cloned();
                if let Some(finding) = finding {
                    if let Some(path) = Self::finding_edit_path(&finding) {
                        self.edit_file_externally(&path);
                    } else {
                        self.status_message = Some(finding.remediation.clone());
                    }
                }
            }
            _ => {}
        }
    }

    fn on_new(&mut self) {
        if self.tab_index == 1 {
            self.new_prompt_name = Some(String::new());
            self.status_message = Some("Enter prompt name, then press Enter".into());
        } else if self.tab_index == 0 && self.selected_agent_is_openclaw() {
            self.on_create_workspace();
        }
    }

    fn on_delete(&mut self) {
        if self.tab_index == 1 {
            if let Some(idx) = self.selected_prompt_index() {
                self.delete_confirm = Some(idx);
                self.status_message = Some(format!("Delete '{}'? (y/n)", self.prompts[idx].name));
            }
        }
    }

    fn load_sync_plan(&mut self) {
        let home = self.home_dir.clone();
        let project_dirs = [home.join("Development")];
        let agents = self.detected_agents.clone();

        let mut results = Vec::new();
        for prompt in &self.prompts {
            // Agent-level sync
            let plan = agentry_sync::planner::plan_sync(prompt, &agents, &home);
            let mappings = agentry_sync::executor::check_sync_status(prompt, &plan.mappings);
            for mapping in mappings {
                results.push(SyncResultEntry {
                    prompt_name: prompt.name.clone(),
                    agent_id: mapping.agent_id.clone(),
                    destination: mapping.destination.display().to_string(),
                    status: mapping.status,
                    action: mapping.action,
                    mapping,
                });
            }

            // Project-level sync for global prompts
            let project_mappings =
                agentry_sync::planner::project_sync_plans(prompt, &project_dirs, &home);
            if !project_mappings.is_empty() {
                let checked = agentry_sync::executor::check_sync_status(prompt, &project_mappings);
                for mapping in checked {
                    results.push(SyncResultEntry {
                        prompt_name: prompt.name.clone(),
                        agent_id: mapping.agent_id.clone(),
                        destination: mapping.destination.display().to_string(),
                        status: mapping.status,
                        action: mapping.action,
                        mapping,
                    });
                }
            }
        }

        self.sync_results = results;
        self.status_message = Some(format!(
            "Sync plan loaded ({} mappings)",
            self.sync_results.len()
        ));
        self.list_selected = 0;
    }

    fn selected_sync_mapping(&self) -> Option<agentry_core::models::SyncMapping> {
        self.selected_sync_entry()
            .map(|entry| entry.mapping.clone())
    }

    fn execute_selected_sync(&mut self) {
        if self.tab_index != 3 {
            return;
        }
        if !self.sync_loaded {
            self.status_message = Some("Sync plan not loaded yet".into());
            return;
        }
        match self.selected_sync_mapping() {
            Some(mapping) if mapping.action != agentry_core::models::SyncAction::Skip => {
                self.prepare_harness(
                    "sync.execute",
                    agentry_harness::ActionInput::SyncExecute {
                        prompt_id: None,
                        mappings: vec![mapping],
                    },
                );
            }
            Some(_) => {
                self.status_message = Some("Selected mapping is skipped".into());
            }
            None => {
                self.status_message = Some("Select a sync entry first".into());
            }
        }
    }

    fn execute_all_sync(&mut self) {
        if self.tab_index != 3 {
            return;
        }
        if !self.sync_loaded {
            self.status_message = Some("Sync plan not loaded yet".into());
            return;
        }
        let mappings: Vec<agentry_core::models::SyncMapping> = self
            .sync_results
            .iter()
            .filter(|entry| entry.action != agentry_core::models::SyncAction::Skip)
            .map(|entry| entry.mapping.clone())
            .collect();
        if mappings.is_empty() {
            self.status_message = Some("No executable sync mappings".into());
            return;
        }
        self.prepare_harness(
            "sync.execute",
            agentry_harness::ActionInput::SyncExecute {
                prompt_id: None,
                mappings,
            },
        );
    }

    fn refresh_sync_plan(&mut self) {
        let selected = self.list_selected;
        self.load_sync_plan();
        self.list_selected = selected;
        self.sync_loaded = true;
    }

    fn harness_context(&self) -> agentry_harness::HarnessContext {
        agentry_harness::HarnessContext::new(
            self.home_dir.clone(),
            self.detected_agents.clone(),
            self.prompts.clone(),
        )
        .with_report(self.audit_report.clone())
    }

    fn resolve_harness_input(
        &self,
        invocation: &crate::ui::keymap::HarnessInvocation,
    ) -> Result<agentry_harness::ActionInput, String> {
        match invocation.action_id.as_str() {
            "fix.apply" => {
                let check_id = self
                    .selected_finding()
                    .map(|f| f.check_id.clone())
                    .ok_or_else(|| "Select a finding first".to_string())?;
                Ok(agentry_harness::ActionInput::FixApply { check_id })
            }
            _ => serde_json::from_str(&invocation.input_json)
                .map_err(|err| format!("invalid harness input: {err}")),
        }
    }

    fn audited_file_write_confirm(&self) -> Option<String> {
        let finding = self.selected_finding()?;
        if finding.category != agentry_audit::report::FindingCategory::Audited {
            return None;
        }
        let fix = finding.fix.as_ref().or(finding.suggested_fix.as_ref())?;
        let agentry_audit::report::FixAction::FileWrite { path, content } = fix else {
            return None;
        };
        let preview: String = content.chars().take(200).collect();
        Some(format!(
            "write {} ({} bytes): '{}'? (y/n)",
            path.display(),
            content.len(),
            preview
        ))
    }

    fn prepare_harness(&mut self, action_id: &str, input: agentry_harness::ActionInput) {
        match self.harness.prepare(action_id, &input) {
            Ok(pending) => match pending.confirmation {
                agentry_harness::Confirmation::None => {
                    self.invoke_harness(HarnessPendingInvocation {
                        action_id: pending.action_id.to_string(),
                        input,
                    });
                }
                agentry_harness::Confirmation::Single | agentry_harness::Confirmation::PerItem => {
                    let message = if action_id == "fix.apply" {
                        self.audited_file_write_confirm()
                            .unwrap_or_else(|| format!("{}? (y/n)", pending.describe))
                    } else {
                        format!("{}? (y/n)", pending.describe)
                    };
                    self.status_message = Some(message);
                    self.harness_confirm = Some(HarnessPendingInvocation {
                        action_id: pending.action_id.to_string(),
                        input,
                    });
                }
                agentry_harness::Confirmation::Unsupported => {
                    self.error_message =
                        Some(format!("action '{action_id}' is not supported here"));
                }
            },
            Err(err) => {
                self.error_message = Some(err.to_string());
            }
        }
    }

    fn invoke_harness(&mut self, pending: HarnessPendingInvocation) {
        let ctx = self.harness_context();
        let action_id = pending.action_id.clone();
        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match rt {
                Ok(rt) => rt.block_on(self.harness.invoke_confirmed(
                    &ctx,
                    &action_id,
                    pending.input,
                )),
                Err(err) => Err(agentry_harness::HarnessError::ExecutionFailed(format!(
                    "harness runtime unavailable: {err}"
                ))),
            }
        });
        match result {
            Ok(output) => self.apply_harness_output(&action_id, output),
            Err(err) => {
                self.error_message = Some(format!("{action_id} failed: {err}"));
            }
        }
    }

    fn apply_harness_output(&mut self, action_id: &str, output: agentry_harness::ActionOutput) {
        match (action_id, output) {
            ("sync.execute", agentry_harness::ActionOutput::SyncExecuted { applied, skipped }) => {
                self.refresh_sync_plan();
                let hint = if self.audit_loaded {
                    " · press r in Audit tab to re-audit"
                } else {
                    ""
                };
                self.status_message = Some(format!(
                    "Synced {applied} mappings ({skipped} skipped){hint}"
                ));
            }
            ("audit.run", agentry_harness::ActionOutput::AuditCompleted(report)) => {
                let finding_count = report.summary.total_findings;
                self.audit_report = Some(report);
                self.audit_loaded = true;
                self.list_selected = 0;
                self.status_message = Some(format!("Audit complete: {finding_count} findings"));
            }
            ("fix.apply", agentry_harness::ActionOutput::FixApplied(outcome)) => {
                self.post_fix_feedback(vec![outcome]);
            }
            ("fix.apply_all", agentry_harness::ActionOutput::FixAppliedAll { outcomes }) => {
                self.post_fix_feedback(outcomes);
            }
            ("auditor.review", agentry_harness::ActionOutput::AuditorMerged { added, report }) => {
                self.audit_report = Some(report);
                self.audit_loaded = true;
                self.list_selected = 0;
                self.status_message =
                    Some(format!("Auditor review complete: {added} findings added"));
            }
            _ => {
                self.status_message = Some(format!("{action_id} completed"));
            }
        }
    }

    fn post_fix_feedback(&mut self, outcomes: Vec<agentry_audit::fix::FixOutcome>) {
        let applied = outcomes.iter().filter(|o| o.success).count();
        let attempted = outcomes.len();
        let succeeded_keys: Vec<(String, Option<String>)> = outcomes
            .iter()
            .filter(|o| o.success)
            .map(|o| (o.check_id.clone(), o.agent_id.clone()))
            .collect();
        if let Some(pre_fix_report) = self.audit_report.clone() {
            if let Err(err) = agentry_audit::history::append_history(
                &self.home_dir,
                &pre_fix_report,
                &succeeded_keys,
            ) {
                self.error_message = Some(format!("failed to append audit history: {err}"));
            }
        }
        self.rerun_audit_after_fix();
        if let Some(history) = agentry_audit::history::load_history(&self.home_dir)
            .ok()
            .or_else(|| Some(Vec::new()))
        {
            if let Some(report) = &mut self.audit_report {
                agentry_audit::history::apply_feedback(report, &history);
            }
        }
        let remaining = self
            .audit_report
            .as_ref()
            .map(|r| r.summary.total_findings)
            .unwrap_or(0);
        let failures: Vec<String> = outcomes
            .iter()
            .filter(|o| !o.success)
            .map(|o| format!("{}: {}", o.check_id, o.message))
            .collect();
        if failures.is_empty() {
            self.status_message = Some(format!(
                "{applied} of {attempted} fixes applied; {remaining} findings remain"
            ));
        } else {
            self.error_message = Some(format!(
                "{applied} of {attempted} fixes applied; failures: {}",
                failures.join("; ")
            ));
        }
    }

    fn rerun_audit_after_fix(&mut self) {
        let ctx = self.harness_context();
        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            match rt {
                Ok(rt) => rt.block_on(self.harness.invoke_confirmed(
                    &ctx,
                    "audit.run",
                    agentry_harness::ActionInput::AuditRun { agent_id: None },
                )),
                Err(err) => Err(agentry_harness::HarnessError::ExecutionFailed(format!(
                    "harness runtime unavailable: {err}"
                ))),
            }
        });
        match result {
            Ok(agentry_harness::ActionOutput::AuditCompleted(report)) => {
                self.audit_report = Some(report);
                self.audit_loaded = true;
            }
            Ok(_) => {
                self.error_message = Some("audit.run returned an unexpected output".to_string());
            }
            Err(err) => {
                self.error_message = Some(format!("audit.run failed: {err}"));
            }
        }
    }

    fn on_run_audit(&mut self) {
        self.prepare_harness(
            "audit.run",
            agentry_harness::ActionInput::AuditRun { agent_id: None },
        );
    }

    fn on_edit(&mut self) {
        if self.tab_index == 1 {
            if let Some(idx) = self.selected_prompt_index() {
                self.edit_with_external_editor(idx);
            }
        }
    }

    fn on_insert(&mut self) {
        if self.tab_index != 2 {
            return;
        }
        if let Some(idx) = self.selected_skill_index() {
            if let Some(ref hub) = self.skill_hub {
                let skills: Vec<_> = hub.skills.values().collect();
                if idx < skills.len() {
                    let skill = skills[idx];
                    if !skill.installed {
                        self.skill_confirm = Some(SkillConfirmAction::Install(skill.name.clone()));
                        self.status_message = Some(format!("Install '{}'? (y/n)", skill.name));
                    } else {
                        self.status_message =
                            Some(format!("'{}' is already installed", skill.name));
                    }
                }
            }
        }
    }

    fn method_prev(&mut self) {
        if self.tab_index != 0 {
            return;
        }
        if self.method_selected > 0 {
            self.method_selected -= 1;
        }
    }

    fn method_next(&mut self) {
        if self.tab_index == 0 {
            if let Some(agent) = self.detected_agents.get(self.list_selected) {
                let max = agent.spec.install_methods.len();
                if max > 0 && self.method_selected < max - 1 {
                    self.method_selected += 1;
                }
            }
        }
    }

    fn on_list_versions(&mut self) {
        if self.tab_index != 0 {
            return;
        }
        if let Some(agent) = self.detected_agents.get(self.list_selected) {
            if let Some(method) = agent.spec.install_methods.get(self.method_selected) {
                let cmd = match method.list_versions_command() {
                    Some(c) => c,
                    None => {
                        self.status_message =
                            Some("Version listing not supported for this method".into());
                        return;
                    }
                };
                self.status_message = Some("Fetching versions...".into());
                match std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                {
                    Ok(output) if output.status.success() => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let versions = parse_versions(&stdout, method);
                        if versions.is_empty() {
                            self.version_list_error = Some("No versions found".into());
                        } else {
                            self.version_list = Some(versions);
                            self.version_selected = 0;
                            self.status_message =
                                Some("Versions loaded. Select with j/k, Enter to confirm".into());
                        }
                    }
                    Ok(_) => {
                        self.version_list_error = Some("Version command failed".into());
                    }
                    Err(e) => {
                        self.version_list_error = Some(format!("Error: {}", e));
                    }
                }
            }
        }
    }

    fn execute_agent_action(&mut self, action: AgentConfirmAction) {
        use crossterm::{
            execute,
            terminal::{
                disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
            },
        };

        let cmd = match &action {
            AgentConfirmAction::Install {
                method, version, ..
            } => method.install_command(version.as_deref()),
            AgentConfirmAction::Update { method, .. } => method.update_command(),
            AgentConfirmAction::Remove { method, .. } => method.remove_command(),
        };

        // Suspend TUI
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .status();

        // Restore TUI
        let _ = execute!(std::io::stdout(), EnterAlternateScreen);
        let _ = enable_raw_mode();
        self.needs_terminal_clear = true;

        match status {
            Ok(s) if s.success() => {
                let verb = match &action {
                    AgentConfirmAction::Install { .. } => "Installed",
                    AgentConfirmAction::Update { .. } => "Updated",
                    AgentConfirmAction::Remove { .. } => "Removed",
                };
                self.status_message = Some(format!("{} {}", verb, action.agent_id()));
                let result = tokio::task::block_in_place(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map(|rt| rt.block_on(async { agentry_agents::detect_all_agents().await }))
                        .map_err(|e| e.to_string())
                });
                match result {
                    Ok(agents) => self.detected_agents = agents,
                    Err(e) => {
                        self.error_message = Some(format!("Failed to re-detect agents: {}", e));
                    }
                }
            }
            Ok(_) => {
                self.error_message = Some(format!("Command failed for {}", action.agent_id()));
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to run command: {}", e));
            }
        }
    }

    fn on_update(&mut self) {
        if self.tab_index == 0 {
            // Agents tab: update via selected method
            if let Some(agent) = self.detected_agents.get(self.list_selected) {
                if let Some(method) = agent.spec.install_methods.get(self.method_selected) {
                    if agent.detected_methods.contains(method) {
                        self.agent_confirm = Some(AgentConfirmAction::Update {
                            agent_id: agent.spec.id.clone(),
                            method: method.clone(),
                        });
                        self.status_message = Some(format!(
                            "Update {} via {}? (y/n)",
                            agent.spec.name,
                            method.label()
                        ));
                    } else {
                        self.status_message = Some(format!(
                            "{} is not installed via {}",
                            agent.spec.name,
                            method.label()
                        ));
                    }
                }
            }
        } else if self.tab_index == 2 {
            if let Some(idx) = self.selected_skill_index() {
                if let Some(ref hub) = self.skill_hub {
                    let skills: Vec<_> = hub.skills.values().collect();
                    if idx < skills.len() {
                        let skill = skills[idx];
                        if skill.installed {
                            self.skill_confirm =
                                Some(SkillConfirmAction::Update(skill.name.clone()));
                            self.status_message = Some(format!("Update '{}'? (y/n)", skill.name));
                        } else {
                            self.status_message =
                                Some(format!("'{}' is not installed", skill.name));
                        }
                    }
                }
            }
        } else {
            self.status_message = Some("Update: only available in Agents and Skills tabs".into());
        }
    }

    fn on_remove(&mut self) {
        if self.tab_index == 0 {
            // Agents tab: remove via selected method
            if let Some(agent) = self.detected_agents.get(self.list_selected) {
                if let Some(method) = agent.spec.install_methods.get(self.method_selected) {
                    if agent.detected_methods.contains(method) {
                        self.agent_confirm = Some(AgentConfirmAction::Remove {
                            agent_id: agent.spec.id.clone(),
                            method: method.clone(),
                        });
                        self.status_message = Some(format!(
                            "Remove {} via {}? (y/n)",
                            agent.spec.name,
                            method.label()
                        ));
                    } else {
                        self.status_message = Some(format!(
                            "{} is not installed via {}",
                            agent.spec.name,
                            method.label()
                        ));
                    }
                }
            }
        } else if self.tab_index == 2 {
            if let Some(idx) = self.selected_skill_index() {
                if let Some(ref hub) = self.skill_hub {
                    let skills: Vec<_> = hub.skills.values().collect();
                    if idx < skills.len() {
                        let skill = skills[idx];
                        if skill.installed {
                            self.skill_confirm =
                                Some(SkillConfirmAction::Remove(skill.name.clone()));
                            self.status_message = Some(format!("Remove '{}'? (y/n)", skill.name));
                        } else {
                            self.status_message =
                                Some(format!("'{}' is not installed", skill.name));
                        }
                    }
                }
            }
        }
    }

    fn on_github(&mut self) {
        if self.tab_index == 2 {
            if let Some(idx) = self.selected_skill_index() {
                if let Some(ref hub) = self.skill_hub {
                    let skills: Vec<_> = hub.skills.values().collect();
                    if idx < skills.len() {
                        let skill = skills[idx];
                        if !skill.source_url.is_empty() {
                            let url = skill.source_url.clone();
                            self.status_message = Some(format!("Open: {}", url));
                            let _ = std::process::Command::new("open").arg(&url).spawn();
                        }
                    }
                }
            }
        }
    }

    fn execute_skill_action(&mut self, action: SkillConfirmAction) {
        match action {
            SkillConfirmAction::Install(name) => {
                let home = self.home_dir.clone();
                let dirs = self.agent_skills_dirs.clone();
                if let Some(ref hub) = self.skill_hub {
                    if let Some(skill) = hub.skills.get(&name) {
                        let source = skill.source.clone();
                        let skill_path = skill.skill_path.clone();
                        // If source is empty, we can't install
                        if source.is_empty() {
                            self.status_message = Some(format!("No source for '{}'", name));
                            return;
                        }
                        let result = agentry_skills::install::install_skill(
                            &home,
                            &source,
                            &skill_path,
                            &dirs,
                        );
                        match result {
                            Ok(r) => {
                                self.status_message = Some(r.message);
                                self.discover_skills(); // Refresh
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Install error: {}", e));
                            }
                        }
                    }
                }
            }
            SkillConfirmAction::Remove(name) => {
                let home = self.home_dir.clone();
                let dirs = self.agent_skills_dirs.clone();
                let result = agentry_skills::install::remove_skill(&home, &name, &dirs);
                match result {
                    Ok(r) => {
                        self.status_message = Some(r.message);
                        self.discover_skills(); // Refresh
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Remove error: {}", e));
                    }
                }
            }
            SkillConfirmAction::Update(name) => {
                let home = self.home_dir.clone();
                let dirs = self.agent_skills_dirs.clone();
                let result = agentry_skills::install::update_skill(&home, &name, &dirs);
                match result {
                    Ok(r) => {
                        self.status_message = Some(r.message);
                        self.discover_skills(); // Refresh
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Update error: {}", e));
                    }
                }
            }
        }
    }

    fn draw(&self, f: &mut Frame) {
        match self.mode {
            AppMode::Intro => ui::draw_intro(f, self),
            AppMode::Dashboard => ui::draw_dashboard(f, self),
            AppMode::Quit => {}
        }
    }

    pub fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }

    fn on_create_workspace(&mut self) {
        if self.tab_index == 0 && self.selected_agent_is_openclaw() {
            let installed = self
                .openclaw_state
                .as_ref()
                .map(|s| s.installed)
                .unwrap_or_else(agentry_openclaw::discovery::is_openclaw_installed);
            if installed {
                self.status_message = Some("Run: openclaw setup".into());
                let _ = std::process::Command::new("openclaw").arg("setup").spawn();
            } else {
                self.status_message =
                    Some("OpenClaw not installed. Install from https://openclaw.dev".into());
            }
        }
    }

    fn on_add_agent(&mut self) {
        if self.tab_index == 0 && self.selected_agent_is_openclaw() {
            let installed = self
                .openclaw_state
                .as_ref()
                .map(|s| s.installed)
                .unwrap_or_else(agentry_openclaw::discovery::is_openclaw_installed);
            if installed {
                self.status_message = Some("Run: openclaw agents add <name>".into());
            } else {
                self.status_message = Some("OpenClaw not installed".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use agentry_audit::report::{
        AgentAudit, AuditFinding, AuditReport, FindingCategory, HealthGrade, Severity,
    };
    use agentry_core::models::DetectedAgent;

    fn finding(severity: Severity, check_id: &str) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity,
            category: FindingCategory::Installation,
            agent_id: Some("codex".to_string()),
            message: "test finding".to_string(),
            remediation: "fix it".to_string(),
            auto_fixable: false,
            fix: None,
            suggested_fix: None,
            evidence: None,
        }
    }

    fn agent_audit(findings: Vec<AuditFinding>) -> AgentAudit {
        let detected = DetectedAgent {
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
        };
        AgentAudit {
            agent_id: "codex".to_string(),
            health_score: 100,
            grade: HealthGrade::Healthy,
            detected,
            findings,
        }
    }

    fn report_with(findings: Vec<AuditFinding>) -> AuditReport {
        let json = r#"{"generated_at":"2026-01-01T00:00:00Z","machine_id":"test","agents":[],"global_findings":[],"summary":{"total_findings":0,"by_severity":{},"by_category":{},"auto_fixable_count":0,"healthy_agents":0,"degraded_agents":0}}"#;
        let mut report: AuditReport = serde_json::from_str(json).unwrap();
        report.agents = vec![agent_audit(findings)];
        report
    }

    fn audit_app(report: AuditReport) -> App {
        let mut app = App::new();
        app.tab_index = 4;
        app.audit_report = Some(report);
        app
    }

    #[test]
    fn list_max_and_selected_finding_agree_on_fixture() {
        let report = report_with(vec![
            finding(Severity::Critical, "critical.one"),
            finding(Severity::Critical, "critical.two"),
            finding(Severity::Warning, "warning.one"),
            finding(Severity::Info, "info.one"),
        ]);
        let mut app = audit_app(report);

        assert_eq!(app.list_max(), 4 + 3);

        assert!(app.selected_finding().is_none());
        app.list_selected = 1;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("critical.one")
        );
        app.list_selected = 2;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("critical.two")
        );
        app.list_selected = 3;
        assert!(app.selected_finding().is_none());
        app.list_selected = 4;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("warning.one")
        );
        app.list_selected = 5;
        assert!(app.selected_finding().is_none());
        app.list_selected = 6;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("info.one")
        );
        app.list_selected = 7;
        assert!(app.selected_finding().is_none());
    }

    #[test]
    fn list_max_is_zero_without_report() {
        let mut app = App::new();
        app.tab_index = 4;
        assert_eq!(app.list_max(), 0);
        assert!(app.selected_finding().is_none());
    }

    #[test]
    fn selected_finding_requires_audit_tab() {
        let mut app = audit_app(report_with(vec![finding(
            Severity::Critical,
            "critical.one",
        )]));
        app.tab_index = 0;
        app.list_selected = 1;
        assert!(app.selected_finding().is_none());
    }

    #[test]
    fn warning_filter_excludes_info_and_suggestion() {
        let report = report_with(vec![
            finding(Severity::Critical, "critical.one"),
            finding(Severity::Warning, "warning.one"),
            finding(Severity::Info, "info.one"),
            finding(Severity::Suggestion, "suggestion.one"),
        ]);
        let mut app = audit_app(report);
        app.audit_filter = Some(Severity::Warning);

        assert_eq!(app.list_max(), 2 + 2);

        app.list_selected = 0;
        assert!(app.selected_finding().is_none());
        app.list_selected = 1;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("critical.one")
        );
        app.list_selected = 2;
        assert!(app.selected_finding().is_none());
        app.list_selected = 3;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("warning.one")
        );
        app.list_selected = 4;
        assert!(app.selected_finding().is_none());
    }

    #[test]
    fn critical_filter_shows_only_critical() {
        let report = report_with(vec![
            finding(Severity::Critical, "critical.one"),
            finding(Severity::Warning, "warning.one"),
            finding(Severity::Info, "info.one"),
        ]);
        let mut app = audit_app(report);
        app.audit_filter = Some(Severity::Critical);

        assert_eq!(app.list_max(), 1 + 1);
        app.list_selected = 1;
        assert_eq!(
            app.selected_finding().map(|f| f.check_id.as_str()),
            Some("critical.one")
        );
    }

    #[test]
    fn filter_cycles_through_all_levels() {
        let mut app = audit_app(report_with(Vec::new()));

        assert_eq!(app.audit_filter, None);
        app.on_cycle_audit_filter();
        assert_eq!(app.audit_filter, Some(Severity::Critical));
        app.on_cycle_audit_filter();
        assert_eq!(app.audit_filter, Some(Severity::Warning));
        app.on_cycle_audit_filter();
        assert_eq!(app.audit_filter, Some(Severity::Info));
        app.on_cycle_audit_filter();
        assert_eq!(app.audit_filter, Some(Severity::Suggestion));
        app.on_cycle_audit_filter();
        assert_eq!(app.audit_filter, None);
    }

    #[test]
    fn filter_cycle_resets_list_selected() {
        let mut app = audit_app(report_with(vec![
            finding(Severity::Critical, "critical.one"),
            finding(Severity::Warning, "warning.one"),
        ]));
        app.list_selected = 2;
        app.on_cycle_audit_filter();
        assert_eq!(app.list_selected, 0);
    }

    #[test]
    fn filter_cycle_ignored_outside_audit_tab() {
        let mut app = audit_app(report_with(Vec::new()));
        app.tab_index = 0;
        app.on_cycle_audit_filter();
        assert_eq!(app.audit_filter, None);
    }

    #[test]
    fn finding_edit_path_for_symlink_recreate() {
        let mut f = finding(Severity::Critical, "skills.symlink");
        f.fix = Some(agentry_audit::report::FixAction::SymlinkRecreate {
            path: "/tmp/skills".into(),
            target: "../../.agents/skills".to_string(),
        });
        assert_eq!(
            App::finding_edit_path(&f),
            Some(std::path::PathBuf::from("/tmp/skills"))
        );
    }

    #[test]
    fn finding_edit_path_none_for_other_fix_kinds() {
        let mut f = finding(Severity::Warning, "config.missing");
        f.fix = Some(agentry_audit::report::FixAction::ShellCommand {
            description: "install".to_string(),
            command: "npm install -g codex".to_string(),
        });
        assert_eq!(App::finding_edit_path(&f), None);
    }

    #[test]
    fn finding_edit_path_none_without_fix() {
        let f = finding(Severity::Info, "prompt.missing");
        assert_eq!(App::finding_edit_path(&f), None);
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, crossterm::event::KeyModifiers::empty()))
            .unwrap();
    }

    #[test]
    fn handle_key_r_dual_maps_run_audit_vs_remove() {
        let mut audit_app = audit_app(report_with(Vec::new()));
        audit_app.home_dir = std::env::temp_dir().join("agentry-test-r-dual");
        press(&mut audit_app, KeyCode::Char('r'));
        assert!(audit_app.audit_report.is_some());
        assert!(audit_app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Audit complete: ")));

        let mut agents_app = App::new();
        agents_app.tab_index = 0;
        agents_app.detected_agents = vec![DetectedAgent {
            spec: agentry_core::models::AgentSpec {
                id: "codex".to_string(),
                name: "codex".to_string(),
                cli_binary: "codex".to_string(),
                config_dir: ".codex".to_string(),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: agentry_core::models::PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![agentry_core::models::InstallMethod::Brew {
                    formula: "codex".to_string(),
                    cask: false,
                }],
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: true,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: vec![agentry_core::models::InstallMethod::Brew {
                formula: "codex".to_string(),
                cask: false,
            }],
        }];
        press(&mut agents_app, KeyCode::Char('r'));
        assert!(agents_app.agent_confirm.is_some());
        assert_eq!(
            agents_app.status_message.as_deref(),
            Some("Remove codex via Homebrew? (y/n)")
        );
    }

    #[test]
    fn handle_key_dispatches_via_keymap_and_ignores_other_tabs() {
        let mut app = App::new();
        app.tab_index = 3;
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(app.audit_filter, None);
        app.tab_index = 4;
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(
            app.audit_filter,
            Some(agentry_audit::report::Severity::Critical)
        );
        press(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit);
        assert_eq!(app.mode, AppMode::Quit);
        press(&mut app, KeyCode::Char('?'));
    }

    fn openclaw_agent() -> DetectedAgent {
        DetectedAgent {
            spec: agentry_core::models::AgentSpec {
                id: "openclaw".to_string(),
                name: "openclaw".to_string(),
                cli_binary: "openclaw".to_string(),
                config_dir: ".openclaw".to_string(),
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
        }
    }

    #[test]
    fn openclaw_enter_edits_first_doc_of_default_workspace() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![openclaw_agent()];
        app.openclaw_state = Some(OpenClawState {
            workspaces: vec![agentry_openclaw::discovery::OpenClawWorkspace {
                id: "main".to_string(),
                name: "main".to_string(),
                workspace_path: std::env::temp_dir().join("agentry-test-ws-main"),
                model: None,
                is_default: true,
                docs: vec![agentry_openclaw::discovery::WorkspaceDoc {
                    name: "AGENTS.md".to_string(),
                    path: std::env::temp_dir()
                        .join("agentry-test-ws-main")
                        .join("AGENTS.md"),
                    doc_type: agentry_openclaw::discovery::DocType::Agents,
                    size_bytes: 10,
                }],
                lobster_workflows: Vec::new(),
                has_agents_md: true,
                has_soul_md: false,
                has_tools_md: false,
                has_identity_md: false,
                has_memory_md: false,
                has_user_md: false,
            }],
            installed: true,
        });
        let doc = app.openclaw_default_doc_path();
        assert!(doc.is_some());
        assert!(doc.unwrap().file_name().unwrap() == "AGENTS.md");
        assert!(app.selected_agent_is_openclaw());
    }

    #[test]
    fn openclaw_keys_gated_on_agents_tab_with_openclaw_selected() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![openclaw_agent()];
        app.openclaw_state = Some(OpenClawState {
            workspaces: Vec::new(),
            installed: true,
        });
        assert_eq!(
            crate::ui::keymap::resolve(0, &app, "a"),
            Some(TuiAction::AddAgent)
        );
        assert_eq!(
            crate::ui::keymap::resolve(0, &app, "c"),
            Some(TuiAction::CreateWorkspace)
        );
        assert_eq!(crate::ui::keymap::resolve(0, &app, "n"), None);
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(
            app.status_message.as_deref(),
            Some("Run: openclaw agents add <name>")
        );

        let mut non_oc = App::new();
        non_oc.tab_index = 0;
        non_oc.detected_agents = vec![DetectedAgent {
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
        assert_eq!(crate::ui::keymap::resolve(0, &non_oc, "a"), None);
        assert_eq!(crate::ui::keymap::resolve(0, &non_oc, "c"), None);
        assert_eq!(crate::ui::keymap::resolve(0, &non_oc, "n"), None);
        press(&mut non_oc, KeyCode::Char('a'));
        press(&mut non_oc, KeyCode::Char('n'));
        assert!(non_oc.status_message.is_none());
        assert!(non_oc.agent_confirm.is_none());
        assert!(non_oc.new_prompt_name.is_none());
    }

    fn sync_test_prompt(name: &str) -> agentry_core::models::UnifiedPrompt {
        agentry_core::models::UnifiedPrompt {
            id: name.to_string(),
            name: name.to_string(),
            description: String::new(),
            frontmatter: std::collections::BTreeMap::new(),
            body: "Test sync content".to_string(),
            xml_tags: vec![],
            scope: agentry_core::models::PromptScope::Global,
            source_format: agentry_core::models::PromptFormat::PlainMd,
            source_path: None,
        }
    }

    fn sync_agent(id: &str, config_dir: &str, filename: &str) -> DetectedAgent {
        DetectedAgent {
            spec: agentry_core::models::AgentSpec {
                id: id.to_string(),
                name: id.to_string(),
                cli_binary: id.to_string(),
                config_dir: config_dir.to_string(),
                prompt_filename: filename.to_string(),
                prompt_format: agentry_core::models::PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: Vec::new(),
            },
            installed: true,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    fn sync_app(name: &str) -> (App, PathBuf) {
        let tmp = std::env::temp_dir().join(format!("agentry-sync-test-{}", name));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut app = App::new();
        app.tab_index = 3;
        app.home_dir = tmp.clone();
        app.prompts = vec![sync_test_prompt("alpha"), sync_test_prompt("beta")];
        app.detected_agents = vec![sync_agent("claude-code", ".claude", "CLAUDE.md")];
        (app, tmp)
    }

    #[test]
    fn sync_tab_entry_autoloads_plan_once() {
        let (mut app, tmp) = sync_app("autoload");
        assert!(!app.sync_loaded);
        assert!(app.sync_results.is_empty());

        app.tab_index = 0;
        app.next_tab();
        app.next_tab();
        app.next_tab();
        assert_eq!(app.tab_index, 3);
        assert!(app.sync_loaded);
        assert!(!app.sync_results.is_empty());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Sync plan loaded")));

        let count_after_first = app.sync_results.len();
        app.prev_tab();
        app.next_tab();
        assert_eq!(app.sync_results.len(), count_after_first);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sync_confirm_flow_executes_and_refreshes_plan() {
        let (mut app, tmp) = sync_app("confirm");
        app.load_sync_plan();
        app.sync_loaded = true;

        let dest = app.sync_results[0].mapping.destination.clone();
        app.list_selected = 1;
        press(&mut app, KeyCode::Char('s'));
        assert!(app.harness_confirm.is_some());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("sync 'alpha' to claude-code (")));

        press(&mut app, KeyCode::Char('y'));
        assert!(app.harness_confirm.is_none());
        assert!(dest.exists());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Synced 1 mappings")));
        assert_eq!(
            app.sync_results[0].mapping.status,
            agentry_core::models::SyncStatus::UpToDate
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sync_confirm_cancelled_with_n() {
        let (mut app, tmp) = sync_app("cancel");
        app.load_sync_plan();
        app.sync_loaded = true;
        app.list_selected = 1;
        press(&mut app, KeyCode::Char('s'));
        assert!(app.harness_confirm.is_some());
        press(&mut app, KeyCode::Char('n'));
        assert!(app.harness_confirm.is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn execute_all_collects_non_skip_only() {
        let (mut app, tmp) = sync_app("all-skip");
        app.detected_agents
            .push(sync_agent("unknown-agent", ".unknown-agent", "AGENTS.md"));
        app.load_sync_plan();
        app.sync_loaded = true;

        let skip_count = app
            .sync_results
            .iter()
            .filter(|e| e.action == agentry_core::models::SyncAction::Skip)
            .count();
        assert!(skip_count > 0);

        app.list_selected = 1;
        press(&mut app, KeyCode::Char('S'));
        let expected = app.sync_results.len() - skip_count;
        assert!(app.harness_confirm.is_some());
        let pending = app.harness_confirm.as_ref().unwrap();
        let agentry_harness::ActionInput::SyncExecute { mappings, .. } = &pending.input else {
            panic!("expected SyncExecute input");
        };
        assert!(mappings
            .iter()
            .all(|m| m.action != agentry_core::models::SyncAction::Skip));
        assert_eq!(mappings.len(), expected);
        assert_eq!(
            app.status_message.as_deref(),
            Some(format!("execute {} sync mappings? (y/n)", expected).as_str())
        );

        press(&mut app, KeyCode::Char('y'));
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with(&format!("Synced {expected} mappings"))));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn execute_selected_skips_skip_action_and_guards_plan() {
        let (mut app, tmp) = sync_app("skip-guard");
        app.execute_selected_sync();
        assert!(app.harness_confirm.is_none());
        assert_eq!(
            app.status_message.as_deref(),
            Some("Sync plan not loaded yet")
        );

        app.detected_agents
            .push(sync_agent("unknown-agent", ".unknown-agent", "AGENTS.md"));
        app.load_sync_plan();
        app.sync_loaded = true;
        let skip_row = app
            .sync_results
            .iter()
            .position(|e| e.action == agentry_core::models::SyncAction::Skip)
            .expect("fixture has a Skip mapping");
        app.list_selected = skip_row + 1;
        app.execute_selected_sync();
        assert!(app.harness_confirm.is_none());
        assert_eq!(
            app.status_message.as_deref(),
            Some("Selected mapping is skipped")
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn sync_enter_is_alias_of_execute_selected() {
        let (mut app, tmp) = sync_app("enter-alias");
        app.load_sync_plan();
        app.sync_loaded = true;
        app.list_selected = 1;
        press(&mut app, KeyCode::Enter);
        assert!(app.harness_confirm.is_some());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn audit_autoruns_once_on_first_tab_entry() {
        let mut app = App::new();
        app.home_dir = std::env::temp_dir().join("agentry-test-audit-auto");
        let _ = std::fs::remove_dir_all(&app.home_dir);
        app.prompts = vec![sync_test_prompt("alpha")];
        assert!(app.audit_report.is_none());
        assert!(!app.audit_loaded);

        app.next_tab();
        app.next_tab();
        app.next_tab();
        app.next_tab();
        assert_eq!(app.tab_index, 4);
        assert!(app.audit_loaded);
        assert!(app.audit_report.is_some());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Audit complete: ")));

        let findings_after_first = app.audit_report.as_ref().map(|r| r.summary.total_findings);
        app.prev_tab();
        app.next_tab();
        assert_eq!(
            app.audit_report.as_ref().map(|r| r.summary.total_findings),
            findings_after_first,
            "second entry must not re-run the audit"
        );

        let _ = std::fs::remove_dir_all(&app.home_dir);
    }

    fn fixable_finding(check_id: &str, path: std::path::PathBuf) -> AuditFinding {
        AuditFinding {
            check_id: check_id.to_string(),
            severity: Severity::Warning,
            category: FindingCategory::Config,
            agent_id: None,
            message: format!("finding {check_id}"),
            remediation: "run the fix".to_string(),
            auto_fixable: true,
            fix: Some(agentry_audit::report::FixAction::FileRemove { path }),
            suggested_fix: None,
            evidence: None,
        }
    }

    fn fix_app(name: &str, findings: Vec<AuditFinding>) -> (App, std::path::PathBuf) {
        let tmp = std::env::temp_dir().join(format!("agentry-fix-test-{}", name));
        let _ = std::fs::remove_dir_all(&tmp);
        let mut app = audit_app(report_with(findings));
        app.home_dir = tmp.clone();
        (app, tmp)
    }

    #[test]
    fn fix_key_a_prepares_confirm_and_applies_through_harness() {
        let (mut app, tmp) = fix_app("fix-a", vec![]);
        let target = tmp.join("stale.md");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "x").unwrap();
        app.audit_report = Some(report_with(vec![fixable_finding(
            "orphan_cleanup",
            target.clone(),
        )]));
        app.list_selected = 1;

        press(&mut app, KeyCode::Char('a'));
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("apply fix for orphan_cleanup")));
        assert!(app.harness_confirm.is_some());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("apply fix for orphan_cleanup")));

        press(&mut app, KeyCode::Char('y'));
        assert!(app.harness_confirm.is_none());
        assert!(!target.exists(), "fix must have removed the file");
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.contains("1 of 1 fixes applied")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fix_key_a_cancelled_with_n() {
        let (mut app, tmp) = fix_app("fix-n", vec![]);
        let target = tmp.join("stale.md");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "x").unwrap();
        app.audit_report = Some(report_with(vec![fixable_finding(
            "orphan_cleanup",
            target.clone(),
        )]));
        app.list_selected = 1;

        press(&mut app, KeyCode::Char('a'));
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("apply fix for orphan_cleanup")));
        assert!(app.harness_confirm.is_some());
        press(&mut app, KeyCode::Char('n'));
        assert!(app.harness_confirm.is_none());
        assert!(target.exists(), "cancelled fix must not touch the file");
        assert_eq!(app.status_message.as_deref(), Some("Cancelled"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fix_key_a_gated_on_fixable_selection() {
        let (mut app, tmp) = fix_app("fix-gate", vec![finding(Severity::Info, "plain.one")]);
        app.list_selected = 1;

        press(&mut app, KeyCode::Char('a'));
        assert!(app.harness_confirm.is_none());
        assert!(app.status_message.is_none() || app.error_message.is_none());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fix_key_a_applies_all_through_harness() {
        let (mut app, tmp) = fix_app("fix-all", vec![]);
        std::fs::create_dir_all(&tmp).unwrap();
        let first = tmp.join("first.md");
        let second = tmp.join("second.md");
        std::fs::write(&first, "1").unwrap();
        std::fs::write(&second, "2").unwrap();
        app.audit_report = Some(report_with(vec![
            fixable_finding("cleanup_one", first.clone()),
            fixable_finding("cleanup_two", second.clone()),
        ]));

        press(&mut app, KeyCode::Char('A'));
        assert!(app.harness_confirm.is_some());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("apply all auto-fixable findings")));

        press(&mut app, KeyCode::Char('y'));
        assert!(!first.exists());
        assert!(!second.exists());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.contains("2 of 2 fixes applied")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn fix_apply_reruns_audit_and_finding_disappears() {
        let (mut app, tmp) = fix_app("fix-reaudit", vec![]);
        let target = tmp.join("stale.md");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&target, "x").unwrap();
        app.audit_report = Some(report_with(vec![fixable_finding(
            "orphan_cleanup",
            target.clone(),
        )]));
        app.list_selected = 1;

        press(&mut app, KeyCode::Char('a'));
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("apply fix for orphan_cleanup")));
        press(&mut app, KeyCode::Char('y'));

        let report = app.audit_report.as_ref().expect("re-audit ran");
        let still_present = report
            .agents
            .iter()
            .flat_map(|a| a.findings.iter())
            .chain(report.global_findings.iter())
            .any(|f| f.check_id == "orphan_cleanup");
        assert!(!still_present, "re-audit must reflect the applied fix");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn auditor_key_l_prepares_confirm_and_merges_no_host_finding() {
        let tmp = std::env::temp_dir().join(format!("agentry-auditor-l-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let config_path = tmp.join(".agents").join("agentry.toml");
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        let mut toml = String::from("[hosts]\npriority = [\"claude-code\", \"codex\", \"gemini-cli\", \"zai\", \"ollama\"]\n");
        for id in ["claude-code", "codex", "gemini-cli", "zai", "ollama"] {
            toml.push_str(&format!(
                "\n[hosts.{id}]\ndetect_binary = \"definitely-not-installed-xyz\"\n"
            ));
        }
        std::fs::write(&config_path, toml).unwrap();
        let mut app = audit_app(report_with(Vec::new()));
        app.home_dir = tmp.clone();
        app.audit_loaded = true;

        press(&mut app, KeyCode::Char('l'));
        assert!(app.harness_confirm.is_some());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("review audit findings with the host LLM")));

        press(&mut app, KeyCode::Char('y'));
        assert!(app.harness_confirm.is_none());
        assert!(app
            .status_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Auditor review complete: 1 findings added")));
        let report = app.audit_report.as_ref().expect("merged report");
        assert!(report
            .global_findings
            .iter()
            .any(|f| f.check_id == "auditor.no_host"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn audited_file_write_confirm_shows_size_and_preview() {
        let mut audited = finding(Severity::Suggestion, "auditor.write");
        audited.category = FindingCategory::Audited;
        audited.auto_fixable = false;
        audited.fix = None;
        audited.suggested_fix = Some(agentry_audit::report::FixAction::FileWrite {
            path: std::path::PathBuf::from("/home/user/.agents/prompts/X.md"),
            content: "body content".to_string(),
        });
        let mut app = audit_app(report_with(vec![audited]));
        app.audit_loaded = true;
        app.list_selected = 1;

        press(&mut app, KeyCode::Char('a'));
        assert!(app.harness_confirm.is_some());
        assert!(app.status_message.as_deref().is_some_and(|m| m.starts_with(
            "write /home/user/.agents/prompts/X.md (12 bytes): 'body content'? (y/n)"
        )));

        press(&mut app, KeyCode::Char('n'));
        assert!(app.harness_confirm.is_none());
    }

    #[test]
    fn audit_autoruns_on_jump_tab_entry() {
        let mut app = App::new();
        app.home_dir = std::env::temp_dir().join("agentry-test-audit-jump");
        let _ = std::fs::remove_dir_all(&app.home_dir);
        app.prompts = vec![sync_test_prompt("alpha")];

        press(&mut app, KeyCode::Char('5'));
        assert_eq!(app.tab_index, 4);
        assert!(app.audit_loaded);
        assert!(app.audit_report.is_some());

        let _ = std::fs::remove_dir_all(&app.home_dir);
    }

    #[test]
    fn jump_tab_resets_selection() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![sync_agent("claude-code", ".claude", "CLAUDE.md")];
        app.list_selected = 1;
        app.method_selected = 1;

        press(&mut app, KeyCode::Char('2'));
        assert_eq!(app.tab_index, 1);
        assert_eq!(app.list_selected, 0);
        assert_eq!(app.method_selected, 0);

        press(&mut app, KeyCode::Char('1'));
        assert_eq!(app.list_selected, 0);
        assert_eq!(app.method_selected, 0);
    }

    #[test]
    fn left_arrow_inert_off_agents_tab() {
        let mut app = App::new();
        app.tab_index = 3;
        app.method_selected = 1;
        press(&mut app, KeyCode::Left);
        assert_eq!(
            app.method_selected, 1,
            "Left must not move method off tab 0"
        );

        app.tab_index = 0;
        app.method_selected = 1;
        press(&mut app, KeyCode::Left);
        assert_eq!(app.method_selected, 0, "Left moves method on tab 0");
    }

    #[test]
    fn i_key_inert_off_skills_tab() {
        let mut app = App::new();
        app.tab_index = 1;
        press(&mut app, KeyCode::Char('i'));
        assert!(
            app.new_prompt_name.is_none(),
            "i must not open new-prompt outside Skills"
        );
        assert!(app.skill_confirm.is_none());
    }

    #[test]
    fn u_s_right_inert_off_tab_without_status() {
        for tab in [1, 3, 4] {
            let mut app = App::new();
            app.tab_index = tab;
            press(&mut app, KeyCode::Char('u'));
            press(&mut app, KeyCode::Char('s'));
            press(&mut app, KeyCode::Right);
            assert!(app.status_message.is_none(), "tab {tab}");
            assert!(app.agent_confirm.is_none());
            assert!(app.harness_confirm.is_none());
            assert_eq!(app.method_selected, 0);
        }
    }

    #[test]
    fn w_key_inert_on_all_tabs() {
        for tab in 0..5 {
            let mut app = App::new();
            app.tab_index = tab;
            app.home_dir = std::env::temp_dir().join("agentry-test-w-key");
            press(&mut app, KeyCode::Char('w'));
            assert!(app.status_message.is_none(), "tab {tab}: w must be inert");
            assert!(!app.home_dir.join(".agents").join("workflows").exists());
        }
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("agentry-test-w-key"));
    }

    fn version_flow_agent() -> DetectedAgent {
        DetectedAgent {
            spec: agentry_core::models::AgentSpec {
                id: "codex".to_string(),
                name: "codex".to_string(),
                cli_binary: "codex".to_string(),
                config_dir: ".codex".to_string(),
                prompt_filename: "AGENTS.md".to_string(),
                prompt_format: agentry_core::models::PromptFormat::PlainMd,
                skills_dir_name: None,
                max_size: None,
                install_methods: vec![agentry_core::models::InstallMethod::Npm {
                    package: "codex".to_string(),
                }],
            },
            installed: false,
            version: None,
            config_dir_exists: true,
            prompt_file_exists: false,
            skills_dir: None,
            skills_symlink_pattern: None,
            installed_skills: Vec::new(),
            detected_methods: Vec::new(),
        }
    }

    #[test]
    fn enter_installs_selected_version_when_version_list_loaded() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![version_flow_agent()];
        app.version_list = Some(vec!["1.0.0".to_string(), "2.1.3".to_string()]);
        app.version_selected = 1;

        press(&mut app, KeyCode::Enter);

        match app.agent_confirm {
            Some(AgentConfirmAction::Install {
                ref agent_id,
                ref version,
                ..
            }) => {
                assert_eq!(agent_id, "codex");
                assert_eq!(version.as_deref(), Some("2.1.3"));
            }
            other => panic!("expected Install confirm, got {:?}", other),
        }
        assert!(app.version_list.is_none(), "picker must close");
        assert_eq!(app.version_selected, 0);
    }

    #[test]
    fn esc_cancels_version_selection() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![version_flow_agent()];
        app.version_list = Some(vec!["1.0.0".to_string()]);
        app.version_selected = 0;

        press(&mut app, KeyCode::Esc);

        assert!(app.agent_confirm.is_none());
        assert!(app.version_list.is_none());
        assert_eq!(app.version_selected, 0);
        assert_eq!(
            app.status_message.as_deref(),
            Some("Version selection cancelled")
        );
    }

    #[test]
    fn esc_silent_without_version_list() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![version_flow_agent()];
        press(&mut app, KeyCode::Esc);
        assert!(app.status_message.is_none());
        assert!(app.agent_confirm.is_none());
    }

    #[test]
    fn jk_moves_version_selection_while_loaded() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![version_flow_agent()];
        app.version_list = Some(vec![
            "1.0.0".to_string(),
            "2.0.0".to_string(),
            "3.0.0".to_string(),
        ]);

        press(&mut app, KeyCode::Char('j'));
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.version_selected, 2);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.version_selected, 2, "clamped at last version");
        press(&mut app, KeyCode::Char('k'));
        assert_eq!(app.version_selected, 1);
        assert_eq!(app.list_selected, 0, "agent list must not move");
    }

    #[test]
    fn parse_versions_filters_unsafe_strings() {
        let method = agentry_core::models::InstallMethod::Npm {
            package: "codex".to_string(),
        };
        let raw = r#"["1.2.3","1;rm -rf /","$(whoami)","1`id`","1 2","a_b-c.d+e",""]"#;
        let parsed = parse_versions(raw, &method);
        assert_eq!(parsed, vec!["1.2.3".to_string(), "a_b-c.d+e".to_string()]);
    }

    #[test]
    fn parse_versions_rejects_unsafe_brew_stable() {
        let method = agentry_core::models::InstallMethod::Brew {
            formula: "codex".to_string(),
            cask: false,
        };
        let raw = r#"{"versions":{"stable":"1.2;evil"}}"#;
        assert!(parse_versions(raw, &method).is_empty());
    }

    #[test]
    fn parse_versions_passes_normal_version() {
        let method = agentry_core::models::InstallMethod::Other {
            description: "x".to_string(),
            install_cmd: "x".to_string(),
        };
        let parsed = parse_versions("1.2.3\n2.0.0-rc.1\nbad version\n", &method);
        assert_eq!(parsed, vec!["1.2.3".to_string(), "2.0.0-rc.1".to_string()]);
    }

    #[test]
    fn confirm_refuses_unsafe_version() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![version_flow_agent()];
        app.version_list = Some(vec!["1.2.3;rm -rf /".to_string()]);
        app.version_selected = 0;

        press(&mut app, KeyCode::Enter);

        assert!(app.agent_confirm.is_none(), "confirm must not be created");
        assert!(app.version_list.is_none(), "picker must close");
        assert_eq!(app.error_message.as_deref(), Some("Invalid version string"));
    }

    #[test]
    fn confirm_allows_safe_version_with_special_chars() {
        let mut app = App::new();
        app.tab_index = 0;
        app.detected_agents = vec![version_flow_agent()];
        app.version_list = Some(vec!["1.2.3-beta+build_1".to_string()]);
        app.version_selected = 0;

        press(&mut app, KeyCode::Enter);

        match app.agent_confirm {
            Some(AgentConfirmAction::Install { ref version, .. }) => {
                assert_eq!(version.as_deref(), Some("1.2.3-beta+build_1"));
            }
            other => panic!("expected Install confirm, got {:?}", other),
        }
    }
}
