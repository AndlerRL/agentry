# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-09-02

### Added

- **Agentry Harness** (ADR-002) — New `agentry-harness` crate unifying systematic and agentic actions: HarnessAction trait, GateTicket consent enforcement (crate-internal minting, action-bound, fail-closed), consent ledger at `~/.agents/agentry/consent.jsonl`, two-phase prepare/invoke. Every TUI/CLI mutation flows through the gated registry
- **Agent Auditor** (ADR-002) — New `agentry-auditor` crate: `agentry auditor review` invokes a headless agent CLI (claude → codex → gemini → ollama, config-driven host registry incl. zai seeded / fal excluded) to analyze the audit report; prompt loads skill-creator + context-engineering skills; findings quarantined as Suggestion-only with gate-validated, consent-required auto-apply
- **`agentry auditor setup`** — Idempotent config + canonical auditor prompt + skill lockfile adoption
- **Audit-fix keys in the TUI** — `a` applies the selected finding's fix, `A` applies all fixable (ADR-001 §4.1 finally complete); `l`/`L` invoke the auditor on the selected finding (explicit keypress, egress-disclosing consent)
- **Persistent keymap bar** — Bottom 2 rows, always visible, registry-driven (dispatch + bar + help are one source of truth), swaps to y/n/Esc during confirm modals
- **Sync tab actionability** — Plan auto-loads on entry; `s` executes selected mapping, `S` executes all (y/n confirm)
- **Audit tab auto-run** on first entry
- **Version picker** — Enter installs the selected version (was a dead end)
- **Host invocation hardening** — argv-split (never `sh -c`), shell-safe interpolation validation, terminal suspend/restore, fail-closed on unparseable output

### Changed

- OpenClaw merged into the Agents tab detail (was a separate tab — 6 tabs → 5: Agents, Prompts, Skills, Sync, Audit)
- Audit report `schema_version` 1 → 2 (Audited finding category + `suggested_fix` field)
- Sync planner excludes `agentry-role`-marked prompts (fail-closed)
- History promotion/demotion skips Audited findings (Suggestion cap cannot be bypassed by recurrence)
- Number keys reset selection; `←` guarded; `i` de-aliased; `w` workflow key removed (returns in P4 as harness palette)

### Fixed

- tokio `Handle::block_on` panics in async TUI context (all invocation sites now use `block_in_place` + current-thread runtime)
- Audited suggested fixes could never execute (gate was airtight, vault door never opened)
- Version strings from registries now validated against a safe charset before shell interpolation

## [0.2.0] - 2026-08-29

### Added

- **Agent Audit capability** (ADR-001) — New `agentry-audit` crate with 24 diagnostic checks across 11 categories (installation, version, config, prompt health, sync drift, cross-agent drift, skills, auth, orphaned files, OpenClaw, ACP), health scoring 0-100 with grades (Healthy/Degraded/Unhealthy/Critical)
- **`agentry audit` CLI** — `--agent` for a single agent, `--json` for machine-readable output (`schema_version` 1), `--severity` filter, and exit codes 0/1/2 so the audit can act as a CI gate
- **`agentry audit --fix [--yes]`** — Applies auto-fixable findings in severity order with a fail-closed shell allowlist (npm-scoped packages and pip pins supported, metacharacter injection refused), then re-audits after fixes
- **Self-evolving feedback loop** — Finding history at `~/.agents/audit/history.jsonl` plus a `checks.json` registry; recurring findings are promoted, dormant checks demoted, and new-check candidates surfaced (deterministic rules, no LLM)
- **TUI Audit tab** (press `6`) — On-demand audit (`r`), severity filter (`f`), grouped findings with detail panel, per-agent health bars, and a health line in the Agents tab
- **Prompt dedup** — Discovery now dedups by (name, scope), preferring the canonical `~/.agents/prompts/` store (fixes duplicate GEMINI entries and the edit-hazard on synced copies)
- New primitives: `verify_symlink_target`, `list_pip_versions`

### Fixed

- List selection offset bugs in Prompts/Skills/Sync tabs (header rows no longer counted)
- Intro screen layout with full ASCII art (23-line logo, no clipping at minimum terminal height)
- Symlink verification resolves targets relative to the symlink's parent (was CWD — caused false broken-symlink reports)
- Nested skills symlink remediation computes depth-correct relative targets
- Empty frontmatter (`---\n---`) no longer flagged invalid
- `cargo fmt` drift across workspace

### Changed

- `machine_id` hashed with SHA-1 (stable across Rust versions, for audit history continuity)
- `DEFAULT_PROJECT_DIR` constant shared across sync/prompts/audit commands

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

[0.2.1]: https://github.com/AndlerRL/agentry/releases/tag/v0.2.1
[0.2.0]: https://github.com/AndlerRL/agentry/releases/tag/v0.2.0
[0.1.0]: https://github.com/AndlerRL/agentry/releases/tag/v0.1.0