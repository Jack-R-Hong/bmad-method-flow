---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
lastStep: 8
status: 'complete'
completedAt: '2026-03-18'
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '../../pulse/architecture.md'
  - '../../pulse/docs/index.md'
  - '../../pulse/docs/project-overview.md'
  - '../../pulse/docs/deep-dive-plugin-system.md'
  - '../../pulse/docs/plugin-development-guide.md'
  - '../../pulse/Cargo.toml'
  - '../../pulse-plugin-test/Makefile'
  - '../../pulse-plugin-test/config/pulse.yaml'
  - '../../pulse-plugin-test/config/plugins/claude-code.yaml'
  - '../../pulse-plugin-test/config/workflows/auto-dev-full.yaml'
  - '../../pulse-plugin-test/config/workflows/auto-dev-implement.yaml'
  - '../../pulse-plugin-test/config/workflows/auto-dev-review.yaml'
workflowType: 'architecture'
project_name: 'bmad-method-flow'
user_name: 'Jack'
date: '2026-03-18'
---

# Architecture Decision Document

_This document builds collaboratively through step-by-step discovery. Sections are appended as we work through each architectural decision together._

## Project Context Analysis

### Requirements Overview

**Functional Requirements:**
45 functional requirements across 9 domains:
- Task Submission & Orchestration (FR1-FR7): Workflow submission via CLI/HTTP, plugin dependency validation, DAG expansion and execution
- Agentic Code Execution (FR8-FR12): Claude CLI spawning, structured JSON output, per-step parameter configuration, permission mode enforcement, health check
- Session & Context Management (FR13-FR15): Session ID forwarding between steps, prior step output injection via `context_from`
- Quality Assurance (FR16-FR19): Test execution as workflow steps, gate condition evaluation, downstream blocking, structured review verdicts
- Git Operations (FR20-FR27): Commit, push, branch, PR creation (GitHub/GitLab), platform auto-detection, destructive operation refusal, structured diff output
- Budget & Resource Control (FR28-FR31): Per-step budget caps, timeout escalation (SIGTERM -> SIGKILL), cost metadata in output, zero leaked processes
- Workflow Composition (FR32-FR35): YAML-only pipeline creation, step library references, conditional execution, plugin dependency declarations
- Agent Persona System (FR36-FR38): 12 BMAD agent personas as WASM step executors, agent-specific system prompts and parameters
- Observability & Monitoring (FR39-FR42): Plugin health status, dashboard step progression, session history, commit attribution

**Non-Functional Requirements:**
19 NFRs across 4 domains:
- Performance (NFR1-5): Plugin startup <500ms, CLI spawn <1s, JSON parse <50ms, submission validation <200ms, git ops <10s
- Security (NFR6-9): Tokens in host config only, workspace path enforcement, output sanitization
- Reliability (NFR10-14): Process timeout enforcement, health check gating, deterministic gate evaluation, step failure isolation, WASM panic containment
- Integration (NFR15-19): Claude CLI version compatibility, git 2.20+ compatibility, GitHub/GitLab API versions, SDK API version contract, workflow schema validation at submission

**Scale & Complexity:**
- Primary domain: Rust plugin development (native + WASM) + YAML workflow configuration
- Complexity level: High
- Estimated architectural components: 4 plugins + 1 step library + 3+ workflow templates + integration test harness

### Technical Constraints & Dependencies

- **Zero core modifications** — all integration through existing `plugin-api` and `pulse-plugin-sdk` hooks
- **Two plugin patterns** — Pattern C (native, `plugin-api` traits) for `claude-code`; Pattern A/B (WASM, WIT bindings) for `git-ops`, `bmad-method`, `test-parser`
- **External dependencies** — `claude` CLI in PATH, `git` 2.20+, GitHub/GitLab tokens in host config
- **Rust toolchain** — Edition 2021, MSRV 1.85, `wasm32-wasip2` target for WASM plugins
- **Existing infrastructure** — DFIR dispatch engine, SQLite board, git worktree workspace manager, `inventory`-based native plugin registration, `wasmtime 28.0` WASM runtime
- **Workflow YAML engine** — Generic `WorkflowStep` with opaque `config: serde_json::Value`, `depends_on` DAG edges, `context_from` output injection, `run_if` conditional execution

### Cross-Cutting Concerns Identified

- **Process lifecycle management** — Spawn, monitor, timeout (SIGTERM -> 5s -> SIGKILL), cleanup for all CLI-spawning plugins
- **Structured output contract** — Every plugin must produce `StepOutput` with `step_id`, `status`, `content`, `execution_time_ms`, `metadata` (including cost fields for LLM steps)
- **Workspace path enforcement** — All plugins receive `workspace_path` via `Task` metadata and must use as `current_dir`
- **Credential isolation** — Git tokens and Claude auth from host config only; never in YAML, step params, outputs, logs, or dashboard
- **Error propagation** — Plugin errors flow through `PluginError` variants -> OnFailure hooks -> retry/escalation/abandon
- **Config parsing** — Each plugin independently parses its opaque `serde_json::Value` config from workflow YAML
- **Session continuity** — `session_id` from step output forwarded to downstream steps via `context_from` mechanism
- **Budget enforcement** — Per-step `max_budget_usd` checked during execution; cost metadata reported in every LLM step output

