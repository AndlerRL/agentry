use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{backend::Backend, Frame, Terminal};

use crate::editor::Editor;
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
    /// Editor state (when editing a prompt)
    pub editor: Option<Editor>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Intro,
    Dashboard,
    Editor,
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

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

impl App {
    pub fn new() -> Self {
        let home_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp"));

        Self {
            mode: AppMode::Intro,
            tab_index: 0,
            list_selected: 0,
            detected_agents: Vec::new(),
            prompts: Vec::new(),
            skill_hub: None,
            agent_skills_dirs: Vec::new(),
            sync_results: Vec::new(),
            editor: None,
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

        // Main event loop
        while !self.should_quit {
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

    async fn run_intro<B: Backend + std::io::Write>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<()> {
        // Animate intro while detecting agents in background
        let agents = agentry_agents::detect_all_agents().await;
        self.detected_agents = agents;

        // Animate progress
        let steps = 20;
        for i in 0..=steps {
            self.intro_progress = i as f32 / steps as f32;
            terminal.draw(|f| ui::draw_intro(f, self))?;

            // Check if user pressed a key to skip
            if crossterm::event::poll(std::time::Duration::from_millis(40))? {
                if let Event::Key(_) = event::read()? {
                    break;
                }
            }
        }

        self.intro_progress = 1.0;
        self.intro_ready = true;
        terminal.draw(|f| ui::draw_intro(f, self))?;

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
        // If in editor mode, forward keys to editor
        if self.mode == AppMode::Editor {
            if let Some(ref mut editor) = self.editor {
                if key.code == KeyCode::Esc && editor.mode == crate::editor::EditorMode::Normal {
                    self.mode = AppMode::Dashboard;
                    self.editor = None;
                    return Ok(());
                }
                editor.handle_key(key);
                // Check for :wq command (save and quit)
                if let Some(ref msg) = editor.message {
                    if msg == "Saved." {
                        // Save the prompt content
                        if let Some(path) = editor.filename.clone() {
                            let content = editor.buffer.content();
                            // Write to the prompt's source path or canonical store
                            let save_path =
                                self.home_dir.join(".agents").join("prompts").join(&path);
                            if let Some(parent) = save_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            let _ = std::fs::write(&save_path, &content);
                        }
                        editor.message = None;
                    }
                }
            }
            return Ok(());
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
                            // Create a new prompt and open editor
                            let mut editor = Editor::with_content("");
                            editor.filename = Some(format!("{}.md", name));
                            editor.modified = false;
                            editor.message = Some("New prompt - press i to insert".into());
                            self.mode = AppMode::Editor;
                            self.editor = Some(editor);
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
            KeyCode::Char('6') => self.tab_index = 5,
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
            _ => {}
        }
        Ok(())
    }

    fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % 6;
        self.list_selected = 0;
    }

    fn prev_tab(&mut self) {
        self.tab_index = if self.tab_index == 0 {
            5
        } else {
            self.tab_index - 1
        };
        self.list_selected = 0;
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

    fn list_max(&self) -> usize {
        match self.tab_index {
            0 | 1 => self.detected_agents.len().max(1),
            2 => self.prompts.len() + 1, // +1 for "New Prompt" entry
            3 => self.skill_hub.as_ref().map(|h| h.skills.len()).unwrap_or(0).max(1),
            4 => self.sync_results.len().max(1),
            _ => 0,
        }
    }

    fn on_enter(&mut self) {
        if self.tab_index == 2 {
            // Prompts tab - open selected prompt for editing
            if self.list_selected < self.prompts.len() {
                let prompt = &self.prompts[self.list_selected];
                let content = if let Some(ref path) = prompt.source_path {
                    std::fs::read_to_string(path).unwrap_or_else(|_| prompt.body.clone())
                } else {
                    prompt.body.clone()
                };
                let mut editor = Editor::with_content(&content);
                editor.filename = Some(prompt.canonical_filename());
                self.mode = AppMode::Editor;
                self.editor = Some(editor);
            }
        }
    }

    fn on_new(&mut self) {
        if self.tab_index == 2 {
            self.new_prompt_name = Some(String::new());
            self.status_message = Some("Enter prompt name, then press Enter".into());
        }
    }

    fn on_delete(&mut self) {
        if self.tab_index == 2 && self.list_selected < self.prompts.len() {
            self.delete_confirm = Some(self.list_selected);
            self.status_message = Some(format!(
                "Delete '{}'? (y/n)",
                self.prompts[self.list_selected].name
            ));
        }
    }

    fn on_sync(&mut self) {
        if self.tab_index == 4 {
            // Sync tab — execute sync for all prompts
            let home = self.home_dir.clone();
            let _project_dirs = [home.join("Development")];
            let agents = self.detected_agents.clone();

            let mut results = Vec::new();
            for prompt in &self.prompts {
                let plan = agentry_sync::planner::plan_sync(prompt, &agents, &home);
                // Check status first
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
            }

            self.sync_results = results;
            self.status_message = Some(format!("Sync plan loaded ({} mappings)", self.sync_results.len()));
            self.list_selected = 0;
        } else {
            self.status_message = Some("Sync: switch to Sync tab (5) to execute".into());
        }
    }

    fn on_edit(&mut self) {
        self.on_enter(); // Same as Enter - open in editor
    }

    fn on_insert(&mut self) {
        // In Skills tab: install selected skill
        if self.tab_index == 3 {
            if let Some(ref hub) = self.skill_hub {
                let skills: Vec<_> = hub.skills.values().collect();
                if self.list_selected < skills.len() {
                    let skill = skills[self.list_selected];
                    if !skill.installed {
                        self.skill_confirm = Some(SkillConfirmAction::Install(skill.name.clone()));
                        self.status_message =
                            Some(format!("Install '{}'? (y/n)", skill.name));
                    } else {
                        self.status_message = Some(format!(
                            "'{}' is already installed",
                            skill.name
                        ));
                    }
                }
            }
        } else {
            self.on_new(); // Same as 'n' - create new prompt
        }
    }

    fn on_update(&mut self) {
        if self.tab_index == 3 {
            // Skills tab - update selected or all
            if let Some(ref hub) = self.skill_hub {
                let skills: Vec<_> = hub.skills.values().collect();
                if self.list_selected < skills.len() {
                    let skill = skills[self.list_selected];
                    if skill.installed {
                        self.skill_confirm = Some(SkillConfirmAction::Update(skill.name.clone()));
                        self.status_message =
                            Some(format!("Update '{}'? (y/n)", skill.name));
                    } else {
                        self.status_message =
                            Some(format!("'{}' is not installed", skill.name));
                    }
                }
            }
        } else {
            self.status_message = Some("Update: only available in Skills tab".into());
        }
    }

    fn on_remove(&mut self) {
        if self.tab_index == 3 {
            // Skills tab - remove selected skill
            if let Some(ref hub) = self.skill_hub {
                let skills: Vec<_> = hub.skills.values().collect();
                if self.list_selected < skills.len() {
                    let skill = skills[self.list_selected];
                    if skill.installed {
                        self.skill_confirm = Some(SkillConfirmAction::Remove(skill.name.clone()));
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

    fn on_github(&mut self) {
        if self.tab_index == 3 {
            // Skills tab - open GitHub source in browser
            if let Some(ref hub) = self.skill_hub {
                let skills: Vec<_> = hub.skills.values().collect();
                if self.list_selected < skills.len() {
                    let skill = skills[self.list_selected];
                    if !skill.source_url.is_empty() {
                        let url = skill.source_url.clone();
                        self.status_message = Some(format!("Open: {}", url));
                        // Try to open in browser
                        let _ = std::process::Command::new("open").arg(&url).spawn();
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
                            self.status_message =
                                Some(format!("No source for '{}'", name));
                            return;
                        }
                        let result = agentry_skills::install::install_skill(
                            &home, &source, &skill_path, &dirs,
                        );
                        match result {
                            Ok(r) => {
                                self.status_message = Some(r.message);
                                self.discover_skills(); // Refresh
                            }
                            Err(e) => {
                                self.status_message =
                                    Some(format!("Install error: {}", e));
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
                self.status_message = Some(format!(
                    "Updated {}/{} skills",
                    ok_count,
                    results.len()
                ));
                self.discover_skills(); // Refresh
            }
        }
    }

    fn draw(&self, f: &mut Frame) {
        match self.mode {
            AppMode::Intro => ui::draw_intro(f, self),
            AppMode::Dashboard => ui::draw_dashboard(f, self),
            AppMode::Editor => {
                if let Some(ref editor) = self.editor {
                    ui::draw_editor(f, editor);
                }
            }
            AppMode::Quit => {}
        }
    }

    pub fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }
}