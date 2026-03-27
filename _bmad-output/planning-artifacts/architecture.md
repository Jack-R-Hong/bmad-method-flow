---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
lastStep: 8
status: 'in-progress'
completedAt: '2026-03-23'
lastUpdated: '2026-03-27'
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/architecture-v1.md'
  - '_bmad-output/planning-artifacts/epics.md'
  - 'docs/architecture.md'
  - 'docs/project-overview.md'
  - 'src/lib.rs'
  - 'src/executor.rs'
  - 'src/workspace.rs'
  - 'src/pack.rs'
  - 'src/validator.rs'
  - '../../pulse/crates/pulse-plugin-sdk/src/traits/agent_definition.rs'
  - '../../pulse/crates/pulse-plugin-sdk/src/traits/agent_mesh.rs'
  - '../../pulse/crates/pulse-plugin-sdk/src/traits/llm_provider.rs'
  - '../../pulse/crates/pulse-plugin-sdk/src/types/llm.rs'
  - '../../pulse/crates/pulse-plugin-sdk/src/types/tools.rs'
supersedes: '_bmad-output/planning-artifacts/architecture-v1.md'
workflowType: 'architecture'
project_name: 'bmad-method-flow'
user_name: 'Jack'
date: '2026-03-23'
---

# Architecture Decision Document — SDK Integration & Agent Mesh

_This document defines the architecture for integrating new pulse-plugin-sdk capabilities (agent definitions, agent mesh, tool calling, unified LLM types) and provider-claude-code agent mesh features into plugin-coding-pack. It supersedes architecture-v1.md for all affected modules. Breaking changes are expected._

## Project Context Analysis

### Requirements Overview

**Integration Scope:**
This is a brownfield integration of upstream SDK and provider capabilities into an existing, working plugin. The plugin-coding-pack already has a functioning DAG executor, two-stage agent execution, quality gates, retry loops, and workspace configuration. The integration adds:

1. **AgentDefinitionProvider** — register BMAD agents as `SdkAgentDefinition` for platform-wide discovery
2. **Agent Mesh** — enable dynamic inter-agent invocation via MCP, replacing rigid DAG dependencies for certain workflows
3. **Unified LLM types** — adopt `CompletionRequest`/`CompletionResponse` with `ResponseChoice::ToolCalls`
4. **ToolExecutor** — async tool execution during LLM conversations
5. **Multi-agent sessions** — `session` step type for deliberation (architecture review, code review)
6. **agents.yaml generation** — generate agent mesh ACL configuration from BMAD persona definitions

**Functional Requirements (new/modified):**

- FR-SDK-1: Plugin implements `AgentDefinitionProvider` trait, exposing 9 BMAD agents via `list_agents()` and `get_agent(name)`
- FR-SDK-2: Each BMAD agent maps to an `SdkAgentDefinition` with `name`, `description`, `system_prompt`, `model_tier`, `skills`, and `tools`
- FR-SDK-3: Executor injects `agent_name`, `mcp_config`, and `env_vars` into provider-claude-code JSON-RPC parameters for agent mesh steps
- FR-SDK-4: Executor sets `PULSE_AGENT_DEPTH` env var and enforces max depth 5 to prevent infinite agent recursion
- FR-SDK-5: New `session` step type supports multi-agent deliberation with configurable `ActivationStrategy` and `ConvergenceStrategy`
- FR-SDK-6: Workspace config resolves agent mesh settings from `config/config.yaml` (new `agent_mesh` section)
- FR-SDK-7: Plugin generates `agents.yaml` from BMAD agent definitions with `can_invoke` and `can_respond_to` ACL fields
- FR-SDK-8: Executor supports `CompletionRequest`/`CompletionResponse` types for future direct LLM invocation (alongside existing JSON-RPC)
- FR-SDK-9: Agent steps can include `tools` in their config, forwarded as `ToolDef` to the LLM provider
- FR-SDK-10: Plugin exposes `list_agents` and `invoke_agent` via MCP stdio JSON-RPC when operating in mesh mode

**Non-Functional Requirements:**

- NFR-SDK-1: Agent mesh depth guard adds <1ms overhead per step
- NFR-SDK-2: `agents.yaml` generation completes in <50ms for 9 agents
- NFR-SDK-3: `AgentDefinitionProvider::list_agents()` returns in <1ms (in-memory data)
- NFR-SDK-4: Session step convergence evaluation adds <10ms per turn
- NFR-SDK-5: Breaking changes to existing executor types are acceptable; no backward compatibility required

**Scale & Complexity:**

- Primary domain: Rust plugin extension — adding new traits, types, and execution paths to existing codebase
- Complexity level: Medium-High — multiple new abstractions, MCP integration, ACL generation
- Estimated new/modified modules: 4 new files, 3 modified files

### Technical Constraints & Dependencies

- **pulse-plugin-sdk**: New traits (`AgentDefinitionProvider`, `AgentMeshProvider`, `LlmProvider`, `ToolExecutor`) and types (`SdkAgentDefinition`, `SdkAgentMeshConfig`, `CompletionRequest`, `CompletionResponse`, `ChatMessage`, `ToolDef`, `ToolCall`, `ToolResult`, `ModelInfo`, `LlmProviderMeta`, `TokenUsage`) are available at `../../pulse/crates/pulse-plugin-sdk`
- **provider-claude-code**: Must accept new config fields: `agent_name`, `mcp_config`, `env_vars`, `tools`. Assumed to already support or will be updated to support these (upstream dependency).
- **No async runtime in main crate**: The plugin uses `std::process::Command` + `std::thread` for process management. `ToolExecutor` is an `async_trait` — the plugin will need to bridge sync/async or use a lightweight async runtime for tool execution.
- **WASM target**: The `cdylib` output must remain WASM-compatible for the plugin loader. `async_trait` and `tokio` are only in dev-dependencies.

### Cross-Cutting Concerns Identified

- **Agent identity propagation**: `agent_name` must flow from workflow step config through executor to provider-claude-code, and be set as `PULSE_AGENT_NAME` env var for child processes
- **Depth guard enforcement**: `PULSE_AGENT_DEPTH` must be incremented before spawning any agent and checked at the boundary
- **Config backward compatibility**: New `agent_mesh` section in `config/config.yaml` must be optional — existing configs without it must continue to work
- **ACL consistency**: `agents.yaml` generation must produce consistent output from the same BMAD definitions (deterministic ordering)
- **Session state**: Multi-agent sessions need state tracking across turns, but the executor is currently stateless per step