## Starter Template Evaluation

### Primary Technology Domain

Rust plugin development (native + WASM) for the Pulse AI Workflow Orchestration Engine — brownfield extension of an existing 34-crate monorepo.

### Starter Options Considered

Not applicable. This is brownfield development extending an existing platform with well-defined plugin interfaces, build tooling, and reference implementations. No starter template is needed.

### Established Technical Foundations

**Language & Runtime:**
- Rust edition 2021, MSRV 1.85
- Tokio 1.40 async runtime (full features)
- TypeScript/YAML for workflow configuration only

**Plugin SDK & Interfaces:**
- Pattern C (native): `plugin-api` crate traits (`TaskExecutor`, `QualityCheck`, etc.) + `inventory`-based registration via `plugin_api::submit_bridged!`
- Pattern A/B (WASM): `pulse-plugin-sdk` WIT bindings via `wit-bindgen 0.53`, `wasm32-wasip2` target, wasmtime 28.0 runtime
- Dev mode: `DevAdapter::new(Plugin).serve_stdio()` for JSON-RPC over stdio iteration

**Build & Deployment:**
- Cargo workspace (primary), Buck2 (optional)
- Native plugins: `crate-type = ["cdylib"]`, exported `pulse_plugin_register` symbol
- WASM plugins: `cargo build --target wasm32-wasip2`, loaded from `PULSE_PLUGIN_DIR`
- Integration testing: `pulse-plugin-test` workspace with Makefile orchestration

**Serialization & Config:**
- serde 1.0 + serde_json for all plugin data types
- serde_yaml 0.9 for workflow and plugin configuration
- Opaque `serde_json::Value` for step config (parsed by each plugin independently)

**Reference Implementations (to follow):**
- Process spawning: `worker-function/executor.rs` (tokio::process::Command, timeout)
- Process lifecycle: `mcp/client.rs` (child process management, shutdown)
- WIT WASM plugin: `git-worktree/src/lib.rs` (wit_bindgen::generate!)
- Native registration: `worker-function/lib.rs` (plugin_api::submit_bridged!)
- Dashboard extension: `opencode/handlers.rs` (PluginExtension)

**Note:** Each plugin in this suite creates a new crate following these established patterns. No project initialization story is needed — crate scaffolding follows standard `cargo new --lib` with workspace membership.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**
1. Plugin repository structure — independent crates under `pulse-plugins/`
2. git-ops as native plugin (Pattern C) for MVP
3. claude-code v2 direct CLI invocation (no sidecar)
4. Structured output contract with normalized content + metadata

**Important Decisions (Shape Architecture):**
5. Zero shared crates — copy shared logic between plugins
6. Trait-based process abstraction for testability
7. Shared ProcessManager pattern (copied, not shared crate)
8. Standardized cost metadata schema for LLM plugins

**Deferred Decisions (Post-MVP):**
- git-ops WASM migration (when host capabilities support process spawning)
- Step library YAML references (when Pulse engine supports `$ref`)
- Workflow dry-run validation mode

### Plugin Repository & Crate Structure

- **Decision:** Independent crates under `pulse-plugins/`, no shared Cargo workspace
- **Rationale:** Matches existing convention (`pulse-plugin-test/Makefile` already references `../pulse-plugins/*`). Independent versioning and build cycles per plugin.
- **Structure:**
  - `pulse-plugins/claude-code-v2/` — Native plugin (Pattern C)
  - `pulse-plugins/git-ops/` — Native plugin (Pattern C) for MVP
  - `pulse-plugins/bmad-method/` — WASM plugin (Pattern A/B), multi-crate
  - `pulse-plugins/test-parser/` — WASM plugin (Pattern A/B)
- **Affects:** All plugins, build pipeline, integration testing

### git-ops Plugin Pattern

- **Decision:** Native plugin (Pattern C) using `plugin-api` traits for MVP
- **Rationale:** WASM sandbox cannot spawn child processes. git-ops needs `git` CLI access for commit, push, branch operations and HTTP for GitHub/GitLab API calls. Native avoids host capability extensions.
- **Migration path:** When Pulse adds `git-command` and `http-request` WIT host imports, migrate to WASM for sandbox security.
- **Affects:** git-ops build target, testing approach, deployment model

### claude-code v2 Process Architecture

- **CLI Invocation:** Direct `tokio::process::Command` per step, spawning `claude --output-format json --session-id {id}`. No sidecar HTTP service.
- **Rationale:** Simpler lifecycle, matches PRD spec, `--session-id` flag handles session continuity natively. Sidecar adds operational overhead without clear benefit for step-per-process model.
- **Timeout Escalation:** SIGTERM -> 5s grace period -> SIGKILL, implemented in a `ProcessManager` struct.
- **Session Continuity:** `session_id` stored in `StepOutput.metadata` JSON. Downstream steps read via `context_from` mechanism. No schema changes to `StepOutput` or `StepConfig`.
- **Affects:** claude-code v2 implementation, workflow session chaining

### Shared Code Strategy

- **Decision:** Zero shared crates. Copy shared patterns (ProcessManager, config parsing) between plugins that need them.
- **Rationale:** Only 2 native plugins share process management (~100 lines). WASM plugins don't use it. Independent crates avoid coupling and simplify versioning.
- **Affects:** claude-code v2, git-ops

