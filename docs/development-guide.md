# plugin-coding-pack — Development Guide

> Generated: 2026-03-27 | Scan Level: quick | Mode: full_rescan

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
export PULSE_LLM_MODEL=claude-sonnet-4-6         # Optional: Model override
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
| `cargo test` | Run all 210 tests |
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

# Parallel multi-reviewer code review
pulse run coding-parallel-review --config ./config \
  -i '{"target": "src/"}'
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

## Board Operations

```bash
# Get Kanban board data
pulse exec plugin-coding-pack -i '{"action": "board-data"}'

# List all epics
pulse exec plugin-coding-pack -i '{"action": "board-epics-list"}'

# Get epic detail
pulse exec plugin-coding-pack -i '{"action": "board-epics/{id}"}'

# Create a new epic
pulse exec plugin-coding-pack -i '{"action": "board-create-epic", "title": "...", "description": "..."}'

# Create a story under an epic
pulse exec plugin-coding-pack -i '{"action": "board-create-story", "epic_id": "...", "title": "..."}'
```

## Tool Provider

```bash
# List available MCP-style tools
pulse exec plugin-coding-pack -i '{"action": "list-tools"}'

# Invoke a tool
pulse exec plugin-coding-pack -i '{"action": "invoke-tool", "tool": "...", "params": {...}}'
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

### Running Tests

```bash
# Run all tests (210 tests)
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test plugin_health_check_returns_true

# Run only unit tests (fast)
cargo test --lib

# Run only integration tests
cargo test --test registration_tests
cargo test --test e2e_tests
cargo test --test e2e_executor_tests
```

### Test Coverage

**Rust tests** cover:
- **Unit tests** (`src/lib.rs`) — Plugin lifecycle, action dispatch, dashboard JSON validity, capability probe
- **Registration tests** (`tests/registration_tests.rs`) — Plugin registration, action routing, board actions, tool provider
- **E2E tests** (`tests/e2e_tests.rs`) — End-to-end plugin integration
- **Executor E2E tests** (`tests/e2e_executor_tests.rs`) — Workflow execution with retry loops, parallel steps, quality gates, context propagation, template variables, working directories, PR extraction

**Dashboard TypeScript tests** (`dashboard/tests/`):
- `coding-pack.test.ts` — Pack overview page validation
- `execute-workflow.test.ts` — Workflow execution form tests
- `scrum-board.test.ts` — Scrum board rendering
- `scrum-board-detail.test.ts` — Card detail popup
- `scrum-board-filters.test.ts` — Board filtering
- `atdd-scrum-board.test.ts` — Acceptance-driven board tests
- `board-tools-e2e.test.ts` — Board tools end-to-end tests

### Test Fixtures

Located in `tests/fixtures/`:
- `mock-plugins/` — Mock plugin executables (bmad-method, provider-claude-code, mock-slow-cmd, mock-test-runner)
- `sample-project/` — Minimal Rust project for testing workspace detection
- `workflows/` — 15 test workflow YAML definitions covering various execution patterns

## Project Structure Quick Reference

```
src/lib.rs              — Main plugin logic, trait impls
src/main.rs             — CLI entry point
src/executor.rs         — Workflow execution engine (DAG dispatch)
src/board.rs            — Scrum board actions
src/board_store.rs      — Board JSON persistence
src/tool_provider.rs    — MCP-style tool provisioning
src/pack.rs             — Pack management and validation
src/config_injector.rs  — Provider config injection
src/workspace.rs        — Workspace detection
src/agent_registry.rs   — Agent definition discovery
src/test_parser.rs      — Test result parsing
src/validator.rs        — Workflow/plugin validation
src/util.rs             — Utility functions
config/config.yaml      — Runtime configuration
config/workflows/       — Workflow YAML definitions (11 files)
plugin-packs/coding.toml — Pack manifest
dashboard/manifest.json — Dashboard page definitions (11 pages)
```