## Starter Template Evaluation

### Primary Technology Domain

Rust plugin development — brownfield extension of an existing WASM-compatible plugin crate.

### Starter Options Considered

Not applicable. This is an extension of an existing, working codebase (`plugin-coding-pack` v0.1.0). No starter template needed. The existing crate structure, build tooling, and dependency graph are established.

### Established Technical Foundations

**Language & Runtime:**
- Rust edition 2021, MSRV 1.85
- `std::process::Command` + `std::thread` for process management (no async runtime in production)
- `pulse-plugin-sdk` for all SDK traits and types

**Build & Deployment:**
- `crate-type = ["cdylib", "rlib"]` for WASM + native
- Binary target at `src/main.rs` for CLI/stdio adapter
- Integration testing via `pulse-plugin-test` with WASM harness

**Serialization & Config:**
- `serde` 1.0 + `serde_json` for all data types
- `serde_yaml` 0.9 for workflow and workspace configuration
- All config structs use `#[serde(default)]` for optional fields

## Core Architectural Decisions

### Decision Priority Analysis

**Critical Decisions (Block Implementation):**
1. Agent definition registry — how BMAD agents are represented as `SdkAgentDefinition`
2. Agent mesh injection — how executor passes mesh config to provider-claude-code
3. Depth guard mechanism — how recursion is prevented in agent mesh
4. `agents.yaml` generation — how ACL config is derived from BMAD agents

**Important Decisions (Shape Architecture):**
5. Session step type — how multi-agent deliberation is orchestrated
6. Unified LLM type adoption — how `CompletionRequest`/`CompletionResponse` integrate
7. Tool calling support — how `ToolDef` flows to providers

**Deferred Decisions (Post-Integration):**
- Direct LLM invocation (bypassing JSON-RPC for in-process providers)
- Cost tracking via `TokenUsage` and cost-tracker plugin integration
- `LlmProviderMeta` registration for model routing
- Multi-agent session persistence across workflow restarts

### Decision 1: BMAD Agent Registry as `AgentDefinitionProvider`

- **Decision:** Implement `AgentDefinitionProvider` as a new struct `BmadAgentRegistry` in `src/agents.rs` that returns hardcoded `SdkAgentDefinition` instances for all 9 BMAD agents
- **Rationale:** The 9 BMAD agents (architect, dev, qa, pm, sm, tech-writer, ux-designer, analyst, quick-flow-solo-dev) have stable definitions. A static registry avoids YAML parsing at runtime and keeps agent definitions co-located with the plugin that owns them. Platform-wide agent discovery queries `AgentDefinitionProvider::list_agents()` which returns owned `Vec<SdkAgentDefinition>` — no lifetime complications.
- **Agent mapping:**

| BMAD Agent | SDK Name | Skills | Model Tier |
|---|---|---|---|
| Winston (Architect) | `bmad/architect` | `["architecture", "system-design", "rust"]` | `smart` |
| Amelia (Developer) | `bmad/developer` | `["coding", "rust", "implementation"]` | `balanced` |
| Quinn (QA) | `bmad/qa` | `["testing", "code-review", "quality"]` | `balanced` |
| Bob (PM) | `bmad/pm` | `["planning", "requirements", "prioritization"]` | `balanced` |
| John (SM) | `bmad/sm` | `["agile", "process", "coordination"]` | `fast` |
| Sally (Tech Writer) | `bmad/tech-writer` | `["documentation", "technical-writing"]` | `fast` |
| Mary (UX Designer) | `bmad/ux-designer` | `["ux", "design", "accessibility"]` | `balanced` |
| Paige (Analyst) | `bmad/analyst` | `["analysis", "data", "research"]` | `balanced` |
| Barry (Quick Dev) | `bmad/quick-dev` | `["coding", "rust", "rapid-prototyping"]` | `fast` |

- **Affects:** `src/agents.rs` (new), `src/lib.rs` (implement trait on `CodingPackPlugin` or expose registry)

### Decision 2: Agent Mesh Config Injection into Provider-Claude-Code

- **Decision:** When a workflow step has `mesh_enabled: true` in its config, the executor enriches the JSON-RPC `parameters` object with three new fields: `agent_name` (string), `mcp_config` (JSON object defining MCP servers for agent mesh), and `env_vars` (key-value pairs including `PULSE_AGENT_DEPTH`)
- **Rationale:** provider-claude-code already accepts an opaque `parameters` map. Adding fields to this map is the least-invasive integration path. provider-claude-code is expected to pass `agent_name` through to the Claude CLI's `--agent-name` flag (or equivalent), inject `mcp_config` as additional MCP server definitions, and set `env_vars` on the spawned child process.
- **MCP config shape:**
```json
{
  "agent_name": "bmad/architect",
  "mcp_config": {
    "mcpServers": {
      "pulse-agents": {
        "command": "plugin-coding-pack",
        "args": ["--mcp-mode"],
        "env": {
          "PULSE_AGENT_DEPTH": "2"
        }
      }
    }
  },
  "env_vars": {
    "PULSE_AGENT_DEPTH": "2",
    "PULSE_AGENT_NAME": "bmad/architect"
  }
}
```
- **Affects:** `src/executor.rs` (agent step execution), `src/workspace.rs` (mesh config resolution)

### Decision 3: Depth Guard via Environment Variable

- **Decision:** The executor reads `PULSE_AGENT_DEPTH` from the environment (defaulting to 0), increments it by 1, and passes the incremented value to child processes. If the incremented value exceeds `max_depth` (default 5, configurable via workspace config), the step fails immediately with `WitPluginError::invalid_input("agent mesh depth limit exceeded")`.
- **Rationale:** Environment variable propagation is the simplest cross-process mechanism. It works whether agents are invoked via JSON-RPC, MCP, or direct process spawning. The check is O(1) — a single env var read + integer comparison at step start.
- **Implementation:**
```rust
fn check_depth_guard(config: &WorkspaceConfig) -> Result<u32, WitPluginError> {
    let current: u32 = std::env::var("PULSE_AGENT_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let next = current + 1;
    let max = config.agent_mesh.as_ref()
        .map(|m| m.max_depth)
        .unwrap_or(5);
    if next > max {
        return Err(WitPluginError::invalid_input(
            format!("agent mesh depth limit exceeded: {} > {}", next, max)
        ));
    }
    Ok(next)
}
```
- **Affects:** `src/executor.rs` (depth check before agent steps), `src/workspace.rs` (max_depth config)