### Structured Output Contract

- **Content field:** Normalized human-readable response text (CLI wrapper stripped)
- **Metadata field:** Machine-parseable JSON with operation-specific data

**LLM plugin metadata schema (claude-code):**
```json
{
  "session_id": "ses_abc123",
  "model": "claude-sonnet-4-20250514",
  "cost_usd": 0.0142,
  "input_tokens": 1250,
  "output_tokens": 830,
  "duration_ms": 4200
}
```

**Git plugin metadata schema (git-ops):**
```json
{
  "commit_sha": "abc1234",
  "files_changed": 3,
  "branch": "feature/auth"
}
```

**Non-LLM plugins** omit cost fields entirely.
- **Affects:** All plugins, budget tracking, downstream step parsing, dashboard display

### Error Handling & Recovery

- **Error mapping:** CLI not found -> `NotFound`, bad config -> `Configuration`, non-zero exit -> `Execution` (stderr in message), timeout -> `Execution` with `"timed out"` message prefix (note: `plugin_api::PluginError` has no `Timeout` variant; available constructors are `execution`, `not_found`, `invalid_input`, `unauthorized`, `compatibility`, `version_mismatch`, `not_implemented`)
- **Credential sanitization:** Git tokens and auth credentials NEVER appear in error messages
- **Retry semantics:** Plugins are stateless and idempotent. No internal retry loops. Retry policy managed by dispatch engine via workflow `OnFailure` hooks.
- **Partial failure:** Report `Execution` error describing what succeeded. No rollback — workflow `OnFailure` handler decides recovery.
- **Affects:** All plugins, workflow error handling, observability

### Testing Strategy

- **Unit testing:** Trait-based process abstraction (`CommandRunner` trait). Production uses `TokioCommandRunner`, tests use `MockCommandRunner` with canned JSON/CLI responses. Applied to both `claude-code` and `git-ops`.
- **Integration testing:** `pulse-plugin-test` workspace with Makefile orchestration. Real workflows against real plugins with real external dependencies.
- **YAML validation:** Schema validation via `serde_yaml` deserialization into `WorkflowStep` structs at submission time (handled by Pulse engine).
- **Affects:** All plugins, CI pipeline, test infrastructure

### Workflow Template Architecture

- **Step organization:** Inline step definitions in self-contained YAML files (MVP). No step library references.
- **Conditional execution:** Handled by Pulse engine's `run_if` field on `WorkflowStep`. Plugins do not implement conditional logic.
- **Plugin dependency:** Declared via step `type` + `config.executor` fields. Engine validates at submission time.
- **Affects:** Workflow template design, user documentation

### Decision Impact Analysis

**Implementation Sequence:**
1. claude-code v2 (highest complexity, most dependencies downstream)
2. git-ops (reuses ProcessManager pattern from claude-code)
3. test-parser (WASM, independent)
4. bmad-method (WASM, depends on claude-code for downstream execution)
5. Workflow templates (compose all plugins)

**Cross-Component Dependencies:**
- git-ops reuses ProcessManager pattern from claude-code (copy, not shared crate)
- bmad-method output feeds claude-code steps via `context_from`
- Workflow templates depend on all 4 plugins being registered
- Integration tests depend on all plugins + Pulse engine running

## Implementation Patterns & Consistency Rules

### Pattern Categories Defined

**6 critical conflict points identified** where AI agents implementing different plugins could make incompatible choices.

### Crate Internal Structure

All plugins follow a flat module layout under `src/`:

```
src/
  lib.rs          # Crate root: pub mod declarations, plugin registration, re-exports
  config.rs       # Plugin config struct + serde deserialization from Value
  executor.rs     # TaskExecutor trait implementation (core execute logic)
  process.rs      # ProcessManager (native plugins only, copied per plugin)
  output.rs       # StepOutput construction + metadata serialization helpers
  error.rs        # Plugin-specific error helpers (optional, if beyond PluginError)
tests/
  integration_test.rs   # Integration tests against real CLI/binaries
  config_parse_test.rs  # Tests config deserialization edge cases
  session_chain_test.rs # Tests session_id forwarding
```

**Rules:**
- No nested `mod.rs` modules (flat `src/*.rs` only)
- `lib.rs` is registration + re-exports only — no business logic
- One file per responsibility, named by what it does
- `#[cfg(test)] mod tests` for unit tests inline at bottom of each file
- `tests/` directory for integration tests requiring external deps

**WASM plugins** (`bmad-method`, `test-parser`) omit `process.rs` and use `wit/` directory for WIT definitions:

```
src/
  lib.rs          # wit_bindgen::generate! + trait implementation
  config.rs       # Config parsing
  personas.rs     # (bmad-method only) Agent persona definitions
wit/
  *.wit           # WIT interface definitions
```

### Config Deserialization Pattern

All plugins deserialize `StepConfig.config` using **typed structs with `serde_json::from_value`**. No hand-parsing `.get()` chains.

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeCodeConfig {
    pub executor: String,
    pub model_tier: Option<String>,
    pub system_prompt: Option<String>,
    pub user_prompt_template: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    pub permission_mode: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub context_from: Vec<String>,
    pub max_budget_usd: Option<f64>,
}

