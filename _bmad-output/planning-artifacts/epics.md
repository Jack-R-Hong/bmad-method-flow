---
stepsCompleted: ['step-01-validate-prerequisites', 'step-02-design-epics', 'step-03-create-stories', 'step-04-final-validation']
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/architecture.md'
---

# Multi-Agent Auto-Dev Plugin Suite - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for Multi-Agent Auto-Dev Plugin Suite, decomposing the requirements from the PRD and Architecture requirements into implementable stories.

## Requirements Inventory

### Functional Requirements

FR1: Operator can submit an auto-dev task via CLI with a workflow name and JSON input parameters
FR2: Operator can submit an auto-dev task via HTTP API (`POST /api/v1/workflows/{name}/execute`)
FR3: System validates all plugins required by a workflow are loaded before accepting submission
FR4: System rejects submission with HTTP 422 and actionable error when required plugins are missing
FR5: System expands workflow YAML into a flat step list with resolved DAG dependencies
FR6: System executes workflow steps in topological order, respecting `depends_on` edges
FR7: System executes independent steps in parallel when no dependency edges exist
FR8: System spawns `claude` CLI as a child process in a specified workspace directory
FR9: System delivers prompt via stdin and captures structured JSON output from stdout
FR10: System configures Claude CLI parameters per step (model, max_turns, max_tokens, permission_mode, allowed_tools, disallowed_tools)
FR11: System enforces read-only mode for plan/review steps via `permission_mode: plan` and `disallowedTools`
FR12: System verifies `claude` CLI installation via health check at plugin startup
FR13: System passes `session_id` from completed step output to downstream step configuration for CLI session continuity
FR14: System injects prior step outputs into downstream steps via `context_from` configuration
FR15: System includes `session_id` in step output metadata for downstream consumption
FR16: System executes test suites as workflow steps and captures exit code + stdout/stderr
FR17: System evaluates gate condition expressions against prior step outputs
FR18: System blocks downstream steps when quality gate condition evaluates to false
FR19: System executes code review steps with read-only permissions producing structured verdict (approve/request-changes)
FR20: System stages and commits changes with auto-generated or user-provided commit message
FR21: System pushes branches with automatic upstream tracking for new branches
FR22: System creates branches with configurable naming convention (`auto-dev/{workflow_id}/{task_id}`)
FR23: System creates pull requests via GitHub REST API (title, body, base branch, draft flag)
FR24: System creates merge requests via GitLab REST API with equivalent parameters
FR25: System auto-detects git hosting platform from remote URL
FR26: System refuses destructive operations (force push, protected branch deletion) unless explicitly configured
FR27: System returns git diff and status as structured step output
FR28: System enforces per-step maximum LLM spend (`max_budget_usd`) and reports cap reached
FR29: System enforces per-step timeout with graceful shutdown escalation (SIGTERM → wait → SIGKILL)
FR30: System includes cost metadata (total_cost_usd, input_tokens, output_tokens, model, duration_ms) in every claude-code step output
FR31: System terminates runaway CLI processes without leaking child or zombie processes
FR32: Workflow designer creates auto-dev pipelines by composing steps in YAML without code
FR33: Workflow designer references reusable step configurations from step library directory
FR34: Workflow designer defines conditional step execution via `run_if` expressions
FR35: Workflow designer declares plugin dependencies via `requires` field
FR36: System loads 12 BMAD agent personas as WASM step executors
FR37: System returns agent-specific system prompts, user context, and suggested LLM parameters for downstream claude-code steps
FR38: System selects agent persona by name via step configuration (`agent_name`)
FR39: System reports plugin health status (loaded, healthy, error) for all registered plugins
FR40: Operator views task step progression and status through dashboard
FR41: Operator views Claude Code session history and cost data through dashboard extension
FR42: System attributes auto-dev commits with identifiable metadata (`Co-authored-by: pulse-auto-dev`)
FR43: Plugin developer implements new step executor via `StepExecutor` trait (Pattern C) or WIT bindings (Pattern A/B)
FR44: System loads native (`.so`/`.dylib`) and WASM (`.wasm`) plugins from configured directory at startup
FR45: System installs plugin packs (plugins + workflow templates) via `pulse plugin install-pack`
FR46: System reads `memory.provider` from `config/config.yaml` and dispatches to the configured backend (gitnexus, greptile, or none)
FR47: System indexes a codebase into a knowledge graph via `plugin-memory index [path]`
FR48: System queries the knowledge graph for symbols, call chains, and execution flows via `plugin-memory query <term>`
FR49: System performs blast radius / impact analysis on a symbol via `plugin-memory impact <symbol>`
FR50: System maps git diff to affected processes with risk assessment via `plugin-memory detect-changes`
FR51: System re-indexes the codebase after each commit when `auto_reindex: true` is configured
FR52: All memory steps in coding workflows are optional — workflows execute correctly when `memory.provider: none` or plugin-memory is absent
FR53: System provides a standalone `coding-memory-index` workflow for initial codebase indexing
FR54: System parses structured test output (cargo test, npm test, pytest, JUnit XML) into per-test pass/fail results with file, line, and assertion details
FR55: System identifies specific failing test names, locations, and error messages from raw test runner output
FR56: System provides structured test results to downstream steps via `context_from` for targeted fix guidance
FR57: System retries failed implementation steps with test failure context injected into the retry prompt
FR58: System enforces bounded retry count (configurable, default 3) for iterative fix loops
FR59: System tracks retry attempt number and includes prior attempt failures in progressive context enrichment
FR60: System exits retry loop when all tests pass OR retry limit is reached, reporting final status
FR61: System extends coding workflow templates to include push and PR creation as final pipeline steps
FR62: System auto-generates PR title and body from workflow context (task description, changes made, test results)
FR63: System executes independent DAG steps concurrently when no dependency edges exist between them
FR64: System manages a thread pool for concurrent CLI process execution with configurable concurrency limit
FR65: System provides E2E integration test harness that validates full workflow execution (submit → plan → implement → test → review → commit → PR)

### NonFunctional Requirements

NFR1: Plugin startup completes within 500ms per plugin. All four loaded within 2 seconds.
NFR2: Claude CLI process spawn completes within 1 second. Stdin prompt delivery within 100ms.
NFR3: Step output JSON parsing completes within 50ms regardless of output size.
NFR4: Workflow submission validation completes within 200ms.
NFR5: git-ops operations complete within 10 seconds for repos up to 10,000 files. PR API call within 5 seconds.
NFR6: API tokens stored exclusively in host config. Never in YAML, step params, outputs, logs, or dashboard.
NFR7: Claude CLI uses host `~/.claude/` credentials. No API keys in plugin config.
NFR8: Each task executes exclusively within assigned worktree. Workspace path enforced by `Command::current_dir()`.
NFR9: Step output sanitized before persistence. Sensitive field patterns (token, key, secret, password) redacted.
NFR10: Every spawned process has enforced timeout. SIGTERM immediate, SIGKILL after 5s. Zero orphaned/zombie processes.
NFR11: Failed health check prevents task acceptance with clear diagnostic message.
NFR12: Quality gate evaluation is deterministic. Gate failures never produce partial commits.
NFR13: Step failures are isolated — no dispatch engine crash, no impact on other in-flight tasks.
NFR14: WASM plugin panics caught by wasmtime without affecting host process stability.
NFR15: Compatible with Claude CLI versions supporting `--print --output-format json --verbose`. Defensive JSON parsing for format changes.
NFR16: git-ops compatible with git 2.20+ (minimum for worktree features).
NFR17: GitHub REST API v3 and GitLab REST API v4 for PR/MR creation. Self-hosted instances via configurable base URL.
NFR18: All plugins conform to Pulse Plugin SDK API version contract. Mismatches detected at load time.
NFR19: Workflow YAML schema validated at submission time, not execution time.

### Additional Requirements

- No starter template needed — brownfield development extending existing 34-crate monorepo with established plugin patterns
- Independent crates under `pulse-plugins/`, no shared Cargo workspace — each plugin versioned and built independently
- git-ops implemented as native plugin (Pattern C) for MVP — WASM migration deferred until host capability extensions exist
- Zero shared crates — copy shared logic (ProcessManager, ~100 lines) between native plugins that need it
- Crate internal structure: flat `src/*.rs` layout (no nested `mod.rs` modules); `lib.rs` is registration + re-exports only
- Config deserialization: typed structs with `serde_json::from_value`, `#[serde(deny_unknown_fields)]` on all config structs
- StepOutput metadata: `snake_case` field names only; mandatory `plugin_name` and `plugin_version` fields in every output
- Logging: structured `tracing` fields with `plugin` and `step_id` always present; never log credentials at any level
- Test organization: inline `#[cfg(test)] mod tests` for unit tests, `tests/` directory for integration tests requiring external deps
- ProcessManager pattern: `spawn_and_wait` single entry point, SIGTERM → 5s grace → SIGKILL escalation, `CommandRunner` trait injection for testability
- Build validation: `cargo clippy -- -D warnings` and `cargo fmt --check` before considering any plugin complete
- No `unwrap()` or `expect()` in production code — always `map_err` to `PluginError`
- No `println!`/`eprintln!` — always use `tracing` macros
- Implementation sequence: claude-code-v2 → git-ops → test-parser → bmad-method → workflow templates
- Plugin-to-plugin communication exclusively via workflow DAG (`context_from`), never direct calls
- WASM sandbox boundary: `bmad-method` and `test-parser` have no filesystem, network, or process spawning access; host provides `config-get`, `kv-get`, `kv-set`, `log` via WIT host-api