### Decision 4: `agents.yaml` Generation from BMAD Definitions

- **Decision:** Add a new action `generate-agents-yaml` to `src/pack.rs` that reads the `BmadAgentRegistry`, generates an `agents.yaml` file conforming to provider-claude-code's expected schema, and writes it to `config/agents.yaml`.
- **Rationale:** The ACL configuration for agent mesh (`can_invoke`, `can_respond_to`, `model`, `max_turns`, `max_budget_usd`, `timeout_secs`, `allowed_tools`) should be derived from the authoritative BMAD agent definitions rather than manually maintained. Generated config prevents drift between the plugin's agent registry and the mesh ACL.
- **Default ACL rules:**
  - All agents can invoke `bmad/developer` (the implementation agent)
  - `bmad/architect` can invoke `bmad/analyst`, `bmad/ux-designer`
  - `bmad/qa` can invoke `bmad/developer` (for fix requests)
  - `bmad/quick-dev` has `can_invoke: []` (self-contained, no mesh)
  - All agents can respond to `bmad/pm` and `bmad/sm` (coordination agents)
- **Generated schema:**
```yaml
agents:
  bmad/architect:
    description: "Winston — Senior Solutions Architect"
    model: "claude-sonnet-4-20250514"
    max_turns: 10
    max_budget_usd: 2.0
    timeout_secs: 600
    can_invoke:
      - bmad/developer
      - bmad/analyst
      - bmad/ux-designer
    can_respond_to:
      - bmad/pm
      - bmad/sm
    allowed_tools:
      - Read
      - Glob
      - Grep
      - Bash
  # ... (other agents follow same pattern)
```
- **Affects:** `src/pack.rs` (new action), `src/agents.rs` (ACL definitions)

### Decision 5: `session` Step Type for Multi-Agent Deliberation

- **Decision:** Add a new step type `session` to the executor that orchestrates a multi-turn conversation between 2+ agents. The session runs for a configurable number of turns or until a convergence condition is met.
- **Rationale:** Architecture reviews and code reviews benefit from adversarial multi-agent deliberation (architect vs QA, reviewer vs developer). The current `agent` step type is single-agent, single-turn. A `session` step type enables structured debates where each agent sees the full conversation history.
- **Session config in workflow YAML:**
```yaml
- id: architecture_review
  type: session
  config:
    participants:
      - agent: bmad/architect
        activation: every_turn
      - agent: bmad/qa
        activation: every_turn
    convergence:
      strategy: fixed_turns
      max_turns: 4
    system_prompt: "Review the proposed architecture. Debate trade-offs."
    context_from: [plan_step]
```
- **Activation strategies (from SDK):**
  - `every_turn` — agent speaks every turn (default)
  - `when_mentioned` — agent speaks only when @mentioned
  - `on_tag` — agent speaks when a topic tag matches
  - `keyword_match` — agent speaks when keywords appear
- **Convergence strategies:**
  - `fixed_turns` — stop after N turns (default: 4)
  - `unanimous` — stop when all agents agree on verdict
  - `stagnation` — stop when no new points are raised for N turns
- **Implementation:** Each turn calls provider-claude-code with the full conversation history as `ChatMessage` array. The executor assembles messages, evaluates activation, checks convergence, and produces a final summary.
- **Affects:** `src/executor.rs` (new `execute_session_step()`), `src/session.rs` (new module for session logic)

### Decision 6: Unified LLM Type Adoption

- **Decision:** Add `CompletionRequest`, `CompletionResponse`, `ChatMessage`, `TokenUsage`, `ResponseChoice`, `ToolDef`, and `ToolCall` types from `pulse_plugin_sdk::types::llm` as re-exports in the plugin. The executor uses these types internally for building LLM requests and parsing responses, but continues to communicate with provider-claude-code via JSON-RPC (which serializes these types as JSON).
- **Rationale:** The SDK types provide compile-time type safety for LLM interactions. Using them internally prevents mismatched field names and ensures compatibility with future direct LLM invocation (when the executor bypasses JSON-RPC to call `LlmProvider::complete()` directly). The JSON-RPC transport serializes/deserializes these types transparently via serde.
- **Migration path:** Phase 1 (this integration) uses SDK types for internal construction but serializes to JSON-RPC. Phase 2 (future) adds direct `LlmProvider::complete()` calls for in-process providers.
- **Affects:** `src/executor.rs` (request construction), `Cargo.toml` (SDK types already available via `pulse-plugin-sdk`)

### Decision 7: Tool Calling Support via Config

- **Decision:** Workflow step config gains an optional `tools` field (array of tool definitions). When present, the executor includes these as `ToolDef` objects in the JSON-RPC parameters, enabling provider-claude-code to expose them to the LLM as callable functions.
- **Rationale:** Tool calling is a first-class LLM capability. Workflow designers should be able to specify which tools an agent step can use. The executor does not execute tools itself — it passes tool definitions to the provider, which manages the tool call loop.
- **Workflow YAML example:**
```yaml
- id: implement
  type: agent
  executor: bmad-method
  config:
    system_prompt: "bmad/developer — implement the feature"
    tools:
      - name: "run_tests"
        description: "Run the project test suite"
        parameters: { "type": "object", "properties": {} }
      - name: "read_file"
        description: "Read a file from the workspace"
        parameters: { "type": "object", "properties": { "path": { "type": "string" } } }
```
- **Affects:** `src/executor.rs` (tool field forwarding), workflow YAML schemas

### Decision Impact Analysis

**Implementation Sequence:**
1. `src/agents.rs` — Agent registry (Decision 1) — foundational, no dependencies
2. `src/workspace.rs` — Agent mesh config (Decision 2 partial) — extends existing module
3. `src/executor.rs` — Depth guard (Decision 3) + mesh injection (Decision 2) + tool forwarding (Decision 7) — modifies existing execution paths
4. `src/session.rs` — Session step type (Decision 5) — new module, depends on executor
5. `src/pack.rs` — `generate-agents-yaml` action (Decision 4) — depends on agent registry
6. SDK type adoption (Decision 6) — can be done incrementally alongside other changes

**Cross-Component Dependencies:**
- `agents.rs` is consumed by `pack.rs` (YAML generation), `executor.rs` (agent lookup), and `lib.rs` (trait impl)
- `workspace.rs` mesh config is consumed by `executor.rs` (depth guard, mesh injection)
- `session.rs` depends on `executor.rs` internal functions (agent step execution, template substitution)
- All changes depend on `pulse-plugin-sdk` new types being available at compile time