impl ClaudeCodeConfig {
    pub fn from_step_config(value: &serde_json::Value) -> Result<Self, PluginError> {
        serde_json::from_value(value.clone())
            .map_err(|e| PluginError::configuration(&format!("Invalid config: {e}")))
    }
}
```

**Rules:**
- `#[serde(deny_unknown_fields)]` on all config structs — fail fast on typos
- Constructor method named `from_step_config(value: &Value) -> Result<Self, PluginError>`
- Map serde errors to `PluginError::configuration`
- All optional fields use `Option<T>`, never default sentinel values
- `#[serde(default)]` only for `Vec<T>` fields (empty vec default)

### StepOutput Metadata Conventions

All metadata JSON uses **`snake_case`** field names. No camelCase, no kebab-case.

**Mandatory fields (all plugins):**
```json
{
  "plugin_name": "claude-code",
  "plugin_version": "2.0.0"
}
```

**LLM plugin additional fields (claude-code, bmad-method via claude):**
```json
{
  "session_id": "ses_abc123",
  "model": "claude-sonnet-4-20250514",
  "cost_usd": 0.0142,
  "input_tokens": 1250,
  "output_tokens": 830,
  "duration_ms": 4200
}
```

**Git plugin additional fields (git-ops):**
```json
{
  "operation": "commit",
  "commit_sha": "abc1234",
  "files_changed": 3,
  "branch": "feature/auth"
}
```

**Test plugin additional fields (test-parser):**
```json
{
  "framework": "cargo-test",
  "tests_passed": 42,
  "tests_failed": 1,
  "tests_skipped": 3
}
```

**Rules:**
- All field names `snake_case`
- Numeric values as numbers, not strings
- Booleans as `true`/`false`, not `1`/`0`
- Absent optional fields omitted entirely (not `null`)
- `plugin_name` and `plugin_version` always present

### Logging Conventions

All plugins use `tracing` with **structured fields**, not string interpolation.

```rust
// CORRECT — structured fields
tracing::info!(plugin = "claude-code", step_id = %step_id, "spawning CLI process");
tracing::debug!(plugin = "claude-code", session_id = %sid, model = %model, "session resumed");
tracing::warn!(plugin = "claude-code", step_id = %step_id, elapsed_ms = elapsed, "approaching timeout");
tracing::error!(plugin = "claude-code", step_id = %step_id, exit_code = code, "process failed");

// WRONG — string interpolation
tracing::info!("claude-code: spawning CLI for step {}", step_id);
tracing::error!("Process failed with code {}", code);
```

**Rules:**
- `plugin` field always present, set to plugin crate name
- `step_id` field on all per-step log lines
- Log levels:
  - `error!` — operation failed, will return `PluginError`
  - `warn!` — degraded but continuing (approaching timeout, retryable failure)
  - `info!` — significant lifecycle events (process spawned, step complete, health check)
  - `debug!` — detailed operational data (config parsed, output received, session ID)
  - `trace!` — raw data dumps (full CLI output, raw JSON)
- **NEVER** log credentials, tokens, or API keys at any level
- **NEVER** log full `StepOutput.content` above `trace!` level (could be large)

### Test Organization

**Unit tests:** Inline `#[cfg(test)] mod tests` at the bottom of each source file.

```rust
// src/executor.rs
pub struct ClaudeExecutor { /* ... */ }

impl TaskExecutor for ClaudeExecutor { /* ... */ }

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_returns_structured_output() { /* ... */ }

    #[tokio::test]
    async fn test_execute_timeout_sends_sigterm() { /* ... */ }
}
```

**Integration tests:** `tests/` directory, one file per test scenario.

**Rules:**
- Test function naming: `test_{method}_{scenario}` (e.g., `test_execute_returns_structured_output`)
- Mock struct naming: `Mock{TraitName}` (e.g., `MockCommandRunner`)
- No `#[ignore]` without a comment explaining why
- Integration tests requiring external deps (`claude`, `git`) documented in test file header comment

### ProcessManager Pattern

Both `claude-code` and `git-ops` copy this pattern. The struct and API must be identical to avoid divergence:

```rust
pub struct ProcessManager {
    timeout: Duration,
    grace_period: Duration,  // default 5s
}

impl ProcessManager {
    pub fn new(timeout: Duration) -> Self { /* ... */ }

    pub async fn spawn_and_wait(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &Path,
        env: &[(String, String)],
    ) -> Result<ProcessOutput, PluginError> { /* ... */ }
}

pub struct ProcessOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}
```

**Rules:**
- `spawn_and_wait` is the single entry point — no `spawn()` + separate `wait()`
- Timeout escalation: SIGTERM -> `grace_period` -> SIGKILL (hardcoded sequence)
- `working_dir` always set via `Command::current_dir()` — never inherit parent
- Environment vars additive — never clear inherited env
- `ProcessOutput` always captured, even on non-zero exit (stderr is diagnostic)
- Testable via `CommandRunner` trait injection (constructor takes `impl CommandRunner`)

### Enforcement Guidelines

**All AI agents MUST:**
- Run `cargo clippy -- -D warnings` before considering any plugin complete
- Run `cargo fmt --check` before considering any plugin complete
- Verify `#[serde(deny_unknown_fields)]` on all config structs
- Include `plugin` field in all `tracing` macro calls
- Use `snake_case` in all metadata JSON fields
- Map all errors to `PluginError` variants — no `unwrap()` or `expect()` in non-test code