### UX Design Requirements

N/A — This is a developer tool/plugin suite with no user interface component. All interaction is via CLI, HTTP API, and YAML configuration.

### FR Coverage Map

FR1: Epic 3 — CLI task submission (existing Pulse capability, validated via workflow templates)
FR2: Epic 3 — HTTP API task submission (existing Pulse capability, validated via workflow templates)
FR3: Epic 3 — Plugin dependency validation at submission (existing Pulse capability)
FR4: Epic 3 — HTTP 422 on missing plugins (existing Pulse capability)
FR5: Epic 3 — Workflow YAML expansion to flat step list with DAG deps (existing Pulse capability)
FR6: Epic 3 — Topological step execution (existing Pulse capability)
FR7: Epic 3 — Parallel independent step execution (existing Pulse capability)
FR8: Epic 1 — Spawn `claude` CLI as child process in workspace directory
FR9: Epic 1 — Deliver prompt via stdin, capture structured JSON output
FR10: Epic 1 — Configure CLI parameters per step (model, max_turns, permission_mode, etc.)
FR11: Epic 1 — Enforce read-only mode for plan/review steps
FR12: Epic 1 — Health check for `claude` CLI at plugin startup
FR13: Epic 1 — Session ID forwarding to downstream steps
FR14: Epic 1 — Prior step output injection via `context_from`
FR15: Epic 1 — Session ID in step output metadata
FR16: Epic 3 — Test suite execution as workflow steps (function step type for MVP)
FR17: Epic 3 — Gate condition evaluation against step outputs (existing Pulse capability)
FR18: Epic 3 — Block downstream steps on gate failure (existing Pulse capability)
FR19: Epic 3 — Code review steps with read-only permissions and structured verdict
FR20: Epic 2 — Stage and commit changes with commit message
FR21: Epic 2 — Push branches with automatic upstream tracking
FR22: Epic 2 — Branch creation with naming convention (`auto-dev/{workflow_id}/{task_id}`)
FR23: Epic 2 — PR creation via GitHub REST API
FR24: Epic 2 — MR creation via GitLab REST API
FR25: Epic 2 — Auto-detect git hosting platform from remote URL
FR26: Epic 2 — Refuse destructive operations unless explicitly configured
FR27: Epic 2 — Return git diff and status as structured step output
FR28: Epic 1 — Per-step budget enforcement (`max_budget_usd`)
FR29: Epic 1 — Timeout with SIGTERM → SIGKILL escalation
FR30: Epic 1 — Cost metadata in every claude-code step output
FR31: Epic 1 — Terminate runaway processes, zero leaked/zombie processes
FR32: Epic 3 — YAML-only pipeline creation without code
FR33: Epic 3 — Reusable step library references
FR34: Epic 3 — Conditional step execution via `run_if` expressions
FR35: Epic 3 — Plugin dependency declaration via `requires` field
FR36: Epic 4 — 12 BMAD agent personas as WASM step executors
FR37: Epic 4 — Agent-specific system prompts, user context, and LLM parameters
FR38: Epic 4 — Agent persona selection by name via step config
FR39: Epic 5 — Plugin health status reporting (loaded, healthy, error)
FR40: Epic 5 — Dashboard task step progression view
FR41: Epic 5 — Dashboard session history and cost data view
FR42: Epic 2 — Auto-dev commit attribution (`Co-authored-by: pulse-auto-dev`)
FR43: Epic 5 — StepExecutor trait (Pattern C) / WIT bindings (Pattern A/B) for extensibility
FR44: Epic 5 — Native + WASM plugin loading from configured directory at startup
FR45: Epic 5 — Plugin pack installation via `pulse plugin install-pack`
FR46: Epic 6 — Multi-provider config dispatch (gitnexus, greptile, none)
FR47: Epic 6 — Codebase indexing via plugin-memory
FR48: Epic 6 — Knowledge graph query (symbols, call chains, flows)
FR49: Epic 6 — Blast radius / impact analysis
FR50: Epic 6 — Git diff → affected processes risk mapping
FR51: Epic 6 — Auto re-index after commit
FR52: Epic 6 — Optional memory steps (graceful degradation)
FR53: Epic 6 — Standalone memory-index workflow
FR54: Epic 7 — Multi-format test output parsing (cargo test, npm test, pytest, JUnit XML)
FR55: Epic 7 — Failing test identification with file, line, assertion details
FR56: Epic 7 — Structured test results via context_from for downstream steps
FR57: Epic 8 — Retry failed implementation with test failure context
FR58: Epic 8 — Bounded retry count (configurable, default 3)
FR59: Epic 8 — Progressive context enrichment across retry attempts
FR60: Epic 8 — Retry loop exit conditions (all pass OR limit reached)
FR61: Epic 9 — Push + PR creation steps in coding workflow templates
FR62: Epic 9 — Auto-generated PR title/body from workflow context
FR63: Epic 10 — Concurrent independent step execution in DAG
FR64: Epic 10 — Thread pool for parallel CLI processes
FR65: Epic 11 — E2E integration test harness for full workflow validation

## Epic List

### Epic 1: Claude Code Executor Plugin (claude-code-v2)

After this epic, developers can run AI-powered coding tasks with structured JSON output, session continuity between workflow steps, and enforced budget/timeout safety controls — the core engine powering all agentic coding in the pipeline.

**FRs covered:** FR8, FR9, FR10, FR11, FR12, FR13, FR14, FR15, FR28, FR29, FR30, FR31

### Epic 2: Git Operations Plugin (git-ops)

After this epic, the auto-dev pipeline closes the loop — changes are committed with attribution, branched with convention, pushed with tracking, and optionally PR'd via GitHub/GitLab API, with safety guards preventing destructive operations.

**FRs covered:** FR20, FR21, FR22, FR23, FR24, FR25, FR26, FR27, FR42

### Epic 3: Development Workflow Templates & Orchestration

After this epic, workflow designers compose end-to-end auto-dev pipelines (feature-dev, bug-fix, code-review) in YAML without code. Developers submit tasks via CLI/API and the system orchestrates plan → implement → test → review → gate → commit with quality gates blocking broken code.

**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR7, FR16, FR17, FR18, FR19, FR32, FR33, FR34, FR35

*Note: FR1-FR7 and FR17-FR18 are existing Pulse engine capabilities — the workflow templates exercise and validate them. FR16 uses function step type for MVP. FR19 configures claude-code-v2 with review permissions.*

### Epic 4: Agent Persona System (bmad-method)

After this epic, pipeline steps use 12 specialized AI personas (architect, developer, QA, reviewer, etc.) with role-specific system prompts, tool permissions, and LLM parameters — producing higher quality results through agent specialization.

**FRs covered:** FR36, FR37, FR38

### Epic 5: Observability & Plugin Ecosystem

After this epic, platform admins monitor plugin health, task step progression, session history, and LLM costs through the dashboard. Plugin developers extend the pipeline with custom step executors using documented traits and WASM bindings.

**FRs covered:** FR39, FR40, FR41, FR43, FR44, FR45

### Epic 6: Knowledge Graph Memory Plugin (plugin-memory)

After this epic, all coding workflows leverage a configurable knowledge graph backend (GitNexus, Greptile, or none) to provide codebase-aware context before implementation, blast radius / risk assessment before commit, and automatic re-indexing after commit — so that AI agents make changes with full understanding of call chains, dependencies, and impact scope.

**FRs covered:** FR46, FR47, FR48, FR49, FR50, FR51, FR52, FR53

### Epic 7: Structured Test Result Parsing (test-parser)

After this epic, the system parses raw test runner output into structured per-test results — identifying exactly which tests failed, where, and why — so that downstream retry and review steps receive precise, actionable failure context instead of raw stdout dumps.

**FRs covered:** FR54, FR55, FR56

### Epic 8: Iterative Fix Loops

After this epic, when tests fail during an auto-dev workflow, the system automatically retries implementation with the specific failure context — progressively refining the code until tests pass or the retry limit is reached, enabling self-correcting autonomous development.

**FRs covered:** FR57, FR58, FR59, FR60

### Epic 9: Automated PR Pipeline

After this epic, coding workflows produce a complete, ready-to-review pull request as their final output — pushed to a branch with auto-generated title/body — so that the developer's only action is clicking "Merge."

**FRs covered:** FR61, FR62, FR21, FR22, FR23, FR25

### Epic 10: Parallel Step Execution

After this epic, independent workflow steps (e.g., adversarial review + edge-case review) execute concurrently instead of sequentially — reducing total workflow execution time by up to 40% for pipelines with parallel branches.

**FRs covered:** FR7, FR63, FR64

### Epic 11: E2E Integration Testing & Validation

After this epic, a comprehensive integration test harness validates the full auto-dev cycle against a real codebase — from submission through PR creation — providing confidence that the system works end-to-end in production.

**FRs covered:** FR65

---

## Epic 1: Claude Code Executor Plugin (claude-code-v2)