## Implementation Patterns & Consistency Rules

### Pattern Categories Defined

**5 critical conflict points identified** where implementation choices must be consistent to avoid integration failures.

### Agent Definition Pattern

All BMAD agents are defined using a consistent builder pattern in `src/agents.rs`:

```rust
use pulse_plugin_sdk::traits::agent_definition::SdkAgentDefinition;

pub fn bmad_architect() -> SdkAgentDefinition {
    SdkAgentDefinition {
        name: "bmad/architect".to_string(),
        description: Some("Winston — Senior Solutions Architect. Designs system architecture, evaluates trade-offs, and ensures technical coherence.".to_string()),
        system_prompt: Some(ARCHITECT_SYSTEM_PROMPT.to_string()),
        model_tier: Some("smart".to_string()),
        model: None, // resolved at runtime via workspace config
        tools: Some(vec!["Read".into(), "Glob".into(), "Grep".into(), "Bash".into()]),
        skills: Some(vec!["architecture".into(), "system-design".into(), "rust".into()]),
        max_tokens: Some(8192),
        temperature: Some(0.3),
        max_tool_rounds: None,
    }
}
```

**Rules:**
- Agent names use `bmad/` prefix followed by kebab-case role name
- `description` always includes the persona name and a one-line role summary
- `model` is always `None` — runtime resolution via workspace defaults or workflow config
- `model_tier` maps to the agent's typical complexity needs (`smart`, `balanced`, `fast`)
- `skills` are lowercase kebab-case tags used by `match_skills()`
- `tools` lists Claude Code tool names the agent is allowed to use
- System prompts are defined as constants in `src/agents.rs` (not inline strings)

### Workspace Config Extension Pattern

New config sections are added as optional fields with `#[serde(default)]`:

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AgentMeshSettings {
    /// Whether agent mesh is enabled for this workspace
    #[serde(default)]
    pub enabled: bool,
    /// Maximum recursion depth (default: 5)
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Path to agents.yaml (default: "config/agents.yaml")
    #[serde(default)]
    pub agents_yaml_path: Option<String>,
}

fn default_max_depth() -> u32 { 5 }
```

**Rules:**
- All new config fields are optional with sensible defaults
- Existing `config/config.yaml` files without the new section continue to work unchanged
- New sections are documented with inline YAML comments in examples
- Config structs always derive `Debug, Clone, Default, Deserialize`
- No `#[serde(deny_unknown_fields)]` on workspace config — forward-compatible parsing

### Session Step Execution Pattern

Session steps follow a turn-based loop pattern:

```rust
fn execute_session_step(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    template_vars: &HashMap<String, String>,
    plugins_dir: &Path,
    ws_config: &WorkspaceConfig,
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let session_config = parse_session_config(step)?;
    let mut conversation: Vec<ChatMessage> = Vec::new();
    let mut turn = 0;

    // Initial context injection
    let context = assemble_context(step, outputs);
    if !context.is_empty() {
        conversation.push(ChatMessage::user(context));
    }

    loop {
        turn += 1;
        for participant in &session_config.participants {
            if !should_activate(participant, turn, &conversation) {
                continue;
            }
            let response = call_agent_in_session(
                participant, &conversation, template_vars, plugins_dir,
            )?;
            conversation.push(ChatMessage::assistant(response));
        }
        if check_convergence(&session_config.convergence, turn, &conversation) {
            break;
        }
    }

    // Produce summary from final conversation state
    build_session_output(step, &conversation, turn)
}
```

**Rules:**
- Session conversation history is maintained as `Vec<ChatMessage>` (SDK type)
- Each agent turn is a separate provider-claude-code call with full history
- Convergence check happens after each complete round (all participants spoke)
- Session output includes the full conversation transcript and a summary
- Depth guard is checked once at session start, not per turn

### JSON-RPC Parameter Extension Pattern

When adding new fields to JSON-RPC parameters, follow this pattern:

```rust
// Always add new fields conditionally — don't send nulls
if let Some(mesh_config) = build_mesh_config(step, ws_config, depth) {
    parameters.insert("agent_name".to_string(), serde_json::json!(mesh_config.agent_name));
    parameters.insert("mcp_config".to_string(), serde_json::json!(mesh_config.mcp_config));
    parameters.insert("env_vars".to_string(), serde_json::json!(mesh_config.env_vars));
}

if let Some(tools) = &step_config.tools {
    parameters.insert("tools".to_string(), serde_json::json!(tools));
}
```

**Rules:**
- New parameters are only included when the feature is enabled (conditional insertion)
- Never send `null` or empty values for optional parameters — omit entirely
- All parameter values must be serializable via `serde_json::json!()` macro
- Parameter keys use `snake_case` (consistent with existing convention)
- New parameters never shadow existing ones (`system_prompt`, `model_tier`, `session_id`, `working_dir`)

### YAML Generation Pattern

Generated YAML files use `serde_yaml` with deterministic ordering:

```rust
use std::collections::BTreeMap; // NOT HashMap — deterministic key order

fn generate_agents_yaml(registry: &BmadAgentRegistry) -> String {
    let mut agents: BTreeMap<String, serde_yaml::Value> = BTreeMap::new();
    for agent in registry.list_agents() {
        let entry = build_agent_yaml_entry(&agent);
        agents.insert(agent.name.clone(), entry);
    }
    let root = serde_yaml::to_value(
        BTreeMap::from([("agents".to_string(), serde_yaml::to_value(&agents).unwrap())])
    ).unwrap();
    serde_yaml::to_string(&root).unwrap()
}
```

**Rules:**
- Use `BTreeMap` for all generated YAML maps — deterministic alphabetical key order
- Generated files include a header comment: `# Generated by plugin-coding-pack. Do not edit manually.`
- Generated files are written atomically (write to temp file, then rename)
- Existing `agents.yaml` is overwritten without merge — generation is idempotent

### Enforcement Guidelines

**All AI Agents MUST:**
- Use `SdkAgentDefinition` from `pulse_plugin_sdk::traits::agent_definition` — never define ad-hoc agent structs
- Use `ChatMessage` from `pulse_plugin_sdk::types::llm` for conversation history — never use raw strings
- Check `PULSE_AGENT_DEPTH` before any agent invocation — never skip the depth guard
- Use `BTreeMap` for all generated config files — never use `HashMap` for serialized output
- Run `cargo clippy -- -D warnings` and `cargo fmt --check` before marking work complete
- Map all errors to `WitPluginError` variants — no `unwrap()` or `expect()` in non-test code

