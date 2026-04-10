# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-09

### Added

- **Agent Detection** — Parallel detection of 11 agent CLIs (Claude Code, Continue, Gemini CLI, Codex, Amp, OpenCode, Firebender, OpenClaw, DeepAgents, Antigravity, Warp) with version, config dir, and skills directory discovery
- **5 Format Converters** — `PlainMarkdown`, `FrontmatterMd`, `Mdc`, `XmlTagMd`, `LobsterYaml` with round-trip support and cross-format conversion via `UnifiedPrompt` intermediate representation
- **Prompt Discovery** — Scans `~/.agents/prompts/`, `~/.continue/prompts/`, `~/.continue/rules/`, `~/.claude/CLAUDE.md`, and project directories
- **Sync Engine** — Plan, preview, and execute prompt distribution with copy, symlink, and skip actions; dry-run mode; project-level sync
- **Skill Hub** — Browse, install, update, and remove community skills from 13+ Git repositories; lockfile management (`~/.agents/.skill-lock.json` v3 schema); relative symlink creation
- **OpenClaw Workspace** — Discover workspaces from `~/.openclaw/openclaw.json`, read/write workspace documents (AGENTS.md, SOUL.md, TOOLS.md, etc.), validate `.lobster` YAML workflows
- **ACP Protocol** — Agent Communication Protocol with message types (PromptRequest/Response, SkillLookup/Result, TaskAssign/Result, WorkflowTrigger/Status), file-based queue and inbox system, capability-based routing, task decomposition, and `.lobster` workflow generation
- **TUI** — Ratatui terminal interface with intro animation, 6-tab dashboard (Dashboard, Agents, Prompts, Skills, Sync, OpenClaw), vim-like editor (normal/insert/visual/command modes), viewport scrolling
- **CLI** — Non-TUI mode via clap: `agentry detect`, `agentry sync`, `agentry skills`, `agentry prompts`, `agentry openclaw`
- **Custom Error Types** — `AgentryCoreError`, `AgentError`, `SyncError`, `SkillError`, `OpenClawError`, `AcpError` using thiserror
- **Tests** — 184 tests across 7 crates (unit + integration)

### Changed

- Fixed silent error discards in TUI prompt save operations — errors now displayed in status bar
- Fixed `HOME` environment variable resolution — uses `dirs::home_dir()` fallback with warning
- Fixed license field inconsistency (MIT → Apache-2.0) to match LICENSE file

[0.1.0]: https://github.com/AndlerRL/agentry/releases/tag/v0.1.0