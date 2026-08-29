# ADR-001: Agent Audit Capability

## Status

Proposed

## Context

Agentry manages prompts and configuration for 11 agent CLIs (Claude Code, Continue, Gemini CLI, Codex, Amp, OpenCode, Firebender, OpenClaw, DeepAgents, Antigravity, Warp). Today it can detect agents, sync prompts, manage skills, and browse OpenClaw workspaces — but it has no way to answer "is this machine's agent setup healthy?"

The product owner wants agentic capabilities to **analyze** every agent entry on the machine, **audit** them for health issues (broken symlinks, drift, missing config, outdated versions, orphaned files, auth state), and present findings in a neo-vim-like TUI where the human can **apply the fixes** the audit recommends.

### What exists today (reusable building blocks)

| Building block | Location | Reuse |
|---|---|---|
| `detect_agent()` / `detect_all_agents()` | `agentry-agents/src/detector.rs:9,74` | Per-agent detection (binary, version, config dir, prompt file, skills dir, install methods) |
| `check_sync_status()` | `agentry-sync/src/executor.rs:179` | Content-equality vs canonical → UpToDate/Missing/Outdated/Conflict |
| `validate_lobster()` | `agentry-openclaw/src/docs.rs:53` | `.lobster` YAML validation |
| `discover_workspaces()` | `agentry-openclaw/src/discovery.rs:120` | Workspace doc inventory incl. `has_agents_md`/`has_soul_md` (discovery.rs:64-65). Note: `scan_workspace` (discovery.rs:221) is private — the audit must use the public entry point |
| `compute_skill_hash()` | `agentry-skills/src/lockfile.rs:74` | SHA-1 drift detection |
| `SkillHub::load()` | `agentry-skills/src/hub.rs:76` | Orphaned skill detection (on-disk, not in lockfile) |
| `detect_symlink_pattern()` | `agentry-agents/src/detector.rs:305` | Classifies symlinks but **never verifies the target resolves**; it is private, samples only 5 entries (`.take(5)`), and never calls `read_link` on targets — the audit's `skills.symlink_broken` must do its own full scan of the skills dir |
| `build_capability_matrix()` | `agentry-acp/src/router.rs:12` | Per-agent capability/skill inventory |
| `list_*_versions()` | `agentry-agents/src/detector.rs:238-302` | Latest-version lookup via brew/npm/cargo (**pip missing — `list_pip_versions` is new work in P1**; deepagents-cli is pip-installed per spec.rs:151) |

### Known gaps (what an audit needs that does not exist)