**Anti-Patterns (NEVER do):**
- Hard-coding agent names as bare strings outside `agents.rs` — always reference registry constants
- Sending `PULSE_AGENT_DEPTH` without incrementing — always pass `current + 1`
- Using `HashMap` for YAML generation — causes non-deterministic output
- Implementing `AgentDefinitionProvider` on `CodingPackPlugin` directly — use a separate `BmadAgentRegistry` struct for testability
- Adding async runtime to production code — keep using `std::process::Command` + `std::thread`
- Modifying `agents.yaml` manually — always regenerate from registry

## Project Structure & Boundaries

### Complete Project Directory Structure

```
src/
├── lib.rs              # Crate root: pub mod declarations, plugin trait impls, re-exports
│                         MODIFIED: add `pub mod agents;` and `pub mod session;`
│                         MODIFIED: impl AgentDefinitionProvider for BmadAgentRegistry
├── main.rs             # Binary entry point (unchanged)
├── agents.rs           # NEW: BmadAgentRegistry, SdkAgentDefinition instances,
│                         agent constants, ACL definitions, AgentDefinitionProvider impl
├── executor.rs         # MODIFIED: depth guard, mesh config injection, tool forwarding,
│                         session step dispatch, SDK type usage
├── session.rs          # NEW: Session step execution, conversation management,
│                         activation strategies, convergence checking
├── pack.rs             # MODIFIED: new "generate-agents-yaml" action,
│                         "list-agents" action using registry
├── validator.rs        # MODIFIED: validate session step configs, validate agents.yaml
├── workspace.rs        # MODIFIED: AgentMeshSettings, resolve mesh config
├── util.rs             # Utility functions (unchanged)
└── test_parser.rs      # Test output parsing (unchanged)

config/
├── config.yaml         # MODIFIED: new `agent_mesh` section (optional)
├── agents.yaml         # NEW: generated by `generate-agents-yaml` action
├── plugins/
│   ├── bmad-method              # Existing plugin binary
│   └── provider-claude-code     # Existing plugin binary
└── workflows/
    ├── coding-quick-dev.yaml    # Existing (may add mesh_enabled, tools)
    ├── coding-feature-dev.yaml  # MODIFIED: add session step for arch review
    ├── coding-review.yaml       # MODIFIED: convert to session step type
    └── ... (other existing workflows)
```

### Architectural Boundaries

**Plugin <-> SDK Boundary:**
- Plugin implements `AgentDefinitionProvider` via `BmadAgentRegistry` — returns owned `SdkAgentDefinition` values
- Plugin uses SDK types (`ChatMessage`, `CompletionRequest`, `ToolDef`) for internal data modeling
- Plugin does NOT implement `LlmProvider` or `ToolExecutor` — those are provider-claude-code's responsibility
- Plugin does NOT implement `AgentMeshProvider` — mesh topology is resolved by the Pulse engine

**Executor <-> Provider Boundary:**
- Executor communicates with provider-claude-code exclusively via JSON-RPC over stdio
- New fields (`agent_name`, `mcp_config`, `env_vars`, `tools`) are added to the `parameters` object in JSON-RPC requests
- Provider-claude-code is responsible for interpreting these fields and configuring the Claude CLI accordingly
- Executor never directly calls the Claude CLI — always delegates to provider-claude-code

**Agent Registry <-> Pack Boundary:**
- `agents.rs` owns all agent definitions and ACL rules
- `pack.rs` consumes agent definitions via the registry API to generate `agents.yaml` and serve `list-agents` / `data-query` endpoints
- Agent definitions are immutable at runtime — no dynamic registration

**Session <-> Executor Boundary:**
- `session.rs` implements the multi-agent turn loop
- `executor.rs` dispatches `type: session` steps to `session.rs`
- Session module reuses executor's `spawn_plugin_rpc()` for individual agent calls
- Session module reuses executor's `substitute_templates()` and `assemble_context()`

### Requirements to Structure Mapping

| Requirement | Module | Key Functions |
|---|---|---|
| FR-SDK-1: AgentDefinitionProvider | `agents.rs` | `BmadAgentRegistry::list_agents()`, `get_agent()` |
| FR-SDK-2: Agent SdkAgentDefinition mapping | `agents.rs` | `bmad_architect()`, `bmad_developer()`, etc. |
| FR-SDK-3: Mesh config injection | `executor.rs` | `build_mesh_config()`, agent step execution |
| FR-SDK-4: Depth guard | `executor.rs` | `check_depth_guard()` |
| FR-SDK-5: Session step type | `session.rs` | `execute_session_step()` |
| FR-SDK-6: Workspace mesh config | `workspace.rs` | `AgentMeshSettings`, `ConfigYaml` |
| FR-SDK-7: agents.yaml generation | `pack.rs`, `agents.rs` | `generate_agents_yaml()` |
| FR-SDK-8: SDK LLM types | `executor.rs` | Internal type usage |
| FR-SDK-9: Tool forwarding | `executor.rs` | `parameters.insert("tools", ...)` |
| FR-SDK-10: MCP tools | `main.rs` (future) | `list_agents`, `invoke_agent` MCP handlers |

### Data Flow

```
Workflow YAML (type: agent, mesh_enabled: true)
    │
    ▼
executor.rs: execute_step()
    │
    ├─→ check_depth_guard(ws_config) → fail if depth > max
    │
    ├─→ agents.rs: BmadAgentRegistry::get_agent(name)
    │   → SdkAgentDefinition { name, skills, tools, system_prompt, ... }
    │
    ├─→ build_mesh_config(step, ws_config, depth)
    │   → { agent_name, mcp_config, env_vars }
    │
    ├─→ JSON-RPC to provider-claude-code
    │   params: { system_prompt, model_tier, session_id, working_dir,
    │             agent_name, mcp_config, env_vars, tools }
    │
    └─→ provider-claude-code spawns Claude CLI with:
        - --agent-name bmad/architect
        - MCP server config injected
        - PULSE_AGENT_DEPTH=2 in env
        - Tools available via MCP


Workflow YAML (type: session)
    │
    ▼
executor.rs: execute_step() → session.rs: execute_session_step()
    │
    ├─→ Initialize conversation: Vec<ChatMessage>
    │
    ├─→ For each turn until convergence:
    │   ├─→ For each participant (activation check):
    │   │   ├─→ agents.rs: get_agent(participant.agent)
    │   │   ├─→ JSON-RPC to provider-claude-code (with full history)
    │   │   └─→ Append response to conversation
    │   └─→ Check convergence (fixed_turns / unanimous / stagnation)
    │
    └─→ Build StepOutput from final conversation


pack.rs: execute_action("generate-agents-yaml")
    │
    ├─→ agents.rs: BmadAgentRegistry::list_agents()
    │   → Vec<SdkAgentDefinition>
    │
    ├─→ For each agent: build ACL entry (can_invoke, can_respond_to, model, etc.)
    │
    ├─→ Serialize to YAML via serde_yaml (BTreeMap for deterministic order)
    │
    └─→ Write to config/agents.yaml (atomic write)
```

