# Release Notes — agentry v0.2.1

**Released:** 2026-09-02

## What's New

### Agent Auditor

The headline of this release: agentry now has its own agent. `agentry auditor
review` runs the deterministic audit, then hands the report to a headless
agent CLI (claude-code, codex, gemini-cli, or ollama — config-driven host
registry, zai seeded, fal excluded) for root-cause analysis and remediation
proposals.

Auditor findings arrive as Suggestions only. A proposed fix is quarantined at
parse time, validated against the same fail-closed gate as deterministic
fixes, and applies only with an explicit per-finding keystroke. Batch apply
never touches Audited findings.

### TUI restructure

Six tabs became five: OpenClaw is a supported client, not a category, so it
merged into the Agents tab detail. The bottom two rows are now a persistent
keymap bar — always visible, always current, rendered from the same registry
that dispatches keys, so the bar, the handlers, and the help overlay cannot
drift.

The Sync and Audit tabs finally do what their hints always claimed. The sync
plan auto-loads on entry; `s` executes the selected mapping, `S` executes all.
The audit auto-runs on first entry; `a` applies the selected finding's fix,
`A` applies all fixable, and `l`/`L` invoke the auditor on a finding — an
explicit keypress, so LLM egress is user-initiated every time.

Underneath, every mutation flows through the Agentry Harness — one gated
action registry, one consent ledger (`~/.agents/agentry/consent.jsonl`).

## Upgrade

```bash
cargo install agentry-tui
```

Or the installer script: `curl -LsSf https://github.com/AndlerRL/agentry/releases/latest/download/agentry-tui-installer.sh | sh`

## Auditor Quickstart

```bash
agentry auditor setup    # write config + canonical prompt (idempotent)
agentry auditor review   # audit, then LLM review, merged report
```

In the TUI: Audit tab, select a finding, `l` to review with the auditor, `a`
to apply a gate-validated suggested fix.

## Known Issues

- Nothing blocking. On the roadmap for P4: onboarding wizard with llmfit,
  the ACP worker (`agentry auditor serve`), and the `w` harness palette.

Full changelog: [CHANGELOG.md](CHANGELOG.md)
