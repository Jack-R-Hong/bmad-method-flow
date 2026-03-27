# plugin-coding-pack — Architecture

> Generated: 2026-03-27 | Scan Level: quick | Mode: full_rescan

## Overview

`plugin-coding-pack` is a Pulse platform meta-plugin built in Rust. It acts as an orchestration layer that validates, loads, and coordinates multiple sibling plugins to deliver AI-driven development workflows, a Scrum board for project tracking, and MCP-style tool provisioning for LLM integration.

```
+------------------------------------------------------------+
|                   plugin-coding-pack                        |
|                     (orchestrator)                          |
|                                                             |
|  ┌─────────────┐  ┌────────────────────┐                   |
|  │ bmad-method  │  │ provider-claude-   │                   |
|  │  (required)  │  │   code (required)  │                   |
|  │ 10 AI agents │  │ Claude Code CLI    │                   |
|  └─────────────┘  └────────────────────┘                   |
|  ┌─────────────┐  ┌────────────────────┐                   |
|  │  git-ops    │  │  git-worktree      │                   |
|  │  (optional) │  │  (optional)        │                   |
|  └─────────────┘  └────────────────────┘                   |
|  ┌─────────────┐  ┌────────────────────┐                   |
|  │  git-pr     │  │  plugin-memory     │                   |
|  │  (optional) │  │  GitNexus (opt)    │                   |
|  └─────────────┘  └────────────────────┘                   |
+------------------------------------------------------------+
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
| `AgentDefinitionProvider` | Agent discovery | `list_agent_definitions()`, `get_agent_definition(id)` |

## Source Modules

| File | Size | Purpose |
|------|------|---------|
| `src/lib.rs` | 20K | Plugin struct, trait impls, dashboard JSON, action routing |
| `src/main.rs` | 8K | Binary entry point, CLI argument handling |
| `src/executor.rs` | 78K | Workflow execution engine — DAG dispatch, retry loops, parallel steps, quality gates |
| `src/board.rs` | 41K | Scrum/Kanban board actions — epic/story/task CRUD, status transitions |
| `src/board_store.rs` | 45K | Board data persistence — JSON file-based store with CRUD operations |
| `src/tool_provider.rs` | 24K | MCP-style tool provisioning — tool definitions, parameter schemas, invocations |
| `src/test_parser.rs` | 25K | Test result parsing — structured output extraction from test runners |
| `src/pack.rs` | 22K | Pack orchestration — plugin validation, workflow listing, action dispatch |
| `src/config_injector.rs` | 14K | Config injection — provider config template rendering into workspace |
| `src/validator.rs` | 14K | Pack validation — workflow YAML parsing, plugin binary checks |
| `src/workspace.rs` | 12K | Workspace detection — project root discovery, config file resolution |
| `src/agent_registry.rs` | 11K | Agent discovery — scans workspace for BMAD agent/skill definitions |
| `src/util.rs` | <1K | Utility functions (is_executable, etc.) |

## Action Dispatch

The `StepExecutorPlugin::execute` method routes JSON input to action handlers organized by domain:

### Pack Management Actions
| Action | Handler | Description |
|--------|---------|-------------|
| `validate-pack` | `pack.rs` | Validate all plugin binaries and workflows |
| `validate-workflows` | `pack.rs` | Validate workflow YAML files |
| `list-workflows` | `pack.rs` | List registered workflows |
| `list-plugins` | `pack.rs` | List installed plugins |
| `status` | `pack.rs` | Pack health and validation summary |
| `__probe__` | `lib.rs` | Capability probe (returns `probe_ok`) |

### Workflow Execution Actions
| Action | Handler | Description |
|--------|---------|-------------|
| `execute-workflow` | `executor.rs` | Execute a complete workflow pipeline |
| `execute-step` | `executor.rs` | Execute a single workflow step |
| `get-workflow-context` | `executor.rs` | Retrieve workflow execution context |

### Board Actions
| Action | Handler | Description |
|--------|---------|-------------|
| `board-data` | `board.rs` | Get Kanban board data (assignments by status) |
| `board-filters` | `board.rs` | Get available board filter definitions |
| `board-assignments/{id}` | `board.rs` | Get assignment detail with tasks/comments |
| `board-epics-list` | `board.rs` | List all epics with progress |
| `board-epics/{id}` | `board.rs` | Get epic detail with stories |
| `board-create-epic` | `board.rs` | Create a new epic |
| `board-update-epic` | `board.rs` | Update epic fields |
| `board-create-story` | `board.rs` | Create a story under an epic |
| `board-update-story` | `board.rs` | Update story fields/status |

### Tool Provider Actions
| Action | Handler | Description |
|--------|---------|-------------|
| `list-tools` | `tool_provider.rs` | List available MCP-style tools |
| `invoke-tool` | `tool_provider.rs` | Invoke a tool with parameters |
| `get-tool-schema` | `tool_provider.rs` | Get tool parameter schema |

### Configuration Actions
| Action | Handler | Description |
|--------|---------|-------------|
| `inject-config` | `config_injector.rs` | Inject config into provider templates |
| `detect-workspace` | `workspace.rs` | Detect workspace root and configuration |
| `list-agents` | `agent_registry.rs` | List discovered agent definitions |
| `get-agent` | `agent_registry.rs` | Get specific agent definition |

## Dashboard Extension

The plugin exposes an 11-page dashboard via the Pulse SDK manifest system:

| Page | Layout Type | Purpose |
|------|-------------|---------|
| Overview | detail | Pack health, workflow count, plugin list |
| Task Board | board (Kanban) | Assignments by status with epic swimlanes |
| Epics | table | All epics with stories and progress bars |
| Workflows | table | Browse all workflows with execute/view actions |
| AI Agents | table | BMAD agent roster and roles |
| Pack Status | detail | Validation results, plugin health |
| Execute | form | Trigger workflow execution with parameters |
| Logs | stream | Real-time SSE execution events |
| Workflow Detail | detail | Step pipeline, parallel groups, execution history |
| Assignment Detail | detail | Task checklist, comments thread |
| Epic Detail | detail | Stories breakdown, completion progress |

### API Routes

All routes prefixed with `/api/v1/plugin-coding-pack`:

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/status` | Pack health and validation |
| GET | `/status/health` | Health badge data |
| GET | `/workflows/list` | All workflows as table data |
| GET | `/workflows/{id}` | Workflow detail with steps |
| POST | `/workflows/{id}/execute` | Trigger workflow execution |
| POST | `/workflows/execute` | Execute workflow from form |
| GET | `/agents/list` | BMAD agent roster |
| GET | `/agents/{id}` | Agent detail |
| GET | `/executions/stream` | SSE execution event stream |
| GET | `/board/data` | Kanban board data |
| GET | `/board/filters` | Board filter definitions |
| GET | `/board/assignments/{id}` | Assignment detail |
| GET | `/board/epics/list` | All epics listing |
| GET | `/board/epics/{id}` | Epic detail with stories |
| GET | `/tasks/{task_id}/workflow-context` | Task workflow context |
| GET | `/tasks/{task_id}/agent-info` | Task agent info |

