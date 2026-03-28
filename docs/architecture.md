# plugin-coding-pack — Architecture

> Generated: 2026-03-28 | Scan Level: exhaustive | Mode: full_rescan

## Overview

plugin-coding-pack is a Pulse meta-plugin that orchestrates an AI-driven software development workflow. It coordinates sibling plugins (bmad-method, provider-claude-code, git-worktree, and others) and exposes BMAD methodology agents as first-class entities within the Pulse ecosystem.

The plugin operates in two modes:
- **Server mode**: Registers capabilities (ConfigInjector, ToolProvider, AgentDefinitionProvider) with Pulse's PluginRegistry via `register()`
- **CLI/stdio mode**: Runs as a binary with a JSON-RPC stdio adapter dispatching 16 methods via `dispatch_combined()`

## Architecture Pattern

**Plugin architecture with capability-based registration and delegation to platform plugins.**

The crate produces three artifacts:
- `cdylib` — Dynamic library for Pulse server plugin loading
- `rlib` — Rust library for direct linking
- `bin` — Standalone binary for JSON-RPC stdio communication

## Technology Stack

| Category | Technology | Version | Purpose |
|----------|-----------|---------|---------|
| Language | Rust | Edition 2021, MSRV 1.85 | Systems-level plugin development |
| Plugin SDK | pulse-plugin-sdk | local | Core plugin framework (traits, types, registry) |
| Serialization | serde + serde_json + serde_yaml | 1.0 / 0.9 | Config parsing, JSON-RPC, YAML workflows |
| Async Runtime | tokio | 1.40 | Async I/O for native targets |
| HTTP Client | reqwest | 0.12 (blocking) | Fallback HTTP communication with Pulse API |
| WASM Bindings | wit-bindgen | 0.53 | WebAssembly component model (wasm32 target) |
| Logging | tracing | 0.1 | Structured logging |
| Database | SQLite (pulse.db) | — | Persistent task and board storage |
| Testing | Playwright | ^1.52.0 | Dashboard E2E tests |
| Methodology | BMAD | v6.2.0 | Dev workflow framework with 9 agent personas |

## Core Components

### 1. CodingPackPlugin (lib.rs)

The main plugin struct implementing three Pulse SDK traits:
- **PluginLifecycle** — `get_info()` (metadata + 5 dependencies), `health_check()` (verifies plugin binaries exist and are executable)
- **StepExecutorPlugin** — `execute()` dispatches to `pack::execute_action()`, handles `__probe__` capability probes
- **DashboardExtensionPlugin** — `get_pages_json()` (7 pages), `get_api_routes_json()` (21+ endpoints), `get_display_customizations_json()` (4 customizations)

### 2. Pack Action Dispatcher (pack.rs — 1,248 LOC)

Central routing for 18 actions across 4 categories:

