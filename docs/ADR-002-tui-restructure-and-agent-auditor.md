# ADR-002: TUI Restructure, the Agentry Harness, and the Agent Auditor

## Status

Proposed — Revision 3. Revision 1's eight open decisions were answered by the product owner; Revision 2 incorporated those answers and the expanded scope they introduced: the **Agentry Harness** (agentry's own execution layer), an **extensible host registry** (ollama, fal, zai), a **persistent keymap bar**, **auditor auto-apply under consent**, and an **onboarding wizard with llmfit integration**. Revision 3 folds Nikaya's Stage 2 review: a **blocker** closing the fix-gate path-bounds gap for LLM-authored `FileWrite`/`FileRemove` (§5.5, §5.7), shell-discipline and ACP pre-consent trust-model decisions (§5.4, §5.6), and six clarifications (GateTicket identity, P1 keymap scope, config ownership, MSRV, table renumbering, `f` key preservation).

## Context

ADR-001 shipped the audit engine, the Audit tab, and the CLI. Real usage exposed two classes of problems the product owner flagged directly:

1. **"OpenClaw tab shouldn't be a tab, it should be part of the agent supported clients."** OpenClaw is one of the 11 agent CLIs (spec id `"openclaw"`, spec.rs:125, already in the Agents list, already has sync mappings at planner.rs:91). Giving it its own tab is a category error — it is a *client*, not a *category*.
2. **"I wasn't sure what to do with the Sync AND Audit, they do not implement the sync nor the audit fix."** Both tabs show state but hide their actions. The Sync tab advertises `s: Execute sync` (dashboard.rs:1108) but `s` only reloads the plan (app.rs:1109-1158); `execute_sync` (executor.rs:15) is reachable only from the CLI (main.rs:248). The Audit tab displays `Auto-fix: yes` (dashboard.rs:1614) but the fix engine (`fix.rs:32 apply_fix`, with the fail-closed shell allowlist) is CLI-only; the `a`/`A` keys ADR-001 §4.1 planned were never bound — `a` is bound to `on_add_agent`, which only works on the OpenClaw tab and only prints guidance text (app.rs:508, 1536-1545).
3. **The audit has no agent behind it.** The owner wants an *agent auditor* — a first-party agent inside agentry that verifies agent entries, whose system prompt loads the `skill-creator` and `context-engineering-collection` skills. Today agentry hosts agents only as config descriptors; no LLM runtime exists anywhere in the workspace.
4. **Navigation keys need review.**

Revision 2 adds four more owner directives:

5. **"You are missing ollama, fal, zai, etc..."** — the host-CLI landscape for LLM delegation is wider than claude/codex/gemini: it includes local-model runtimes (ollama) and newer agent/API CLIs (fal, zai). A hardcoded match arm per CLI does not scale; the registry must be extensible by config.
6. **"only explicit keypress (map keys must show always at bottom of TUI and always up to date to current tab)"** — key discoverability is a hard requirement, not a hint: a persistent bottom bar showing the focused tab's active keys, always visible, always current.
7. **"Wire it but not with lobster but with a custom harness for this; overall agentry agent actions must be throughout a custom harness focused for agentry that combines both systematic/programmatically functionality (what we have created so far) with agentic harness."** — agentry's own actions need a unifying execution layer: the **Agentry Harness**. `.lobster` remains an OpenClaw interop format, not agentry's internal execution model.
8. **Onboarding + llmfit** — "make it part of it however, make it simple to start with: the workflow should setup an agentry harness across all agents and ask to install any missing agent CLI that can be installed in the machine AND suggest llms that may be able to run by wrapping llmfit core functions into the agentry (agentry install the dependency) and uses to analyze the machine capabilities and sets up a basic workflow on the user selections when onboarding or updating the setup."

### Verified defects (explored, file:line)

| # | Defect | Evidence |
| --- | --- | --- |
| 1 | Sync `s` loads plan, never executes; detail panel lies | app.rs:1109-1158, dashboard.rs:1108; `execute_sync` unused in TUI |
| 2 | `Enter` unbound on Sync tab | `on_enter` arms cover tabs 0,1,2,4,5 only (app.rs:1007-1087) |
| 3 | Audit `a`/`A` fix keys never bound (ADR-001 §4.1 promised them) | app.rs:508 binds `a` to `on_add_agent`; no `A` handler exists |
| 4 | TUI audit weaker than CLI: no version lookups, `apply_feedback` never called in TUI | main.rs:621,699 are CLI-only call sites |
| 5 | `w` generates `.lobster` files referencing `agentry task assign` — a subcommand that does not exist | app.rs:1547-1589; orchestrator.rs:83,96,210,223,258; `Commands` enum has no Task (main.rs:19-68) |
| 6 | Version selection dead end: `v` loads versions, hint says "Enter to confirm" (app.rs:1265), but `on_enter` never reads `version_list` | app.rs:1217-1277, 1007-1087 |
| 7 | OpenClaw detail hints lie: `g: Open in shell` (g is Skills-only, app.rs:1422-1438), `a: Add sub-agent` (prints text only) | dashboard.rs:1376, 1170 |
| 8 | README documents a vim-like editor (`:w`/`:q`) that does not exist | README.md:96,113,116 |
| 9 | Number keys `1`-`6` jump tabs without resetting selection (Tab does reset) | app.rs:482-487 vs 519-533 |
| 10 | `r` dual-mapped (Remove on Agents/Skills, Re-run audit on tab 5) with no per-tab hint | app.rs:499-505 |
| 11 | `←` bound globally to `method_prev` with no tab guard (`→`/`method_next` guards on tab 0) | app.rs:509 vs 1206-1215 |
| 12 | `i` silently aliases to new-prompt outside Skills tab | app.rs:1177-1198 (`else { self.on_new() }`) |
| 13 | `version_input` field is dead | app.rs:77,167 — written once, never read |

### Reusable substrate