**Anti-Patterns (NEVER do):**
- `unwrap()` or `expect()` in production code — always `map_err` to `PluginError`
- `println!` or `eprintln!` — always use `tracing` macros
- String concatenation for error messages — use `format!` with structured context
- Logging credentials at any level, including `trace!`
- Nested `mod.rs` module organization
- Hand-parsing JSON with `.get().and_then()` chains instead of typed deserialization
- Internal retry loops — retry policy belongs to the dispatch engine

## Project Structure & Boundaries

### Complete Project Directory Structure

```
pulse-plugins/
├── claude-code-v2/                    # Native plugin — Pattern C (FR8-15, FR28-31)
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── lib.rs                     # Plugin registration, pub mod, re-exports
│   │   ├── config.rs                  # ClaudeCodeConfig struct + from_step_config()
│   │   ├── executor.rs                # TaskExecutor impl — CLI spawn, output parse
│   │   ├── process.rs                 # ProcessManager — spawn, timeout, SIGTERM/SIGKILL
│   │   ├── output.rs                  # StepOutput construction, metadata serialization
│   │   └── session.rs                 # Session ID extraction, context_from handling
│   └── tests/
│       ├── cli_spawn_test.rs          # Integration: real claude CLI
│       ├── config_parse_test.rs       # Config deserialization edge cases
│       └── session_chain_test.rs      # Session ID forwarding between steps
│
├── git-ops/                           # Native plugin — Pattern C (FR20-27)
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── lib.rs                     # Plugin registration, pub mod, re-exports
│   │   ├── config.rs                  # GitOpsConfig struct + from_step_config()
│   │   ├── executor.rs                # TaskExecutor impl — git command dispatch
│   │   ├── process.rs                 # ProcessManager — copied from claude-code-v2
│   │   ├── output.rs                  # StepOutput construction, git metadata
│   │   ├── operations.rs              # Git operations: commit, push, branch, diff
│   │   └── safety.rs                  # Destructive operation detection + refusal
│   └── tests/
│       ├── git_ops_test.rs            # Integration: real git commands
│       ├── config_parse_test.rs       # Config deserialization edge cases
│       └── safety_test.rs             # Destructive operation refusal
│
├── bmad-method/                       # WASM plugin — Pattern A/B (FR36-38)
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── rust-toolchain.toml
│   ├── crates/
│   │   ├── bmad-plugin/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── lib.rs             # wit_bindgen::generate! + StepExecutor impl
│   │   │       ├── config.rs          # BmadConfig struct + from_step_config()
│   │   │       └── personas.rs        # 12 agent persona definitions + system prompts
│   │   ├── bmad-types/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       └── lib.rs             # Shared types for persona, config
│   │   └── bmad-converter/
│   │       ├── Cargo.toml
│   │       └── src/
│   │           └── lib.rs             # Persona output → claude-code input formatting
│   ├── wit/
│   │   └── *.wit                      # WIT interface definitions
│   ├── src/
│   │   └── lib.rs                     # Crate root re-exports
│   └── tests/
│       └── persona_test.rs            # Persona selection + output formatting
│
├── test-parser/                       # WASM plugin — Pattern A/B (FR16-19)
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── lib.rs                     # wit_bindgen::generate! + QualityCheck impl
│   │   ├── config.rs                  # TestParserConfig struct + from_step_config()
│   │   └── parser.rs                  # Test output parsing (cargo test, pytest, jest)
│   ├── wit/
│   │   └── *.wit                      # WIT interface definitions
│   └── tests/
│       └── parser_test.rs             # Test output parsing fixtures
│
├── workflows/                         # YAML workflow templates (FR32-35)
│   ├── auto-dev-full.yaml             # Full pipeline: requirements → arch → impl → test → review
│   ├── auto-dev-implement.yaml        # Quick: implement → test
│   ├── auto-dev-review.yaml           # Review: review → fix → verify
│   ├── bmad-analysis.yaml             # BMAD: persona analysis → claude execution
│   └── git-commit-pr.yaml            # Git: implement → test → commit → PR
│
├── tests/                             # Cross-plugin integration test fixtures
│   └── fixtures/
│       └── sample-project/            # Sample Rust project for testing workflows
│           ├── Cargo.toml
│           └── src/
│               └── lib.rs
│
└── bmad-method-flow/                  # This project — planning docs (not a plugin)
    └── _bmad-output/
        └── planning-artifacts/
            ├── prd.md
            └── architecture.md        # This document
```

### Architectural Boundaries

**Plugin <-> Pulse Engine Boundary:**
- Plugins communicate with Pulse exclusively through `plugin-api` trait interfaces (`TaskExecutor`, `QualityCheck`)
- Data flows via `Task` (input) -> plugin logic -> `StepOutput` (output)
- No direct access to Pulse internals (SQLite board, dispatch engine, workspace manager)
- Plugin config arrives as opaque `serde_json::Value` in `StepConfig.config`

**Plugin <-> External Tool Boundary:**
- `claude-code-v2` -> `claude` CLI via `ProcessManager.spawn_and_wait()`
- `git-ops` -> `git` CLI via `ProcessManager.spawn_and_wait()`
- `git-ops` -> GitHub/GitLab REST API via `reqwest` HTTP client (Phase 2: PR creation)
- All external tool interaction isolated in `executor.rs` — never in `lib.rs` or `config.rs`