After this epic, developers can run AI-powered coding tasks with structured JSON output, session continuity between workflow steps, and enforced budget/timeout safety controls — the core engine powering all agentic coding in the pipeline.

### Story 1.1: Crate Scaffolding & Process Manager

As a plugin developer,
I want a claude-code-v2 crate with a robust ProcessManager that spawns CLI processes with timeout escalation and health checks,
So that the foundation exists for safe, reliable CLI process management with zero leaked processes.

**Acceptance Criteria:**

**Given** the crate `pulse-plugins/claude-code-v2/` does not exist
**When** this story is implemented
**Then** a Rust crate exists with `Cargo.toml`, `lib.rs`, `process.rs`, and `error.rs`
**And** `lib.rs` contains plugin registration via `plugin_api::submit_bridged!` with re-exports only
**And** `Cargo.toml` declares `crate-type = ["cdylib"]` with dependencies on `plugin-api`, `tokio`, `serde`, `serde_json`, `tracing`

**Given** a `ProcessManager` struct is initialized with a timeout duration
**When** `spawn_and_wait("echo", &["hello"], &working_dir, &[])` is called
**Then** it returns `ProcessOutput { stdout, stderr, exit_code, duration_ms }`
**And** `working_dir` is set via `Command::current_dir()`, never inheriting parent

**Given** a process exceeds the configured timeout
**When** the timeout fires
**Then** SIGTERM is sent immediately
**And** after a 5-second grace period, SIGKILL is sent if the process is still running
**And** no zombie or orphaned processes remain

**Given** a `CommandRunner` trait exists for testability
**When** unit tests run
**Then** `MockCommandRunner` provides canned responses without spawning real processes
**And** tests verify timeout escalation, successful execution, and non-zero exit handling

**Given** the plugin starts up
**When** the health check runs
**Then** `claude --version` is executed via ProcessManager
**And** exit code 0 means healthy; non-zero returns `PluginError::not_found` with diagnostic message
**And** the health result is cached after first check

### Story 1.2: Config Parsing & CLI Parameter Configuration

As a workflow designer,
I want claude-code step configuration to be validated through typed Rust structs,
So that YAML typos are caught at deserialization time rather than causing runtime failures.

**Acceptance Criteria:**

**Given** a workflow step contains a `config` JSON block for claude-code
**When** `ClaudeCodeConfig::from_step_config(value)` is called
**Then** it deserializes into a typed struct with fields: `executor`, `model_tier`, `system_prompt`, `user_prompt_template`, `allowed_tools`, `disallowed_tools`, `permission_mode`, `session_id`, `context_from`, `max_budget_usd`, `max_turns`, `max_tokens`
**And** `#[serde(deny_unknown_fields)]` rejects any unexpected fields
**And** deserialization errors map to `PluginError::configuration` with the original serde message

**Given** optional fields (`model_tier`, `system_prompt`, `session_id`, `max_budget_usd`) are absent from config
**When** deserialization completes
**Then** those fields are `None`, not default sentinel values
**And** `Vec<String>` fields (`allowed_tools`, `disallowed_tools`, `context_from`) default to empty via `#[serde(default)]`

**Given** the config struct is defined in `src/config.rs`
**When** the file is reviewed
**Then** it contains only the struct definition, serde derives, and `from_step_config()` constructor
**And** no business logic exists in this file

### Story 1.3: CLI Execution & Structured Output Parsing

As a workflow engine,
I want the claude-code plugin to spawn the Claude CLI with proper arguments and parse its JSON output into a StepOutput with cost metadata,
So that downstream steps receive structured, machine-parseable results with LLM usage tracking.

**Acceptance Criteria:**

**Given** a `Task` and `StepConfig` are provided to the `TaskExecutor::execute` method
**When** execution begins
**Then** the CLI is invoked as `claude --print --output-format json --verbose` with additional args derived from config
**And** the prompt is delivered via stdin pipe
**And** `current_dir` is set to the workspace path from `Task` metadata

**Given** the Claude CLI returns valid JSON output
**When** output parsing completes
**Then** a `StepOutput` is constructed with `status: Success`, human-readable `content`, and `metadata` containing:
  - `plugin_name: "claude-code"`
  - `plugin_version: "2.0.0"`
  - `session_id` (extracted from CLI output)
  - `model` (the model used)
  - `cost_usd` (total cost)
  - `input_tokens`, `output_tokens` (token counts)
  - `duration_ms` (wall-clock execution time)
**And** all metadata field names are `snake_case`

**Given** the Claude CLI returns non-zero exit code
**When** the error is processed
**Then** `PluginError::execution` is returned with stderr content in the message
**And** no credentials or tokens appear in the error message

**Given** the Claude CLI output format is unexpected or malformed
**When** JSON parsing fails
**Then** the raw stdout is preserved in a fallback `StepOutput` with `status: Error`
**And** a `tracing::warn!` is emitted with `plugin = "claude-code"` and `step_id`

### Story 1.4: Permission Modes & Safety Controls

As a workflow designer,
I want plan and review steps to enforce read-only mode and budget limits to prevent unintended file modifications and cost overruns,
So that the agentic pipeline operates safely within configured boundaries.

**Acceptance Criteria:**

**Given** a step config has `permission_mode: "plan"`
**When** CLI args are constructed
**Then** `--permission-mode plan` is passed to the Claude CLI
**And** `disallowed_tools` from config are passed as `--disallowed-tools` arguments

**Given** a step config has `permission_mode: "bypassPermissions"` (implement steps)
**When** CLI args are constructed
**Then** `--permission-mode bypassPermissions` is passed
**And** `allowed_tools` from config are passed as `--allowed-tools` arguments

**Given** a step config has `max_budget_usd: 5.0`
**When** execution completes and `cost_usd` is extracted from CLI output
**Then** if `cost_usd > max_budget_usd`, the step returns `PluginError::execution` with message "Budget cap reached: $X spent of $Y limit"
**And** `tracing::warn!` is emitted with plugin, step_id, cost, and budget fields

**Given** a step config has `max_turns` or `max_tokens` set
**When** CLI args are constructed
**Then** `--max-turns` and/or `--max-tokens` are passed to the CLI

### Story 1.5: Session Continuity & Context Management

As a workflow engine,
I want the claude-code plugin to support session continuity and context injection between workflow steps,
So that downstream steps inherit prior context and the implement step builds on the plan step's reasoning.

**Acceptance Criteria:**

**Given** a step config includes `session_id: "ses_abc123"`
**When** CLI args are constructed
**Then** `--session-id ses_abc123` is passed to the Claude CLI
**And** `tracing::debug!` logs the session resumption with plugin and session_id fields

**Given** a step config includes `context_from: ["plan-step", "analyze-step"]`
**When** execution begins
**Then** the outputs from referenced steps are injected into the prompt
**And** the injection format prepends each prior output with a header identifying its source step

**Given** the Claude CLI returns output containing a `session_id`
**When** the `StepOutput` is constructed
**Then** `metadata.session_id` contains the session ID string
**And** downstream steps can read this via the `context_from` mechanism

**Given** `session_id` is not present in the CLI output
**When** the `StepOutput` is constructed
**Then** `metadata.session_id` is omitted entirely (not null)
**And** `tracing::debug!` notes the absence

**Given** session.rs contains the session handling logic
**When** the file is reviewed
**Then** it contains `extract_session_id(output: &str) -> Option<String>` and `build_context_prompt(prior_outputs: &[StepOutput]) -> String`
**And** no other business logic

---

## Epic 2: Git Operations Plugin (git-ops)

After this epic, the auto-dev pipeline closes the loop — changes are committed with attribution, branched with convention, pushed with tracking, and optionally PR'd via GitHub/GitLab API, with safety guards preventing destructive operations.

### Story 2.1: Crate Setup & Core Git Operations

As a workflow engine,
I want a git-ops plugin that commits changes with attribution and returns structured diff/status output,
So that the auto-dev pipeline can close the loop by persisting agentic code changes to git.

**Acceptance Criteria:**

**Given** the crate `pulse-plugins/git-ops/` does not exist
**When** this story is implemented
**Then** a Rust crate exists with `Cargo.toml`, `lib.rs`, `config.rs`, `executor.rs`, `process.rs`, `output.rs`, `operations.rs`
**And** `lib.rs` contains plugin registration via `plugin_api::submit_bridged!`
**And** `process.rs` is a copy of the ProcessManager from claude-code-v2 (identical API)
**And** `Cargo.toml` declares `crate-type = ["cdylib"]`

**Given** a step config specifies `operation: "commit"` with `message: "feat: add pagination"`
**When** the executor runs
**Then** `git add -A` stages all changes in the workspace
**And** `git commit -m "feat: add pagination"` creates the commit
**And** the commit includes `Co-authored-by: pulse-auto-dev` trailer (FR42)
**And** `StepOutput.metadata` contains `commit_sha`, `files_changed`, `plugin_name: "git-ops"`, `plugin_version`

**Given** a step config specifies `operation: "diff"`
**When** the executor runs
**Then** `git diff` and `git status --porcelain` are executed
**And** `StepOutput.content` contains the human-readable diff
**And** `StepOutput.metadata` contains `files_changed` count and `operation: "diff"`

**Given** a step config specifies `operation: "status"`
**When** the executor runs
**Then** `git status` is executed and returned as structured output