### Development Workflow Integration

**Build:**
```bash
cargo build --release  # Produces plugin-coding-pack binary + cdylib
```

**Test:**
```bash
cargo test             # Unit tests including agent registry, depth guard, session
cargo clippy -- -D warnings
cargo fmt --check
```

**Generate agents.yaml:**
```bash
echo '{"action": "generate-agents-yaml"}' | ./target/release/plugin-coding-pack
# Writes config/agents.yaml
```

**Deployment:**
- Binary → `config/plugins/plugin-coding-pack`
- `config/agents.yaml` → generated on install or via `generate-agents-yaml` action
- Workspace `config/config.yaml` → optionally add `agent_mesh` section

## Architecture Validation Results

### Coherence Validation

**Decision Compatibility:** All decisions are internally consistent. The agent registry (Decision 1) feeds both YAML generation (Decision 4) and executor lookup (Decisions 2, 5). The depth guard (Decision 3) works regardless of invocation path (agent step or session step). Tool forwarding (Decision 7) is orthogonal to mesh injection (Decision 2) — both add fields to the same JSON-RPC parameters map without conflict.

**Pattern Consistency:** Implementation patterns align with existing codebase conventions. New config uses `#[serde(default)]` like existing fields. New executor functions follow the same `Result<(StepOutput, Option<String>), WitPluginError>` return type. Generated YAML uses `BTreeMap` for determinism.

**Structure Alignment:** New files (`agents.rs`, `session.rs`) follow the flat module layout established in architecture-v1. Modified files (`executor.rs`, `workspace.rs`, `pack.rs`) extend existing patterns rather than replacing them.

### Requirements Coverage Validation

**Functional Requirements Coverage:** 10/10 FR-SDK requirements have architectural support.

| Requirement | Status | Notes |
|---|---|---|
| FR-SDK-1: AgentDefinitionProvider | Covered | `BmadAgentRegistry` in `agents.rs` |
| FR-SDK-2: Agent mapping | Covered | 9 agents with full `SdkAgentDefinition` fields |
| FR-SDK-3: Mesh config injection | Covered | JSON-RPC parameter enrichment in `executor.rs` |
| FR-SDK-4: Depth guard | Covered | `PULSE_AGENT_DEPTH` check in `executor.rs` |
| FR-SDK-5: Session step type | Covered | `session.rs` with activation + convergence |
| FR-SDK-6: Workspace mesh config | Covered | `AgentMeshSettings` in `workspace.rs` |
| FR-SDK-7: agents.yaml generation | Covered | `generate-agents-yaml` action in `pack.rs` |
| FR-SDK-8: SDK LLM types | Covered | Internal type adoption in `executor.rs` |
| FR-SDK-9: Tool forwarding | Covered | `tools` field in step config + JSON-RPC |
| FR-SDK-10: MCP tools | Deferred | Future `--mcp-mode` flag on binary entry point |

**Non-Functional Requirements Coverage:** 5/5 NFR-SDK requirements have architectural support.
- NFR-SDK-1: Depth guard is O(1) env var read + integer compare
- NFR-SDK-2: YAML generation uses in-memory data + single `serde_yaml::to_string` call
- NFR-SDK-3: Agent registry is a static `Vec` — no I/O
- NFR-SDK-4: Convergence evaluation is simple condition check per turn
- NFR-SDK-5: Breaking changes are the explicit design choice

### Gap Analysis Results

**Critical Gaps:** None. All implementation-blocking decisions are documented.

**Known Deferrals:**
- FR-SDK-10 (MCP tools): The `--mcp-mode` binary flag for exposing `list_agents`/`invoke_agent` via MCP stdio is deferred to a follow-up. Current agent mesh works via provider-claude-code injecting MCP config.
- Direct `LlmProvider::complete()` invocation: The executor continues to use JSON-RPC for now. Direct in-process LLM calls are a Phase 2 optimization.
- Cost tracking via `TokenUsage`: Extracting `TokenUsage` from provider responses and integrating with cost-tracker plugin is deferred.
- Session persistence: Multi-agent sessions are currently ephemeral (state lost if executor crashes mid-session).

**Minor Gaps (story-level detail):**
- Exact system prompt text for each BMAD agent persona (content, not architecture)
- Provider-claude-code's handling of new `mcp_config` and `env_vars` fields (upstream dependency)
- Session step YAML schema validation rules (implementation detail for `validator.rs`)

---

## Auto-Dev Loop — Board-Driven Autonomous Execution

_Added 2026-03-27. Extends the existing architecture to close the gap between the board/task system and the workflow executor, enabling fully autonomous agent-driven development._

### Problem Statement

The board system (`board_store.rs`) and workflow executor (`executor.rs`) are architecturally disconnected. Tasks can be created on the board and workflows can be triggered, but there is no automated path from "task is ready-for-dev" to "workflow runs" to "board reflects results." All orchestration is manual.

**Goal:** Enable an end-to-end autonomous development loop:

```
Board Task (ready-for-dev) → Pick Task → Run Workflow → Run Tests → Update Board → Pick Next
```

### Decision 8: Auto-Dev Orchestration Model — Hybrid (Single-Shot + Watch)

- **Decision:** Implement two new actions in `pack.rs`:
  1. `auto-dev-next` — picks one `ready-for-dev` task from the board, executes the appropriate workflow, updates the board with results. Returns JSON with task_id, workflow_id, outcome, and test results.
  2. `auto-dev-watch` — loops `auto-dev-next` continuously until no `ready-for-dev` tasks remain or a configurable max iterations is reached.
