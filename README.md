
<pre>

████████████████████████████████████████████████████████████████████████████████████████████
█▌                                                                                        ▐█
█▌                                                                                        ▐█
█▌                                                                                        ▐█
█▌                                                     I8                                 ▐█
█▌                                                     I8                                 ▐█
█▌                                                   88888888                             ▐█
█▌                                                     I8                                 ▐█
█▌       ,gggg,gg    ,gggg,gg   ,ggg,    ,ggg,,ggg,    I8    ,gggggg,  gg     gg          ▐█
█▌      dP"  "Y8I   dP"  "Y8I  i8" "8i  ,8" "8P" "8,   I8    dP""""8I  I8     8I          ▐█
█▌     i8'    ,8I  i8'    ,8I  I8, ,8I  I8   8I   8I  ,I8,  ,8'    8I  I8,   ,8I          ▐█
█▌    ,d8,   ,d8b,,d8,   ,d8I  'YbadP' ,dP   8I   Yb,,d88b,,dP     Y8,,d8b, ,d8I          ▐█
█▌    P"Y8888P"'Y8P"Y8888P"888888P"Y8888P'   8I   'Y88P""Y88P      'Y8P""Y88P"888         ▐█
█▌                       ,d8I'                                              ,d8I'         ▐█
█▌                     ,dP'8I                                             ,dP'8I          ▐█
█▌                    ,8"  8I                                            ,8"  8I          ▐█
█▌                    I8   8I                                            I8   8I          ▐█
█▌                    '8, ,8I                                            '8, ,8I          ▐█
█▌                     'Y8P"                                              'Y8P"           ▐█
█▌                                                                                        ▐█
█▌                                                                                        ▐█
█▌                                                                                        ▐█
████████████████████████████████████████████████████████████████████████████████████████████
</pre>

# agentry — The Multi-Agent Prompt Manager

[![Crates.io](https://img.shields.io/crates/v/agentry-tui.svg)](https://crates.io/crates/agentry-tui)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/github/actions/workflow/status/AndlerRL/agentry/ci.yml?branch=main)](https://github.com/AndlerRL/agentry/actions)

> One source of truth for prompts. Format-aware sync to every agent CLI.

<!-- TODO: Add screenshot once available -->
<!--
![agentry TUI screenshot](assets/agentry-screenshot.png)
-->

## What is agentry?

agentry is a terminal UI application that manages prompts for **11 agent CLIs** from a single unified source. Instead of maintaining separate prompt files scattered across `.claude/CLAUDE.md`, `.continue/`, `.codex/AGENTS.md`, and so on, agentry lets you write prompts once and sync them everywhere -- automatically converting between each agent's native file format.

### Supported Agents

| Agent | CLI Binary | Config Dir | Prompt File | Format |
|-------|-----------|------------|-------------|--------|
| Claude Code | `claude` | `.claude` | `CLAUDE.md` | Plain Markdown |
| Continue | `continue` | `.continue` | `prompts/` | XML Tag + MD |
| Gemini CLI | `gemini` | `.gemini` | `GEMINI.md` | Plain Markdown |
| Codex | `codex` | `.codex` | `AGENTS.md` | Plain Markdown |
| Amp | `amp` | `.amp` | `AGENTS.md` | Plain Markdown |
| OpenCode | `opencode` | `.opencode` | `AGENTS.md` | Frontmatter + MD |
| Firebender | `firebender` | `.firebender` | `rules/` (.mdc) | MDC |
| OpenClaw | `openclaw` | `.openclaw` | `AGENTS.md` | Plain Markdown |
| DeepAgents | `deepagents` | `.deepagents` | `AGENTS.md` | Plain Markdown |
| Antigravity | `antigravity` | `.antigravity` | `SKILL.md` | Frontmatter + MD |
| Warp | `warp-cli` | `.warp` | `AGENTS.md` | Frontmatter + MD |

## Key Features

- **One source of truth** -- Write prompts once in `~/.agents/prompts/`, sync to all agents
- **5 format converters** -- Automatic conversion between PlainMD, FrontmatterMD, MDC, XmlTagMD, and LobsterYAML
- **Agent detection** -- Parallel discovery of installed agents, versions, config dirs, and skills
- **Sync engine** -- Plan, preview, and execute prompt distribution across agents (copy, symlink, or skip)
- **Skill hub** -- Browse, install, and update community skills from Git repositories
- **OpenClaw workspace** -- Discover and manage OpenClaw `.lobster` workflows
- **ACP orchestration** -- Agent Communication Protocol for multi-agent routing
- **Vim-like TUI** -- Terminal interface with modal editing and familiar keybindings

## Format Converters

agentry understands five distinct prompt formats and can convert between them:

| Converter | Format | Used By |
|-----------|--------|---------|
| `PlainMarkdownConverter` | Plain Markdown | Claude Code, Gemini CLI, Codex, Amp, OpenClaw, DeepAgents |
| `FrontmatterMdConverter` | YAML frontmatter + Markdown body | OpenCode, Antigravity, Warp |
| `MdcConverter` | MDC (frontmatter with globs) | Firebender |
| `XmlTagMdConverter` | Frontmatter + XML tag wrappers | Continue (`<expertise>`, `<base_rules>`) |
| `LobsterYamlConverter` | Lobster YAML workflows | OpenClaw |

Each converter implements the `FormatConverter` trait with `parse()` and `serialize()` methods, enabling lossless round-tripping and cross-format conversion through the `UnifiedPrompt` intermediate representation.

## TUI

The terminal UI features an intro animation with ASCII art and a progress bar while agents are detected in parallel, followed by a 6-tab dashboard:

| Tab | Key | Description |
|-----|-----|-------------|
| Dashboard | `1` | Overview of detected agents and system status |
| Agents | `2` | Browse detected agents, versions, skills, and config paths |
| Prompts | `3` | Manage and edit prompts (Phase 2) |
| Skills | `4` | Browse and install skill hub entries (Phase 4) |
| Sync | `5` | Plan and execute prompt sync (Phase 3) |
| OpenClaw | `6` | Manage OpenClaw workspaces (Phase 5) |

### Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` / Arrow keys | Navigate list |
| `Tab` / `Shift+Tab` | Switch tabs forward/backward |
| `1` -- `6` | Jump directly to tab |
| `Enter` | Open/edit selected item |
| `n` | New prompt |
| `d` | Delete prompt |
| `s` | Sync to agents |
| `e` | Edit prompt |
| `u` | Update skills |
| `i` / `a` | Insert mode |
| `:w` / `:q` | Write/quit (vim-like commands) |
| `?` | Toggle help overlay |
| `q` | Quit |

## Architecture

agentry is organized as a Cargo workspace with 7 crates:

```
agentry/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── agentry-core/           # Core types, models, and format converters
│   ├── agentry-agents/         # Agent specs, detection, and registry
│   ├── agentry-sync/            # Sync planner and executor
│   ├── agentry-skills/          # Skill hub, lockfile, and installer
│   ├── agentry-openclaw/        # OpenClaw workspace discovery and docs
│   ├── agentry-acp/             # ACP protocol and message router
│   └── agentry-tui/             # Terminal UI (binary crate: `agentry`)
└── tests/
    └── integration/             # Integration tests
```

### Crate Dependencies

```
agentry-tui
├── agentry-core
├── agentry-agents
├── agentry-sync
├── agentry-skills
├── agentry-openclaw
└── agentry-acp

agentry-agents ──> agentry-core
agentry-sync   ──> agentry-core, agentry-agents
agentry-skills ──> agentry-core
agentry-openclaw ──> agentry-core, agentry-agents
agentry-acp   ──> agentry-core, agentry-agents, agentry-skills
```

### Core Models

- **`UnifiedPrompt`** -- Canonical representation of a prompt from any format, with frontmatter, body, XML tags, scope, and source format
- **`AgentSpec`** -- Static specification of a known agent (binary name, config dir, prompt filename, format)
- **`DetectedAgent`** -- Runtime detection result (installed status, version, skills, symlinks)
- **`SyncPlan`** / **`SyncMapping`** -- Planned sync actions with status tracking (UpToDate, Missing, Outdated, Conflict)
- **`SkillEntry`** -- A skill from the hub with source repo and install state
- **`AppConfig`** -- User configuration at `~/.agents/agentry.toml`

## Installation

### From Source

```bash
cargo install --git https://github.com/AndlerRL/agentry
```

### Build Locally

```bash
git clone https://github.com/AndlerRL/agentry.git
cd agentry
cargo build --release
# Binary at target/release/agentry
```

### Run Directly

```bash
cargo run --manifest-path crates/agentry-tui/Cargo.toml
```

## Configuration

agentry reads configuration from `~/.agents/agentry.toml`:

```toml
[project_dirs]
dirs = ["~/Development"]

[sync_defaults]
dry_run = true
conflict_strategy = "overwrite"  # overwrite | keep | merge | diff

[extra_skill_sources]
repos = []
```

## Development Roadmap

| Phase | Status | Scope |
|-------|--------|-------|
| Phase 1 | **Complete** | Core scaffolding, agent detection, TUI shell with intro animation and dashboard |
| Phase 2 | Planned | Prompt editor, unified prompt management, format conversion |
| Phase 3 | Planned | Sync engine -- planner, executor, dry-run, conflict resolution |
| Phase 4 | Planned | Skill hub -- browse, install, update, lockfile management |
| Phase 5 | Planned | OpenClaw workspace discovery and `.lobster` workflow management |
| Phase 6 | Planned | ACP protocol -- multi-agent orchestration and message routing |

## Built With

- [Rust](https://www.rust-lang.org/) -- Language
- [ratatui](https://github.com/ratatui/ratatui) -- Terminal UI framework
- [crossterm](https://github.com/crossterm-rs/crossterm) -- Cross-platform terminal manipulation
- [tokio](https://tokio.rs/) -- Async runtime
- [serde](https://serde.rs/) -- Serialization framework (JSON, YAML, TOML)
- [clap](https://github.com/clap-rs/clap) -- CLI argument parsing
- [git2](https://github.com/rust-lang/git2-rs) -- Git operations for skill hub

## License

This project is licensed under the [Apache License 2.0](LICENSE).