**Given** the `GitOpsConfig` struct is defined with `#[serde(deny_unknown_fields)]`
**When** an unknown field is present in the config
**Then** deserialization fails with `PluginError::configuration`

### Story 2.2: Branch Management & Push Operations

As a workflow engine,
I want the git-ops plugin to create branches with a naming convention and push with upstream tracking,
So that auto-dev work is isolated on properly named branches that are ready for PR creation.

**Acceptance Criteria:**

**Given** a step config specifies `operation: "branch"` with `workflow_id` and `task_id` in the task metadata
**When** the executor runs
**Then** a branch named `auto-dev/{workflow_id}/{task_id}` is created
**And** `git checkout -b auto-dev/{workflow_id}/{task_id}` is executed
**And** `StepOutput.metadata` contains `branch: "auto-dev/{workflow_id}/{task_id}"`

**Given** a step config specifies `operation: "push"`
**When** the branch has no upstream tracking
**Then** `git push -u origin {branch_name}` is executed (sets upstream)
**And** `StepOutput.metadata` contains `branch` and `pushed: true`

**Given** a step config specifies `operation: "push"`
**When** the branch already has upstream tracking
**Then** `git push` is executed without `-u`

**Given** a health check runs at first execution
**When** `git --version` is executed
**Then** the version is parsed and verified >= 2.20
**And** versions below 2.20 return `PluginError::not_found` with "git 2.20+ required for worktree features"

### Story 2.3: Destructive Operation Safety Guards

As a platform administrator,
I want the git-ops plugin to refuse force pushes and protected branch deletions by default,
So that auto-dev pipelines cannot accidentally destroy repository history or critical branches.

**Acceptance Criteria:**

**Given** a step config specifies `operation: "push"` with `force: false` (or force absent)
**When** the push target is any branch
**Then** `git push` is used (never `git push --force`)

**Given** a step config specifies `operation: "push"` with `force: true`
**When** the push would be a force push
**Then** `git push --force` is executed only if `force: true` is explicitly set
**And** `tracing::warn!` logs the force push with plugin, step_id, and branch fields

**Given** a step config specifies branch deletion
**When** the target branch matches `main`, `master`, or `develop`
**Then** the operation is refused with `PluginError::execution` containing "Refusing to delete protected branch: {name}"
**And** this check applies regardless of `force` setting

**Given** safety.rs contains the destructive operation detection logic
**When** the file is reviewed
**Then** it contains `is_destructive(operation: &str, config: &GitOpsConfig) -> bool` and `is_protected_branch(name: &str) -> bool`
**And** protected branch patterns are: `main`, `master`, `develop`, `release/*`

### Story 2.4: Platform Detection & PR/MR Creation

As a workflow designer,
I want the git-ops plugin to auto-detect the hosting platform and create PRs via GitHub or merge requests via GitLab,
So that the auto-dev pipeline can produce ready-to-review pull requests without manual intervention.

**Acceptance Criteria:**

**Given** the git remote URL is `https://github.com/org/repo.git` or `git@github.com:org/repo.git`
**When** platform detection runs
**Then** the platform is identified as "github"
**And** `StepOutput.metadata.platform` is set to "github"

**Given** the git remote URL is `https://gitlab.com/org/repo.git` or `git@gitlab.com:org/repo.git`
**When** platform detection runs
**Then** the platform is identified as "gitlab"

**Given** a step config specifies `operation: "create-pr"` on a GitHub repository
**When** the executor runs
**Then** a POST request is made to `https://api.github.com/repos/{owner}/{repo}/pulls` (GitHub REST API v3)
**And** the request body includes `title`, `body`, `head` (branch), `base` (target branch), `draft` (flag)
**And** the GitHub token is read from host configuration (environment variable or config file), never from YAML or step params
**And** `StepOutput.metadata` contains `pr_url`, `pr_number`, `platform: "github"`

**Given** a step config specifies `operation: "create-mr"` on a GitLab repository
**When** the executor runs
**Then** a POST request is made to `{gitlab_base_url}/api/v4/projects/{id}/merge_requests` (GitLab REST API v4)
**And** the request includes `title`, `description`, `source_branch`, `target_branch`
**And** self-hosted GitLab instances are supported via configurable `base_url`

**Given** no platform token is available in host configuration
**When** PR/MR creation is attempted
**Then** `PluginError::configuration` is returned with "No {platform} token found. Set {ENV_VAR} or configure in host settings."
**And** the token name is never logged or included in error messages beyond the env var name

---

## Epic 3: Development Workflow Templates & Orchestration

After this epic, workflow designers compose end-to-end auto-dev pipelines (feature-dev, bug-fix, code-review) in YAML without code. Developers submit tasks via CLI/API and the system orchestrates plan → implement → test → review → gate → commit with quality gates blocking broken code.

### Story 3.1: Feature-Dev End-to-End Pipeline

As a developer,
I want a feature-dev workflow that chains plan → implement → test → review → gate → commit steps,
So that I can submit a feature description and receive a committed, tested implementation without manual intervention.

**Acceptance Criteria:**

**Given** the file `pulse-plugins/workflows/auto-dev-full.yaml` does not exist
**When** this story is implemented
**Then** a valid workflow YAML defines these steps in order:
  1. `plan` — claude-code step with `permission_mode: plan` (read-only)
  2. `implement` — claude-code step with `permission_mode: bypassPermissions`, `depends_on: [plan]`, `context_from: [plan]`, `session_id` from plan output
  3. `run-tests` — function step executing the project test command, `depends_on: [implement]`
  4. `review` — claude-code step with `permission_mode: plan`, `depends_on: [run-tests]`, `context_from: [implement, run-tests]`
  5. `gate` — gate step evaluating review verdict and test pass, `depends_on: [review]`
  6. `commit` — git-ops step with `operation: commit`, `depends_on: [gate]`
**And** `requires: [claude-code, git-ops]` declares plugin dependencies

**Given** the workflow is submitted via `pulse submit -w auto-dev-full`
**When** the plan step fails
**Then** all downstream steps are skipped (DAG enforcement)

**Given** the gate step evaluates
**When** the review verdict is "request-changes" or tests failed
**Then** the gate blocks and the commit step does not execute

**Given** the workflow YAML is loaded
**When** schema validation runs
**Then** all step IDs, `depends_on` references, and `context_from` references resolve correctly

### Story 3.2: Quick Implementation & Test Workflow

As a developer,
I want a quick implement-and-test workflow for well-understood changes,
So that I can skip planning and review for simple, well-scoped modifications.

**Acceptance Criteria:**

**Given** the file `pulse-plugins/workflows/auto-dev-implement.yaml` does not exist
**When** this story is implemented
**Then** a valid workflow YAML defines:
  1. `implement` — claude-code step with `permission_mode: bypassPermissions`
  2. `run-tests` — function step, `depends_on: [implement]`
**And** `requires: [claude-code]` declares the only plugin dependency

**Given** a test step is included
**When** the test command exits with code 0
**Then** the step output captures stdout/stderr and `status: Success`

**Given** the test command exits with non-zero code
**When** the step output is constructed
**Then** `status: Error` with captured stdout/stderr for diagnostic visibility

### Story 3.3: Code Review & Quality Gate Workflow

As a developer,
I want a review-only workflow that performs code review with read-only permissions and produces a structured verdict,
So that I can run automated reviews on existing branches without modifying code.

**Acceptance Criteria:**

**Given** the file `pulse-plugins/workflows/auto-dev-review.yaml` does not exist
**When** this story is implemented
**Then** a valid workflow YAML defines:
  1. `review` — claude-code step with `permission_mode: plan`, system prompt focused on code review, producing structured verdict (approve/request-changes)
  2. `fix` — claude-code step with `permission_mode: bypassPermissions`, `depends_on: [review]`, `run_if: "review.verdict == 'request-changes'"`
  3. `verify` — claude-code step with `permission_mode: plan`, `depends_on: [fix]`, `run_if: "fix.status == 'success'"`
**And** `requires: [claude-code]` declares the plugin dependency
**And** the `run_if` expressions demonstrate conditional step execution (FR34)

**Given** the review step returns a verdict of "approve"
**When** the `run_if` condition on the fix step is evaluated
**Then** the fix step is skipped

**Given** the review step returns "request-changes"
**When** the fix step runs
**Then** it receives the review output via `context_from: [review]`

### Story 3.4: Git Commit & PR Pipeline

As a developer,
I want a workflow that implements, tests, commits, and creates a PR in one pipeline,
So that feature work flows from code to pull request without manual git operations.

**Acceptance Criteria:**

**Given** the file `pulse-plugins/workflows/git-commit-pr.yaml` does not exist
**When** this story is implemented
**Then** a valid workflow YAML defines:
  1. `implement` — claude-code step
  2. `run-tests` — function step, `depends_on: [implement]`
  3. `gate` — gate step, `depends_on: [run-tests]`
  4. `branch` — git-ops step with `operation: branch`, `depends_on: [gate]`
  5. `commit` — git-ops step with `operation: commit`, `depends_on: [branch]`
  6. `push` — git-ops step with `operation: push`, `depends_on: [commit]`
  7. `create-pr` — git-ops step with `operation: create-pr`, `depends_on: [push]`
**And** `requires: [claude-code, git-ops]` declares both plugin dependencies