1. Config file parsing (settings.json, config.toml, auth state) — no error variant or parser exists yet; `ConfigRead` is a new addition to `agentry-core/src/error.rs`
2. Health scoring model — no "healthy vs degraded" concept
3. Broken symlink verification (target resolution)
4. Cross-agent drift detection (compare agents' prompt files against each other)
5. Version comparison (semver, latest available)
6. Auth/credential state probing
7. Prompt file content analysis at detection time (size, format validity, max_size compliance)
8. Timestamps/mtime tracking (last synced/changed)
9. Orphaned prompt file detection (files not in canonical store)
10. Audit result type (findings with severity)
11. CLI `agentry audit` subcommand
12. TUI Audit tab / audit view

## Decision

### 1. Audit data model

#### 1.1 Crate placement: new crate `agentry-audit`

Create a new workspace crate `crates/agentry-audit` that depends on all domain crates. Do **not** extend `agentry-core`.

Rationale:

- `agentry-core` is the foundation (models + discovery) and must not depend on `agentry-agents`, `agentry-sync`, `agentry-skills`, `agentry-openclaw`, or `agentry-acp`. An audit engine reuses all of them, so it cannot live in core without inverting the dependency graph.
- `agentry-agents` is detection-only; `agentry-sync` is sync-only. Audit spans both plus skills/openclaw/acp — it is a distinct concern with its own lifecycle.
- The dependency direction stays clean: `core ← agents/sync/skills/openclaw/acp ← audit ← tui`.
- The audit engine is testable in isolation (temp-dir fixtures, no TUI, no CLI).
- The TUI and CLI both consume `agentry-audit` the same way they consume the other domain crates.

```
crates/agentry-audit/
├── Cargo.toml
└── src/
    ├── lib.rs          // pub use report, checks, engine, fix, history
    ├── report.rs       // AuditReport, AgentAudit, AuditFinding, Severity, FindingCategory
    ├── checks/
    │   ├── mod.rs      // CheckRegistry, Check trait, run_all()
    │   ├── install.rs  // INSTALL_* checks
    │   ├── version.rs  // VERSION_* checks
    │   ├── config.rs   // CONFIG_* checks
    │   ├── prompt.rs   // PROMPT_* checks
    │   ├── sync.rs     // SYNC_* checks
    │   ├── drift.rs    // DRIFT_* checks
    │   ├── skills.rs   // SKILLS_* checks
    │   ├── auth.rs     // AUTH_* checks
    │   ├── orphan.rs   // ORPHAN_* checks
    │   ├── openclaw.rs // OPENCLAW_* checks
    │   └── acp.rs      // ACP_* checks
    ├── engine.rs       // run_audit(), scoring, remediation plan
    ├── fix.rs          // FixAction, apply_fix(), FixOutcome
    └── history.rs      // finding history (JSONL), check registry
```

#### 1.2 Core types (`report.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Critical,
    Warning,
    Info,
    Suggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    Installation,
    Version,
    Config,
    PromptFile,
    SyncDrift,
    CrossAgentDrift,
    Skills,
    Auth,
    OrphanedFiles,
    OpenClaw,
    Acp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub check_id: String,
    pub severity: Severity,
    pub category: FindingCategory,
    pub agent_id: Option<String>,
    pub message: String,
    pub remediation: String,
    pub auto_fixable: bool,
    pub fix: Option<FixAction>,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixAction {
    ShellCommand { description: String, command: String },
    FileWrite { path: PathBuf, content: String },
    FileRemove { path: PathBuf },
    SymlinkRecreate { path: PathBuf, target: String },
    SyncPrompt { prompt_id: String, agent_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAudit {
    pub agent_id: String,
    pub health_score: u8,
    pub grade: HealthGrade,
    pub detected: DetectedAgent,
    pub findings: Vec<AuditFinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthGrade {
    Healthy,
    Degraded,
    Unhealthy,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub total_findings: usize,
    pub by_severity: BTreeMap<Severity, usize>,
    pub by_category: BTreeMap<FindingCategory, usize>,
    pub auto_fixable_count: usize,
    pub healthy_agents: usize,
    pub degraded_agents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub generated_at: DateTime<Utc>,
    pub machine_id: String,
    pub agents: Vec<AgentAudit>,
    pub global_findings: Vec<AuditFinding>,
    pub summary: AuditSummary,
}
```

#### 1.3 Health scoring

Score starts at 100 per agent and deducts per finding, capped at 0:

| Severity | Deduction |
|---|---|
| Critical | 25 |
| Warning | 10 |
| Info | 3 |
| Suggestion | 1 |

Grade bands: 90+ `Healthy`, 70+ `Degraded`, 40+ `Unhealthy`, <40 `Critical`. The weights live in the check registry (see §3.3) so they can be tuned without code changes.

### 2. Audit check catalog

Each check is a pure function `fn(&CheckContext) -> Vec<AuditFinding>` registered in the `CheckRegistry`. `CheckContext` carries the home dir, detected agents, discovered prompts, skill hub, OpenClaw state, and ACP matrix — built once per audit run, shared by all checks.

| ID | Category | Severity | Probes | Remediation | Auto-fixable |
|---|---|---|---|---|---|
| `install.binary_missing` | Installation | Warning | `cli_binary` on PATH | Install via preferred method (`InstallMethod::install_command`) | Yes (shell) |
| `install.config_dir_missing` | Installation | Info | `config_dir` exists | Run the CLI once to generate config | No |
| `install.method_conflict` | Installation | Info | >1 install method detected for same agent | Remove redundant method | No |
| `version.unparseable` | Version | Info | `--version` output has no semver-like token | Manual check | No |
| `version.outdated` | Version | Warning | Compare installed vs latest via `list_*_versions` (brew/npm/cargo/pip) | `InstallMethod::update_command` | Yes (shell) |
| `version.latest_unknown` | Version | Suggestion | No version source available for method | Manual check | No |
| `config.unparseable` | Config | Warning | Parse the agent's config files per the audit crate's hardcoded `CONFIG_FILES` table (e.g. `settings.json`, `config.toml`, `config.yaml` — keyed by agent id; `AgentSpec` has no config file list, so the table lives in `checks/config.rs`) | Fix the file | No |
| `config.stale` | Config | Info | Config file mtime older than 90 days (same `CONFIG_FILES` table) | Review config | No |
| `prompt.missing` | PromptFile | Warning | `prompt_filename` absent in config dir | `agentry sync --prompt <name>` | Yes (sync) |
| `prompt.empty` | PromptFile | Warning | Prompt file exists but is 0 bytes / whitespace-only | Sync or edit | Yes (sync) |
| `prompt.oversized` | PromptFile | Warning | File size > `spec.max_size` (Codex: 32 KiB) | Trim content | No |
| `prompt.frontmatter_invalid` | PromptFile | Warning | YAML frontmatter fails to parse (FrontmatterMd/Mdc formats) | Fix frontmatter | No |
| `prompt.format_mismatch` | PromptFile | Info | File parses as a different format than `spec.prompt_format` declares (e.g. spec says FrontmatterMd but file is plain markdown) — distinct from `sync.drift`, which compares content against the canonical store | Re-sync with conversion | Yes (sync) |
| `sync.drift` | SyncDrift | Warning | `check_sync_status` → Outdated/Conflict vs canonical store | `agentry sync --prompt <name>` | Yes (sync) |
| `sync.missing` | SyncDrift | Info | `check_sync_status` → Missing | `agentry sync --prompt <name>` | Yes (sync) |
| `drift.cross_agent` | CrossAgentDrift | Info | Same prompt name, different content across ≥2 agents (normalize each file via `convert_to` (format.rs:378) to a common format before comparing) | Pick canonical, re-sync all | No |
| `skills.symlink_broken` | Skills | Warning | Symlink in skills dir whose target does not resolve (`read_link` + `canonicalize`) — full scan of the skills dir; `detect_symlink_pattern` (detector.rs:305) is not reusable (private, 5-entry sample, no target resolution) | Recreate symlink to `~/.agents/skills/<name>` | Yes (symlink) |
| `skills.orphaned` | Skills | Info | Skill dir on disk not in lockfile (via `SkillHub::load` — note: it only flags dirs **with** a `SKILL.md`, hub.rs:125-126; dirs without one are invisible to the hub and need a separate scan) | `agentry skills remove` or re-add to lockfile | No |
| `skills.hash_mismatch` | Skills | Warning | `compute_skill_hash` differs from lockfile `skillFolderHash` | `agentry skills update` | Yes (shell) |
| `auth.not_logged_in` | Auth | Info | Probeable auth state (e.g. `claude` config has no oauth token, `codex login status` fails) | Run the CLI's login command | No |
| `files.orphaned_prompt` | OrphanedFiles | Info | Prompt file in agent config dir not present in canonical store | Import to `~/.agents/prompts/` or delete | No |
| `openclaw.lobster_invalid` | OpenClaw | Warning | `validate_lobster` returns invalid for a `.lobster` workflow | Fix YAML per validation warnings | No |
| `openclaw.workspace_incomplete` | OpenClaw | Info | Workspace missing core docs (AGENTS.md / SOUL.md) | Create missing docs | No |
| `acp.capability_mismatch` | Acp | Info | Installed agent whose matrix entry contains only `general` (spec-coverage signal: `build_capability_matrix` router.rs:12 always includes every installed agent and never yields an empty list — unknown specs fall through to `_ => caps.push("general")` at router.rs:93-95) | Add explicit capability arms to `build_capability_matrix` | No |

Checks are **skipped** (not failed) when their precondition is absent: e.g. `skills.*` only runs for agents with a skills dir, `openclaw.*` only when OpenClaw is installed, `auth.*` only for agents with a defined probe.

### 3. Agentic / self-evolving aspect

#### 3.1 `--fix` mode

`agentry audit --fix` applies every finding where `auto_fixable == true`, in severity order, with per-finding confirmation (`y/n/skip/all`). Each `FixAction` is executed through the same suspend-TUI/run/restore pattern the TUI already uses for agent install/update (`app.rs:1156`). After each fix, the affected check re-runs and the finding is marked `fixed` or `failed` in the report. `--fix --yes` skips confirmation for scripted use.

#### 3.2 Remediation plan

`engine.rs` groups findings into an ordered plan:

1. **Critical fixes first** (broken symlinks, oversized prompts, unparseable configs)
2. **Auto-fixable actions** (shell commands, syncs, symlink recreates)
3. **Manual actions** (auth logins, config edits) — rendered as a checklist with exact commands

The plan is a `Vec<RemediationStep>` where each step references one or more `AuditFinding` ids, so the TUI can show "3 findings fixed by this step".

#### 3.3 Feedback loop: finding history + check registry

Pragmatic, data-structure-first design. Two files under `~/.agents/audit/`:

- `history.jsonl` — append-only, one JSON object per finding per run: `{ run_id, generated_at, machine_id, check_id, agent_id, severity, category, fixed: bool }`. This is the raw material for learning.
- `checks.json` — the check registry with mutable metadata: `{ check_id, enabled, severity_weight, threshold }`. The registry is what makes the audit self-tuning without code changes.

The "learning" rules are simple and deterministic (no ML):

- A check that fires on the same agent across ≥3 consecutive runs is flagged `recurring` in the report and its findings are promoted one severity level (Warning → Critical).
- A check that has not fired in the last 10 runs on any agent is demoted to `Suggestion`.
- A check that fires on ≥80% of machines (across `machine_id`s in history) is a candidate for a new default check — surfaced as a `Suggestion`-level finding `audit.new_check_candidate` with the check_id, so the human (or a future agentic pass) can promote it into the catalog.

This gives the "self-evolving" property through data, not through an LLM inside the audit engine. The engine stays deterministic and testable; the history file is the interface any future agentic layer can consume.

### 4. TUI integration

#### 4.1 New Audit tab (6th tab)

`Tab::Audit` added to `ui/mod.rs` (index 5). `next_tab`/`prev_tab` modulo changes from 5 to 6; `'6'` key added alongside `'1'`–`'5'`. The audit runs on-demand (key `r` or on first tab entry) — not at startup, to keep intro latency unchanged.

```
┌─ Agents ─ Prompts ─ Skills ─ Sync ─ OpenClaw ─ Audit ──────────────────────┐
│ AUDIT SUMMARY                        HEALTH SCORES                         │
│ 24 findings · 3 critical · 8 warning │ claude-code  ████████░░ 82 Degraded │
│ 9 info · 4 suggestion · 6 fixable    │ codex        ██████████ 95 Healthy  │
│                                      │ gemini-cli   ████░░░░░░ 40 Unhealthy│
├─ FINDINGS (by severity) ──────────────────────────────────────────────────┤
│ ▼ CRITICAL (3)                                                             │
│   [codex] prompt.oversized      AGENTS.md 41.2 KiB > 32 KiB limit           │
│   [claude-code] skills.symlink_broken  git → ../../.agents/skills/git ✗    │
│ ▼ WARNING (8)                                                              │
│   [gemini-cli] version.outdated  v0.9.1 → v1.2.0 available                 │
│   [claude-code] sync.drift       CLAUDE.md differs from canonical           │
│   ...                                                                       │
├─ DETAIL ───────────────────────────────────────────────────────────────────┤
│ skills.symlink_broken · Critical · Skills · auto-fixable                   │
│ Symlink ~/.claude/skills/git → ../../.agents/skills/git does not resolve.  │
│ Remediation: recreate symlink to ~/.agents/skills/git                       │
│ [a] Apply fix   [A] Apply all fixable   [r] Re-run audit   [f] Filter       │
└────────────────────────────────────────────────────────────────────────────┘
```

Navigation follows the existing j/k/Enter pattern: `j`/`k` move through the grouped findings list (severity headers are skipped by a `selected_finding()` helper, same pattern as `selected_prompt_index()` at `app.rs:635`), `Enter` opens the detail panel for the selected finding.

New keybindings (Audit tab only):

| Key | Action | Phase |
|---|---|---|
| `a` | Apply fix for selected finding (confirms via existing y/n pattern) | 3 |
| `A` | Apply all auto-fixable findings (one confirmation, then sequential) | 3 |
| `r` | Re-run audit | 2 |
| `f` | Cycle severity filter (All / Critical / Warning / Info / Suggestion) | 2 |
| `Enter` | Toggle detail panel for selected finding | 2 |

> Amendment (P2 implementation): `Enter` opens the finding's file in `$EDITOR` when its fix carries a path (`SymlinkRecreate`), otherwise shows the remediation text; the detail panel is persistent rather than toggled, matching all other tabs.

Phase 2 binds only `r`/`f`; `a`/`A` are wired in Phase 3 when fix mode lands, so no key is bound to a non-existent action in P2. The mockup footer above shows the end state (P3).

#### 4.2 Agents tab health integration

The Agents tab detail panel (`draw_agent_detail_enhanced`) gains a health line: `Health: 82/100 (Degraded)` plus the count of critical/warning findings, sourced from the cached `AuditReport` in `App`. If no audit has run this session, the line reads `Health: not audited (press r in Audit tab)`.

#### 4.3 Vim-like editor (separate phase)

The in-TUI editor is explicitly **out of scope for audit v1** — editing continues to shell out to `$EDITOR` (`app.rs:857`). The audit view is designed to be editor-friendly: findings are a flat, ordered list with stable ids, the detail panel shows the exact file path for every file-backed finding, and `e` on a finding opens that path in `$EDITOR`. When the editor lands, the audit view's list/detail split maps directly onto a buffer/split layout.

### 5. CLI surface

```
agentry audit                     # full report, human-readable
agentry audit --agent <id>        # single agent (e.g. claude-code)
agentry audit --fix               # apply auto-fixable findings (interactive)
agentry audit --fix --yes         # apply without confirmation
agentry audit --json              # machine-readable AuditReport (agentic use)
agentry audit --severity warning  # filter output by minimum severity
```

`--json` emits the full `AuditReport` (serde) — stable schema, versioned in the report as `schema_version: 1`. Exit code: 0 = no critical findings, 1 = critical findings present, 2 = audit error. This makes `agentry audit --json` usable as a CI/agentic gate. Implementation note: `main` returns `Result<()>` (main.rs:101), so distinct exit codes require explicit `std::process::exit()` inside `cmd_audit()` rather than returning an error value.

### 6. Implementation phases

#### Phase 1 — Core audit engine + checks + CLI

**Scope:** new `agentry-audit` crate, all checks in §2, scoring, `agentry audit` CLI (no `--fix`).

**Files:**

| File | Change |
|---|---|
| `crates/agentry-audit/Cargo.toml` | New (deps: core, agents, sync, skills, openclaw, acp, serde, serde_yaml, toml, chrono, semver, tokio) |
| `crates/agentry-audit/src/{lib,report,engine}.rs` | New — types, scoring, orchestration |
| `crates/agentry-audit/src/checks/*.rs` | New — 24 checks |
| `crates/agentry-tui/src/main.rs` | Add `Audit` subcommand + `cmd_audit()` |
| `Cargo.toml` | Add `agentry-audit` to workspace members |
| `crates/agentry-agents/src/detector.rs` | Add `verify_symlink_target()` (target resolution) and `list_pip_versions()` (deepagents-cli) — small, reusable by audit |
| `crates/agentry-core/src/error.rs` | Add `ConfigRead` variant (new — error.rs:5-28 has no such variant today) — used by config checks |

**Dependencies:** no new external crates — `serde_yaml`, `toml`, `tokio` are already workspace deps (root `Cargo.toml:25,26,34`), pulled in as `workspace = true`. **Effort:** 2–3 days. **Exit criteria:** `agentry audit --json` produces a valid report on a real machine; all checks unit-tested with temp-dir fixtures.

#### Phase 2 — TUI Audit tab

**Scope:** 6th tab, summary header, grouped findings list, detail panel, Agents-tab health line.

**Files:**

| File | Change |
|---|---|
| `crates/agentry-tui/src/ui/mod.rs` | `Tab::Audit`, `ALL` → 6 entries, index/title |
| `crates/agentry-tui/src/ui/dashboard.rs` | `draw_audit_*` renderers, tab bar |
| `crates/agentry-tui/src/app.rs` | `audit_report: Option<AuditReport>` field, `run_audit()`, `selected_finding()`, key handlers `r`/`f` only (`a`/`A` land in Phase 3 with fix mode — no dead keys in P2), `next_tab`/`prev_tab` modulo 6, `'6'` key, health line in Agents detail, new Audit-tab arm in `list_max()` (app.rs:537, currently `_ => 0` at app.rs:630) |

**Dependencies:** Phase 1. **Effort:** 1–2 days. **Exit criteria:** browser-free manual pass — navigate, filter, detail panel, health line renders; `cargo test` + `cargo clippy -D warnings` clean.

#### Phase 3 — Fix mode + agentic feedback

**Scope:** `--fix`/`--fix --yes`, remediation plan, history + check registry, severity promotion rules.

**Files:**

| File | Change |
|---|---|
| `crates/agentry-audit/src/fix.rs` | New — `apply_fix()`, `FixOutcome`, re-check after fix |
| `crates/agentry-audit/src/history.rs` | New — JSONL history, `checks.json` registry, promotion rules |
| `crates/agentry-audit/src/engine.rs` | Remediation plan generation, recurring-finding promotion |
| `crates/agentry-tui/src/main.rs` | `--fix`, `--fix --yes` flags |
| `crates/agentry-tui/src/app.rs` | `a`/`A` wired to `apply_fix()` with y/n confirm |

**Dependencies:** Phases 1–2. **Effort:** 2–3 days. **Exit criteria:** `agentry audit --fix` repairs a broken symlink and re-checks green; history file grows one line per run; promotion rule unit-tested.

### 7. Risks & decisions

#### Risks

| Risk | Mitigation |
|---|---|
| **Config format drift** — agents change settings.json/config.toml schemas; parse failures become false positives | Config checks are `Warning` not `Critical`; parse errors include the raw error as `evidence`; checks are disabled via `checks.json` registry without code changes |
| **Version-check latency** — brew/npm/cargo/pip lookups are slow (seconds each) | Version checks run in parallel (tokio, same pattern as `detect_all_agents`), results cached in the report; `--severity` filter can skip them |
| **Auth probing is fragile and sensitive** — reading credential files is invasive | Only probe auth state where a safe, documented signal exists (e.g. `codex login status` exit code, presence of oauth token file); never read token contents into the report; `auth.*` checks are `Info` and opt-out via registry |
| **False positives erode trust** | Every finding carries `evidence` (path, size, diff excerpt); severity weights are registry-tunable; recurring-finding promotion is the only automatic escalation and it is reversible |
| **Symlink verification on non-Unix** | `SymlinkRecreate` fix is `#[cfg(unix)]`-gated; on Windows the check reports but the fix is not offered |
| **JSON schema drift breaks agentic consumers** | `schema_version` field in `AuditReport`; additive changes only within a major version |

#### Decisions to confirm with Andler

1. **Health score weights** — 25/10/3/1 deductions and 90/70/40 grade bands are my proposal. Confirm or provide target bands.
2. **Auth probing scope** — which agents get `auth.*` checks, and is reading token-file *presence* (never contents) acceptable?
3. **Audit trigger** — on-demand only (my recommendation, keeps startup fast) vs. background run at TUI startup.
4. **`--fix` blast radius** — auto-fixable set is currently: shell install/update commands, sync re-runs, symlink recreates, file removals for orphaned prompts. Confirm file removal is in scope for `--fix --yes`.
5. **Machine identity** — `machine_id` for history: hostname (readable, but PII-ish) vs. hash of hostname (anonymous, stable). I recommend hash.
6. **Exit-code contract** — 0/1/2 semantics for `agentry audit` as a CI gate. Confirm this is a desired contract.

## Consequences

**Easier:**

- A single `agentry audit` answers "is this machine's agent setup healthy?" and `--fix` repairs what is safely repairable
- The check catalog is data-driven (registry + history), so the audit improves without code changes
- `--json` output makes the audit consumable by other agents/tools (the "agentic" story)
- Reuses proven building blocks (sync status, lobster validation, skill hashing) instead of re-implementing them
- New crate keeps the dependency graph acyclic and the engine unit-testable

**More difficult:**

- One more crate to maintain and one more tab to keep in sync with the tab bar
- Config parsing adds a maintenance surface that tracks upstream agent config formats
- The history/registry files add state under `~/.agents/` that must be migrated if the schema changes
- `--fix` mode raises the stakes: a bad auto-fix is worse than a bad report, so fix actions need conservative review before shipping