**Plugin <-> Plugin Boundary:**
- Plugins never call each other directly
- Inter-plugin communication is exclusively via workflow DAG: step A output -> `context_from` -> step B input
- Session continuity flows through `StepOutput.metadata.session_id`

**WASM Sandbox Boundary:**
- `bmad-method` and `test-parser` run in wasmtime sandbox
- No filesystem, network, or process spawning access
- Host provides: `config-get`, `kv-get`, `kv-set`, `log` via WIT host-api
- KV store is namespace-isolated per plugin (`plugin:{name}:{key}`)

### Requirements to Structure Mapping

| FR Domain | Plugin | Key Files |
|---|---|---|
| FR8-12 Agentic Execution | claude-code-v2 | `executor.rs`, `process.rs`, `session.rs` |
| FR13-15 Session & Context | claude-code-v2 | `session.rs`, `output.rs` |
| FR16-19 Quality Assurance | test-parser | `parser.rs`, `lib.rs` (QualityCheck) |
| FR20-27 Git Operations | git-ops | `operations.rs`, `safety.rs`, `executor.rs` |
| FR28-31 Budget & Resources | claude-code-v2 | `output.rs` (cost metadata), `process.rs` (timeout) |
| FR32-35 Workflow Composition | workflows/ | `*.yaml` template files |
| FR36-38 Agent Personas | bmad-method | `personas.rs`, `bmad-converter/` |
| FR39-42 Observability | All plugins | `tracing` structured logging in every module |

### Data Flow

```
Workflow YAML -> Pulse Engine -> plugin-api::TaskExecutor::execute()
                                    |
                    +---------------+-------------------+
                    v               v                   v
              claude-code-v2    git-ops           bmad-method
              (spawn claude)   (spawn git)     (WASM persona logic)
                    |               |                   |
                    v               v                   v
              StepOutput        StepOutput          StepOutput
              {content,         {content,           {content,
               metadata:         metadata:           metadata:
               session_id,       commit_sha,         persona,
               cost_usd}         branch}              agent_config}
                    |               |                   |
                    +---------------+-------------------+
                                    v
                          Pulse Engine (context_from)
                                    v
                          Next step receives upstream
                          output in Task.input / metadata
```

### Development Workflow Integration

**Build (per plugin):**
```bash
# Native plugins
cd pulse-plugins/claude-code-v2 && cargo build --release
cd pulse-plugins/git-ops && cargo build --release

# WASM plugins
cd pulse-plugins/test-parser && cargo build --target wasm32-wasip2 --release
cd pulse-plugins/bmad-method && cargo build --target wasm32-wasip2 --release
```

**Test (per plugin):**
```bash
cd pulse-plugins/claude-code-v2 && cargo test
cd pulse-plugins/git-ops && cargo test
```

**Integration test (full stack):**
```bash
cd pulse-plugin-test && make run-test
```

**Deployment:**
- Native plugin binaries -> `pulse-plugin-test/plugins/bin/` (or configured `plugin_dir`)
- WASM plugin artifacts -> same directory, `.wasm` extension
- Workflow YAML -> `pulse-plugin-test/config/workflows/`
- Plugin config YAML -> `pulse-plugin-test/config/plugins/`

## Architecture Validation Results

### Coherence Validation

**Decision Compatibility:** All technology choices are internally consistent. Native plugins (Pattern C) and WASM plugins (Pattern A/B) use non-overlapping toolchains with well-defined boundaries. No version conflicts between dependencies.

**Pattern Consistency:** Implementation patterns (crate structure, config deserialization, logging, testing) align with Rust ecosystem conventions and Pulse's existing plugin patterns. ProcessManager API is specified identically for both native plugins.

**Structure Alignment:** Project directory structure directly maps to architectural decisions. Each plugin has clear boundaries, consistent internal organization, and explicit integration points through `plugin-api` traits.

### Requirements Coverage Validation

**Functional Requirements Coverage:** 45/45 FRs have architectural support.