---

## Epic 4: Agent Persona System (bmad-method)

After this epic, pipeline steps use 12 specialized AI personas (architect, developer, QA, reviewer, etc.) with role-specific system prompts, tool permissions, and LLM parameters — producing higher quality results through agent specialization.

### Story 4.1: WASM Crate Setup & WIT Interface

As a plugin developer,
I want a bmad-method WASM plugin crate with proper WIT bindings and multi-crate layout,
So that the agent persona system runs in the wasmtime sandbox with defined host capabilities.

**Acceptance Criteria:**

**Given** the crate `pulse-plugins/bmad-method/` does not exist
**When** this story is implemented
**Then** a multi-crate layout exists:
  - `bmad-method/Cargo.toml` (workspace root)
  - `bmad-method/crates/bmad-plugin/` — main plugin crate with `wit_bindgen::generate!`
  - `bmad-method/crates/bmad-types/` — shared types for persona, config
  - `bmad-method/crates/bmad-converter/` — persona output → claude-code input formatting
  - `bmad-method/wit/` — WIT interface definitions
  - `bmad-method/rust-toolchain.toml` — specifying `wasm32-wasip2` target support

**Given** the WIT interface is defined
**When** the plugin is compiled with `cargo build --target wasm32-wasip2`
**Then** the resulting `.wasm` file implements the `step-executor-plugin` world
**And** the plugin exposes `get-info`, `health-check`, and `execute` functions

**Given** the WASM sandbox boundary is enforced
**When** the plugin runs in wasmtime
**Then** it has NO filesystem, network, or process spawning access
**And** it can only use host-provided `config-get`, `kv-get`, `kv-set`, `log` via WIT host-api

**Given** a `BmadConfig` struct exists in `bmad-plugin/src/config.rs`
**When** step config is deserialized
**Then** `#[serde(deny_unknown_fields)]` validates config structure
**And** `agent_name` is a required field

### Story 4.2: Agent Persona Definitions & Selection

As a workflow designer,
I want 12 BMAD agent personas selectable by name, each with role-specific system prompts,
So that pipeline steps use specialized AI behavior appropriate to their function.

**Acceptance Criteria:**

**Given** `personas.rs` is implemented in the bmad-plugin crate
**When** the file is reviewed
**Then** it defines 12 agent personas: architect, developer, qa, reviewer, pm, analyst, ux-designer, tech-writer, scrum-master, devops, security, data-engineer
**And** each persona has: `name`, `system_prompt`, `description`, `suggested_model_tier`, `suggested_max_turns`

**Given** a step config specifies `agent_name: "architect"`
**When** the plugin executes
**Then** the architect persona is selected
**And** `StepOutput.content` contains the persona's system prompt and configuration
**And** `StepOutput.metadata` contains `persona: "architect"`, `plugin_name: "bmad-method"`, `plugin_version`

**Given** a step config specifies an invalid `agent_name: "nonexistent"`
**When** the plugin executes
**Then** `PluginError::configuration` is returned with "Unknown agent persona: nonexistent. Available: [architect, developer, ...]"

**Given** persona data is defined
**When** the data is reviewed
**Then** system prompts are stored as Rust string constants (codegen from markdown source)
**And** no filesystem reads occur at runtime (WASM sandbox cannot read files)

### Story 4.3: Persona Output & Claude Step Integration

As a workflow engine,
I want the bmad-method plugin output to be directly consumable by downstream claude-code steps,
So that agent specialization integrates seamlessly into multi-step workflows.

**Acceptance Criteria:**

**Given** the bmad-converter crate exists
**When** a persona is selected and executed
**Then** the output includes a structured JSON block with:
  - `system_prompt` — the full persona system prompt
  - `suggested_config` — recommended claude-code config overrides (`model_tier`, `max_turns`, `permission_mode`, `allowed_tools`)
  - `user_context` — any additional context the persona requires

**Given** a workflow chains bmad-method → claude-code via `context_from`
**When** the claude-code step receives the bmad-method output
**Then** it can extract `system_prompt` and `suggested_config` from the injected context
**And** the system prompt is prepended to the user prompt

**Given** a persona suggests `model_tier: "opus"` and `max_turns: 20`
**When** the downstream claude-code step uses these suggestions
**Then** the suggestions are advisory — the workflow YAML config takes precedence if explicitly set

**Given** the BMAD analysis workflow template exists
**When** `pulse-plugins/workflows/bmad-analysis.yaml` is created
**Then** it defines:
  1. `select-persona` — bmad-method step with `agent_name` from input
  2. `execute` — claude-code step with `context_from: [select-persona]`
**And** `requires: [bmad-method, claude-code]`

---

## Epic 5: Observability & Plugin Ecosystem

After this epic, platform admins monitor plugin health, task step progression, session history, and LLM costs through the dashboard. Plugin developers extend the pipeline with custom step executors using documented traits and WASM bindings.

### Story 5.1: Plugin Health Status & Reporting

As a platform administrator,
I want all plugins to report health status (loaded, healthy, error) via structured logging,
So that I can monitor plugin availability and diagnose startup failures.

**Acceptance Criteria:**

**Given** all four plugins (claude-code-v2, git-ops, bmad-method, test-parser) implement health checks
**When** each plugin starts up
**Then** it emits `tracing::info!(plugin = "{name}", status = "healthy", version = "{version}")` on success
**And** it emits `tracing::error!(plugin = "{name}", status = "error", reason = "{msg}")` on failure

**Given** the claude-code-v2 plugin health check fails (claude CLI not found)
**When** a task is submitted requiring claude-code
**Then** the task is rejected with `PluginError::not_found` containing "claude CLI not found or not functional"
**And** the error message includes install instructions

**Given** the git-ops plugin health check fails (git version < 2.20)
**When** a task is submitted requiring git-ops
**Then** the task is rejected with a diagnostic message specifying the minimum version requirement

**Given** a WASM plugin (bmad-method) panics during execution
**When** wasmtime catches the panic
**Then** the host process remains stable (NFR14)
**And** `PluginError::execution` is returned with "WASM plugin panic" context

### Story 5.2: Dashboard Extensions for Auto-Dev Monitoring

As a platform administrator,
I want dashboard views showing task step progression and Claude Code session history with cost data,
So that I can monitor auto-dev pipeline execution and track LLM spend.

**Acceptance Criteria:**

**Given** a task is in progress with multiple completed steps
**When** the dashboard is queried
**Then** it shows each step's status (pending, running, success, error, skipped), duration, and output summary
**And** step ordering matches the workflow DAG topology

**Given** claude-code steps have completed with cost metadata
**When** the session history view is queried
**Then** it displays: session_id, model, cost_usd, input_tokens, output_tokens, duration_ms per step
**And** a total cost summary for the entire task

**Given** the dashboard extension follows the PluginExtension pattern
**When** it is implemented
**Then** it uses the existing `opencode/handlers.rs` pattern from the claude-code v1 reference
**And** it registers HTTP handlers for step progression and session history endpoints

**Given** cost metadata is displayed
**When** the data is reviewed
**Then** no API tokens, credentials, or session content are exposed in the dashboard
**And** only aggregated cost and token counts are shown

### Story 5.3: Plugin Extensibility & Pack Installation

As a plugin developer,
I want clear documentation and tooling for creating new step executors and installing plugin packs,
So that the auto-dev pipeline can be extended with custom functionality without modifying core code.

**Acceptance Criteria:**

**Given** a developer wants to create a new native plugin (Pattern C)
**When** they follow the documented pattern
**Then** they implement `TaskExecutor` trait with `name()`, `version()`, `execute()`
**And** register via `plugin_api::submit_bridged!`
**And** the plugin is loaded from `PULSE_PLUGIN_DIR` at startup

**Given** a developer wants to create a new WASM plugin (Pattern A/B)
**When** they follow the documented pattern
**Then** they use `wit_bindgen::generate!` to implement the `step-executor-plugin` world
**And** compile with `cargo build --target wasm32-wasip2`
**And** the `.wasm` file is loaded by wasmtime from `PULSE_PLUGIN_DIR`

**Given** a plugin pack TOML manifest exists
**When** `pulse plugin install-pack auto-dev` is executed
**Then** all plugins in the pack are compiled (or downloaded) and placed in `PULSE_PLUGIN_DIR`
**And** workflow templates are copied to the workflows configuration directory
**And** plugin configuration files are placed in the plugins configuration directory

**Given** a plugin has incompatible API version
**When** the plugin loader attempts to load it
**Then** the mismatch is detected at load time (NFR18)
**And** the plugin is skipped with a clear diagnostic message logged via `tracing::error!`

---

## Epic 6: Knowledge Graph Memory Plugin (plugin-memory)

After this epic, all coding workflows leverage a configurable knowledge graph backend (GitNexus, Greptile, or none) to provide codebase-aware context before implementation, blast radius / risk assessment before commit, and automatic re-indexing after commit — so that AI agents make changes with full understanding of call chains, dependencies, and impact scope.

### Story 6.1: Multi-Provider Config & Plugin Shell Wrapper

As a workflow designer,
I want plugin-memory to read `memory.provider` from `config/config.yaml` and dispatch commands to the configured backend,
So that teams can switch between GitNexus, Greptile, or disable memory entirely without changing workflows.

**Acceptance Criteria:**

