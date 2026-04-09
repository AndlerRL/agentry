use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::{backend::Backend, Frame, Terminal};

use crate::ui;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Intro,
    Dashboard,
    Quit,
}

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

impl App {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Intro,
            tab_index: 0,
            list_selected: 0,
            detected_agents: Vec::new(),
            intro_progress: 0.0,
            intro_ready: false,
            spinner_frame: 0,
            should_quit: false,
            status_message: None,
            show_help: false,
        }
    }

    pub async fn run<B: Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        // Run intro animation with agent detection
        self.run_intro(terminal).await?;

        if self.should_quit {
            return Ok(());
        }

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

    async fn run_intro<B: Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
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
            // Other keys per tab handled in match below
            KeyCode::Enter => self.on_enter(),
            KeyCode::Char('n') => self.on_new(),
            KeyCode::Char('d') => self.on_delete(),
            KeyCode::Char('s') => self.on_sync(),
            KeyCode::Char('e') => self.on_edit(),
            KeyCode::Char('i') => self.on_insert(),
            KeyCode::Char('u') => self.on_update(),
            _ => {}
        }
        Ok(())
    }

    fn next_tab(&mut self) {
        self.tab_index = (self.tab_index + 1) % 6;
        self.list_selected = 0;
    }

    fn prev_tab(&mut self) {
        self.tab_index = if self.tab_index == 0 { 5 } else { self.tab_index - 1 };
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
            1 => self.detected_agents.len(),
            _ => 0,
        }
    }

    fn on_enter(&mut self) {
        // Tab-specific: open selected item
    }

    fn on_new(&mut self) {
        self.status_message = Some("New: not yet implemented".into());
    }

    fn on_delete(&mut self) {
        self.status_message = Some("Delete: not yet implemented".into());
    }

    fn on_sync(&mut self) {
        self.status_message = Some("Sync: not yet implemented".into());
    }

    fn on_edit(&mut self) {
        self.status_message = Some("Edit: not yet implemented".into());
    }

    fn on_insert(&mut self) {
        self.status_message = Some("Insert: not yet implemented".into());
    }

    fn on_update(&mut self) {
        self.status_message = Some("Update: not yet implemented".into());
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
}