| FR Domain | Status | Notes |
|---|---|---|
| FR1-7 Orchestration | Covered | Pulse engine + workflow YAML templates |
| FR8-12 Agentic Execution | Covered | claude-code-v2 — health check addressed below |
| FR13-15 Session & Context | Covered | session.rs + StepOutput.metadata forwarding |
| FR16-19 Quality Assurance | Covered | test-parser QualityCheck trait |
| FR20-27 Git Operations | Covered | git-ops native — PR creation (FR24-25) deferred to Phase 2 |
| FR28-31 Budget & Resources | Covered | cost metadata in output + ProcessManager timeout |
| FR32-35 Workflow Composition | Covered | workflows/*.yaml inline templates |
| FR36-38 Agent Personas | Covered | bmad-method personas.rs + bmad-converter |
| FR39-42 Observability | Covered | tracing structured logging, Pulse engine dashboard |

**Non-Functional Requirements Coverage:** 19/19 NFRs have architectural support.
- Performance (NFR1-5): tokio async, direct CLI spawn, serde_json typed deserialization
- Security (NFR6-9): Credential isolation in host config, workspace path enforcement, output sanitization via error mapping
- Reliability (NFR10-14): ProcessManager timeout escalation, PluginError propagation, WASM sandbox containment
- Integration (NFR15-19): Version constraints documented, YAML schema validation at submission

### Health Check Implementation Note

**FR12 Resolution:** The `plugin-api` trait set does not include a dedicated `health_check()` method. Health verification is implemented at two levels:

1. **Registration-time:** The plugin-loader's `CompatibilityChecker` validates API version compatibility and plugin metadata during startup. Plugins that fail compatibility are logged and skipped.

2. **Runtime health (per-plugin):** Native plugins implement a lightweight health probe in their `TaskExecutor::execute()` path. Before first execution, `claude-code-v2` runs `claude --version` and verifies exit code 0. `git-ops` runs `git --version` and verifies exit code 0 and version >= 2.20. Health failures return `PluginError::not_found` with a diagnostic message.

```rust
// In executor.rs — called once on first execute, result cached
async fn check_tool_health(process_mgr: &ProcessManager, working_dir: &Path) -> Result<(), PluginError> {
    let output = process_mgr.spawn_and_wait("claude", &["--version"], working_dir, &[]).await?;
    if output.exit_code != 0 {
        return Err(PluginError::not_found("claude CLI not found or not functional"));
    }
    Ok(())
}
```

Health status is reported via `tracing::info!` at startup and via `PluginError::not_found` on failure. No custom health endpoint is needed — the Pulse engine treats `NotFound` errors as plugin unavailability.

### Gap Analysis Results

**Critical Gaps:** None. All implementation-blocking decisions are documented.

**Known Deferrals (Phase 2):**
- PR creation (FR24-25) — requires `reqwest` HTTP client for GitHub/GitLab APIs
- Step library YAML references — requires Pulse engine `$ref` support
- Workflow dry-run validation mode
- git-ops WASM migration — requires host capability extensions

**Minor Gaps (story-level detail):**
- Exact `claude` CLI flags for `--output-format json` response parsing (depends on Claude CLI version)
- GitHub vs GitLab API endpoint differences for PR creation (Phase 2)
- bmad-method persona prompt content (12 personas, content is implementation detail)

### Architecture Completeness Checklist

**Requirements Analysis**
- [x] Project context thoroughly analyzed (45 FRs, 19 NFRs across 9 domains)
- [x] Scale and complexity assessed (High — multi-plugin, dual compilation targets)
- [x] Technical constraints identified (zero core modifications, WASM sandbox limits)
- [x] Cross-cutting concerns mapped (8 concerns: process lifecycle, output contract, workspace path, credential isolation, error propagation, config parsing, session continuity, budget enforcement)

**Architectural Decisions**
- [x] Critical decisions documented (8 decisions with rationale)
- [x] Technology stack fully specified (Rust 2021, tokio 1.40, wasmtime 28.0, serde 1.0)
- [x] Plugin patterns defined (Pattern C native, Pattern A/B WASM)
- [x] Error handling and recovery specified
- [x] Testing strategy documented

**Implementation Patterns**
- [x] Crate internal structure defined (flat src/*.rs layout)
- [x] Config deserialization pattern specified (typed structs, deny_unknown_fields)
- [x] StepOutput metadata conventions established (snake_case, mandatory fields)
- [x] Logging conventions defined (structured tracing fields, level guidelines)
- [x] Test organization specified (inline unit tests, tests/ integration)
- [x] ProcessManager API specified (spawn_and_wait, timeout escalation)
- [x] Enforcement guidelines and anti-patterns documented

**Project Structure**
- [x] Complete directory structure defined (4 plugins, workflows, test fixtures)
- [x] Architectural boundaries established (plugin-engine, plugin-tool, plugin-plugin, WASM sandbox)
- [x] Requirements to structure mapping complete (FR domain -> plugin -> files table)
- [x] Data flow documented
- [x] Build, test, and deployment commands specified

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION

**Confidence Level:** High

**Key Strengths:**
- Brownfield context eliminates technology risk — all patterns proven in existing Pulse plugins
- Clear separation between native (process-spawning) and WASM (sandboxed) plugins
- Concrete code examples for every implementation pattern
- Explicit anti-pattern list prevents common AI agent mistakes
- FR-to-file mapping gives agents precise implementation targets

**Areas for Future Enhancement:**
- Shared `ProcessManager` crate if a third native plugin is added
- WASM host capability extensions for git-ops migration
- Step library references when Pulse engine supports `$ref`
- Dashboard extension for plugin-specific monitoring views

### Implementation Handoff

**AI Agent Guidelines:**
- Follow all architectural decisions exactly as documented
- Use implementation patterns consistently across all plugins
- Respect project structure and architectural boundaries
- Refer to this document for all architectural questions
- Run `cargo clippy -- -D warnings` and `cargo fmt --check` before marking any work complete

**Implementation Sequence:**
1. claude-code-v2 (highest complexity, most downstream dependencies)
2. git-ops (reuses ProcessManager pattern from claude-code-v2)
3. test-parser (WASM, independent)
4. bmad-method (WASM, depends on claude-code for downstream execution)
5. Workflow templates (compose all plugins)
6. plugin-memory (shell wrapper, workflow integration)

---

## Addendum: Epic 6 — Knowledge Graph Memory Plugin Architecture

*Added: 2026-03-20*

### Overview

plugin-memory is a **shell script plugin** (not a compiled Rust binary) that wraps external knowledge graph providers. It follows a multi-provider dispatcher pattern, reading `config/config.yaml` to determine which backend to use.

### Architectural Decisions

#### Shell Script Plugin (Not Rust)

- **Decision:** Implement plugin-memory as a bash shell script, not a Rust crate
- **Rationale:** The plugin delegates entirely to external tools (GitNexus via npx, Greptile via curl). No business logic requires Rust's type system or performance. A shell script is trivially extensible — adding a new provider requires only a new `xxx_exec()` function.
- **Affects:** Build pipeline (no cargo build needed), deployment (copy script + chmod), testing (integration only)

#### Multi-Provider Config Dispatch

- **Decision:** Provider selection via `config/config.yaml` → `memory.provider` field
- **Rationale:** Teams use different indexing backends. GitNexus requires Node.js, Greptile requires API access. The `none` provider cleanly disables all memory steps without workflow changes.
- **Config schema:**
  ```yaml
  memory:
    provider: gitnexus|greptile|none
    auto_reindex: true|false
    gitnexus:
      package: "gitnexus@latest"
    greptile:
      api_key_env: "GREPTILE_API_KEY"
      remote: "github:owner/repo"
  ```

#### Optional Workflow Steps

- **Decision:** All memory steps in coding workflows are marked `optional: true`
- **Rationale:** Memory is an enhancement, not a requirement. Workflows must function identically without plugin-memory installed. The `optional` flag ensures the Pulse engine skips the step gracefully rather than failing the workflow.
- **Affects:** All 6 coding workflow YAMLs, workflow validator

### Provider Capability Matrix

| Command | GitNexus | Greptile | None |
|---------|----------|----------|------|
| index | `npx gitnexus analyze` | `POST /v2/repositories` | skip |
| reindex | `npx gitnexus analyze --preserve-embeddings` | `POST /v2/repositories` (reload) | skip |
| query | `npx gitnexus query` | `POST /v2/query` | skip |
| impact | `npx gitnexus impact` | `POST /v2/query` (with impact prompt) | skip |
| context | `npx gitnexus context` | `POST /v2/query` (with context prompt) | skip |
| detect-changes | `npx gitnexus detect-changes` | `POST /v2/query` (with diff prompt) | skip |
| rename | `npx gitnexus rename` | ❌ not supported | skip |
| mcp | `npx gitnexus mcp` | ❌ not supported | skip |

### Workflow Integration Pattern

Memory steps are injected at three points in every coding workflow:

```
┌─────────────────┐
│ memory_context   │  ← Query knowledge graph with user input
│ (optional)       │     Provides: call chains, dependencies, clusters
└────────┬────────┘
         ↓
┌─────────────────┐
│ architect/plan   │  ← Receives memory context via context_from
│ (existing step)  │
└────────┬────────┘
         ↓
    ... (implement, QA review) ...
         ↓
┌──────────────────────┐
│ memory_detect_changes │  ← Map git diff → affected processes + risk
│ (optional)            │
└────────┬─────────────┘
         ↓
┌─────────────────┐
│ git_commit       │  ← Existing commit step
│ (existing step)  │
└────────┬────────┘
         ↓
┌─────────────────┐
│ memory_reindex   │  ← Incremental re-index preserving embeddings
│ (optional)       │
└─────────────────┘
```

### Data Flow

```
config/config.yaml ──→ plugin-memory (read config)
                              │
                    ┌─────────┼──────────┐
                    ↓         ↓          ↓
              GitNexus    Greptile     None
              (npx)      (curl API)   (noop)
                    │         │
                    ↓         ↓
              JSON output → workflow step output
                              │
                              ↓
                    context_from → downstream agent steps
```

### File Structure

```
config/
├── config.yaml                      # memory.provider config
├── plugins/
│   └── plugin-memory                # Shell script (executable)
└── workflows/
    ├── coding-memory-index.yaml     # Standalone indexing workflow
    ├── coding-feature-dev.yaml      # +3 memory steps
    ├── coding-bug-fix.yaml          # +3 memory steps
    ├── coding-quick-dev.yaml        # +2 memory steps
    ├── coding-refactor.yaml         # +3 memory steps
    ├── coding-story-dev.yaml        # +3 memory steps
    └── coding-review.yaml           # +1 memory step
```

### Adding a New Provider

To add a new knowledge graph provider (e.g., `sourcegraph`):

1. Add provider config section to `config/config.yaml`:
   ```yaml
   memory:
     sourcegraph:
       endpoint: "https://sourcegraph.example.com"
       token_env: "SRC_ACCESS_TOKEN"
   ```

2. Add `sourcegraph_exec()` function to `config/plugins/plugin-memory`:
   ```bash
   sourcegraph_exec() {
     local cmd="$1"; shift
     case "$cmd" in
       query) src search "$@" ;;
       # ...
     esac
   }
   ```

3. Add dispatch case:
   ```bash
   case "$PROVIDER" in
     gitnexus)    gitnexus_exec "$COMMAND" "$@" ;;
     greptile)    greptile_exec "$COMMAND" "$@" ;;
     sourcegraph) sourcegraph_exec "$COMMAND" "$@" ;;
   esac
   ```

No workflow changes needed — the provider abstraction handles routing.