**Given** `config/config.yaml` contains `memory.provider: gitnexus`
**When** `plugin-memory query "auth"` is executed
**Then** the command is dispatched to GitNexus via `npx -y gitnexus@latest query "auth"`
**And** the GitNexus npm package spec is read from `memory.gitnexus.package` (default: `gitnexus@latest`)

**Given** `config/config.yaml` contains `memory.provider: greptile`
**When** `plugin-memory query "auth"` is executed
**Then** the command is dispatched to the Greptile REST API (`POST /v2/query`)
**And** the API key is read from the environment variable specified in `memory.greptile.api_key_env`
**And** the repository identifier is read from `memory.greptile.remote`

**Given** `config/config.yaml` contains `memory.provider: none`
**When** any plugin-memory command is executed
**Then** it returns `{"status":"skipped","provider":"none","reason":"memory disabled in config"}`
**And** exit code is 0 (non-blocking)

**Given** `memory.provider` is set to an unknown value (e.g. `"foo"`)
**When** any plugin-memory command is executed
**Then** it returns `{"status":"error","error":"unknown provider: foo"}`
**And** exit code is 1

**Given** `plugin-memory health` is executed
**When** the provider is `gitnexus`
**Then** it checks that `npx` is available and returns `{"status":"healthy","provider":"gitnexus","npx":true}`

**Given** `plugin-memory health` is executed
**When** the provider is `greptile` and the API key env var is not set
**Then** it returns `{"status":"unhealthy","provider":"greptile","error":"GREPTILE_API_KEY not set"}`

**Given** `plugin-memory info` is executed
**When** any provider is configured
**Then** it returns JSON with `name`, `version`, `provider`, `auto_reindex`, and `commands` fields

**Given** `config/config.yaml` does not contain a `memory` section
**When** `plugin-memory` is executed
**Then** it defaults to `provider: gitnexus` and `auto_reindex: true`

### Story 6.2: Coding Workflow Memory Step Integration

As a developer,
I want all coding workflows to automatically query the knowledge graph before implementation and assess risk before commit,
So that AI agents have codebase-aware context and changes are validated against the dependency graph.

**Acceptance Criteria:**

**Given** a coding workflow (feature-dev, bug-fix, refactor, story-dev, quick-dev, review)
**When** the workflow YAML is loaded
**Then** it declares `plugin: plugin-memory` with `optional: true` in the `requires` section
**And** memory steps do not block execution when plugin-memory is absent

**Given** the `coding-feature-dev` workflow executes
**When** `memory.provider` is not `none` and plugin-memory is available
**Then** `memory_context` step runs first, querying the knowledge graph with `{{input}}`
**And** the architect step receives `memory_context` output via `context_from`
**And** `memory_detect_changes` runs after QA review, before git commit
**And** `memory_reindex` runs after git commit to update the index

**Given** the `coding-bug-fix` workflow executes
**When** plugin-memory is available
**Then** `memory_context` provides call chain / dependency context to the bug analysis step
**And** the architect's system prompt includes: "Use knowledge graph context (if available) to understand call chains and impact scope"

**Given** the `coding-refactor` workflow executes
**When** plugin-memory is available
**Then** `memory_impact` step provides blast radius analysis to the refactor planning step
**And** `memory_detect_changes` provides risk assessment before commit

**Given** the `coding-review` workflow executes
**When** plugin-memory is available
**Then** both parallel review steps (adversarial, edge-case) receive `memory_context` via `context_from`

**Given** any coding workflow executes
**When** `memory.provider: none` or plugin-memory is not installed
**Then** all `optional: true` memory steps are skipped
**And** the workflow completes successfully with only the non-memory steps

**Given** `memory.auto_reindex: true` in config
**When** a workflow's git commit step completes
**Then** `memory_reindex` step runs with `--preserve-embeddings` to update the index incrementally

**Given** `memory.auto_reindex: false` in config
**When** a workflow's git commit step completes
**Then** `memory_reindex` step is skipped

### Story 6.3: Standalone Memory Index Workflow & Pack Integration

As a developer,
I want a `coding-memory-index` workflow for initial codebase indexing and the memory plugin registered in the pack manifest,
So that I can bootstrap the knowledge graph and install it as part of the coding pack.

**Acceptance Criteria:**

**Given** the file `config/workflows/coding-memory-index.yaml` exists
**When** the workflow is loaded
**Then** it declares `requires: [plugin: plugin-memory]`
**And** step 1 (`index_codebase`) runs `plugin-memory index .` with a 300s timeout
**And** step 2 (`verify_index`) runs `plugin-memory health` with a 30s timeout, depending on step 1

**Given** `plugin-packs/coding.toml` is loaded
**When** the pack manifest is parsed
**Then** `plugin-memory` is listed as an optional plugin with `source: "local:config/plugins/plugin-memory"`
**And** `coding-memory-index.yaml` is included in the workflows list
**And** `npx` is listed in prerequisites with `check: "npx --version"` and install hint

**Given** `src/lib.rs` declares plugin dependencies
**When** `get_info()` is called
**Then** `plugin-memory` is listed with `optional: true` and `version_req: ">=0.1.0"`

**Given** `src/pack.rs` runs `validate-pack`
**When** plugin-memory is not installed
**Then** it reports `"MISSING optional plugin: plugin-memory (non-blocking)"`
**And** the pack is still considered valid

**Given** `config/config.yaml` is loaded
**When** the memory section is parsed
**Then** it contains `provider`, `auto_reindex`, and provider-specific subsections for `gitnexus` and `greptile`
**And** `config.defaults` in `coding.toml` provides default values for `memory.provider` and `memory.auto_reindex`

**Given** `pulse run coding-memory-index` is executed
**When** the indexing completes
**Then** subsequent coding workflows can leverage `memory_context`, `memory_impact`, and `memory_detect_changes` steps

---

## Epic 7: Structured Test Result Parsing (test-parser)

After this epic, the system parses raw test runner output into structured per-test results — identifying exactly which tests failed, where, and why — so that downstream retry and review steps receive precise, actionable failure context instead of raw stdout dumps.

### Story 7.1: WASM Crate Setup & Multi-Format Parser Core

As a workflow engine,
I want a test-parser WASM plugin that parses cargo test output into structured per-test results,
So that downstream steps receive machine-parseable test data instead of raw stdout.

**Acceptance Criteria:**

**Given** the crate `pulse-plugins/test-parser/` does not exist
**When** this story is implemented
**Then** a Rust crate exists with `Cargo.toml`, `lib.rs`, `config.rs`, `parser.rs`, `wit/` directory
**And** `lib.rs` uses `wit_bindgen::generate!` to implement the `step-executor-plugin` world
**And** `Cargo.toml` targets `wasm32-wasip2`

**Given** raw cargo test output is provided as input
**When** the parser processes it
**Then** it returns structured JSON with `total`, `passed`, `failed`, `skipped` counts
**And** each failing test includes `test_name`, `file_path`, `line_number` (if available), `assertion_message`, and `stdout` capture
**And** parsing completes within 50ms (NFR3)

**Given** a `TestParserConfig` struct exists with `#[serde(deny_unknown_fields)]`
**When** step config specifies `framework: "cargo-test"`
**Then** the parser uses the cargo test output format
**And** unknown `framework` values return `PluginError::configuration` with supported list

### Story 7.2: Additional Framework Parsers (npm test, pytest, JUnit XML)

As a workflow designer,
I want test-parser to support npm test (Jest/Mocha), pytest, and JUnit XML output formats,
So that auto-dev workflows work across polyglot codebases.

**Acceptance Criteria:**

**Given** raw Jest/Mocha output is provided with `framework: "jest"`
**When** the parser processes it
**Then** it returns structured results with `test_name`, `suite_name`, `error_message`, and `stack_trace` per failure
**And** `PASS`/`FAIL` summary lines are correctly parsed

**Given** raw pytest output is provided with `framework: "pytest"`
**When** the parser processes it
**Then** it returns structured results with `test_name`, `file_path::function_name`, `assertion_message`, and `short_test_summary`

**Given** JUnit XML input is provided with `framework: "junit-xml"`
**When** the parser processes it
**Then** it parses `<testcase>` elements, extracting `name`, `classname`, `time`, and `<failure>` message/type
**And** this serves as a universal fallback since many runners can produce JUnit XML

**Given** the test output doesn't match the expected format
**When** parsing fails gracefully
**Then** raw output is preserved in a fallback structured result with `format_detected: false`
**And** downstream steps still receive usable (raw) context

### Story 7.3: Workflow Integration & context_from Output Contract

As a workflow engine,
I want test-parser output to flow seamlessly into downstream steps via `context_from`,
So that review and retry steps receive structured failure data.

**Acceptance Criteria:**

**Given** a test-parser step completes with failures
**When** a downstream step specifies `context_from: ["run-tests"]`
**Then** it receives structured JSON including: `summary` (counts), `failures` (array of detailed test failures), `raw_output` (original stdout)
**And** `StepOutput.metadata` contains `plugin_name: "test-parser"`, `plugin_version`, `framework`, `tests_passed`, `tests_failed`, `tests_skipped`

**Given** a test-parser step completes with all tests passing
**When** the output is constructed
**Then** `failures` array is empty, `status: Success`
**And** `tests_failed: 0` in metadata

