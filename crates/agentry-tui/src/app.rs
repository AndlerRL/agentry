use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{backend::Backend, Frame, Terminal};

use crate::ui;

/// A single sync result for display in the Sync tab.
#[derive(Debug, Clone)]
pub struct SyncResultEntry {
    pub prompt_name: String,
    pub agent_id: String,
    pub destination: String,
    pub status: agentry_core::models::SyncStatus,
    pub action: agentry_core::models::SyncAction,
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
    /// Current tab index (0-5)
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
    /// Sync plan entries (populated when user presses 's' on Sync tab)
    pub sync_results: Vec<SyncResultEntry>,
    /// OpenClaw workspace data
    pub openclaw_state: Option<OpenClawState>,
    /// ACP capability matrix
    pub acp_capabilities: Vec<agentry_acp::protocol::AgentCapability>,
    /// New prompt name (when creating a new prompt)
    pub new_prompt_name: Option<String>,
    /// Delete confirmation pending
    pub delete_confirm: Option<usize>,
    /// Home directory
    pub home_dir: PathBuf,
    /// Intro animation progress (0.0 → 1.0)
    pub intro_progress: f32,
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
    /// When Some, user is typing a version string for install.
    #[allow(dead_code)]
    pub version_input: Option<String>,
    /// Cached version list for the selected install method.
    pub version_list: Option<Vec<String>>,
    /// Error fetching versions.
    pub version_list_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Intro,
    Dashboard,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SkillConfirmAction {
    Install(String),
    Remove(String),
    Update(String),
    UpdateAll,
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
            openclaw_state: None,
            acp_capabilities: Vec::new(),
            new_prompt_name: None,
            delete_confirm: None,
            home_dir,
            intro_progress: 0.0,
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
            version_input: None,
            version_list: None,
            version_list_error: None,
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

        // Build ACP capability matrix
        self.discover_capabilities();

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

    fn discover_capabilities(&mut self) {
        if let Ok(caps) = agentry_acp::router::build_capability_matrix(&self.home_dir) {
            self.acp_capabilities = caps;
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
                            let template = format!(
                                "# {}\n\n<!-- Write your prompt content here -->\n",
                                name
                            );
                            if let Err(e) = std::fs::write(&prompt_path, &template) {
                                self.error_message =
                                    Some(format!("Failed to create prompt: {}", e));
                            } else {
                                self.status_message =
                                    Some(format!("Created prompt: {}", name));
                                // Reload prompts
                                self.discover_prompts();
                                // Open it in external editor
                                self.edit_file_externally(&prompt_path);
                                // Reload the content
                                if let Ok(content) = std::fs::read_to_string(&prompt_path) {
                                    // Find the new prompt and update it
                                    if let Some(p) = self
                                        .prompts
                                        .iter_mut()
                                        .find(|p| p.name == name)
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

        match key.code {
            // Global keybindings
            KeyCode::Char('q') => {
                self.mode = AppMode::Quit;
                self.should_quit = true;
            }
            KeyCode::Char('?') => self.show_help = !self.show_help,
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.prev_tab(),
            KeyCode::Char('1') => self.tab_index = 0,
            KeyCode::Char('2') => self.tab_index = 1,
            KeyCode::Char('3') => self.tab_index = 2,
            KeyCode::Char('4') => self.tab_index = 3,
            KeyCode::Char('5') => self.tab_index = 4,
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => self.list_next(),
            KeyCode::Char('k') | KeyCode::Up => self.list_prev(),
            // Actions
            KeyCode::Enter => self.on_enter(),
            KeyCode::Char('n') => self.on_new(),
            KeyCode::Char('d') => self.on_delete(),
            KeyCode::Char('s') => self.on_sync(),
            KeyCode::Char('e') => self.on_edit(),
            KeyCode::Char('i') => self.on_insert(),
            KeyCode::Char('u') => self.on_update(),
            KeyCode::Char('r') => self.on_remove(),
            KeyCode::Char('g') => self.on_github(),
            KeyCode::Char('c') => self.on_create_workspace(),
            KeyCode::Char('a') => self.on_add_agent(),
            KeyCode::Left => self.method_prev(),
            KeyCode::Right => self.method_next(),
            KeyCode::Char('v') => self.on_list_versions(),
            KeyCode::Char('w') => self.on_workflow(),
            _ => {}
        }
        Ok(())
    }

    fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % 5;
        self.list_selected = 0;
        self.method_selected = 0;
    }

    fn prev_tab(&mut self) {
        self.tab_index = if self.tab_index == 0 {
            4
        } else {
            self.tab_index - 1
        };
        self.list_selected = 0;
        self.method_selected = 0;
    }

    fn list_next(&mut self) {
        let max = self.list_max();
        if max > 0 && self.list_selected < max - 1 {
            self.list_selected += 1;
        }
    }

    fn list_prev(&mut self) {
        if self.list_selected > 0 {
            self.list_selected -= 1;
        }
    }

    /// Total number of items in the current tab's list (including headers/actions).
    fn list_max(&self) -> usize {
        match self.tab_index {
            0 => self.detected_agents.len().max(1), // Agents tab
            1 => { // Prompts tab
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
                    let mut groups: std::collections::BTreeMap<&str, Vec<&crate::app::SyncResultEntry>> =
                        std::collections::BTreeMap::new();
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
                // OpenClaw: status header + spacer + workspaces (each on one line with doc badges)
                if let Some(ref oc_state) = self.openclaw_state {
                    if oc_state.workspaces.is_empty() {
                        5 // status message + spacer + 2 hints + empty
                    } else {
                        1 + 1 + oc_state.workspaces.len() // status header + spacer + workspaces
                    }
                } else {
                    1
                }
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

    /// Resolve the selected workspace index (OpenClaw tab).
    fn selected_workspace_index(&self) -> Option<usize> {
        if self.tab_index != 4 {
            return None;
        }
        let oc_state = self.openclaw_state.as_ref()?;
        if oc_state.workspaces.is_empty() {
            return None;
        }
        // Layout: status header (row 0) + spacer (row 1) + workspace entries (row 2+)
        let ws_row = self.list_selected.saturating_sub(2);
        if ws_row < oc_state.workspaces.len() {
            Some(ws_row)
        } else {
            None
        }
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
        let result = std::process::Command::new(&editor)
            .arg(file_path)
            .status();

        // Restore TUI
        use crossterm::{
            terminal::{enable_raw_mode, EnterAlternateScreen},
        };
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

    fn on_enter(&mut self) {
        match self.tab_index {
            0 => {
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
                let skill_info = self.selected_skill().map(|s| {
                    (s.name.clone(), s.installed, s.source.clone())
                });
                if let Some((name, installed, source)) = skill_info {
                    if !installed && !source.is_empty() {
                        self.skill_confirm =
                            Some(SkillConfirmAction::Install(name.clone()));
                        self.status_message =
                            Some(format!("Install '{}'? (y/n)", name));
                    } else if installed {
                        self.status_message =
                            Some(format!("'{}' is already installed", name));
                    }
                }
            }
            4 => {
                // OpenClaw tab - edit selected workspace doc
                let doc_path: Option<std::path::PathBuf> = {
                    let oc_state = self.openclaw_state.as_ref();
                    let ws_idx = self.selected_workspace_index();
                    oc_state
                        .and_then(|s| ws_idx.and_then(|i| s.workspaces.get(i)))
                        .and_then(|ws| ws.docs.first().map(|d| d.path.clone()))
                };
                let has_ws = self.selected_workspace_index().is_some();
                if let Some(path) = doc_path {
                    self.edit_file_externally(&path);
                } else if has_ws {
                    self.status_message = Some("No docs in this workspace".into());
                }
            }
            _ => {}
        }
    }

    fn on_new(&mut self) {
        if self.tab_index == 1 {
            self.new_prompt_name = Some(String::new());
            self.status_message = Some("Enter prompt name, then press Enter".into());
        } else if self.tab_index == 4 {
            // OpenClaw tab - create workspace
            self.on_create_workspace();
        }
    }

    fn on_delete(&mut self) {
        if self.tab_index == 1 {
            if let Some(idx) = self.selected_prompt_index() {
                self.delete_confirm = Some(idx);
                self.status_message = Some(format!(
                    "Delete '{}'? (y/n)",
                    self.prompts[idx].name
                ));
            }
        }
    }

    fn on_sync(&mut self) {
        if self.tab_index == 3 {
            // Sync tab — execute sync for all prompts (agents + projects)
            let home = self.home_dir.clone();
            let project_dirs = [home.join("Development")];
            let agents = self.detected_agents.clone();

            let mut results = Vec::new();
            for prompt in &self.prompts {
                // Agent-level sync
                let plan = agentry_sync::planner::plan_sync(prompt, &agents, &home);
                let mappings = agentry_sync::executor::check_sync_status(prompt, &plan.mappings);
                for mapping in &mappings {
                    results.push(SyncResultEntry {
                        prompt_name: prompt.name.clone(),
                        agent_id: mapping.agent_id.clone(),
                        destination: mapping.destination.display().to_string(),
                        status: mapping.status,
                        action: mapping.action,
                    });
                }

                // Project-level sync for global prompts
                let project_mappings =
                    agentry_sync::planner::project_sync_plans(prompt, &project_dirs, &home);
                if !project_mappings.is_empty() {
                    let checked =
                        agentry_sync::executor::check_sync_status(prompt, &project_mappings);
                    for mapping in &checked {
                        results.push(SyncResultEntry {
                            prompt_name: prompt.name.clone(),
                            agent_id: mapping.agent_id.clone(),
                            destination: mapping.destination.display().to_string(),
                            status: mapping.status,
                            action: mapping.action,
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
        } else {
            self.status_message = Some("Sync: switch to Sync tab (4) to execute".into());
        }
    }

    fn on_edit(&mut self) {
        if self.tab_index == 1 {
            if let Some(idx) = self.selected_prompt_index() {
                self.edit_with_external_editor(idx);
            }
        }
    }

    fn on_insert(&mut self) {
        if self.tab_index == 2 {
            if let Some(idx) = self.selected_skill_index() {
                if let Some(ref hub) = self.skill_hub {
                    let skills: Vec<_> = hub.skills.values().collect();
                    if idx < skills.len() {
                        let skill = skills[idx];
                        if !skill.installed {
                            self.skill_confirm =
                                Some(SkillConfirmAction::Install(skill.name.clone()));
                            self.status_message =
                                Some(format!("Install '{}'? (y/n)", skill.name));
                        } else {
                            self.status_message =
                                Some(format!("'{}' is already installed", skill.name));
                        }
                    }
                }
            }
        } else {
            self.on_new();
        }
    }

    fn method_prev(&mut self) {
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
                        self.status_message = Some("Version listing not supported for this method".into());
                        return;
                    }
                };
                self.status_message = Some("Fetching versions...".into());
                match std::process::Command::new("sh").arg("-c").arg(&cmd).output() {
                    Ok(output) if output.status.success() => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        // Parse versions depending on the method type
                        let versions: Vec<String> = match method {
                            agentry_core::models::InstallMethod::Brew { .. } => {
                                // brew info --json=v2 returns JSON
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&stdout) {
                                    let v = val["versions"]["stable"].as_str().unwrap_or("unknown");
                                    vec![v.to_string()]
                                } else {
                                    vec!["parse error".into()]
                                }
                            }
                            agentry_core::models::InstallMethod::Npm { .. } => {
                                serde_json::from_str::<Vec<String>>(&stdout).unwrap_or_default()
                            }
                            _ => {
                                stdout.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
                            }
                        };
                        if versions.is_empty() {
                            self.version_list_error = Some("No versions found".into());
                        } else {
                            self.version_list = Some(versions);
                            self.status_message = Some("Versions loaded. Select with j/k, Enter to confirm".into());
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
            terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        };

        let cmd = match &action {
            AgentConfirmAction::Install { method, version, .. } => {
                method.install_command(version.as_deref())
            }
            AgentConfirmAction::Update { method, .. } => method.update_command(),
            AgentConfirmAction::Remove { method, .. } => method.remove_command(),
        };

        // Suspend TUI
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

        let status = std::process::Command::new("sh").arg("-c").arg(&cmd).status();

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
                // Re-detect agents synchronously (in a TUI context we use block_on)
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    self.detected_agents = agentry_agents::detect_all_agents().await;
                });
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
                            self.status_message =
                                Some(format!("Update '{}'? (y/n)", skill.name));
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
                            self.status_message =
                                Some(format!("Remove '{}'? (y/n)", skill.name));
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
            SkillConfirmAction::UpdateAll => {
                let home = self.home_dir.clone();
                let dirs = self.agent_skills_dirs.clone();
                let results = agentry_skills::install::update_all_skills(&home, &dirs);
                let ok_count = results.iter().filter(|r| r.success).count();
                self.status_message =
                    Some(format!("Updated {}/{} skills", ok_count, results.len()));
                self.discover_skills(); // Refresh
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
        if self.tab_index == 4 {
            if agentry_openclaw::discovery::is_openclaw_installed() {
                self.status_message = Some("Run: openclaw setup".into());
                let _ = std::process::Command::new("openclaw").arg("setup").spawn();
            } else {
                self.status_message =
                    Some("OpenClaw not installed. Install from https://openclaw.dev".into());
            }
        }
    }

    fn on_add_agent(&mut self) {
        if self.tab_index == 4 {
            if agentry_openclaw::discovery::is_openclaw_installed() {
                self.status_message = Some("Run: openclaw agents add <name>".into());
                // Could prompt for name in the future, for now show guidance
            } else {
                self.status_message = Some("OpenClaw not installed".into());
            }
        }
    }

    fn on_workflow(&mut self) {
        if self.tab_index == 3 {
            if !self.acp_capabilities.is_empty() {
                let task = if let Some(entry) = self.selected_sync_entry() {
                    format!(
                        "Sync {} to {}",
                        entry.prompt_name, entry.agent_id
                    )
                } else {
                    "Sync all prompts".to_string()
                };
                let decomp =
                    agentry_acp::orchestrator::decompose_task(&task, &self.acp_capabilities);
                let subtask_count = decomp.subtasks.len();
                let agent_names: Vec<_> = decomp
                    .subtasks
                    .iter()
                    .map(|s| s.assigned_agent.clone())
                    .collect();

                // Save the generated workflow
                let workflow_dir = self.home_dir.join(".agents").join("workflows");
                if let Err(e) = std::fs::create_dir_all(&workflow_dir) {
                    self.error_message = Some(format!("Failed to create workflow dir: {}", e));
                }
                let workflow_path = workflow_dir.join(format!("{}.lobster", decomp.workflow.name));
                if let Err(e) =
                    agentry_acp::orchestrator::save_workflow(&decomp.workflow, &workflow_path)
                {
                    self.status_message = Some(format!("Workflow save error: {}", e));
                } else {
                    self.status_message = Some(format!(
                        "Workflow: {} subtask(s) → {} (saved to {})",
                        subtask_count,
                        agent_names.join(", "),
                        workflow_path.display()
                    ));
                }
            } else {
                self.status_message = Some("No agent capabilities found".into());
            }
        } else {
            self.status_message =
                Some("Workflow: switch to Sync tab (4) to generate workflows".into());
        }
    }
}
