# plugin-coding-pack — Development Guide

## Prerequisites

| Requirement | Version | Purpose |
|-------------|---------|---------|
| Rust | 1.85+ | Core language |
| Pulse CLI | latest | Plugin host runtime |
| Claude Code CLI | latest | AI provider (`npm install -g @anthropic-ai/claude-code`) |
| Git | 2.20+ | Version control |
| Node.js / npx | 18+ | For GitNexus memory provider |
| Anthropic API Key | — | Required for Claude Code |

## Environment Setup

### Environment Variables

```bash
export PULSE_DB_PATH=sqlite:pulse.db?mode=rwc   # Required: SQLite connection
export PULSE_LLM_PROVIDER=anthropic              # Optional: LLM provider
export PULSE_LLM_MODEL=claude-sonnet-4-6            # Optional: Model override
```

### Initial Setup

```bash
# Clone and enter the project
cd bmad-method-flow

# Build this plugin
cargo build --release

# Build sibling plugins
for d in provider-claude-code git-ops git-worktree bmad-method; do
  (cd ../pulse-plugins/$d && cargo build --release)
done

# Install all binaries
./install.sh

# Or skip build and just copy binaries
./install.sh --skip-build

# Validate setup
PULSE_DB_PATH=sqlite:pulse.db?mode=rwc \
  pulse registry validate --config ./config
```

## Build Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Debug build |
| `cargo build --release` | Release build |
| `cargo test` | Run all 16 unit tests |
| `./install.sh` | Full build + install all plugin binaries |
| `./install.sh --skip-build` | Install without rebuilding |
| `./uninstall.sh` | Remove installed binaries |

## Running Workflows

All workflows are executed via `pulse run`:

```bash
# Quick development (most common, 3 steps)
pulse run coding-quick-dev --config ./config \
  -i '{"input": "Add input validation to login endpoint"}'

# Full feature development (5 steps)
pulse run coding-feature-dev --config ./config \
  -i '{"input": "Implement user notification system"}'

# Story-driven development (6 steps)
pulse run coding-story-dev --config ./config \
  -i '{"input": "As a user, I want to export CSV reports"}'

# Bug fix (4 steps)
pulse run coding-bug-fix --config ./config \
  -i '{"input": "GET /api/profile returns 500 when user_id is null"}'

# Refactoring (4 steps)
pulse run coding-refactor --config ./config \
  -i '{"input": "Extract UserService DB operations into Repository pattern"}'

# Code review (3 steps)
pulse run coding-review --config ./config \
  -i '{"target": "src/auth/"}'
```

## Plugin Management

```bash
# Check pack health
pulse exec plugin-coding-pack -i '{"action": "status"}'

# Validate all plugins
pulse exec plugin-coding-pack -i '{"action": "validate-pack"}'

# List workflows
pulse exec plugin-coding-pack -i '{"action": "list-workflows"}'

# List installed plugins
pulse exec plugin-coding-pack -i '{"action": "list-plugins"}'
```

## Bootstrap (Self-Evolution) Workflows

```bash
# Develop a single plugin
pulse run bootstrap-plugin --config ./config \
  -i '{"input": "Add step dependency cycle detection to validator"}'

# Rebuild all plugins
pulse run bootstrap-rebuild --config ./config

# Full self-evolution cycle (8 steps: plan -> implement -> test -> review -> rebuild -> install -> validate -> commit)
pulse run bootstrap-cycle --config ./config \
  -i '{"input": "Refactor pack.rs error handling with thiserror"}'
```

## Testing

### Unit Tests

```bash
# Run all tests (16 tests)
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test plugin_health_check_returns_true
```

### Test Coverage

Tests in `src/lib.rs` cover:
- Plugin lifecycle (health check, info, dependencies)
- Action dispatch (validate-pack, list-workflows, list-plugins)
- Error handling (unknown action, missing input)
- Capability probe
- Dashboard JSON validity (pages, API routes, display customizations)

Dashboard tests in `dashboard/tests/`:
- `coding-pack.test.ts` — Pack-level tests
- `execute-workflow.test.ts` — Workflow execution tests

## Project Structure Quick Reference

```
src/lib.rs          — Main plugin logic + 16 unit tests
src/main.rs         — CLI entry point
src/pack.rs         — Action dispatch and pack validation
src/validator.rs    — Workflow/plugin validation
src/util.rs         — Utility functions
config/config.yaml  — Runtime configuration
plugin-packs/coding.toml — Pack manifest
```