**Local operations:**
- `validate-pack` — Check required/optional plugin binaries and workflow files
- `validate-workflows` — Run structural validation on all YAML workflows
- `list-workflows` — List available workflows respecting filter rules
- `list-plugins` — Inventory installed plugins with size and executable status
- `status` — Combined pack validation + workflow list + plugin list
- `data-query` — Dashboard proxy: routes endpoint paths to internal data functions (status, health, workflows/list, agents/list, board/summary, tasks/*/workflow-context, tasks/*/agent-info, workflows/*)
- `data-mutate` — Dashboard proxy for mutations (board mutations moved to plugin-board)
- `generate-agents-yaml` — Generate agents.yaml ACL config from registry

**Delegated to plugin-auto-loop:**
- `execute-workflow` — Trigger workflow execution via Pulse engine
- `auto-dev-status` — Current auto-dev loop / board status
- `auto-dev-next` — Pick next ready-for-dev task and run its workflow
- `auto-dev-watch` — Process multiple tasks in watch mode

**Delegated to other platform plugins:**
- `sync-github-issues` — via plugin-issue-sync
- `cleanup-worktrees`, `worktree-status`, `recover-worktrees` — via plugin-workspace-tracker
- `check-pr-reviews`, `build-fix-context` — via plugin-feedback-loop

### 3. Plugin Bridge (plugin_bridge.rs)

Thin HTTP/RPC bridge replacing direct module calls with delegation to platform plugins:
- **Server mode**: Tries `call_capability()` first via SDK host functions
- **CLI mode**: Falls back to HTTP POST to Pulse API (`http://127.0.0.1:{PULSE_API_PORT}/api/v1/...`)
- **WASM mode**: Returns error (bridge not available)

Delegates to 5 platform plugins:
- `plugin-auto-loop` — Task pickup, workflow dispatch, validation
- `plugin-issue-sync` — GitHub issue synchronization
- `plugin-feedback-loop` — PR review feedback processing
- `plugin-workspace-tracker` — Worktree lifecycle management
- `plugin-trigger-cron` — Scheduled triggering

### 4. BmadAgentInjector (config_injector.rs)

ConfigInjector that loads agent personas from `_bmad/_config/agent-manifest.csv`:
- Parses CSV with custom multi-line quote handling (`split_csv_rows`, `parse_csv_row`)
- Caches 9 agent personas in memory (HashMap keyed by `bmad/{name}`)
- `applies_to()` matches only `bmad/` prefixed agent names present in manifest
- `provide_injections()` returns 2 injections per agent:
  1. System prompt (identity + communication style + role) at priority 100, prepended
  2. Principles at priority 110, appended

### 5. BmadToolProvider (tool_provider.rs)

ToolProvider exposing 6 LLM-callable tools:
- `bmad_validate_pack` (Low sensitivity)
- `bmad_list_workflows` (Low)
- `bmad_list_plugins` (Low)
- `bmad_data_query` (Low) — requires `endpoint` parameter
- `bmad_data_mutate` (Medium) — requires `endpoint`, optional `payload`
- `bmad_auto_dev_next` (High) — delegates to plugin-auto-loop

### 6. BmadAgentRegistry (agent_registry.rs)

AgentDefinitionProvider for workspace-based discovery:
- Loads same CSV manifest as BmadAgentInjector
- Maps agents to `SdkAgentDefinition` with skills from CSV `capabilities` column
- Sorted alphabetically for deterministic ordering
- ACL rules (static, architecture-defined):
  - `bmad/architect` can invoke: analyst, developer, ux-designer
  - `bmad/qa` can invoke: developer only
  - `bmad/quick-flow-solo-dev` can invoke: none (solo agent)
  - All others default to: can invoke developer
  - All agents can respond to: pm, sm

### 7. Workspace Configuration (workspace.rs)

Resolution priority: explicit `workspace_dir` > `PULSE_WORKSPACE_DIR` env > inferred from binary path > "."

Key settings:
- `WorkflowFilter` — enabled/disabled workflow lists (disabled takes priority)
- `DefaultSettings` — default_model, max_budget_usd, memory provider
- `AutoDevConfig` — max_retries (1), max_tasks (10), skip_validation
- `GitHubSyncConfig` — filter_labels, filter_milestone, review_poll_interval_secs (60)
- `AgentMeshSettings` — enabled, max_depth (5), agents_yaml_path

### 8. Validator (validator.rs)

Structural validation for two YAML formats:

**Workflow validation:**
- Required fields: name, version, non-empty steps array
- Per-step: id required, agent steps need system_prompt, executor binary must exist
- Session steps: minimum 2 participants with bmad/ prefix, valid convergence strategy (fixed_turns/unanimous/stagnation)
- DAG validation: depends_on references must exist, cycle detection via DFS
- context_from references validated against step IDs
- Plugin dependency verification (requires block)

**Agents.yaml validation:**
- Each agent must have: description, can_invoke, can_respond_to
- Reference validation: can_invoke/can_respond_to names must exist as top-level keys

### 9. JSON-RPC Dispatch (main.rs)

Stdio-based JSON-RPC adapter handling 16 methods:

| Method | Component |
|--------|-----------|
| `plugin-lifecycle.get-info` | CodingPackPlugin |
| `plugin-lifecycle.health-check` | CodingPackPlugin |
| `step-executor.execute` | CodingPackPlugin |
| `dashboard-extension.get-pages-json` | CodingPackPlugin |
| `dashboard-extension.get-api-routes-json` | CodingPackPlugin |
| `dashboard-extension.get-display-customizations-json` | CodingPackPlugin |
| `config-injector.injector-name` | BmadAgentInjector |
| `config-injector.priority` | BmadAgentInjector |
| `config-injector.applies-to` | BmadAgentInjector |
| `config-injector.provide-injections` | BmadAgentInjector |
| `tool-provider.provider-name` | BmadToolProvider |
| `tool-provider.available-tools` | BmadToolProvider |
| `tool-provider.execute-tool` | BmadToolProvider |
| `agent-definition.provider-name` | BmadAgentRegistry |
| `agent-definition.list-agents` | BmadAgentRegistry |
| `agent-definition.get-agent` | BmadAgentRegistry |

## Dashboard Extension

### Pages (7)

overview, workflows, workflow-detail, agents, status, execute, logs

Page layouts include: table, detail, form, stream

### API Endpoints (21+)

All under prefix `/api/v1/plugin-coding-pack`:

- GET /status, /status/health
- GET /workflows/list, /workflows/{id}
- POST /workflows/{id}/execute
- GET /agents/list, /agents/{id}
- GET /executions/stream (SSE)
- GET /tasks/{task_id}/workflow-context, /tasks/{task_id}/agent-info
- GET /board/data, /board/epics/list, /board/filters, /board/summary
- GET /board/epics/{id}, /board/stories/{id}
- POST /board/sync, /board/epics, /board/stories
- PUT /board/status/{id}, /board/epics/{id}, /board/stories/{id}

### Display Customizations (4)

1. **coding-pack-health** — Pack health badge on workflow view
2. **coding-workflow-info** — Workflow context fields on task view
3. **coding-pack-agent** — BMAD agent badge on task view (color-coded per agent)
4. **sprint-progress** — Sprint progress badge on workflow view

## WASM Support

Conditional compilation with `cfg(not(target_arch = "wasm32"))`:
- Native-only modules: `agent_registry`, `config_injector`, `tool_provider`
- Native-only dependencies: `async-trait`, `tokio`, `reqwest`
- WASM target: uses `wit-bindgen 0.53` for component model bindings
- WASM main() is empty (no-op)

## Plugin Dependencies

### Pack plugins (`plugin-packs/coding.toml`)

Built from source and loaded into `config/plugins/` at install time:

| Plugin | Required | Repo | Purpose |
|--------|----------|------|---------|
| `bmad-method` | Yes | Jack-R-Hong/bmad-method | BMAD methodology engine (12 AI agents) |
| `provider-claude-code` | Yes | Jack-R-Hong/provider-claude-code | Claude Code LLM provider |
| `plugin-git-ops` | Yes | Jack-R-Hong/pulse-plugins-git-ops | Git commit, push, branch operations |
| `plugin-git-worktree` | No | Jack-R-Hong/plugin-git-worktree | Git worktree management |
| `plugin-memory` | No | _(script in this repo)_ | Knowledge graph / memory |

### Bridge plugins (`src/plugin_bridge.rs`)

Called at runtime via HTTP; must be running as separate Pulse plugins:

| Plugin | Required | Repo | Purpose |
|--------|----------|------|---------|
| `plugin-board` | No | Jack-R-Hong/plugin-board | Scrum board management |
| `plugin-auto-loop` | No | Jack-R-Hong/plugin-auto-loop | Task pickup, workflow dispatch, validation |
| `plugin-issue-sync` | No | Jack-R-Hong/plugin-issue-sync | GitHub issue synchronization |
| `plugin-test-runner` | No | Jack-R-Hong/plugin-test-runner | Test execution and result parsing |
| `plugin-feedback-loop` | No | Jack-R-Hong/plugin-feedback-loop | PR review feedback processing |
| `plugin-trigger-cron` | No | Jack-R-Hong/plugin-trigger-cron | Scheduled triggering |
| `plugin-workspace-tracker` | No | Jack-R-Hong/plugin-workspace-tracker | Worktree lifecycle management |

## Testing Strategy

| Layer | Tool | Files | Coverage |
|-------|------|-------|----------|
| Unit tests | cargo test (in-module) | 11 src files | Per-module: config parsing, validation, CSV parsing, action dispatch, ACL rules |
| Integration tests | cargo test (tests/) | registration_tests.rs | SDK PluginRegistry: register(), injection pipeline, tool dispatch, agent routing |
| E2E tests | cargo test (tests/) | e2e_tests.rs, e2e_executor_tests.rs | Workflow execution end-to-end |
| Dashboard E2E | Playwright | 7 test files | Dashboard endpoint responses, board operations |
| Test fixtures | — | tests/fixtures/ | Sample project, mock plugins, 15 workflow YAMLs |

Total: ~6,156 lines of test code across Rust + TypeScript.

## Auto-Dev Loop

The auto-dev system routes tasks to workflows based on labels:

| Label | Workflow |
|-------|----------|
| story | coding-story-dev |
| bug | coding-bug-fix |
| refactor | coding-refactor |
| quick | coding-quick-dev |
| feature | coding-feature-dev |
| review | coding-review |
| pr-fix | coding-pr-fix |
| default | coding-quick-dev |

Configuration: max_retries=3, validation_enabled=true, poll_interval_secs=300
