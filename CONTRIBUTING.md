# Contributing to agentry

Thank you for your interest in contributing to agentry! This guide covers the development setup and contribution process.

## Development Setup

### Prerequisites

- Rust 1.75+ (stable)
- Git

### Build

```bash
git clone https://github.com/AndlerRL/agentry.git
cd agentry
cargo build
```

### Run Tests

```bash
cargo test              # All unit + integration tests
cargo test -p agentry-agents  # Single crate
cargo test --test integration_tests  # Integration tests only
```

### Lint & Format

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Project Structure

```
agentry/
├── crates/
│   ├── agentry-core/       # Data models, format converters, prompt discovery
│   ├── agentry-agents/     # Agent specs, detection, registry
│   ├── agentry-sync/       # Sync planner and executor
│   ├── agentry-skills/     # Skill hub, lockfile, installer
│   ├── agentry-openclaw/   # OpenClaw workspace discovery
│   ├── agentry-acp/        # ACP protocol and message router
│   └── agentry-tui/        # Terminal UI (binary crate)
└── tests/
    └── integration/        # Cross-crate integration tests
```

## Making Changes

1. Create a feature branch from `master`
2. Make your changes with appropriate tests
3. Ensure `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check` all pass
4. Open a pull request against `master`

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Formatting (no code changes)
- `refactor:` Code restructuring (no behavior changes)
- `test:` Adding or updating tests
- `chore:` Build process, CI, or tooling changes

## Code Style

- Follow `rustfmt` defaults (`cargo fmt`)
- Address all `clippy` warnings
- Add `#[cfg(test)] mod tests` sections to source files
- Use `anyhow::Result` in application code (TUI), custom error types in library crates
- Keep `#[allow(dead_code)]` to a minimum — prefer implementing or removing unused code

## Testing

- Unit tests go in the same file under `#[cfg(test)] mod tests`
- Integration tests go in `tests/integration/tests/`
- Use temp directories for filesystem-dependent tests (avoid reading real home directory)
- All tests must pass on both macOS and Linux

## Architecture Notes

- **Backward compatibility**: Must preserve existing `~/.agents/.skill-lock.json` v3 schema and symlink patterns
- **OpenClaw**: Workspace creation must redirect to `openclaw` CLI, never create dirs directly
- **Format converters**: Must support lossless round-tripping through `UnifiedPrompt`