**Given** the existing `run-tests` function steps in workflow YAMLs
**When** this story is complete
**Then** a `parse-tests` step can be chained after `run-tests` to add structured parsing
**And** existing workflows continue to work without test-parser (backwards compatible)

---

## Epic 8: Iterative Fix Loops

After this epic, when tests fail during an auto-dev workflow, the system automatically retries implementation with the specific failure context — progressively refining the code until tests pass or the retry limit is reached, enabling self-correcting autonomous development.

### Story 8.1: Retry Loop Engine in Workflow Executor

As a workflow engine,
I want the executor to support a `retry` configuration on steps that re-executes the step with failure context when a downstream test step fails,
So that the system can self-correct without human intervention.

**Acceptance Criteria:**

**Given** a workflow step has `retry: { max_attempts: 3, on_failure_of: "run-tests" }`
**When** the `run-tests` step fails (non-zero exit code)
**Then** the executor re-executes the retryable step (e.g., `implement`) with the test failure output injected as additional context
**And** the retry count increments (attempt 2 of 3)
**And** after re-implementation, `run-tests` is re-executed automatically

**Given** the retry limit (e.g., 3) is reached and tests still fail
**When** the final attempt completes
**Then** the workflow marks the step as `Failed` with status `"retry_limit_reached"`
**And** downstream steps (review, commit) are blocked
**And** the final test output and all attempt summaries are included in the step output

**Given** a retried implementation succeeds on attempt N (tests pass)
**When** the retry loop exits
**Then** the step is marked `Success` with `metadata.attempts: N`
**And** downstream steps proceed normally

**Given** no `retry` configuration exists on a step
**When** a downstream test fails
**Then** behavior is unchanged — the test step fails and downstream is blocked (backwards compatible)

### Story 8.2: Progressive Context Enrichment

As a workflow engine,
I want each retry attempt to include progressively richer context — prior attempt's code changes, test failures, and error patterns,
So that the AI agent learns from each failure and converges toward a working solution.

**Acceptance Criteria:**

**Given** retry attempt 2 begins after attempt 1 failed tests
**When** the prompt is constructed
**Then** it includes: original task description, attempt 1's implementation summary, attempt 1's specific test failures (from test-parser if available, raw output otherwise)
**And** a directive: "Your previous implementation failed these tests. Fix the issues while preserving working functionality."

**Given** retry attempt 3 begins after attempt 2 also failed
**When** the prompt is constructed
**Then** it includes: original task, attempt 1 failures, attempt 2 failures, and a pattern analysis noting recurring vs new failures
**And** `metadata.retry_history` contains an array of `{ attempt, failures_count, new_failures, resolved_failures }`

**Given** structured test results are available from test-parser (Epic 7)
**When** retry context is assembled
**Then** only the specific failing test names, files, and assertion messages are injected (not full raw output)
**And** this produces a more focused retry prompt

**Given** structured test results are NOT available (test-parser not installed)
**When** retry context is assembled
**Then** raw test stdout/stderr is injected as context (graceful degradation)

### Story 8.3: Retry-Enabled Workflow Templates

As a developer,
I want the feature-dev, bug-fix, and story-dev workflow templates to include retry loops on implementation steps,
So that auto-dev runs self-correct on test failures without requiring re-submission.

**Acceptance Criteria:**

**Given** `coding-feature-dev.yaml` is updated
**When** the `implement` step is configured
**Then** it includes `retry: { max_attempts: 3, on_failure_of: "run_tests" }`
**And** the workflow DAG remains valid with retry edges

**Given** `coding-bug-fix.yaml` is updated
**When** the `implement_fix` step is configured
**Then** it includes `retry: { max_attempts: 2, on_failure_of: "run_tests" }`
**And** bug-fix uses fewer retries since the fix scope is narrower

**Given** `coding-story-dev.yaml` is updated
**When** the `implement` step is configured
**Then** it includes `retry: { max_attempts: 3, on_failure_of: "run_tests" }`

**Given** `coding-quick-dev.yaml` is updated
**When** the `implement` step is configured
**Then** it includes `retry: { max_attempts: 2, on_failure_of: "run_tests" }`

**Given** any retry-enabled workflow runs and all retries exhaust
**When** the workflow completes
**Then** the execution output clearly shows: total attempts, which tests failed each time, and whether any progress was made (tests resolved across attempts)

---

## Epic 9: Automated PR Pipeline

After this epic, coding workflows produce a complete, ready-to-review pull request as their final output — pushed to a branch with auto-generated title/body — so that the developer's only action is clicking "Merge."

### Story 9.1: Push & PR Steps in git-ops Executor

As a workflow engine,
I want the in-plugin executor to support `push` and `create-pr` function steps that call git-ops operations,
So that workflows can push branches and create PRs as automated pipeline steps.

**Acceptance Criteria:**

**Given** a function step specifies `command: ["plugin-git-ops", "push"]`
**When** the executor runs it
**Then** `git push -u origin {branch_name}` is executed in the workspace directory
**And** on success, `StepOutput.metadata` contains `branch`, `remote: "origin"`, `pushed: true`
**And** if the branch already has upstream tracking, `git push` runs without `-u`

**Given** a function step specifies `command: ["plugin-git-ops", "create-pr"]` with template vars `{{pr_title}}` and `{{pr_body}}`
**When** the executor runs on a GitHub repository
**Then** a POST request is made to `https://api.github.com/repos/{owner}/{repo}/pulls`
**And** the request includes `title`, `body`, `head` (current branch), `base` (default branch), `draft: false`
**And** the GitHub token is read from `GITHUB_TOKEN` environment variable
**And** `StepOutput.metadata` contains `pr_url`, `pr_number`, `platform: "github"`

**Given** the git remote URL matches a GitLab pattern
**When** `create-pr` is executed
**Then** a POST request is made to `{gitlab_base_url}/api/v4/projects/{id}/merge_requests`
**And** the token is read from `GITLAB_TOKEN` environment variable
**And** self-hosted GitLab instances use `base_url` from config

**Given** no platform token is available
**When** PR creation is attempted
**Then** the step fails with a clear error: "No {platform} token found. Set {ENV_VAR} environment variable."
**And** the token name is never included in logs or output beyond the env var name

### Story 9.2: Auto-Generated PR Title & Body

As a developer,
I want the system to auto-generate meaningful PR titles and bodies from workflow context,
So that PRs are immediately reviewable without manual description writing.

**Acceptance Criteria:**

**Given** a workflow completes with context from plan, implement, test, and review steps
**When** the PR creation step assembles the title
**Then** the title is derived from the original task description, truncated to 72 characters
**And** prefixed with the workflow type (e.g., `feat:`, `fix:`, `refactor:`)

**Given** workflow step outputs are available via `context_from`
**When** the PR body is assembled
**Then** it includes sections:
- **Summary:** condensed from the plan step output (what was done and why)
- **Changes:** file list from git diff
- **Test Results:** pass/fail summary from test step
- **Review Notes:** key findings from review step (if verdict was `approve`)
**And** the body is formatted in GitHub-flavored markdown
**And** a footer notes `Generated by Pulse Auto-Dev`

**Given** the implement step used retry loops (Epic 8)
**When** the PR body is assembled
**Then** it includes a note: "Implementation required {N} attempts" with brief summary of what was fixed across retries

**Given** the plan step output is very long (>2000 chars)
**When** the PR body is assembled
**Then** the summary is truncated with "..." and a note to see the full plan in workflow logs

### Story 9.3: PR Pipeline Integration in Coding Workflows

As a developer,
I want feature-dev, bug-fix, and story-dev workflows to automatically push and create PRs as their final steps,
So that submitting a task produces a ready-to-review PR with zero manual git operations.

**Acceptance Criteria:**

**Given** `coding-feature-dev.yaml` is updated
**When** the workflow is loaded
**Then** after `git_commit`, three new steps exist:
- `git_push` — function step calling `plugin-git-ops push`, `depends_on: [git_commit]`
- `generate_pr_body` — agent step that synthesizes PR content from `context_from: [architect, implement, run_tests, qa_review]`
- `create_pr` — function step calling `plugin-git-ops create-pr`, `depends_on: [git_push, generate_pr_body]`
**And** `create_pr` is marked `optional: true` (PR creation should not fail the workflow if token is missing)

**Given** `coding-bug-fix.yaml` is updated
**When** the workflow is loaded
**Then** equivalent push + PR steps exist after commit, with PR title prefix `fix:`

**Given** `coding-story-dev.yaml` is updated
**When** the workflow is loaded
**Then** equivalent push + PR steps exist after commit

**Given** `coding-quick-dev.yaml` is updated
**When** the workflow is loaded
**Then** push + PR steps are added but both marked `optional: true` (quick dev may not always need a PR)

**Given** any workflow runs without a platform token configured
**When** the `create_pr` step is reached
**Then** it is skipped gracefully (marked `optional: true`)
**And** the workflow still reports overall `Success` with a note: "PR creation skipped — no platform token configured"
**And** the committed branch is still pushed and available for manual PR creation

---

## Epic 10: Parallel Step Execution

After this epic, independent workflow steps (e.g., adversarial review + edge-case review) execute concurrently instead of sequentially — reducing total workflow execution time by up to 40% for pipelines with parallel branches.