- **Rationale:** The single-shot `auto-dev-next` is deterministic and testable — an E2E test can create a task, call `auto-dev-next`, and verify the board was updated. The watch loop is a thin wrapper for production use. This avoids async complexity (no task queue, no event system) and fits the existing sync execution model.
- **Task Selection Strategy:** Pick the highest-priority `ready-for-dev` assignment. Priority order: `critical` > `high` > `medium` > `low`. Within same priority, pick the oldest (first in array).
- **Implementation sketch:**

```rust
// src/auto_dev.rs (new module)

pub struct AutoDevResult {
    pub task_id: String,
    pub workflow_id: String,
    pub outcome: AutoDevOutcome,
    pub test_passed: bool,
    pub comment: String,
}

pub enum AutoDevOutcome {
    Success,       // workflow completed, tests passed → status: "review"
    TestFailure,   // workflow completed, tests failed → status: "in-progress" + comment
    WorkflowError, // workflow failed → status: "backlog" + error comment
}

pub fn auto_dev_next(config: &WorkspaceConfig) -> Result<Option<AutoDevResult>, WitPluginError> {
    // 1. Query board for ready-for-dev tasks
    // 2. Pick highest priority task
    // 3. Set status → "in-progress", add start comment
    // 4. Resolve workflow_id from task
    // 5. Execute workflow with task description as input
    // 6. Run validation (cargo test / npm test)
    // 7. Update board based on outcome
    // 8. Return result
}

pub fn auto_dev_watch(
    config: &WorkspaceConfig,
    max_iterations: Option<u32>,
) -> Result<Vec<AutoDevResult>, WitPluginError> {
    let max = max_iterations.unwrap_or(10);
    let mut results = Vec::new();
    for _ in 0..max {
        match auto_dev_next(config)? {
            Some(result) => results.push(result),
            None => break, // no more ready-for-dev tasks
        }
    }
    Ok(results)
}
```

- **Affects:** `src/auto_dev.rs` (new), `src/pack.rs` (new actions), `src/lib.rs` (step dispatch)

### Decision 9: Task-to-Workflow Mapping — Convention + Explicit Override

- **Decision:** The workflow to run for a task is determined by:
  1. **Explicit override:** If the assignment has a `workflow_id` field (new optional field on `StoreAssignment`), use it directly.
  2. **Label convention:** If no explicit workflow_id, check labels: `"story"` → `coding-story-dev`, `"bug"` → `coding-bug-fix`, `"refactor"` → `coding-refactor`, `"quick"` → `coding-quick-dev`, `"feature"` → `coding-feature-dev`.
  3. **Default:** If no matching label, use `coding-quick-dev` (fastest, most general).
- **Rationale:** Labels are already on assignments and require no schema migration. Explicit `workflow_id` override handles edge cases where the convention doesn't fit. Default to `coding-quick-dev` because it's the lightest workflow and most forgiving for ad-hoc tasks.
- **Implementation:**

```rust
fn resolve_workflow_id(assignment: &StoreAssignment) -> &str {
    // 1. Check explicit workflow_id field
    if let Some(wf) = &assignment.workflow_id {
        if !wf.is_empty() { return wf; }
    }
    // 2. Check labels for convention mapping
    for label in &assignment.labels {
        match label.as_str() {
            "story" => return "coding-story-dev",
            "bug" => return "coding-bug-fix",
            "refactor" => return "coding-refactor",
            "quick" => return "coding-quick-dev",
            "feature" => return "coding-feature-dev",
            "review" => return "coding-review",
            _ => {}
        }
    }
    // 3. Default
    "coding-quick-dev"
}
```

- **Schema change:** Add optional `workflow_id: String` to `StoreAssignment` with `#[serde(default)]`. Backward compatible — existing board-store.json files without this field continue to work.
- **Affects:** `src/board_store.rs` (field addition), `src/auto_dev.rs` (routing logic)

### Decision 10: Board State Machine Integration — Bookend Updates with Comments

- **Decision:** The auto-dev loop updates the board at three points:
  1. **Before execution:** Set assignment status to `in-progress`. Add comment: `"[auto-dev] Starting workflow '{workflow_id}' at {timestamp}"`.
  2. **After success (tests pass):** Set status to `review`. Add comment: `"[auto-dev] Workflow completed. Tests passed. Ready for review."` with workflow output summary.
  3. **After failure:** Keep status as `in-progress` (test failure) or revert to `backlog` (workflow error). Add comment with error details: `"[auto-dev] Tests failed: {failure_summary}"` or `"[auto-dev] Workflow error: {error_message}"`.
- **Rationale:** Bookend updates are simple and give full visibility into what happened. Comments create an audit trail. Using `in-progress` for test failures (not `backlog`) keeps the task visible as actively being worked on — a retry or manual fix can continue from where the agent left off.
- **State transitions:**

```
ready-for-dev → [auto-dev picks up] → in-progress
in-progress   → [workflow + tests pass] → review
in-progress   → [tests fail] → in-progress (+ failure comment)
in-progress   → [workflow error] → backlog (+ error comment)
```

- **Affects:** `src/auto_dev.rs` (state updates via `board_store` functions)

### Decision 11: Validation Gate — Test Execution as Quality Gate

- **Decision:** After the workflow completes, `auto-dev-next` runs a validation step before marking the task as done:
  1. Detect project type: check for `Cargo.toml` (Rust), `package.json` (Node), `pyproject.toml` (Python)
  2. Run appropriate test command: `cargo test`, `npm test`, `pytest`
  3. Parse test output using existing `test_parser::parse_test_output()`
  4. If tests pass: proceed to success path
  5. If tests fail: add failure details to board comment, optionally trigger retry
- **Rationale:** The existing `test_parser.rs` already handles Cargo, Jest, Pytest, and JUnit XML output parsing. Reusing it provides consistent test result extraction. The validation gate is what makes auto-dev trustworthy — without it, agents could "complete" tasks with broken code.
- **Retry behavior:** If `auto_dev_config.max_retries` > 0 (default: 1), the auto-dev loop re-invokes the workflow with the test failure output appended to the input, giving the agent a chance to self-correct. This mirrors the existing `retry` mechanism in `coding-story-dev.yaml`.
- **Configuration:**

