# plugin-coding-pack — Architecture

## Overview

`plugin-coding-pack` is a Pulse platform meta-plugin built in Rust. It acts as an orchestration layer that validates, loads, and coordinates multiple sibling plugins to deliver AI-driven development workflows.

```
+---------------------------------------------------+
|              plugin-coding-pack                     |
|                (orchestrator)                        |
|                                                     |
|  +---------------+  +--------------------+          |
|  | bmad-method   |  | provider-claude-   |          |
|  |  (required)   |  |   code (required)  |          |
|  | 10 AI agents  |  | Claude Code CLI    |          |
|  +---------------+  +--------------------+          |
|  +---------------+  +--------------------+          |
|  |  git-ops      |  | plugin-git-        |          |
|  |  (optional)   |  |  worktree (opt)    |          |
|  +---------------+  +--------------------+          |
|  +--------------------+                             |
|  | plugin-memory (opt) |                            |
|  | GitNexus / Greptile  |                           |
|  +--------------------+                             |
+---------------------------------------------------+
```

## Architecture Pattern

**Plugin-based orchestrator with WASM component model**

The crate produces two artifacts:
1. **Binary** (`src/main.rs`) — CLI entry point for direct execution
2. **Library** (`src/lib.rs`, cdylib + rlib) — WASM-compatible plugin for the Pulse runtime

The plugin system uses Rust traits from `pulse-plugin-sdk` to define a standard interface:

### Trait Implementations

| Trait | Purpose | Key Methods |
|-------|---------|-------------|
| `PluginLifecycle` | Identity and health | `get_info()`, `health_check()` |
| `StepExecutorPlugin` | Action dispatch | `execute(task, config)` |
| `DashboardExtensionPlugin` | UI integration | `get_pages_json()`, `get_api_routes_json()`, `get_display_customizations_json()` |

## Source Modules

| File | Purpose |
|------|---------|
| `src/lib.rs` | Plugin struct, trait impls, dashboard JSON, tests (16 unit tests) |
| `src/main.rs` | Binary entry point |
| `src/pack.rs` | Pack validation logic, action dispatch (`CodingPackInput`, `execute_action`) |
| `src/validator.rs` | Workflow and plugin validation logic |
| `src/util.rs` | Utility functions (`is_executable`) |

## Action Dispatch

The `StepExecutorPlugin::execute` method routes JSON input to action handlers:

| Action | Description |
|--------|-------------|
| `validate-pack` | Validate all plugin binaries and workflows |
| `validate-workflows` | Validate workflow YAML files |
| `list-workflows` | List registered workflows |
| `list-plugins` | List installed plugins |
| `status` | Pack health and validation summary |
| `__probe__` | Capability probe (returns `probe_ok`) |

## Dashboard Extension

The plugin exposes a 7-page dashboard via the Pulse SDK manifest system:

| Page | Layout Type | Purpose |
|------|-------------|---------|
| Overview | detail | Pack health, workflow count, plugin list |
| Workflows | table | Browse all workflows with execute/view actions |
| Workflow Detail | detail | Step pipeline, execution history |
| AI Agents | table | BMAD agent roster |
| Pack Status | detail | Validation results, plugin health |
| Execute | form | Trigger workflow execution |
| Logs | stream | Real-time SSE execution events |

### API Routes

All routes prefixed with `/api/v1/plugin-coding-pack`:

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/status` | Pack health and validation |
| GET | `/status/health` | Health badge data |
| GET | `/workflows/list` | All workflows as table data |
| GET | `/workflows/{id}` | Workflow detail with steps |
| POST | `/workflows/{id}/execute` | Trigger workflow execution |
| GET | `/agents/list` | BMAD agent roster |
| GET | `/agents/{id}` | Agent detail |
| GET | `/executions/stream` | SSE execution event stream |
| GET | `/tasks/{task_id}/workflow-context` | Task workflow context |
| GET | `/tasks/{task_id}/agent-info` | Task agent info |

## Workflow System

Workflows are defined as YAML files in `config/config/workflows/` and declared in `plugin-packs/coding.toml`:

### Coding Workflows
- **coding-quick-dev** (3 steps) — Small features, quick changes
- **coding-feature-dev** (5 steps) — Full feature development with architecture design
- **coding-story-dev** (6 steps) — User story-driven development
- **coding-bug-fix** (4 steps) — Root cause analysis and fix
- **coding-refactor** (4 steps) — Safe incremental refactoring
- **coding-review** (3 steps) — Parallel adversarial + edge case review

### Bootstrap Workflows
- **bootstrap-plugin** (5 steps) — Develop a single plugin
- **bootstrap-rebuild** (3 steps) — Rebuild all plugins
- **bootstrap-cycle** (8 steps) — Full self-evolution cycle

## Configuration

### config/config.yaml
```yaml
db_path: "pulse.db"          # SQLite database
log_level: "info"             # Logging level
plugin_dir: "config/plugins"  # Binary location
memory:
  provider: gitnexus          # gitnexus | greptile | none
  auto_reindex: true
```

### Environment Variables
| Variable | Purpose |
|----------|---------|
| `PULSE_DB_PATH` | SQLite connection string (e.g., `sqlite:pulse.db?mode=rwc`) |
| `PULSE_LLM_PROVIDER` | LLM provider override (optional) |
| `PULSE_LLM_MODEL` | Model override (optional) |

## Dependencies

### Runtime Dependencies
- `pulse-plugin-sdk` — Plugin trait definitions and types
- `serde` / `serde_json` / `serde_yaml` — Serialization
- `tracing` — Structured logging
- `wit-bindgen` (wasm32 only) — WASM component model bindings

### Dev Dependencies
- `pulse-plugin-test` (with `wasm-harness` feature) — Integration test harness
- `tokio` (full features) — Async test runtime
- `tempfile` — Temporary file handling in tests
- `serde_json` — Test assertions

## Testing Strategy

- 16 unit tests in `src/lib.rs` covering:
  - Plugin lifecycle (health check, info, dependencies)
  - Action dispatch (validate-pack, list-workflows, list-plugins, unknown action, missing input)
  - Capability probe
  - Dashboard JSON validity (pages, API routes, display customizations)
- Dashboard TypeScript tests in `dashboard/tests/`
- Test command: `cargo test`