## Workflow System

Workflows are defined as YAML files in `config/workflows/` and declared in `plugin-packs/coding.toml`:

### Coding Workflows
- **coding-quick-dev** (3 steps) — Small features, quick changes
- **coding-feature-dev** (5 steps) — Full feature development with architecture design
- **coding-story-dev** (6 steps) — User story-driven development
- **coding-bug-fix** (4 steps) — Root cause analysis and fix
- **coding-refactor** (4 steps) — Safe incremental refactoring
- **coding-review** (3 steps) — Parallel adversarial + edge case review
- **coding-parallel-review** — Multi-reviewer parallel code review

### Bootstrap Workflows
- **bootstrap-plugin** (5 steps) — Develop a single plugin
- **bootstrap-rebuild** (3 steps) — Rebuild all plugins
- **bootstrap-cycle** (8 steps) — Full self-evolution cycle

### Utility Workflows
- **coding-memory-index** — Re-index knowledge graph

### Execution Engine Features
- DAG-based step dispatch with dependency resolution
- Retry loops with configurable backoff
- Parallel step execution
- Quality gates with pass/fail criteria
- Context propagation between steps
- PR extraction and pipeline integration
- Template variable substitution

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

### Provider Config Templates
Located in `config/provider-configs/_default/`:
- `AGENT.md` — Agent persona template
- `RULE.md` — Rule definition template
- `SKILL.md` — Skill definition template

These templates are injected into workspace provider configurations by `config_injector.rs`.

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
- `reqwest` (non-wasm) — HTTP client for external calls
- `tokio` + `async-trait` (non-wasm) — Async runtime
- `wit-bindgen` (wasm32 only) — WASM component model bindings

### Dev Dependencies
- `pulse-plugin-test` (with `wasm-harness` feature) — Integration test harness
- `tokio` (full features) — Async test runtime
- `tempfile` — Temporary file handling in tests
- `serde_json` — Test assertions

## Testing Strategy

- **210 tests** total across Rust and TypeScript
- **Unit tests** in `src/lib.rs` — Plugin lifecycle, action dispatch, dashboard JSON validity
- **Integration tests** in `tests/registration_tests.rs` — Plugin registration, action routing
- **E2E tests** in `tests/e2e_executor_tests.rs` — Workflow execution end-to-end (23+ tests)
- **E2E module** in `tests/e2e/mod.rs` — Test harness for full pipeline testing
- **Dashboard tests** in `dashboard/tests/` (TypeScript) — 8 test files covering:
  - Pack overview, workflow execution, scrum board, card details, filters, ATDD board tests, board tools E2E
- **Test fixtures** — 15 workflow YAML definitions, 4 mock plugin executables, sample project
- **Test command**: `cargo test`