```rust
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AutoDevConfig {
    /// Maximum retries on test failure (default: 1)
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Maximum tasks to process in watch mode (default: 10)
    #[serde(default = "default_max_tasks")]
    pub max_tasks: u32,
    /// Skip validation gate (not recommended)
    #[serde(default)]
    pub skip_validation: bool,
}
```

- **Affects:** `src/auto_dev.rs` (validation logic), `src/workspace.rs` (config section), reuses `src/test_parser.rs`

### Decision Impact Analysis — Auto-Dev Loop

**Implementation Sequence:**
1. `src/board_store.rs` — Add `workflow_id` field to `StoreAssignment` (trivial, backward compatible)
2. `src/workspace.rs` — Add `AutoDevConfig` section (follows existing pattern)
3. `src/auto_dev.rs` — Core logic: task selection, workflow routing, execution, validation, board updates
4. `src/pack.rs` — Register `auto-dev-next` and `auto-dev-watch` actions
5. `src/tool_provider.rs` — Expose `bmad_auto_dev_next` as LLM-callable tool
6. E2E test — Create task, run `auto-dev-next`, verify board updated

**Cross-Component Dependencies:**
- `auto_dev.rs` depends on: `board_store` (task CRUD), `executor` (workflow execution), `test_parser` (validation), `workspace` (config)
- `pack.rs` gains two new action branches
- `tool_provider.rs` gains one new tool (optional — allows LLM agents to trigger auto-dev)
- No changes to existing workflow YAML files required
- No changes to dashboard manifest required (board already displays assignments)

**E2E Test Design:**

```rust
#[test]
fn auto_dev_next_picks_task_runs_workflow_updates_board() {
    let dir = tempfile::tempdir().unwrap();
    // 1. Create a board store with one ready-for-dev task
    // 2. Place a simple test workflow YAML in config/workflows/
    // 3. Call auto_dev_next()
    // 4. Assert: task status changed to "review" or "in-progress"
    // 5. Assert: comment added with workflow results
    // 6. Assert: no ready-for-dev tasks remain
}
```

### Architecture Validation — Auto-Dev Decisions

**Coherence with Existing Architecture:**
- Decisions 8-11 build on top of Decisions 1-7 without modifying them
- `auto_dev.rs` uses the same `execute_workflow_with_config()` entry point as `pack.rs`
- Board updates use existing `board_store` CRUD functions (no new persistence layer)
- Validation reuses `test_parser.rs` (no duplicate test parsing logic)
- Config follows the `#[serde(default)]` optional pattern from Decision workspace config

**Requirements Coverage:**
- FR-AUTO-1: Auto-dev loop picks ready-for-dev tasks → Decision 8
- FR-AUTO-2: Task-to-workflow routing → Decision 9
- FR-AUTO-3: Board state transitions → Decision 10
- FR-AUTO-4: Test validation gate → Decision 11
- FR-AUTO-5: LLM-callable auto-dev trigger → Decision 8 (tool_provider extension)

**Known Limitations:**
- Auto-dev requires all workflow plugins to be installed (provider-claude-code, bmad-method, etc.)
- Watch mode is synchronous — one task at a time (parallel execution is a future enhancement)
- No inter-task dependency awareness (tasks are picked independently by priority)

### Architecture Completeness Checklist

**Requirements Analysis**
- [x] Integration scope thoroughly analyzed (10 FRs, 5 NFRs)
- [x] Upstream SDK types and traits reviewed at source level
- [x] Existing codebase fully read and understood
- [x] Cross-cutting concerns mapped (identity propagation, depth guard, config compat, ACL consistency, session state)

**Architectural Decisions**
- [x] Critical decisions documented (7 decisions with rationale and code examples)
- [x] Technology choices aligned with existing stack (Rust, serde, std process model)
- [x] Integration patterns defined (JSON-RPC parameter extension, env var propagation)
- [x] Error handling follows existing `WitPluginError` patterns
- [x] Breaking changes explicitly accepted

**Implementation Patterns**
- [x] Agent definition pattern specified (builder with constants)
- [x] Config extension pattern specified (optional fields, serde defaults)
- [x] Session execution pattern specified (turn loop with convergence)
- [x] JSON-RPC extension pattern specified (conditional parameter insertion)
- [x] YAML generation pattern specified (BTreeMap, atomic write)
- [x] Enforcement guidelines and anti-patterns documented

**Project Structure**
- [x] Complete file listing with new/modified annotations
- [x] Architectural boundaries established (plugin-SDK, executor-provider, registry-pack, session-executor)
- [x] Requirements to structure mapping complete
- [x] Data flow documented for all three execution paths (agent mesh, session, YAML generation)
- [x] Build, test, and deployment commands specified

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION

**Confidence Level:** High

**Key Strengths:**
- All upstream SDK types reviewed at source — no assumptions about API shapes
- Minimal invasive changes to existing working code (executor additions, not rewrites)
- Clear separation between new modules (`agents.rs`, `session.rs`) and modified modules
- Depth guard is trivially simple and impossible to bypass
- YAML generation is idempotent and deterministic
- Breaking changes accepted — no backward compatibility burden

**Areas for Future Enhancement:**
- MCP mode for direct agent mesh tool exposure (`--mcp-mode` flag)
- Direct `LlmProvider::complete()` for in-process LLM calls (skip JSON-RPC overhead)
- Session persistence for crash recovery
- Cost tracking integration via `TokenUsage`
- Dynamic agent registration (runtime plugin-provided agents beyond BMAD set)
- `match_skills()` integration for automatic agent selection based on task requirements

### Implementation Handoff

**AI Agent Guidelines:**
- Follow all architectural decisions exactly as documented
- Use implementation patterns consistently across all new and modified modules
- Respect project structure and architectural boundaries
- Reference this document for all architectural questions
- Run `cargo clippy -- -D warnings` and `cargo fmt --check` before marking any work complete

**Implementation Sequence:**
1. `src/agents.rs` — Agent registry with all 9 BMAD agents (foundational, no deps)
2. `src/workspace.rs` — `AgentMeshSettings` and config parsing (extends existing)
3. `src/executor.rs` — Depth guard + mesh injection + tool forwarding (modifies existing)
4. `src/session.rs` — Session step type with activation + convergence (new module)
5. `src/pack.rs` — `generate-agents-yaml` + `list-agents` actions (depends on registry)
6. `src/lib.rs` — Wire up new modules, expose `AgentDefinitionProvider`
7. `src/validator.rs` — Session step validation, agents.yaml validation
8. Workflow YAML updates — Add session steps, mesh config, tool definitions
