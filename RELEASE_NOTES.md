# Release Notes — agentry v0.2.0

**Released:** 2026-08-29

## What's New

### Agent Audit

agentry can now answer "is this machine's agent setup healthy?" `agentry audit`
runs 24 diagnostic checks across 11 categories — installation, versions, config
files, prompt health, sync drift, cross-agent drift, skills, auth, orphaned
files, OpenClaw, and ACP. Each agent gets a health score (0-100) with a grade,
and every finding includes a remediation. `--fix` repairs what is safely
repairable (fail-closed shell allowlist, metacharacter injection refused).

`agentry audit --json` emits a stable, versioned report (`schema_version` 1)
with exit codes 0 (clean), 1 (critical findings), and 2 (error) — usable as a
gate in CI pipelines or agentic workflows.

### Self-evolving feedback loop

Audit runs append findings to `~/.agents/audit/history.jsonl`. Recurring
findings are promoted, dormant checks demoted, and new-check candidates
surfaced — deterministic rules, no LLM in the loop.

### TUI Audit tab

Press `6` in the TUI: run the audit on demand (`r`), filter by severity (`f`),
browse grouped findings with a detail panel, and see per-agent health bars.
The Agents tab now shows a health line per agent.

### Prompt dedup and TUI fixes

Prompt discovery dedups by (name, scope), preferring the canonical
`~/.agents/prompts/` store — this removes duplicate GEMINI entries and the
edit-hazard on synced copies. List selection offset bugs in the Prompts,
Skills, and Sync tabs are fixed, and the intro screen renders the full ASCII
logo without clipping.

## Upgrade

```bash
cargo install agentry-tui
```

Pre-built binaries: [GitHub Releases](https://github.com/AndlerRL/agentry/releases).

## Audit Quickstart

```bash
agentry audit              # full report with health scores
agentry audit --json       # machine-readable report (CI/agents)
agentry audit --fix --yes  # apply auto-fixable findings, no prompts
```

## Known Issues

- `agentry skills update` bug (pre-existing) and cross-distro install issues are under investigation

Full changelog: [CHANGELOG.md](CHANGELOG.md)