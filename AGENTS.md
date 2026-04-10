# AGENTS.md — Project Instructions for agentry

## Overview

agentry is a Rust TUI application for managing prompts across 11 agent CLIs. It's organized as a Cargo workspace with 7 crates.

## Build & Test Commands

```bash
cargo build                  # Build all crates
cargo test                   # Run all 184 tests
cargo clippy -- -D warnings  # Lint
cargo fmt --check            # Check formatting
cargo fmt                    # Apply formatting
```

## Crate Architecture

| Crate | Purpose | Key Types |
|-------|---------|-----------|
| `agentry-core` | Models, format converters, prompt discovery | `UnifiedPrompt`, `AgentSpec`, `DetectedAgent`, `FormatConverter` |
| `agentry-agents` | Agent detection, specs, registry | `detect_agent()`, `detect_all_agents()`, `AgentRegistry` |
| `agentry-sync` | Sync planner and executor | `plan_sync()`, `execute_sync()`, `SyncPlan`, `SyncMapping` |
| `agentry-skills` | Skill hub, lockfile, installer | `SkillHub`, `install_skill()`, `update_skill()`, `remove_skill()` |
| `agentry-openclaw` | OpenClaw workspace discovery | `discover_workspaces()`, `read_doc()`, `write_doc()`, `validate_lobster()` |
| `agentry-acp` | ACP protocol, routing, orchestration | `AcpMessage`, `route_prompt()`, `decompose_task()`, `LobsterWorkflow` |
| `agentry-tui` | Terminal UI (binary: `agentry`) | `App`, `Editor`, 6-tab dashboard |

## Key Conventions

- **Error types**: Library crates use thiserror-based custom errors. TUI uses `anyhow::Result`.
- **HOME path**: Use `dirs::home_dir()` fallback chain, never `unwrap_or_default()` for paths.
- **Tests**: Use temp directories, never read real `~/.agents/` in tests.
- **Lockfile**: Must preserve v3 schema (`~/.agents/.skill-lock.json`) with camelCase JSON fields.
- **Symlinks**: Skill symlinks use relative paths (`../../.agents/skills/<name>`).
- **OpenClaw**: Workspace creation redirects to `openclaw` CLI, never creates dirs directly.

## CLI Subcommands

- `agentry` (no args) — Launch TUI
- `agentry detect` — List detected agents
- `agentry sync --prompt <name> | --all [--dry-run]` — Sync prompts
- `agentry skills list|install|update|remove` — Manage skills
- `agentry prompts list|new` — Manage prompts
- `agentry openclaw workspaces` — List OpenClaw workspaces