| Building block | Location | Reuse |
| --- | --- | --- |
| `AcpMessage` protocol (PromptRequest/Response, TaskAssign/TaskResult, SkillLookup) | agentry-acp/src/protocol.rs:9-26 | P4 worker transport into the harness |
| File queue under `~/.agents/acp/` (`enqueue_message`, `read_inbox`, `dequeue_message`) | protocol.rs:144-269 | P4 worker; `init_acp_dirs` (protocol.rs:157) is **never called in production** — only tests |
| Capability matrix | router.rs:12 | Host-CLI inventory for auditor setup |
| `decompose_task` audit branch + `decomposition_to_assignments` | orchestrator.rs:58, 300 | P4 task routing |
| `SkillHub::load()` orphan detection; `install_skill`/`update_skill`/`remove_skill` | hub.rs:76; install.rs:21,176,309 | Skill inventory + skill-request loop |
| `skill-creator` **installed** in lockfile; `context-engineering-collection` **on disk but orphaned** (not in lockfile — the audit's own `skills.orphaned` check flags it) | `~/.agents/.skill-lock.json`, `~/.agents/skills/` | P3 prerequisite: adopt the collection into the lockfile |
| `AuditReport` is serde with `schema_version` | report.rs:98-107 | Natural context payload for the LLM |
| Suspend-TUI/run/restore pattern | app.rs:1295-1307 (`execute_agent_action`), app.rs:952 (`edit_file_externally`) | Headless delegation without losing terminal state; reused by the harness invocation layer |
| `from_agent: "agentry"` precedent in protocol tests | agentry-acp tests | First-party agent identity convention |
| `UnifiedPrompt` with arbitrary frontmatter map (`frontmatter: BTreeMap<String, serde_yaml::Value>`) | models.rs:292-306 | Auditor prompt as a canonical prompt — no format changes needed |
| `InstallMethod::install_command`/`update_command` | agentry-agents spec/detector | Onboarding install offers for missing agent CLIs |
| llmfit-core (crates.io `llmfit-core` 1.1.12, MIT, edition 2024; hardware detection + model fit + provider integration; deps: sysinfo, ureq, objc2-metal on macOS) | github.com/AlexsJones/llmfit (`llmfit-core` crate) | Onboarding machine-capability analysis and local-model suggestions |

## Decision

### 1. The Agentry Harness

The architectural headline of this revision. Every action agentry performs on the user's behalf — deterministic engine calls *and* LLM invocations — goes through one action model: the **Agentry Harness**. This replaces the revision-1 plan of routing agentry's own workflows through lobster/`.lobster` generation.

**Owner directive (verbatim intent):** agentry's agent actions must go through a custom harness focused on agentry that combines the systematic/programmatic functionality built so far with an agentic harness.

#### 1.1 Action model

```rust
pub enum ActionKind {
    Systematic,
    Agentic,
}

pub enum Confirmation {
    None,
    Single,
    PerItem,
}

pub trait HarnessAction {
    fn id(&self) -> &'static str;
    fn kind(&self) -> ActionKind;
    fn describe(&self, input: &ActionInput) -> String;
    fn confirmation(&self, input: &ActionInput) -> Confirmation;
    async fn execute(
        &self,
        ctx: &HarnessContext,
        input: ActionInput,
        ticket: &GateTicket,
    ) -> Result<ActionOutput, HarnessError>;
}
```

- **`ActionKind::Systematic`** — a deterministic function into an existing engine: sync executor, audit engine, fix engine, skill hub, discovery. No LLM, no network egress, fully unit-testable with temp-dir fixtures. These are the engines agentry already has; the harness wraps them, it does not reimplement them.
- **`ActionKind::Agentic`** — an LLM-driven step via headless CLI delegation through the host registry (§4). The harness owns context packaging, invocation, timeout, and response normalization for every agentic action, so no agentic action re-implements transport or consent.
- **`HarnessContext`** — home dirs, detected agents, skill hub, loaded config; built once per session and shared across actions (the same pattern as ADR-001's `CheckContext`).
- **`ActionInput`/`ActionOutput`** — serde-tagged enums, so every action is invocable identically from the TUI, the CLI, and (P4) the ACP worker:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ActionInput {
    SyncExecute { prompt_id: Option<String> },
    AuditRun { agent_id: Option<String> },
    FixApply { check_id: String },
    FixApplyAll,
    AuditorReview { focus_check_id: Option<String> },
    SkillInstall { name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ActionOutput {
    SyncExecuted { applied: usize, skipped: usize },
    AuditCompleted(AuditReport),
    FixApplied { outcome: FixOutcome },
    AuditorMerged { added: usize },
    SkillInstalled { name: String },
}
```

#### 1.2 Confirmation gates live in the harness

The harness owns the consent policy. One honesty note first: `HarnessAction::execute` is a public trait method, and the trait **cannot be sealed** — `agentry-auditor` implements it from outside the harness crate — so "callers cannot bypass gates" is not enforceable by visibility alone. Enforcement comes from the **gate-ticket pattern**: `execute` takes a `&GateTicket` whose constructor is private to the harness crate, and `HarnessRegistry` is the only issuer, minting a ticket only after its confirmation flow completes and recording the consent alongside it. Any crate can *call* `execute`, but no crate outside the harness can *construct* a ticket, so an ungated invocation is unrepresentable in external code. The registry's invoke path (§1.3) is the only sanctioned entry point — TUI, CLI, and ACP worker all go through it:

| Action kind | Consent rule |
| --- | --- |
| Read-only systematic (`audit.run`, plan loads) | `Confirmation::None` |
| Mutating systematic (`sync.execute`, `fix.apply`, `skills.install`) | `Single` or `PerItem` y/n; `--yes` exists only where the CLI already exposes it (`audit --fix --yes`) |
| Agentic (any LLM invocation) | `Single`, always — never auto-run, never on a timer; egress to the LLM provider is user-initiated every time |

The TUI renders the confirmation from `describe()`; the CLI uses the same text. One consent implementation, three entry points.

**Ticket identity (nit, Stage 2):** a `GateTicket` carries the `action_id` it was minted for plus the id of its recorded consent entry; the registry's invoke path asserts `ticket.action_id == action.id()` before calling `execute` and refuses a mismatched ticket (fail-closed). This closes the gap where a ticket minted for one action could be passed to another — e.g. a `Confirmation::None` ticket for `audit.run` being reused to authorize `fix.apply`.

#### 1.3 Action registry

`HarnessRegistry` maps action ids to implementations and is the **sole issuer of `GateTicket`s** (§1.2) — its invoke path runs the confirmation flow, mints the ticket, and records consent. The TUI keymap (§3), the CLI subcommands, and the P4 ACP worker all resolve actions through the registry — the same single-source-of-truth discipline as the keymap bar. `agentry harness actions` lists the registry (discoverability for humans and for agentic consumers of `--json` surfaces).

**Trust boundary (Stage 2):** `invoke_confirmed` records the consent entry unconditionally — it assumes the **caller** has already obtained user consent (TUI y/n prompt, CLI `--yes` flag) and treats the ledger as an **audit trail, not an enforcement mechanism**. The registry never asks the user anything; it mints a ticket and executes. Consequence for P3: any agentic action (`auditor.review` and successors) must add a **pre-existing-consent check** before execution — the worker verifies a consent record for the specific action exists *before* invoking, rather than trusting the caller's assertion — because an agentic caller cannot be assumed to have prompted the user.

#### 1.4 Existing features become harness actions

| Action id | Kind | Wraps |
| --- | --- | --- |
| `sync.execute` | Systematic | `execute_sync` (executor.rs:15) — selected mapping or all non-`Skip` |
| `audit.run` | Systematic | `run_audit` (ADR-001 engine) |
| `fix.apply` / `fix.apply_all` | Systematic | `apply_fix` (fix.rs:32) with the fail-closed allowlist + path-bounds gate (§5.5) |
| `skills.install` / `skills.update` / `skills.remove` | Systematic | agentry-skills install/update/remove |
| `auditor.review` | **Agentic** | The agent auditor (§5) — the first agentic action |
| `harness.sequence` | Systematic | P4: run an ordered list of action inputs (the `w` palette's sequences) |

#### 1.5 Relationship to lobster and ACP

- `.lobster` stays an **OpenClaw interop format**: `validate_lobster` and the OpenClaw checks are untouched. Agentry's internal execution model is the harness, not lobster workflows. The revision-1 plan to have `w` generate `.lobster` files is dropped.
- The ACP file queue (`~/.agents/acp/`) becomes a **transport into the harness**: P4's `agentry auditor serve` consumes `TaskAssign` messages and resolves each to a harness action via the registry, under the same confirmation rules (queued tasks carry pre-consent recorded at enqueue time; the worker refuses actions whose consent record is absent). **v1 scope: Systematic actions only** (§5.6 trust model) — agentic actions are never resolvable from the queue until the P4+ trust assumption is formally documented.
- `agentry task assign` (P4) still lands — it is the enqueue side of that transport, and it makes externally-driven automation possible without agentry adopting lobster as its execution model.

### 2. TUI restructure: five tabs

`Tab::OpenClaw` is deleted. Tabs become **Agents, Prompts, Skills, Sync, Audit** (Audit moves to index 4). All `tab_index` literals shift down by one for tabs ≥ 4; `next_tab`/`prev_tab` modulo 5; number keys `1`-`5`.

```
┌─ Agents ─ Prompts ─ Skills ─ Sync ─ Audit ─────────────────────────────────┐
│ AGENTS (6/11)                    │ AGENT DETAIL ─ OpenClaw                 │
│ [ON] claude-code   npm  v1.0.2   │ OpenClaw · v1.2.3 · npm                 │
│ [ON] codex         brew  v0.9.1  │ Config:   ~/.openclaw                   │
│ [ON] openclaw      npm  v1.2.3 ←─┼─ ── Workspaces (2) ──────────────────    │
│ ...                              │  ★ main         [AGENTS.md ✓] [SOUL.md ✓] [3 workflows]
│                                  │    side-project [AGENTS.md ✓] [SOUL.md ✗]
│                                  │ Enter: edit first doc · c: openclaw setup
├──────────────────────────────────┴─────────────────────────────────────────┤
│ status: sync plan loaded · 3 mappings ready                                │
│ j/k:navigate  s:execute selected  S:execute all                            │ ← keymap bar (§3)
└────────────────────────────────────────────────────────────────────────────┘
```

The mockup shows the P1 state of the Sync tab's bar; `a`/`A`/`l`/`L`/`w` appear as their phases land (§9), each added to the tab's registry table.

#### 2.1 OpenClaw dissolves into Agents detail

All plumbing already exists — `openclaw_state` (app.rs:44), workspace discovery (app.rs:246-256), doc editing via `$EDITOR` (app.rs:1060-1075), the `openclaw setup` spawn (app.rs:1524-1534). The merge is:

- `draw_agent_detail_enhanced` gains a **Workspaces section** rendered only when `agent.spec.id == "openclaw"`: workspace list with doc badges (AGENTS.md/SOUL.md/workflow counts, same badges as the old tab), installed status, and the real key hints.
- `on_enter` arm 0 (Agents) gains an openclaw branch: Enter edits the first doc of the default (★) workspace — same behavior as the old tab's Enter. Documented simplification: per-doc selection is dropped in the merged view; the detail panel prints every doc path so `$EDITOR` remains one paste away, and `agentry openclaw workspaces` (main.rs:417, unchanged) remains the power path.
- `c` (openclaw setup spawn) and `n` (create workspace guidance) re-gate from "tab 4" to "Agents tab AND openclaw selected". `a` (add sub-agent) re-gates the same way with an honest label — the hint becomes `a: show 'openclaw agents add' command` since it never did more than print guidance. The lying `g: Open in shell` hint is removed; `g` stays Skills-only.
- `draw_openclaw_list`/`draw_openclaw_detail`, the `Tab::OpenClaw` arms, and the OpenClaw `list_max` arm are deleted.
- **CLI subcommand `agentry openclaw workspaces` is unchanged** — no breaking change.

#### 2.2 Sync becomes actionable — two-stroke execution (confirmed)

The owner confirmed the execution model: **"Two key strokes for that: one for all and other with specific sync."** The plan auto-loads on tab entry, which frees `s` from plan-reload duty:

| Key | Before | After |
| --- | --- | --- |
| *(tab entry)* | empty detail, "Press 's' to load sync plan" | plan auto-loads on first entry (same pure computation as today's `s` — no subprocess, no latency risk) |
| `s` | reload plan, advertised as "Execute sync" | **execute the selected mapping** — the `sync.execute` action with the selected `SyncMapping`, y/n confirm (direct `execute_sync` call in P1; harness adapter in P2 — see phase placement below). **Primary key for execute-selected is `s`**; `Enter` is a documented alias (same action, listed in the keymap table as `Enter: execute selected (alias of s)`) so the muscle memory from the old hint still works |
| `S` | unbound | **execute all** non-`Skip` mappings, one y/n confirm, one `execute_sync` call per prompt, summary in status line |
| *(refresh)* | `s` reloaded the plan | automatic: the plan re-computes on tab entry and after every execution (pure function, no key needed) |

Supporting change: `SyncResultEntry` (app.rs:11-17) currently stores only display strings, so execution would have to re-plan. It gains the underlying `SyncMapping` so `s` executes exactly what the user selected. After any execution, `check_sync_status` re-runs over the loaded plan to refresh statuses in place; if an audit has run this session, the status line suggests `r` to re-audit.

**Phase placement (prevents a P1 harness stub):** in **P1**, sync execution lands as **direct engine calls** — `s`/`S` invoke `execute_sync` (executor.rs:15) directly, exactly as the CLI does today (main.rs:248), with the y/n confirm handled in TUI code. No harness exists yet in P1, and none is stubbed. In **P2**, once the harness crate exists, sync mutations (and fix/audit mutations) migrate to **thin adapters through `HarnessRegistry`** — the engines are untouched; only the call site changes. An implementer must not build a harness stub in P1 to "get ahead" of P2; the direct-call P1 code is the intended end state for that phase, and the P2 migration is a mechanical re-route, not a rewrite.

#### 2.3 Audit becomes actionable — with consent-gated auto-apply

- **Auto-run on first entry**: entering the Audit tab with `audit_report.is_none()` runs the audit (the TUI engine path is synchronous and cheap — no version lookups). `r` still re-runs.
- `a`/`A` finally wired per ADR-001 §4.1 — Phase 2 (§9), so no key is bound to a non-existent action in P1.
- **Auditor auto-apply (confirmed amendment):** auditor findings are Suggestions by default, and the human can apply one with a single keystroke — `a` on a selected `Audited` finding whose `suggested_fix` passes the fix gate (§5.5, §5.7). One keystroke = human consent. The apply routes through the **same** fail-closed fix gate as deterministic fixes; there is no auditor bypass. `A` (apply-all) never touches `Audited` findings.
- `l`/`L` (auditor review) land in Phase 3.

#### 2.4 Navigation and honesty fixes

| Fix | Change |
| --- | --- |
| Number keys | `1`-`5` also reset `list_selected`/`method_selected`, matching Tab (app.rs:519-533) |
| `←` guard | `method_prev` gated on tab 0, matching `method_next` |
| `r` dual-mapping | Kept context-sensitive (Remove / Re-run audit) — the keymap bar (§3) states each tab's meaning, which was the actual defect |
| `i` alias | `i` outside Skills does nothing (no silent new-prompt) |
| Version dead end | **Wired, not removed**: with `version_list` loaded, `Enter` on the Agents tab installs the *selected version* (`AgentConfirmAction::Install { version: Some(...) }` — the field already exists, app.rs:1018), then clears `version_list`; `Esc` cancels selection. The `v` flow becomes: `v` → j/k → Enter = install pinned version |
| Per-tab footers | Superseded by the persistent keymap bar (§3) — a hard requirement from the owner |
| Help overlay | Rendered from the same keymap registry as the bar (§3); rewritten to match reality: `1-5`, `S`/`s`, `Enter` per tab, `f` (audit severity filter — exists today, app.rs:513, carried into the Audit tab's registry table so the rewrite does not orphan it), `a`/`A`/`l`/`L` when they land, `w` as harness palette in P4; drop the "Load/execute sync plan" lie (dashboard.rs:1792) |
| README | 5-tab table; remove vim-like editor claims (`:w`/`:q`, "vim-like editor" — README.md:96,113,116); keybinding table generated from the same registry semantics |

#### 2.5 Dead code disposition

| Item | Disposition |
| --- | --- |
| `w` workflow key | **Removed in P1.** It writes `.lobster` files invoking `agentry task assign`, which does not exist — every generated workflow is broken on arrival. `w` **returns in P4 as the harness action palette** (§1.3): a global key listing registered harness actions and running simple sequences through the harness gates. No `.lobster` generation. `decompose_task`/`save_workflow` stay in `agentry-acp` (library-level, tested) |
| `version_input` field | Deleted (never read) |
| `version_list`/`version_list_error` | Kept — the flow is completed, not removed (§2.4) |

### 3. Persistent keymap bar (hard requirement)

**Owner directive:** "map keys must show always at bottom of TUI and always up to date to current tab." This upgrades revision 1's "per-tab footer hints" into a first-class TUI component with a single source of truth.

#### 3.1 Design

- **Region**: the last two lines of every frame, in every tab, always rendered. Line 1: transient status/error message (existing semantics, unchanged priority). Line 2: the focused tab's active key hints.
- **Single source of truth**: a per-tab keymap registry — one module, `ui/keymap.rs`, holding one `Vec<KeyBinding>` table per tab:

```rust
pub struct KeyBinding {
    pub key: &'static str,
    pub action: TuiAction,
    pub hint: &'static str,
    pub when: Option<fn(&App) -> bool>,
}

pub enum TuiAction {
    Navigate,
    Harness(HarnessInvocation),
    Local(LocalAction),
}
```

- `on_key` **dispatches through the table**: key lookup on the focused tab's bindings, then match on `TuiAction`. `Harness(...)` invocations route through the `HarnessRegistry` (§1.3); `Local(...)` covers pure-UI behaviors (navigate, filter, open help). **Phase note:** the `TuiAction::Harness` variant lands in **P2** with the harness crate; P1 keymap tables use `Local` actions only (sync execution in P1 is a direct engine call, §2.2), with the enum variant declared but unexercised so the P2 migration is additive, not structural.
- The bottom bar renders the focused tab's table filtered by `when` — so a hint can only drift from its handler if someone edits the one table where both live. Drift is structurally prevented, not policed by review.
- The help overlay renders from the same tables (all tabs, ignoring `when`), so the overlay and the bar can never disagree.
- Conditional hints: `when` predicates keep the bar honest — e.g. `a: apply suggestion` appears on the Audit tab only when the selected finding is `Audited` with a gate-validable `suggested_fix`; `Enter: install pinned version` appears only while `version_list` is loaded.
- Width overflow: enabled bindings render left-to-right; if the terminal is too narrow the bar wraps to at most 2 lines rather than truncating, showing MORE keys. Navigation keys (`j/k`, `Tab`) always render first. Bindings that no longer fit on either line are dropped.
- Confirm dialogs (`y/n`) are themselves bindings in the table, so during a confirm the bar shows exactly the two keys that work.

#### 3.2 Testing

A unit test renders the bar for every tab in a fixed `App` state and asserts equality with the registry's filtered bindings — the bar is a pure projection of the registry, and the test pins that invariant.

### 4. Extensible host registry

**Owner directive:** "You are missing ollama, fal, zai, etc..." The set of invocable host CLIs is a **config-driven registry**, not hardcoded match arms. A new CLI becomes a config addition, not a code change.

#### 4.1 Host profile model

```rust
pub struct HostProfile {
    pub id: String,
    pub display_name: String,
    pub kind: HostKind,
    pub detect_binary: String,
    pub headless_command: Option<String>,
    pub model_argument: Option<String>,
    pub transport: Transport,
}

pub enum HostKind {
    AgentCli,
    LocalRuntime,
    ApiCli,
}

pub enum Transport {
    Stdin,
    Argv,
}
```

- `headless_command` is a template; the prompt is delivered per `Transport` (stdin preferred — avoids ARG_MAX and shell-escaping hazards for multi-KiB prompts, §5.4).
- **Built-in registry** (compiled defaults, merged with user config at load):

| id | kind | Default headless template | Notes |
| --- | --- | --- | --- |
| `claude-code` | AgentCli | `claude -p --output-format text` | verified pattern |
| `codex` | AgentCli | `codex exec -` | verified pattern |
| `gemini-cli` | AgentCli | `gemini -p` | verified pattern |
| `zai` | AgentCli | seeded default, **verify flags at implementation** | Z.ai / GLM CLI |
| `fal` | ApiCli | seeded default, **verify flags at implementation time** | fal.ai; no CLI binary — out of reach in v1 (see ApiCli note below); seeded-but-excluded |
| `ollama` | LocalRuntime | `ollama run {model}` | prompt via stdin; model from `[local]` config; best-effort JSON compliance (§10 risks) |

- **ApiCli honesty (v1 scope)**: `Transport` has only `Stdin` and `Argv` — there is **no HTTP transport in v1**. API-platform hosts without a CLI binary are therefore out of reach: the registry can only invoke executables, so a raw-API host (fal's platform API, for example) cannot be a harness target in v1. HTTP transport is future work. `fal` stays **seeded-but-excluded** — present in the built-ins as config, absent from the default priority chain — until either a CLI wrapper exists that the registry can invoke as a binary, or an HTTP transport lands.
- **Extensibility rule**: a host id present in user config but not in the built-ins is valid if it declares `headless_command` + `detect_binary`; detection falls back to binary presence. Code changes are required only for a new `HostKind` or `Transport` — i.e., a genuinely new invocation shape, not a new CLI.
- **Fallback/priority chain** is config, not code: `hosts.priority = [...]` (default `claude-code → codex → gemini-cli → zai → ollama`). Agentic actions walk the chain of *installed* hosts; `command_template` overrides remain per-host.
- Detection reuses `detect_agent()` (installed + version) for `AgentCli` kinds and binary presence for the rest.

#### 4.2 AgentSpec extensibility (noted, out of scope)

The same config-driven treatment will eventually apply to agentry's own hardcoded `AgentSpec` list (11 agents in spec.rs) — new agent CLIs would be config additions too. This is a real future need but is **explicitly out of scope** for this ADR; the host registry establishes the pattern to copy.

### 5. Agent Auditor

The headline capability of revision 1, now riding on the harness: agentry gets its own first-party agent that reviews audit findings with an LLM, guided by the owner's skill library. The deterministic audit engine (ADR-001) is untouched; the auditor is an **agentic harness action** over it.

#### 5.1 The auditor is a first-party agent definition, not an AgentSpec

`AgentSpec` describes the 11 host CLIs. The auditor is a *consumer* of them. It gets:

- **Canonical prompt**: `~/.agents/prompts/agentry-auditor.md` — a real `UnifiedPrompt`. It appears in the Prompts tab, participates in drift checks, and is versioned like any prompt. agentry manages its own auditor's prompt with agentry. The template ships in-repo (`crates/agentry-auditor/assets/agentry-auditor.md`, `include_str!`) and `agentry auditor setup` writes it **only if absent** (never clobbers user edits).
- **Role marker**: frontmatter `agentry-role: auditor`. This is load-bearing — see the sync hazard below.
- **`AuditorConfig`** as the `[auditor]` section of `~/.agents/agentry.toml` (revision 2 supersedes revision 1's separate `~/.agents/auditor.json` — one config surface, one onboarding marker; nothing shipped yet, so the fold is free): `host_cli` (host registry id), `command_template` (override escape hatch), `model` (optional), `timeout_secs` (default 120), `max_findings` (default 20). Written by `agentry auditor setup` and by onboarding.

**Sync hazard (must-handle):** default `plan_sync` behavior maps every canonical prompt to every detected agent's prompt file. A naive canonical auditor prompt would make `agentry sync --all` overwrite `CLAUDE.md`, `AGENTS.md`, etc. on every host with auditor instructions. Therefore: `plan_sync` **excludes role-marked prompts from default mappings** (fail-closed — any prompt with an unrecognized `agentry-role` is never default-synced). In P4, role-marked prompts gain a dedicated mapping: auditor prompt → the host CLI's *agent/subagent config* location (reference implementation: `~/.claude/agents/agentry-auditor.md` for claude-code), so the prompt is delivered as a subagent system prompt by agentry's own sync — the prompt participates in sync, but to the right destination.

#### 5.2 System prompt contract

The auditor prompt must instruct the host CLI to:

1. **Load skills from the agentry hub before analysis**: read `~/.agents/skills/skill-creator/SKILL.md` and `~/.agents/skills/context-engineering-collection/SKILL.md` and apply their evaluation rubrics. Path-based loading is deliberate — it works identically across all host CLIs regardless of whether the host natively reads `~/.agents/skills/`.
2. **Analyze** the provided AuditReport context: triage findings, identify root causes the deterministic checks missed, propose remediations grounded in the evidence.
3. **Emit a strict JSON array** of findings: `{ "check_id": "auditor.<name>", "severity": ..., "category": "audited", "message", "remediation", "evidence", "suggested_fix" }` — matching the `AuditFinding` schema so parsing is mechanical. `suggested_fix` is optional and follows the `FixAction` schema.
4. **Stay advisory by default**: findings arrive as Suggestions; execution happens only through the human's per-finding apply keystroke through the fix gate (§5.7).
5. **Request skills when needed** (§5.6).

#### 5.3 Context packaging

```rust
pub struct AuditorContext {
    pub report: AuditReport,
    pub focus: Option<AuditFinding>,
    pub excerpts: Vec<FileExcerpt>,
    pub skills_inventory: Vec<String>,
}

pub struct FileExcerpt {
    pub path: PathBuf,
    pub withheld: bool,
    pub content: Option<String>,
}
```

Budget: context capped at ~32 KiB; findings prioritized Critical → Warning → Info; excerpts truncated per-file. Auth findings contribute **status only** ("not logged in"), never token contents — consistent with ADR-001 §7's auth rule. Packaging is a harness-provided concern (§1.1): every agentic action gets the same budgeting and withholding machinery.

#### 5.4 Invocation runtime: headless CLI delegation via the host registry

No LLM client is added to the workspace. The `auditor.review` harness action shells out to an installed host from the registry (§4):

- **Selection**: `AuditorConfig.host_cli`; fallback walks `hosts.priority` over installed hosts; none installed → the run aborts with an `auditor.no_host` Info finding ("install a supported agent CLI to enable LLM-assisted audit") — the deterministic audit remains fully functional without any host.
- **Prompt transport**: via **stdin** by default (`Transport::Stdin`), not argv.
- **Host invocation shell discipline**: host CLIs are invoked **argv-split, never `sh -c`** — `std::process::Command::new(binary).args(split_args)` with no intermediate shell, so no shell metacharacter can ever be interpreted. Interpolated config values (`{model}` from `[local]`, per-host template overrides) pass the **same `is_safe_shell_arg` charset check agentry already owns** (fix.rs:20-29 — rejects metacharacters, command substitution, pipes, semicolons); a config value failing the check aborts the invocation fail-closed (`auditor.run_failed`), never silently. This is the inverse discipline of the fix engine: the fix engine validates *LLM-proposed* commands before `sh -c`; the harness never spawns a shell at all and validates *config-sourced* arguments before argv.
- **Terminal**: suspend TUI (`disable_raw_mode` + `LeaveAlternateScreen`) → run child process capturing stdout → restore (`EnterAlternateScreen` + `enable_raw_mode` + `needs_terminal_clear`) — the exact pattern of `execute_agent_action` (app.rs:1295-1307), provided by the harness invocation layer so every agentic action inherits it.
- **Timeout**: child killed at `timeout_secs`; non-zero exit / timeout / unparseable output → error status + `auditor.run_failed` Info finding with stderr excerpt as evidence. Fail closed, never silently.
- **Local runtimes**: `ollama` is a first-class registry entry. The `[local]` config section (runtime + model, chosen at onboarding, §6) supplies `{model}`. Local models are weaker at strict JSON contracts; the tolerant parser and fail-closed handling carry the weight, and the finding cap bounds the damage. Local invocation also means **no egress** — a privacy-positive default for sensitive machines.

#### 5.5 Response parsing, the `Audited` category, and `suggested_fix`

- `FindingCategory` gains an **`Audited`** variant (report.rs:19-31 — serde snake_case). **`schema_version` bumps to 2.** Although the variant is additive at the Rust level, it is breaking for strict deserializers of `--json` output: a consumer that deserializes `FindingCategory` as a closed enum will reject any report containing an audited finding. Claiming "additive, stays 1" would be dishonest about that failure mode, so the version bumps and the change is documented as breaking: **migration note** — consumers of `agentry auditor review --json` must accept `schema_version: 2` (or filter on it); reports from a v1 reader against v2 output fail with a version mismatch, which is the correct, loud failure — not a silent deserialization error mid-report.
- `AuditFinding` gains **`suggested_fix: Option<FixAction>`** (additive, serde default — schema-compatible). This is the auto-apply channel.
- Parsing: extract the last JSON array from the response (fenced or bare), deserialize, then **sanitize**:
  - `category` forced to `Audited`; `check_id` prefixed `auditor.` (mangled if the model omitted the prefix);
  - `severity` **forced to `Suggestion`** (trust level, §5.7);
  - `auto_fixable` forced `false` and `fix` forced `None` — always. The LLM cannot mark anything batch-fixable;
  - a model-proposed fix is stored in `suggested_fix` **only if** it deserializes as a `FixAction` and passes `fix::validate()` (the deterministic allowlist + path-bounds check — see §5.7 and the path-bounds bullet below). Anything the gate would reject is dropped to `None` at parse time; the remediation *text* survives either way;
  - **path bounds (blocker fix)**: `fix::validate()` as it exists today has **no path bounds for `FileWrite` or `FileRemove`** — `write_file` (fix.rs:161-178) creates parent dirs and writes any path, `remove_file` (fix.rs:180-185) removes any path; only `SymlinkRecreate` carries a home-dir prefix rule (fix.rs:187-197). The extracted `fix::validate()` **MUST add path bounds for both variants**: minimum `home_dir` prefix (matching SymlinkRecreate's rule); recommended allowlist of `~/.agents/` plus the detected agent config dirs. An out-of-bounds `suggested_fix` is dropped to `None` at parse time — the remediation *text* survives, the executable fix does not. Rationale: the LLM is an **untrusted producer**; adversarial content in a file excerpt could otherwise inject `FileWrite { path: ~/.claude/CLAUDE.md, content: <attacker text> }`, which passes the spec'd gate as written and executes with one keystroke. With path bounds, parse-time quarantine refuses it before it is ever stored;
  - deduplicated against existing check_ids; count capped at `max_findings`.
- Merge: findings with `agent_id` append to that agent's `findings`; others to `global_findings`; summary recomputed. `history::apply_feedback` records them, but the recurring-finding **promotion rules exclude category `Audited`** so the Suggestion cap cannot be circumvented by the recurrence rule.
- Rendering: findings list shows a distinct `[AI]` badge for `category == Audited`.

#### 5.6 Skill-request loop (Wobblus precedent) and the `w` palette

The owner's own orchestrator (Wobblus) already operates on this pattern: an allowlist-gated skill tool where skills are callable resources — discovery → gated execution. Agentry adopts the same shape:

- The auditor prompt includes the installed-skills inventory and instructs: *if the task needs a capability you lack, emit `{"skill_request": "<skill-name>"}`.*
- **P3 (MVP)**: skill requests are surfaced as `auditor.skill_request` Suggestion findings with remediation `agentry skills install <name>` — the human installs.
- **P4 (closed loop)**: agentry checks the request against the skill hub — **the lockfile is the allowlist** (mirroring Wobblus's allowlist gate) — installs from the hub if available (a `skills.install` harness action, with its confirmation gate), re-invokes the auditor once with the new skill loaded (one retry max, loop bounded), and records the request in history. **Consent chain**: the re-invocation is not keyed to its own prompt — it is chained off the install consent. The `skills.install` confirmation surface must therefore **disclose the one bounded re-invocation** ("installing this skill will re-run the auditor once with it loaded"), or the flow must require a **second explicit confirm** before the re-invoke. Silent chained execution is not permitted: the user who consents to the install must have been told, on that same surface, exactly what follows.
- **P4 also lands `agentry task assign`** (enqueue `TaskAssign` into the `~/.agents/acp/` file queue — `init_acp_dirs` finally called in production) plus `agentry auditor serve`, a file-queue worker that resolves queued tasks to harness actions. **Pre-consent trust model (decided)**: `~/.agents/acp/` is a plain file queue any local process can write, so a forged consent entry would produce egress without a keystroke. For v1, `agentry auditor serve` resolves **`ActionKind::Systematic` actions only** — option (a), chosen as safer and simpler: no queued task can trigger an LLM invocation or any egress, so the forged-consent vector has nothing to reach. Option (b) — treating queue-write access as the consent authority — is the **documented trust assumption for P4+**: if agentic actions are ever served from the queue, the ADR must state explicitly that queue-write access ≡ machine access ≈ consent authority (anyone who can write the queue already owns the machine), and the consent-record check (§1.5) becomes the enforcement of that assumption rather than a security boundary. Until that assumption is formally accepted, serve stays Systematic-only. The `w` key returns as the **harness action palette**: registered actions, simple sequences (e.g. audit → fix-all → sync-all → auditor review), each step through its own confirmation gate.

#### 5.7 Security boundaries

| Surface | Rule |
| --- | --- |
| Prompt contents | Paths, finding metadata, bounded excerpts of config/prompt files. **No token/credential contents** — auth findings carry status only; files matching credential-shaped paths (`auth.json`, `*.token`, `.env`, oauth stores) are excerpted as `withheld: true` |
| Auditor output | Severity ≤ `Suggestion`; `auto_fixable` forced false; `fix` always stripped; `suggested_fix` quarantined and gate-validated at parse time; count-capped; deduplicated |
| Execution | **Suggestions-only with optional auto-apply under human consent** (owner-confirmed). Applying an `Audited` finding requires an explicit per-finding keystroke (`a`), and the proposed fix must pass the **same deterministic fail-closed gate** (`fix::validate` — shell allowlist **plus path bounds for `FileWrite`/`FileRemove`**, §5.5) as any human-initiated fix. No bypass path exists: the apply call is a `fix.apply` harness action. Batch apply (`A`) excludes `Audited` findings. An LLM cannot inject shell commands that the allowlist would reject, and cannot point a file fix outside the path bounds |
| Confirm surface | **FileWrite confirmations must show content size (or the content itself)** — `fix_description` (fix.rs:293-309) renders FileWrite as `write <path>` with the content invisible, which is not informed consent for an LLM-authored write. The TUI confirm dialog for an `Audited` finding with a `FileWrite` fix displays the byte size and a bounded preview of the content; the CLI `--yes` path is unavailable for LLM-authored `FileWrite` (per-finding keystroke only, §2.3) |
| Execution environment | The host CLI runs under **its own** permission configuration; the prompt instructs read-only behavior. Documented that headless permission posture is the user's host-CLI responsibility |
| Consent | Auditor invocation is an explicit key press (`l`/`L`) or explicit CLI call — a harness `Single`-confirmation agentic action (§1.2). **Never auto-run** — audit context leaves the machine to the LLM provider when invoked (except local runtimes), and that egress must be user-initiated every time |
| Injection | File excerpts could carry adversarial content; blast radius is bounded because output is Suggestion-capped, sanitized, and gate-validated (§5.5). Excerpt size capped |

### 6. Onboarding wizard + llmfit integration

**Owner directive:** "make it simple to start with: the workflow should setup an agentry harness across all agents and ask to install any missing agent CLI that can be installed in the machine AND suggest llms that may be able to run by wrapping llmfit core functions into the agentry... and sets up a basic workflow on the user selections when onboarding or updating the setup."

#### 6.1 Triggers

- **First run**: the TUI (and any CLI command that needs config) detects the absence of `~/.agents/agentry.toml` and offers the wizard. Declining leaves agentry fully functional with defaults — onboarding is an offer, never a wall.
- **Update path**: `agentry setup` re-runs the wizard at any time, pre-filled with the current config, idempotent.

#### 6.2 Step flow

| Step | What happens | Mechanism |
| --- | --- | --- |
| 1. Detect | `detect_all_agents()` → table of installed / not-installed / not-detected agents | existing detector |
| 2. Offer installs | for each known-but-missing agent with an installable method: "install codex via brew? y/n" — **per-agent explicit consent, never batch-silent** | `InstallMethod::install_command` through the `skills.install`-style harness gate |
| 3. Machine analysis | hardware profile: CPU, RAM, GPU/VRAM, accelerators — **local-only, no egress** | llmfit-core (§6.3) |
| 4. Local model suggestions | ranked "models this machine can run" list; user optionally selects a runtime (ollama) + model | llmfit-core fit scoring |
| 5. Agentic host selection | pick the default host for agentic actions from installed agent CLIs (registry §4) | host registry |
| 6. Write config | `~/.agents/agentry.toml` — the **basic workflow**: which agents are harness targets, which host runs agentic actions, which local runtime/model serves local steps | one serde-typed config |

"Sets up a basic workflow on the user selections" = the written config **is** the basic workflow. No scheduler, no pipelines, no `.lobster` — the harness config enables the actions; the user (and the `w` palette) composes sequences later. Keep it simple to start.

#### 6.3 llmfit integration: library dependency (evaluated, recommended)

Andler's directive is to wrap llmfit's **core functions** and have agentry install the dependency. Verified: llmfit is a Rust workspace (MIT) whose `llmfit-core` crate is published on crates.io (v1.1.12, "Core library for llmfit — hardware detection, model fitting, and provider integration"; deps: sysinfo, ureq, serde; objc2-metal on macOS; model catalog embedded at build time).

| Option | Pros | Cons |
| --- | --- | --- |
| **Library dep on `llmfit-core`** (recommended) | typed API, no external binary requirement, in-process hardware detection + fit scoring, offline-capable (catalog embedded), matches the owner's "wrapping core functions / install the dependency" wording | dependency weight (sysinfo, ureq, objc2-metal); fast-moving upstream (94 releases); edition 2024 raises MSRV |
| Shell out to the `llmfit` binary (`llmfit recommend --json`) | zero compile-time coupling | users must install a second binary; JSON contract drift risk; extra process; contradicts the owner's stated mechanism |

**Recommendation: library dependency.** `llmfit-core = "1.1"` pinned to a minor line, behind a cargo feature `llmfit` (default-on), consuming only the hardware-profile and fit-scoring surface. If the upstream cadence or MSRV becomes a burden, the binary path (`llmfit recommend --json`) is the documented fallback — the onboarding step degrades to a suggestion, never a failure. Onboarding step 3/4 skip gracefully when the feature is compiled out.

**MSRV decision (stated explicitly, Stage 2):** `llmfit-core` is edition 2024, which raises the compile floor to **Rust ≥ 1.85** whenever the default-on `llmfit` feature compiles. The workspace **adopts MSRV 1.85+ explicitly** with the feature default-on — the alternative (default-off to preserve a lower floor) buys compatibility for older toolchains at the cost of a two-tier build matrix, and the binary fallback already covers machines that cannot upgrade. Toolchains older than 1.85 build agentry with `--no-default-features`; onboarding's llmfit steps skip gracefully (above), and everything else in agentry is unaffected.

#### 6.4 Config artifact: `~/.agents/agentry.toml`

```toml
[onboarding]
setup_completed_at = "2026-09-01T12:00:00Z"

[harness]
enabled_agents = ["claude-code", "codex", "openclaw"]

[hosts]
priority = ["claude-code", "codex", "gemini-cli", "zai", "ollama"]

[hosts.zai]
display_name = "Z.ai GLM"
kind = "agent-cli"
detect_binary = "zai"
headless_command = "zai -p"
transport = "stdin"

[auditor]
host_cli = "claude-code"
model = ""
timeout_secs = 120
max_findings = 20

[local]
runtime = "ollama"
model = "qwen2.5-coder:7b"
```

One file, four sections: onboarding marker, harness targets, host registry (extensions + priority), auditor defaults, local runtime/model. `AuditorConfig` moves here (§5.1). Schema is serde with defaults throughout, so partial files and future sections are non-breaking.

**Config ownership (Stage 2):** the serde types for `[local]`, `[harness]`, and `[onboarding]` — and the file-write for `~/.agents/agentry.toml` — belong to the **harness crate's `context.rs`** (the natural owner: `HarnessContext` already loads and shares config across actions, §1.1). The auditor crate reads `[auditor]` through the harness context rather than owning the file; onboarding (§6) writes through the same harness config API. One owner per file, no split-brain writes.

### 7. Crate placement

Two new crates, same reasoning as ADR-001 §1.1 — the harness spans audit + all engines; the auditor spans audit + agents + skills + harness. Dependency direction stays clean and inward:

```
core ← agents/sync/skills/openclaw/acp ← audit ← harness ← auditor ← tui
```

- `agentry-harness` houses the action model, registry, gates, context, **and the host registry + headless invocation machinery** (transport, timeout, suspend/restore) — every agentic action inherits them.
- `agentry-auditor` implements the first agentic action against the harness trait and keeps its own concerns: prompt template, context packaging, tolerant parsing, config.
- The TUI assembles the registry (it already depends on both) and binds keys to harness invocations through the keymap tables (§3).

```
crates/agentry-harness/
├── Cargo.toml          # deps: agentry-core, agentry-agents, agentry-audit, agentry-sync, agentry-skills, serde, serde_json, tokio
└── src/
    ├── lib.rs
    ├── action.rs       # HarnessAction trait, ActionKind, Confirmation, ActionInput/ActionOutput
    ├── registry.rs     # HarnessRegistry: id → action, lookup + listing
    ├── context.rs      # HarnessContext (shared env: home dirs, agents, hub, config)
    ├── gate.rs         # confirmation policy, GateTicket (private ctor, registry-issued), consent recording (TUI/CLI/ACP shared)
    └── hosts/
        ├── mod.rs      # HostProfile, HostKind, Transport, built-ins, config merge, priority chain
        └── invoke.rs   # headless invocation: stdin transport, timeout, suspend/restore helper

crates/agentry-auditor/
├── Cargo.toml          # deps: agentry-audit, agentry-agents, agentry-skills, agentry-core, agentry-harness, serde, serde_json, tokio
├── assets/
│   └── agentry-auditor.md   # canonical prompt template (include_str!)
└── src/
    ├── lib.rs
    ├── action.rs       # AuditorReviewAction — implements HarnessAction (first agentic action)
    ├── config.rs       # [auditor] section of ~/.agents/agentry.toml
    ├── context.rs      # AuditorContext, FileExcerpt, byte-budget packaging
    ├── prompt.rs       # template + context → final prompt string
    └── parse.rs        # response → sanitized Vec<AuditFinding> (Audited, Suggestion-capped, suggested_fix quarantined)
```

### 8. CLI surface

```
agentry setup                    # onboarding wizard (first-run detection + update path)
agentry harness actions          # list registered harness actions (systematic + agentic)
agentry auditor setup            # write [auditor] config, canonical prompt if absent, adopt
                                 # orphaned context-engineering-collection into the lockfile
agentry auditor review           # audit → LLM review → merged report (human-readable)
agentry auditor review --json    # machine-readable AuditReport incl. Audited findings
agentry auditor serve            # P4: ACP file-queue worker resolving tasks to harness actions
agentry task assign ...          # P4: enqueue a task into ~/.agents/acp/ (external transport)
```

`agentry audit` (ADR-001) is unchanged; the auditor and harness are additive.

### 9. Implementation phases

#### Phase 1 — TUI restructure + persistent keymap bar

**Scope:** §2 in full except `a`/`A`/`l`/`L`/`w` (later phases), plus §3 (keymap bar). 5 tabs; sync auto-load + `s`/`S` execution; audit auto-run on first entry; all §2.4 nav fixes; §2.5 dead-code removal; help/README truth-up. The keymap bar lands here because every later key lands *through* it. Existing keys — including `f` (audit severity filter, app.rs:513) — are migrated into their tab's registry table; none are orphaned by the dispatch rewrite.

| File | Change |
| --- | --- |
| `crates/agentry-tui/src/ui/mod.rs` | `Tab::OpenClaw` removed; `ALL` → 5 entries; Audit index 4 |
| `crates/agentry-tui/src/ui/keymap.rs` | New — per-tab `KeyBinding` tables, `TuiAction` enum, bar/help renderers |
| `crates/agentry-tui/src/app.rs` | modulo 5; number keys reset selection; `←` guard; `on_enter` Sync arm (execute selected mapping) + Agents openclaw-doc branch + version-install branch; `on_sync_all` (`S`); `SyncResultEntry` carries `SyncMapping`; audit auto-run on first Audit entry; sync auto-load on first Sync entry; remove `on_workflow`, `version_input`; c/n/a re-gated to Agents+openclaw; `i` de-aliased; `on_key` migrated to keymap dispatch |
| `crates/agentry-tui/src/ui/dashboard.rs` | Workspaces section in `draw_agent_detail_enhanced`; delete openclaw renderers; keymap bar region (last 2 lines); honest sync/audit detail hints; help overlay rendered from registry |
| `README.md` | 5-tab table; remove editor claims; keybinding truth-up |
| `crates/agentry-tui/src/app.rs` (tests) | Tab-index fixtures (5 → 4); 5-tab nav test; sync-execute + version-install unit tests; **keymap-bar-equals-registry test for every tab** |

**Dependencies:** none. **Effort:** 2–3 days. **Exit criteria:** every advertised key does what its hint says *and every hint comes from the registry*; `cargo test` + `cargo clippy -D warnings` clean; manual pass: Enter syncs one mapping, `S` syncs all with confirm, `v`→Enter installs a pinned version, openclaw docs editable from Agents tab, bar correct on all 5 tabs.

#### Phase 2 — Fix keys + harness foundation

**Scope:** ADR-001 §4.1 finally complete; the harness exists and everything mutating routes through it.

| File | Change |
| --- | --- |
| `crates/agentry-harness/*` | New crate per §7: trait, registry, context, gate |
| `crates/agentry-tui/src/app.rs` | `a` on Audit tab → `fix.apply` (selected) with y/n; `A` → `fix.apply_all`, one confirm, sequential; `history::apply_feedback` after each run (parity with main.rs:621); affected check re-run post-fix; outcome in status line |
| `crates/agentry-tui/src/ui/keymap.rs` | Audit-tab bindings with `when` predicates; bar labels update |
| `crates/agentry-tui/src/main.rs` + `app.rs` | sync/fix/audit mutations resolve through `HarnessRegistry` (thin adapters; engines untouched) |

**Dependencies:** P1. **Effort:** 2–3 days. **Exit criteria:** broken symlink repaired from the TUI with y/n confirm; history grows one line per fix; `r` shows the finding resolved; every TUI/CLI mutation flows through a harness gate (asserted in tests).

#### Phase 3 — Auditor MVP + host registry

**Scope:** §4 (host registry), §5.1–5.5, §5.6 (MVP half), §7 (auditor crate), §8 (`setup`, `review`), `l`/`L` keys, consent-gated auto-apply.

| File | Change |
| --- | --- |
| `crates/agentry-harness/src/hosts/*` | Host profile model, built-ins (claude-code, codex, gemini-cli, zai, fal, ollama), config merge, priority chain, headless invocation (stdin, timeout, suspend/restore) |
| `crates/agentry-auditor/*` | New crate per §7 |
| `crates/agentry-audit/src/report.rs` | `FindingCategory::Audited` variant; `AuditFinding.suggested_fix` field |
| `crates/agentry-audit/src/fix.rs` | `validate()` extracted from `apply_fix` (gate reused by parse-time quarantine and apply-time enforcement) **with path-bound validation for `FileWrite`/`FileRemove`** (minimum home-dir prefix, recommended `~/.agents/` + detected agent config dirs) **+ unit tests: outside-home refusal, agent-config allowlist acceptance** |
| `crates/agentry-audit/src/history.rs` | Promotion rules exclude `Audited` category |
| `crates/agentry-sync/src/planner.rs` | Role-marked prompts excluded from default mappings (fail-closed) |
| `crates/agentry-tui/src/app.rs` | `l`/`L` → `auditor.review` harness action with suspend/restore; `a` on Audited finding applies `suggested_fix` through the gate; **confirm dialog for LLM-authored `FileWrite` shows content size + bounded preview (§5.7 confirm surface)** |
| `crates/agentry-tui/src/ui/keymap.rs` | `l`/`L`/conditional `a` bindings; `[AI]` badge rendering in dashboard |
| `crates/agentry-tui/src/main.rs` | `Auditor { Setup, Review { json } }` subcommands |
| `~/.agents/.skill-lock.json` | Adopt `context-engineering-collection` (prereq — currently orphaned on disk) |

**Dependencies:** P2. **Effort:** 3–4 days. **Exit criteria:** `agentry auditor review --json` on a real machine returns Audited findings capped at Suggestion; TUI `l` on a finding shows `[AI]` suggestions; a gate-valid suggested fix applies with one keystroke, a non-valid one is refused with the gate's reason; **an out-of-bounds `FileWrite`/`FileRemove` suggested_fix (outside home, outside the agent-config allowlist) is dropped at parse time and its remediation text still renders**; a `FileWrite` confirm shows content size/preview; with no host CLI installed, the deterministic audit still completes with `auditor.no_host`.

#### Phase 4 — Harness expansion + onboarding + llmfit

**Scope:** §5.6 closed loop; `agentry auditor serve`; `agentry task assign`; `w` harness palette; P4 auditor-prompt sync delivery (subagent destination); §6 onboarding wizard + llmfit.

| File | Change |
| --- | --- |
| `crates/agentry-tui/src/main.rs` | `auditor serve`, `task assign`, `setup`, `harness actions` |
| `crates/agentry-acp/src/protocol.rs` | `init_acp_dirs` called by `serve` at startup (first production call) |
| `crates/agentry-auditor/src/*` | Skill-request handling (lockfile allowlist, one-retry re-invoke via `skills.install` action) |
| `crates/agentry-sync/src/planner.rs` | Role-marked prompt → host subagent-config destination (claude-code reference arm) |
| `crates/agentry-tui/src/app.rs` | `w` returns as the **harness action palette** (registered actions + simple sequences, each step gated) |
| `crates/agentry-tui/src/onboarding/*` | Wizard (TUI + CLI paths): detect → install offers → llmfit analysis → selections → write `agentry.toml` |
| root `Cargo.toml` | `llmfit-core = "1.1"` behind feature `llmfit` (default-on) |
| `crates/agentry-tui/src/ui/keymap.rs` | `w` binding (global), onboarding entry hints |

**Dependencies:** P3. **Effort:** 3–4 days. **Exit criteria:** fresh-machine onboarding end-to-end (detect → install offer accepted/declined → hardware profile → config written → auditor host resolves); `agentry setup` re-runs idempotently; `echo '{"type":"task_assign",...}' | agentry auditor serve` resolves a queued **Systematic** task to a harness action end-to-end, and a queued agentic task is refused (v1 trust model, §5.6); a skill request installs from the hub and the re-invocation sees the new skill; `w` palette runs an audit→fix→sync sequence through the gates.

*Sequencing note:* onboarding's minimal path (steps 1, 2, 5, 6 — no llmfit) has no hard dependency on P3 and may be pulled into P3 if capacity allows; llmfit analysis is the only genuinely new machinery and is the natural P4 item. The owner's "simple to start with" is satisfied either way — the wizard is the minimal viable harness setup, not a platform.

### 10. Risks & decisions

#### Risks

| Risk | Mitigation |
| --- | --- |
| **Host CLI dependency** — agentic actions need an installed host | Config-driven registry with fallback chain (`hosts.priority`); per-host `command_template` override; without any host, audit is fully functional and `auditor.no_host` explains how to enable; local runtimes (ollama) work with **zero egress** |
| **Seeded host templates unverified (zai, fal)** | Templates are config-overridable by design (§4.1); verification is a P3 task; a wrong default fails closed to `auditor.run_failed`, never silently |
| **Local models' JSON compliance** (ollama) | Tolerant parser + severity clamp + fail-closed `auditor.run_failed`; finding cap bounds damage; llmfit-guided model selection favors instruction-following models; ollama labeled best-effort in the registry |
| **LLM cost/latency in the audit loop** | Strictly opt-in per key press (harness `Single` confirmation); one call per invocation; 120 s timeout; 32 KiB context cap; never auto-run, never on a timer |
| **Hallucinated or malicious findings** | Suggestion-only severity, `auto_fixable` forced false, `fix` stripped, `suggested_fix` gate-validated at parse *and* apply time, count-capped, deduplicated; batch apply excludes Audited |
| **Prompt injection via file excerpts** | Suggestions-only blast radius; credential-shaped files withheld; excerpt size capped; gate validation is deterministic and LLM-independent |
| **`sync --all` clobbering host prompts with the auditor prompt** | Role-marker exclusion is fail-closed: unrecognized `agentry-role` values are never default-synced |
| **Tab-index shift regressions** | Mechanical but wide (every `tab_index` literal); test fixtures updated in the same commit; 5-tab nav integration test |
| **Keymap drift between bar, handlers, and help** | Prevented by construction: one registry per tab feeds dispatch, bar, and overlay; equality test per tab (§3.2) |
| **Parsed-JSON drift from host models** | Schema pinned in the prompt; tolerant parsing (prefix-mangle, severity clamp); malformed output fails closed to `auditor.run_failed` |
| **OpenClaw merge losing functionality** | All plumbing reused, not rewritten; CLI subcommand untouched; doc-badge rendering ported verbatim into the detail section |
| **llmfit-core coupling** — fast upstream (94 releases), edition 2024 MSRV | Pin to a minor line; cargo feature gate; consume only hardware-profile + fit types; documented binary fallback (`llmfit recommend --json`); onboarding degrades to a skipped step, never a failure |
| **Onboarding install step runs package managers** | Per-agent explicit y/n consent through the harness gate; never batch, never silent; same allowlist discipline as the fix gate |
| **Forged consent in the ACP file queue** — any local process can write `~/.agents/acp/`, so a forged pre-consent entry could authorize egress without a keystroke | v1: `auditor serve` resolves **Systematic actions only** (§5.6) — no queued task can reach an LLM invocation, so the vector has nothing to target. P4+: agentic serving requires the documented trust assumption (queue-write ≡ machine access ≈ consent authority) plus the consent-record check; until accepted, serve stays Systematic-only |
| **LLM-authored file fix pointing outside bounds** — adversarial excerpt content could propose `FileWrite` to arbitrary paths (e.g. `~/.claude/CLAUDE.md`) | Parse-time path bounds in `fix::validate()` (§5.5): out-of-bounds `suggested_fix` dropped to `None` before it is ever stored; apply-time gate re-validates; confirm surface shows content size/preview (§5.7) |
| **Harness over-abstraction** | Trait capped at five methods; no dynamic plugin loading; hosts are the only config-extensible surface in v1; systematic actions are thin adapters over tested engines |

#### Decisions confirmed by Andler (revision 2)

| # | Decision | Andler's answer | Resolution |
| --- | --- | --- | --- |
| 1 | Auditor host CLI | "You are missing ollama, fal, zai, etc..." | **CONFIRMED, expanded**: extensible config-driven host registry (§4); built-ins now include ollama, fal, zai |
| 2 | Cost/latency policy | "only explicit keypress (map keys must show always at bottom of TUI and always up to date to current tab)" | **CONFIRMED, upgraded**: explicit keypress stands; the per-tab footer idea became the persistent keymap bar — a hard requirement (§3) |
| 3 | Trust level | "Suggestions-only with an optional auto-apply (human key stroke)" | **CONFIRMED with amendment**: Suggestions-only stands; per-finding apply keystroke added, routed through the same fail-closed gate (§5.5, §5.7) |
| 4 | Sync execution keys | "Two key strokes for that: one for all and other with specific sync" | **CONFIRMED**: `S` = execute all (one confirm), `s` = execute selected; plan auto-loads on tab entry, freeing `s` (§2.2) |
| 5 | `w` deferral | "Wire it but not with lobster but with a custom harness for this; overall agentry agent actions must be throughout a custom harness..." | **SUPERSEDED**: no lobster generation; `w` returns in P4 as the harness action palette (§2.5, §1.3); the Agentry Harness (§1) is the new architectural headline |
| 6 | Onboarding + llmfit | "make it part of it however, make it simple to start with..." | **CONFIRMED as new scope**: onboarding wizard + llmfit-core library integration (§6) |

#### Still open (carried from revision 1, unanswered; renumbered 7–9 to continue the sequence — were rev-1 items 4, 6, 7)

7. **OpenClaw CLI subcommand** — `agentry openclaw workspaces` stays exactly as-is; the tab merge is TUI-only. Default: no breaking change.
8. **Version dead end** — rev-1 proposal stands as default: *wire* it (Enter installs the selected version, Esc cancels) rather than remove `v`.
9. **Auditor prompt delivery** — `agentry-role: auditor` frontmatter as the sync-exclusion marker, with P4 subagent-config delivery. Default: proceed with the marker.

#### New decisions created by this revision (need confirmation)

| # | Decision | Recommendation |
| --- | --- | --- |
| N1 | **llmfit integration mechanism** | Library dependency on `llmfit-core` (crates.io, MIT), feature-gated default-on, pinned to a minor line; binary fallback documented. The owner's "wrapping core functions / install the dependency" points at the lib |
| N2 | **Default host registry + priority chain** | Built-ins: claude-code, codex, gemini-cli, zai, fal, ollama. Default priority: claude-code → codex → gemini-cli → zai → ollama. fal seeded but excluded from the default auditor chain until its headless template is verified (P3 task) |
| N3 | **Onboarding trigger timing** | First TUI run without `~/.agents/agentry.toml` *offers* the wizard (declinable); `agentry setup` always available for updates |
| N4 | **Auto-apply trust boundary** | LLM-proposed fixes execute only after (a) deterministic gate validation — shell allowlist **plus path bounds for `FileWrite`/`FileRemove`** (§5.5) — at parse time and again at apply time, and (b) an explicit per-finding keystroke with an informed confirm surface (content size/preview for file writes). Batch apply never includes Audited findings. This shifts rev-1's "LLM output never executes" to "LLM output executes only through the identical human-consented gate" — confirm the boundary move |
| N5 | **Single config surface** | `~/.agents/agentry.toml` absorbs `AuditorConfig` (rev-1's `auditor.json` is superseded before anything shipped) and gains `[harness]`/`[hosts]`/`[local]`/`[onboarding]` sections |
| N6 | **`w` palette scope (P4)** | `w` opens a global harness action palette: run any registered action, run simple sequences; saved sequences live as `[workflows.*]` sections in `agentry.toml`. No `.lobster` generation. Confirm the palette scope vs a narrower "run predefined full-check sequence only" |

## Consequences

**Easier:**

- The tab bar matches the mental model: OpenClaw is a supported client in the Agents list, not a parallel universe
- Sync and Audit tabs do what their hints claim — the owner's "they do not implement the sync nor the audit fix" is resolved at the root, and ADR-001 §4.1 is finally complete
- **One execution model for everything agentry does**: sync, fix, audit, skills, and the auditor are all harness actions with one consent mechanism, one context-packaging path, and one registry — the TUI, CLI, and ACP worker cannot develop divergent behavior
- New host CLIs (and eventually new agent specs) are config additions, not code changes — the owner's "you are missing ollama, fal, zai" class of feedback becomes a config edit
- Key discoverability is structural: the keymap bar cannot drift from the handlers because both come from one table per tab
- New users get a working harness in minutes: onboarding detects, installs (with consent), sizes the machine via llmfit, and writes one config file
- The audit gains LLM reasoning (root-cause analysis, remediation quality) while the deterministic engine, scoring, and fix allowlist stay exactly as tested
- The auditor prompt is itself managed by agentry — discovered, drift-checked, and (in P4) synced like any prompt: agentry dogfoods its own core loop
- The skill-request loop establishes the allowlist-gated "skills are callable resources" pattern (Wobblus precedent) that any future first-party agentry agent can reuse
- The ACP file queue gets its first production consumer as a transport into the harness, making external invocation real instead of decorative

**More difficult:**

- The 5-tab index shift touches every `tab_index` literal and test fixture — mechanical but wide
- The keymap registry is now mandatory discipline: every new key must be added to its tab's table or it does not exist — which is the point, but it is discipline nonetheless
- The harness adds an indirection layer over engines that are already tested; the adapters must stay thin or the abstraction becomes a second implementation
- Two more crates, plus a new class of dependency: prompt-template versioning and tolerant response parsing need maintenance as host models drift
- `llmfit-core` couples agentry to a fast-moving upstream and an edition-2024 MSRV; pinned, feature-gated, and fallback-documented, but it is a real dependency
- Local-model hosts widen the parsing-quality spread; the tolerant parser and finding caps carry weight they did not have to carry for claude/codex/gemini alone
- Audit context leaves the machine on every cloud-host auditor invocation — consent UX and documentation are permanent obligations, not one-time setup (local runtimes reduce, not remove, this obligation)
- `~/.agents/` gains `agentry.toml` (harness/hosts/auditor/local/onboarding) and the canonical auditor prompt; both need migration handling if their schemas change
- The role-marker exclusion adds a rule to `plan_sync` that every future prompt-features change must respect
- Onboarding writes config and offers package-manager installs — a new class of side effect that needs the same conservative review the fix gate got