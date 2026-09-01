
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
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Build Status](https://img.shields.io/github/actions/workflow/status/AndlerRL/agentry/ci.yml?branch=master)](https://github.com/AndlerRL/agentry/actions)

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
- **Agent audit** -- Health scores (0-100) from 24 diagnostic checks, with `--fix` mode gated by a fail-closed shell allowlist and a self-evolving feedback loop
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
| Agents | `1` | Browse detected agents, versions, skills, and config paths |
| Prompts | `2` | Manage and edit prompts with vim-like editor |
| Skills | `3` | Browse and install skill hub entries |
| Sync | `4` | Plan and execute prompt sync |
| OpenClaw | `5` | Manage OpenClaw workspaces |
| Audit | `6` | Run agent health audits and review findings |

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

Audit tab keys:

| Key | Action |
|-----|--------|
| `r` | Re-run audit |
| `f` | Cycle severity filter (All / Critical / Warning / Info / Suggestion) |
| `Enter` | Open finding file / show remediation |

## CLI Usage

Run `agentry` without arguments to launch the TUI, or use subcommands for non-interactive mode:

```bash
# Detect installed agents
agentry detect

# Sync a specific prompt to all agents
agentry sync --prompt software-architect

# Sync all prompts (dry run first)
agentry sync --all --dry-run
agentry sync --all

# List installed and available skills
agentry skills list

# Install a skill
agentry skills install deploy-to-vercel

# Update all installed skills
agentry skills update

# Remove a skill
agentry skills remove deploy-to-vercel

# List discovered prompts
agentry prompts list

# Browse OpenClaw workspaces
agentry openclaw workspaces

# Run a full agent health audit
agentry audit

# Audit a single agent
agentry audit --agent claude-code

# Machine-readable report (CI/agentic gate)
agentry audit --json

# Apply auto-fixes
agentry audit --fix --yes
```

## Architecture

agentry is organized as a Cargo workspace with 8 crates:

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
│   ├── agentry-audit/           # Agent health audit engine and remediation
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
├── agentry-acp
└── agentry-audit

agentry-agents ──> agentry-core
agentry-sync   ──> agentry-core, agentry-agents
agentry-skills ──> agentry-core
agentry-openclaw ──> agentry-core, agentry-agents
agentry-acp   ──> agentry-core, agentry-agents, agentry-skills
agentry-audit ──> agentry-core, agentry-agents, agentry-sync, agentry-skills, agentry-openclaw, agentry-acp
```

### Core Models

- **`UnifiedPrompt`** -- Canonical representation of a prompt from any format, with frontmatter, body, XML tags, scope, and source format
- **`AgentSpec`** -- Static specification of a known agent (binary name, config dir, prompt filename, format)
- **`DetectedAgent`** -- Runtime detection result (installed status, version, skills, symlinks)
- **`SyncPlan`** / **`SyncMapping`** -- Planned sync actions with status tracking (UpToDate, Missing, Outdated, Conflict)
- **`SkillEntry`** -- A skill from the hub with source repo and install state
- **`AppConfig`** -- User configuration at `~/.agents/agentry.toml`

## Installation

### Pre-built Binaries (macOS & Linux)

Download the latest release from [GitHub Releases](https://github.com/AndlerRL/agentry/releases):

```bash
# macOS (Apple Silicon)
curl -L https://github.com/AndlerRL/agentry/releases/latest/download/agentry-macos-arm64.tar.gz | tar xz
chmod +x agentry-aarch64-apple-darwin
sudo mv agentry-aarch64-apple-darwin /usr/local/bin/agentry

# macOS (Intel)
curl -L https://github.com/AndlerRL/agentry/releases/latest/download/agentry-macos-x86_64.tar.gz | tar xz
chmod +x agentry-x86_64-apple-darwin
sudo mv agentry-x86_64-apple-darwin /usr/local/bin/agentry

# Linux (x86_64)
curl -L https://github.com/AndlerRL/agentry/releases/latest/download/agentry-linux-x86_64.tar.gz | tar xz
chmod +x agentry-x86_64-unknown-linux-gnu
sudo mv agentry-x86_64-unknown-linux-gnu /usr/local/bin/agentry
```

> **macOS Gatekeeper**: If you see "cannot be opened because it is from an unidentified developer", run:
> ```bash
> xattr -d com.apple.quarantine /usr/local/bin/agentry
> ```

### Install with Cargo

```bash
cargo install --git https://github.com/AndlerRL/agentry --bin agentry
```

### Build from Source

```bash
git clone https://github.com/AndlerRL/agentry.git
cd agentry
cargo build --release
# Binary at target/release/agentry
cp target/release/agentry /usr/local/bin/
```

> **Linux prerequisites**: The build links against OpenSSL via `git2`. Install the native dependencies first:
> ```bash
> # Debian/Ubuntu
> sudo apt install libssl-dev pkg-config
>
> # Fedora
> dnf install openssl-devel
> ```
>
> **Windows is not yet supported.**

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
| Phase 2 | **Complete** | Prompt editor, unified prompt management, format conversion |
| Phase 3 | **Complete** | Sync engine -- planner, executor, dry-run, conflict resolution, project-level sync |
| Phase 4 | **Complete** | Skill hub -- browse, install, update, lockfile management, symlink creation |
| Phase 5 | **Complete** | OpenClaw workspace discovery and `.lobster` workflow management |
| Phase 6 | **Complete** | ACP protocol -- multi-agent orchestration, capability routing, `.lobster` generation |
| Phase 7 | **Complete** | Polish, testing, error handling, docs, CI improvements, release |

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
