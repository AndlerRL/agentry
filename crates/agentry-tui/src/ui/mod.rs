mod intro;
mod dashboard;

use ratatui::layout::Rect;

pub use intro::draw_intro;
pub use dashboard::draw_dashboard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Agents,
    Prompts,
    Skills,
    Sync,
    OpenClaw,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Dashboard,
        Tab::Agents,
        Tab::Prompts,
        Tab::Skills,
        Tab::Sync,
        Tab::OpenClaw,
    ];

    pub fn _index(self) -> usize {
        match self {
            Tab::Dashboard => 0,
            Tab::Agents => 1,
            Tab::Prompts => 2,
            Tab::Skills => 3,
            Tab::Sync => 4,
            Tab::OpenClaw => 5,
        }
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        Tab::ALL.get(i).copied()
    }

    pub fn title(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Agents => "Agents",
            Tab::Prompts => "Prompts",
            Tab::Skills => "Skills",
            Tab::Sync => "Sync",
            Tab::OpenClaw => "OpenClaw",
        }
    }
}

fn _centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}