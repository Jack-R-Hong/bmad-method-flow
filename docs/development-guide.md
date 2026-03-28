# plugin-coding-pack — Development Guide

> Generated: 2026-03-28 | Scan Level: exhaustive | Mode: full_rescan

## Prerequisites

| Requirement | Version | Notes |
|-------------|---------|-------|
| Rust | 1.85+ | MSRV specified in Cargo.toml |
| Cargo | Latest stable | Comes with Rust |
| Pulse CLI | Latest | [pulsate-labs/pulse](https://github.com/pulsate-labs/pulse) |
| Node.js | 18+ | For Playwright dashboard tests |
| npm/npx | Latest | For Playwright test runner |

Optional:
- `wasm32-wasi` target (`rustup target add wasm32-wasi`) for WASM builds
- SQLite CLI for inspecting `pulse.db`

## Installation

```bash
# Clone the repository
git clone <repo-url> pulse-plugins/bmad-method-flow
cd pulse-plugins/bmad-method-flow

# Full install: build this plugin + all sibling plugins
./install.sh

# Skip build, only sync binaries (if already built)
./install.sh --skip-build

# Uninstall
./uninstall.sh
```

The install script builds and copies binaries for:
- plugin-coding-pack (this crate)
- provider-claude-code
- git-ops
- git-worktree
- bmad-method

All binaries are placed in `config/plugins/`.

## Build

```bash
# Debug build
cargo build

# Release build
cargo build --release

# WASM build (requires wasm32-wasi target)
cargo build --target wasm32-wasi
```

### Build Artifacts

| Target | Output | Path |
|--------|--------|------|
| Native (debug) | `plugin-coding-pack` binary + `libplugin_coding_pack.so/dylib` | `target/debug/` |
| Native (release) | Same, optimized | `target/release/` |
| WASM | `plugin_coding_pack.wasm` | `target/wasm32-wasi/` |

## Running

### As Binary (JSON-RPC stdio mode)

```bash
# Direct execution — reads JSON-RPC from stdin, writes to stdout
cargo run

# With workspace override
PULSE_WORKSPACE_DIR=/path/to/workspace cargo run
```

### As Plugin (server mode)

When loaded by Pulse's plugin-loader, `register()` is called which returns a `PluginRegistration` containing:
- `HookPoint::ConfigInjector` (BmadAgentInjector)
- `HookPoint::ToolProvider` (BmadToolProvider)
- `HookPoint::AgentDefinitionProvider` (BmadAgentRegistry)

## Configuration

### Main Config (`config/config.yaml`)

```yaml
db_path: "pulse.db"
log_level: "info"
plugin_dir: "config/plugins"

memory:
  provider: gitnexus    # gitnexus | greptile | none
  auto_reindex: true

# Optional settings:
# workflows:
#   enabled: [coding-quick-dev, coding-bug-fix]  # whitelist
#   disabled: [bootstrap-cycle]                    # blacklist (takes priority)
# defaults:
#   default_model: "fast"
#   max_budget_usd: 5.0
# use_injection_pipeline: true
# auto_dev:
#   max_retries: 3
#   max_tasks: 5
#   skip_validation: false
# github_sync:
#   filter_labels: [auto-dev]
#   filter_milestone: "Sprint 5"
#   review_poll_interval_secs: 120
# agent_mesh:
#   enabled: true
#   max_depth: 3
#   agents_yaml_path: "custom/agents.yaml"
```

### Auto-Loop Config (`config/auto-loop.yaml`)

Maps task labels to workflow IDs for automatic routing:

```yaml
routing:
  - label: "story"     → workflow: "coding-story-dev"
  - label: "bug"       → workflow: "coding-bug-fix"
  - label: "refactor"  → workflow: "coding-refactor"
  - label: "quick"     → workflow: "coding-quick-dev"
  - label: "feature"   → workflow: "coding-feature-dev"
  - label: "review"    → workflow: "coding-review"
  - label: "pr-fix"    → workflow: "coding-pr-fix"
  - default: "coding-quick-dev"
max_retries: 3
validation_enabled: true
poll_interval_secs: 300
```

### Workspace Resolution

Priority order:
1. Explicit `workspace_dir` parameter in action input
2. `PULSE_WORKSPACE_DIR` environment variable
3. Inferred from binary path (`{workspace}/config/plugins/plugin-coding-pack`)
4. Current directory (`.`)

## Testing

### Rust Tests

```bash
# Run all tests (unit + integration + e2e)
cargo test

# Run specific test module
cargo test --test registration_tests
cargo test --test e2e_tests

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test test_name
```

### Dashboard E2E Tests (Playwright)

```bash
cd dashboard

# Install Playwright browsers (first time)
npx playwright install

# Run all tests
npx playwright test

# Run with UI
npx playwright test --ui

# Run specific test file
npx playwright test tests/scrum-board.test.ts
```

### Test Structure

| Type | Location | Count | Description |
|------|----------|-------|-------------|
| Unit | `src/*.rs` (inline) | ~120+ | Per-module: config parsing, validation, CSV parsing, action dispatch |
| Integration | `tests/registration_tests.rs` | ~25 | SDK PluginRegistry: register(), injection pipeline, tool dispatch |
| E2E (Rust) | `tests/e2e_tests.rs`, `tests/e2e_executor_tests.rs` | ~15 | Workflow execution end-to-end |
| E2E (Dashboard) | `dashboard/tests/*.test.ts` | 7 files | Dashboard endpoint responses, board operations |
| Fixtures | `tests/fixtures/` | 15 workflows | Happy path, failure paths, timeouts, retries, parallel steps |

### Test Fixtures

Located at `tests/fixtures/`:
- `sample-project/` — Minimal Cargo.toml + lib.rs for testing
- `mock-plugins/` — Stub plugin binaries (bmad-method, provider-claude-code, mock-test-runner, mock-slow-cmd)
- `workflows/` — 15 test workflow YAMLs covering function-only, parallel, optional skip, required fail, timeout, template vars, agent mock, quality gate, retry loop, context flow, missing plugin, working dir, command resolution, PR extraction, worktree extract, autodev simple, autodev with tests

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `PULSE_API_PORT` | 8080 | Port for Pulse API HTTP fallback |
| `PULSE_WORKSPACE_DIR` | — | Override workspace root directory |
| `GREPTILE_API_KEY` | — | API key for Greptile memory provider |

## Key Development Patterns

### Adding a New Pack Action
1. Add action string match in `pack::execute_action()`
2. Implement the handler function
3. If delegated: add bridge function in `plugin_bridge.rs`
4. Update the "Available" error message in the unknown action handler
5. Add test in `pack::tests`

### Adding a New LLM Tool
1. Add constant in `tool_provider.rs` (e.g., `const TOOL_NEW: &str = "bmad_new_tool"`)
2. Add mapping in `tool_name_to_action()` if wrapping a pack action
3. Add `ToolDef` to `available_tools()` with appropriate sensitivity
4. Handle in `execute_tool()` if special logic needed
5. Add tests

### Adding a New Dashboard Data Endpoint
1. Add route match in `pack::execute_data_query()`
2. Implement `*_value()` function returning `serde_json::Value`
3. Add mock response in `dashboard/mock-responses/`
4. Add Playwright test in `dashboard/tests/`

### Adding a New BMAD Agent
1. Add row to `_bmad/_config/agent-manifest.csv`
2. Update test assertions for agent count (currently 9)
3. If special ACL needed: add match arm in `agent_registry::get_acl()`