### Story 10.1: Concurrent Step Dispatcher

As a workflow engine,
I want the executor to identify independent steps at the same DAG level and execute them concurrently using a thread pool,
So that workflows with parallel branches complete faster.

**Acceptance Criteria:**

**Given** a workflow DAG has steps A and B that both depend only on step C (no edge between A and B)
**When** step C completes successfully
**Then** steps A and B are dispatched concurrently, not sequentially
**And** both steps' outputs are collected before any step depending on A or B is dispatched

**Given** the executor's topological sort groups steps by dependency level
**When** a level contains multiple steps with satisfied dependencies
**Then** all steps in that level are spawned as concurrent tasks
**And** the executor waits for all tasks in the level to complete before advancing to the next level

**Given** a configurable concurrency limit exists (default: 4, configurable via `config/config.yaml` as `executor.max_parallel_steps`)
**When** a level has more steps than the concurrency limit
**Then** steps are dispatched in batches of `max_parallel_steps`
**And** the next batch starts only after a slot frees up

**Given** the executor uses `std::thread` (not async tokio) for process management
**When** concurrent steps are dispatched
**Then** each step runs in its own thread with its own `std::process::Command` child process
**And** thread panics are caught and mapped to `StepStatus::Failed` without crashing the executor

**Given** a workflow has no parallel branches (fully sequential DAG)
**When** the executor runs
**Then** behavior is identical to the current sequential executor (backwards compatible)

### Story 10.2: Concurrent Output Collection & Context Assembly

As a workflow engine,
I want concurrent step outputs to be safely collected and made available via `context_from` to downstream steps,
So that parallel execution doesn't break the context injection mechanism.

**Acceptance Criteria:**

**Given** steps A and B run concurrently and both complete
**When** step D specifies `context_from: ["A", "B"]`
**Then** it receives both outputs, regardless of which completed first
**And** output ordering in the context is deterministic (alphabetical by step_id)

**Given** step A succeeds but step B fails during concurrent execution
**When** step D depends on both A and B
**Then** step D is blocked (dependency not satisfied)
**And** step A's output is still stored and available for any step that depends only on A

**Given** concurrent steps both write to `template_vars` (e.g., `session_id`, `branch_name`)
**When** outputs are merged
**Then** thread-safe collection (e.g., `Arc<Mutex<HashMap>>`) prevents data races
**And** if both steps produce `session_id`, the later-completing step's value wins with a `tracing::debug!` noting the override

**Given** step A times out during concurrent execution
**When** the timeout fires
**Then** only step A's child process is killed (SIGTERM → SIGKILL)
**And** step B continues executing unaffected
**And** step A is marked `StepStatus::TimedOut`

### Story 10.3: Parallel Review Workflow Optimization

As a developer,
I want the coding-review workflow's adversarial and edge-case review steps to execute in parallel,
So that code reviews complete in half the time.

**Acceptance Criteria:**

**Given** `coding-review.yaml` has `adversarial_review` and `edge_case_review` steps
**When** both steps depend only on `memory_context` (or have no shared dependencies beyond the input)
**Then** the executor dispatches them concurrently
**And** the final `synthesis` step waits for both reviews to complete before running

**Given** `coding-feature-dev.yaml` has sequential steps that could be parallelized
**When** the workflow DAG is analyzed
**Then** `memory_context` and `architect` are identified as the only truly sequential pair
**And** no additional parallelization is possible in the feature-dev pipeline (each step depends on the previous)

**Given** a new workflow template `coding-parallel-review.yaml` is created
**When** it defines 3 parallel review agents (adversarial, edge-case, security)
**Then** all 3 run concurrently with `depends_on: [implement]`
**And** a `review_synthesis` step with `depends_on: [adversarial_review, edge_case_review, security_review]` and `context_from` from all three produces a unified verdict

**Given** the parallel executor is enabled
**When** execution logs are reviewed
**Then** concurrent steps show overlapping timestamps
**And** `tracing::info!` logs include `parallel_batch: N` field indicating which concurrency batch each step belongs to

---

## Epic 11: E2E Integration Testing & Validation

After this epic, a comprehensive integration test harness validates the full auto-dev cycle against a real codebase — from submission through PR creation — providing confidence that the system works end-to-end in production.

### Story 11.1: Test Fixture Project & Harness Setup

As a platform engineer,
I want a sample Rust project as a test fixture and a test harness that can invoke the full workflow executor,
So that E2E tests run against a realistic codebase with real git operations.

**Acceptance Criteria:**

**Given** the directory `tests/fixtures/sample-project/` does not exist
**When** this story is implemented
**Then** a minimal Rust project exists with:
- `Cargo.toml` declaring a library crate
- `src/lib.rs` with 2-3 simple functions and existing tests
- `.git/` initialized as a valid git repository with at least one commit
- A deliberately incomplete function (stub) that the auto-dev agent can implement

**Given** a test harness module `tests/e2e/` exists
**When** the harness is initialized for a test
**Then** it creates a temporary git worktree from the fixture project
**And** sets up the working directory, config, and plugin paths
**And** provides helper functions: `submit_workflow(name, input)`, `assert_step_status(step_id, expected)`, `read_step_output(step_id)`

**Given** the test harness invokes the workflow executor
**When** a workflow runs
**Then** it uses the real `execute_workflow()` function from `src/executor.rs`
**And** plugin binaries are resolved from `config/plugins/`
**And** the test captures the full execution result JSON

**Given** the test completes (pass or fail)
**When** cleanup runs
**Then** the temporary worktree is deleted
**And** no orphaned processes remain

### Story 11.2: Happy Path E2E — Feature Dev Full Cycle

As a platform engineer,
I want an E2E test that validates the complete feature-dev workflow (plan → implement → test → review → commit),
So that we have automated proof the full auto-dev loop works.

**Acceptance Criteria:**

**Given** the sample project fixture and test harness are ready
**When** `submit_workflow("coding-feature-dev", {"input": "Add a function multiply(a, b) that returns a * b with unit tests"})` is called
**Then** the workflow executes all steps in order: `memory_context` (skipped if no memory), `architect`, `implement`, `run_tests`, `qa_review`, `git_commit`

**Given** the workflow completes successfully
**When** the execution result is inspected
**Then** `status: "completed"` at the workflow level
**And** `architect` step produced a plan mentioning `multiply`
**And** `implement` step modified `src/lib.rs` to add the function
**And** `run_tests` step exited with code 0 (all tests pass, including new ones)
**And** `qa_review` step produced `verdict: approve`
**And** `git_commit` step created a commit with the changes

**Given** the git history of the test worktree is inspected after the workflow
**When** `git log --oneline -1` is run
**Then** the most recent commit contains the implementation changes
**And** the commit message includes `Co-authored-by: pulse-auto-dev`

**Given** this E2E test is run in CI
**When** the `claude` CLI is not available
**Then** the test is marked `#[ignore]` with comment "Requires claude CLI and API key"
**And** a CI-specific flag `PULSE_E2E_ENABLED=1` controls whether ignored tests run

### Story 11.3: Failure Path E2E — Quality Gate Blocks Commit

As a platform engineer,
I want an E2E test that validates the quality gate blocks commits when tests fail or review rejects,
So that we have automated proof the safety mechanisms work.

**Acceptance Criteria:**

**Given** the sample project fixture is modified to have a deliberately failing test
**When** `submit_workflow("coding-quick-dev", {"input": "Add a function divide(a, b) that returns a / b"})` is called
**Then** the `implement` step succeeds (agent writes code)
**And** the `run_tests` step fails (the deliberate failing test triggers)

**Given** the workflow has a quality gate
**When** the `run_tests` step fails
**Then** `git_commit` step is skipped (blocked by failed dependency)
**And** the workflow status is `"failed"`
**And** no commit was created in the worktree

**Given** the sample project is configured to trigger a review rejection
**When** a workflow runs with a review step that produces `verdict: request-changes`
**Then** the `git_commit` step is blocked
**And** the execution result includes the review findings in the step output

**Given** both failure E2E tests run
**When** the tests complete
**Then** they verify: no accidental commits, no orphaned processes, worktree is clean after failure

### Story 11.4: Retry Loop E2E Validation

As a platform engineer,
I want an E2E test that validates the iterative fix loop works end-to-end,
So that we have automated proof that self-correction produces working code.

**Acceptance Criteria:**

**Given** the sample project fixture has a function stub and a test that will fail on naive implementation
**When** `submit_workflow("coding-feature-dev", {"input": "Implement safe_divide(a, b) that returns Result<f64, String> — return Err for division by zero"})` is called with retry enabled
**Then** if the first attempt misses the zero-check edge case, the retry loop catches it via test failure
**And** the second attempt receives the failing test context and adds the zero-check
**And** the workflow eventually succeeds (within max_attempts)

**Given** the retry loop E2E test completes
**When** the execution result is inspected
**Then** `metadata.attempts` shows how many tries were needed
**And** `retry_history` shows the progression from failure to success
**And** the final commit contains the correct implementation

**Given** this test may be flaky (LLM output is non-deterministic)
**When** the test is configured
**Then** it allows up to 3 attempts at the workflow level (test-level retry)
**And** success on any attempt counts as a pass
**And** `#[ignore]` with `PULSE_E2E_ENABLED=1` gate applies
