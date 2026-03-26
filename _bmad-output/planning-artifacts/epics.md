---
stepsCompleted: [1, 2, 3, 4]
lastStep: 4
status: 'complete'
completedAt: '2026-03-23'
lastUpdated: '2026-03-23'
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
  - '_bmad-output/planning-artifacts/architecture.md'
workflowType: 'epics'
project_name: 'bmad-method-flow'
user_name: 'Jack'
date: '2026-03-23'
---

# bmad-method-flow - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for bmad-method-flow, decomposing the requirements from the PRD and Architecture (SDK Integration & Agent Mesh) into implementable stories. This covers the integration of new pulse-plugin-sdk capabilities (agent definitions, agent mesh, tool calling, unified LLM types) and provider-claude-code agent mesh features into plugin-coding-pack.

## Requirements Inventory

### Functional Requirements

FR-SDK-1: Plugin implements `AgentDefinitionProvider` trait, exposing 9 BMAD agents via `list_agents()` and `get_agent(name)`
FR-SDK-2: Each BMAD agent maps to an `SdkAgentDefinition` with `name`, `description`, `system_prompt`, `model_tier`, `skills`, and `tools`
FR-SDK-3: Executor injects `agent_name`, `mcp_config`, and `env_vars` into provider-claude-code JSON-RPC parameters for agent mesh steps
FR-SDK-4: Executor sets `PULSE_AGENT_DEPTH` env var and enforces max depth 5 to prevent infinite agent recursion
FR-SDK-5: New `session` step type supports multi-agent deliberation with configurable `ActivationStrategy` and `ConvergenceStrategy`
FR-SDK-6: Workspace config resolves agent mesh settings from `config/config.yaml` (new `agent_mesh` section)
FR-SDK-7: Plugin generates `agents.yaml` from BMAD agent definitions with `can_invoke` and `can_respond_to` ACL fields
FR-SDK-8: Executor supports `CompletionRequest`/`CompletionResponse` types for future direct LLM invocation (alongside existing JSON-RPC)
FR-SDK-9: Agent steps can include `tools` in their config, forwarded as `ToolDef` to the LLM provider
FR-SDK-10: Plugin exposes `list_agents` and `invoke_agent` via MCP stdio JSON-RPC when operating in mesh mode

### NonFunctional Requirements

NFR-SDK-1: Agent mesh depth guard adds <1ms overhead per step
NFR-SDK-2: `agents.yaml` generation completes in <50ms for 9 agents
NFR-SDK-3: `AgentDefinitionProvider::list_agents()` returns in <1ms (in-memory data)
NFR-SDK-4: Session step convergence evaluation adds <10ms per turn
NFR-SDK-5: Breaking changes to existing executor types are acceptable; no backward compatibility required

### Additional Requirements

- No starter template needed — this is a brownfield extension of existing plugin-coding-pack v0.1.0
- All agent definitions use `SdkAgentDefinition` from `pulse_plugin_sdk::traits::agent_definition` — never ad-hoc structs
- All conversation history uses `ChatMessage` from `pulse_plugin_sdk::types::llm` — never raw strings
- `PULSE_AGENT_DEPTH` must be checked before any agent invocation — never skip the depth guard
- `BTreeMap` must be used for all generated config files — never `HashMap` for serialized output
- All errors must map to `WitPluginError` variants — no `unwrap()` or `expect()` in non-test code
- `cargo clippy -- -D warnings` and `cargo fmt --check` must pass before marking work complete
- Agent names use `bmad/` prefix followed by kebab-case role name
- `model` field is always `None` on agent definitions — resolved at runtime via workspace config
- New config sections added as optional fields with `#[serde(default)]` for backward compatibility
- JSON-RPC parameters use conditional insertion — never send null or empty values for optional fields
- Generated YAML uses `BTreeMap` for deterministic alphabetical key order
- No async runtime in production code — keep using `std::process::Command` + `std::thread`
- `AgentDefinitionProvider` implemented on separate `BmadAgentRegistry` struct (not `CodingPackPlugin` directly) for testability
- WASM compatibility must be maintained — `cdylib` + `rlib` crate types

### UX Design Requirements

N/A — This is a Rust plugin with no user interface. No UX design document exists.

### FR Coverage Map

FR-SDK-1: Epic 1 - Agent registry implementing AgentDefinitionProvider trait
FR-SDK-2: Epic 1 - 9 BMAD agents as SdkAgentDefinition with full field mapping
FR-SDK-3: Epic 2 - Executor injects agent_name, mcp_config, env_vars into JSON-RPC
FR-SDK-4: Epic 2 - Depth guard via PULSE_AGENT_DEPTH environment variable
FR-SDK-5: Epic 4 - Session step type for multi-agent deliberation
FR-SDK-6: Epic 2 - AgentMeshSettings in workspace config
FR-SDK-7: Epic 5 - agents.yaml generation from BMAD registry
FR-SDK-8: Epic 3 - CompletionRequest/CompletionResponse type adoption
FR-SDK-9: Epic 3 - Tool definitions forwarded to LLM provider
FR-SDK-10: Epic 5 - MCP stdio JSON-RPC for list_agents and invoke_agent
NFR-SDK-1: Epic 2 - Depth guard O(1) performance (<1ms)
NFR-SDK-2: Epic 5 - agents.yaml generation performance (<50ms)
NFR-SDK-3: Epic 1 - In-memory agent registry performance (<1ms)
NFR-SDK-4: Epic 4 - Convergence evaluation performance (<10ms)
NFR-SDK-5: Epic 3 - Breaking changes acceptable across executor types

## Epic List

### Epic 1: BMAD Agent Registry and Discovery
Enable platform-wide discovery of all 9 BMAD agents by implementing the `AgentDefinitionProvider` trait via a new `BmadAgentRegistry` in `src/agents.rs`. After this epic, the Pulse platform can query the plugin for available agents, their capabilities, skills, tools, and model tier preferences.
**FRs covered:** FR-SDK-1, FR-SDK-2
**NFRs covered:** NFR-SDK-3

### Epic 2: Agent Mesh Configuration and Depth Guard
Enable safe agent-to-agent invocation by extending workspace config with `AgentMeshSettings`, injecting mesh configuration into provider-claude-code JSON-RPC calls, and enforcing recursion depth limits via `PULSE_AGENT_DEPTH`. After this epic, workflow steps with `mesh_enabled: true` can invoke other agents through MCP without risk of infinite recursion.
**FRs covered:** FR-SDK-3, FR-SDK-4, FR-SDK-6
**NFRs covered:** NFR-SDK-1

### Epic 3: Tool Calling Support and Unified LLM Types
Adopt SDK `CompletionRequest`/`CompletionResponse` types internally and support forwarding tool definitions from workflow step config to LLM providers. After this epic, workflow designers can specify which tools an agent step can use, and the executor uses type-safe SDK LLM types for all request construction and response parsing.
**FRs covered:** FR-SDK-8, FR-SDK-9
**NFRs covered:** NFR-SDK-5

### Epic 4: Multi-Agent Session Deliberation
Implement a new `session` step type in `src/session.rs` that orchestrates multi-turn conversations between 2+ agents with configurable activation and convergence strategies. After this epic, workflow designers can create architecture review or code review sessions where agents debate trade-offs across multiple turns until convergence.
**FRs covered:** FR-SDK-5
**NFRs covered:** NFR-SDK-4

### Epic 5: Agent Mesh ACL Generation and Pack Actions
Generate `agents.yaml` ACL configuration from the BMAD agent registry and expose agent management through pack actions (`generate-agents-yaml`, `list-agents`). After this epic, the agent mesh ACL is automatically derived from authoritative agent definitions, preventing drift between the plugin registry and mesh configuration.
**FRs covered:** FR-SDK-7, FR-SDK-10
**NFRs covered:** NFR-SDK-2

### Epic 6: Module Wiring, Validation, and Workflow Integration
Wire all new modules into `src/lib.rs`, extend `src/validator.rs` to validate session step configs and agents.yaml schemas, and update workflow YAML templates to use new step types and mesh features. After this epic, the full SDK integration is complete, validated, and ready for use in production workflows.
**FRs covered:** Cross-cutting integration (all FR-SDKs validated end-to-end)

---

## Epic 1: BMAD Agent Registry and Discovery

Enable platform-wide discovery of all 9 BMAD agents by implementing the `AgentDefinitionProvider` trait via a new `BmadAgentRegistry` in `src/agents.rs`. After this epic, the Pulse platform can query the plugin for available agents, their capabilities, skills, tools, and model tier preferences.

### Story 1.1: Create BmadAgentRegistry struct with agent definition constants

As a plugin developer,
I want a `BmadAgentRegistry` struct in `src/agents.rs` that contains system prompt constants and builder functions for all 9 BMAD agents,
So that agent definitions are centralized, co-located with the plugin, and available for platform-wide discovery.

**Acceptance Criteria:**

**Given** the `src/agents.rs` file does not yet exist
**When** a developer creates the `BmadAgentRegistry` struct
**Then** the file contains a public struct `BmadAgentRegistry` with a `new()` constructor
**And** 9 system prompt constants are defined (one per agent: architect, developer, qa, pm, sm, tech-writer, ux-designer, analyst, quick-dev)
**And** each constant is a `&str` containing the agent's role-specific system prompt text
**And** the struct derives `Debug, Clone, Default`

**Given** the `BmadAgentRegistry` struct exists
**When** a developer calls individual agent builder functions (e.g., `bmad_architect()`, `bmad_developer()`)
**Then** each function returns an `SdkAgentDefinition` from `pulse_plugin_sdk::traits::agent_definition`
**And** the `name` field uses `bmad/` prefix with kebab-case role (e.g., `bmad/architect`, `bmad/developer`)
**And** the `description` field includes the persona name and a one-line role summary (e.g., "Winston -- Senior Solutions Architect...")
**And** the `model_tier` field matches the architecture mapping (architect=smart, developer=balanced, qa=balanced, pm=balanced, sm=fast, tech-writer=fast, ux-designer=balanced, analyst=balanced, quick-dev=fast)
**And** the `model` field is always `None`
**And** the `skills` field contains the architecture-specified skill tags as lowercase kebab-case strings
**And** the `tools` field contains the allowed Claude Code tool names (e.g., `["Read", "Glob", "Grep", "Bash"]`)
**And** `max_tokens` is set to `Some(8192)` and `temperature` to `Some(0.3)`

**Given** the code compiles
**When** `cargo clippy -- -D warnings` is run
**Then** no warnings or errors are reported for `src/agents.rs`

### Story 1.2: Implement AgentDefinitionProvider trait on BmadAgentRegistry

As a platform operator,
I want the `BmadAgentRegistry` to implement the `AgentDefinitionProvider` trait,
So that the Pulse platform can discover and query BMAD agents through the standard SDK interface.

**Acceptance Criteria:**

**Given** `BmadAgentRegistry` exists with all 9 agent builder functions
**When** `AgentDefinitionProvider` is implemented on `BmadAgentRegistry`
**Then** `list_agents()` returns a `Vec<SdkAgentDefinition>` containing all 9 agents
**And** the returned list is deterministically ordered (alphabetical by agent name)
**And** `get_agent(name)` returns `Some(SdkAgentDefinition)` for valid agent names (e.g., `"bmad/architect"`)
**And** `get_agent(name)` returns `None` for unknown agent names

**Given** `list_agents()` is called
**When** the result is measured for performance
**Then** the call completes in <1ms (NFR-SDK-3: in-memory data, no I/O)

**Given** the module is declared in `src/lib.rs`
**When** `pub mod agents;` is added
**Then** the crate compiles successfully with the new module
**And** `BmadAgentRegistry` is accessible from outside the crate

### Story 1.3: Add unit tests for agent registry completeness and correctness

As a plugin developer,
I want comprehensive unit tests for the `BmadAgentRegistry`,
So that agent definitions cannot silently regress or become inconsistent with the architecture specification.

**Acceptance Criteria:**

**Given** the `BmadAgentRegistry` is implemented
**When** `test_list_agents_returns_all_nine` runs
**Then** `list_agents()` returns exactly 9 agents
**And** the set of names matches: `bmad/analyst`, `bmad/architect`, `bmad/developer`, `bmad/pm`, `bmad/qa`, `bmad/quick-dev`, `bmad/sm`, `bmad/tech-writer`, `bmad/ux-designer`

**Given** the registry contains all agents
**When** `test_get_agent_returns_correct_definition` runs for each agent name
**Then** the returned definition has non-empty `description`, `system_prompt`, `model_tier`, and `skills` fields
**And** the `name` field matches the queried name exactly
**And** the `model` field is `None`

**Given** an unknown agent name like `"bmad/nonexistent"`
**When** `test_get_agent_unknown_returns_none` runs
**Then** `get_agent("bmad/nonexistent")` returns `None`

**Given** all tests exist
**When** `cargo test` is run
**Then** all agent registry tests pass

---

## Epic 2: Agent Mesh Configuration and Depth Guard

Enable safe agent-to-agent invocation by extending workspace config with `AgentMeshSettings`, injecting mesh configuration into provider-claude-code JSON-RPC calls, and enforcing recursion depth limits via `PULSE_AGENT_DEPTH`. After this epic, workflow steps with `mesh_enabled: true` can invoke other agents through MCP without risk of infinite recursion.

### Story 2.1: Add AgentMeshSettings to workspace configuration

As a platform operator,
I want an optional `agent_mesh` section in workspace configuration,
So that I can control agent mesh behavior (enable/disable, max depth, agents.yaml path) per workspace without breaking existing configs.

**Acceptance Criteria:**

**Given** `src/workspace.rs` contains the existing `WorkspaceConfig` struct
**When** `AgentMeshSettings` is added as a new struct
**Then** it has fields: `enabled: bool` (default false), `max_depth: u32` (default 5), `agents_yaml_path: Option<String>` (default None)
**And** it derives `Debug, Clone, Default, Deserialize`
**And** `max_depth` uses `#[serde(default = "default_max_depth")]` with `fn default_max_depth() -> u32 { 5 }`
**And** `enabled` uses `#[serde(default)]`

**Given** `AgentMeshSettings` exists
**When** `WorkspaceConfig` is extended with an `agent_mesh` field
**Then** the field is typed `Option<AgentMeshSettings>` (or uses `#[serde(default)]`)
**And** existing `config/config.yaml` files without an `agent_mesh` section continue to parse without errors
**And** a config with `agent_mesh: { enabled: true, max_depth: 3 }` parses correctly

**Given** the config deserialization is tested
**When** `cargo test` runs workspace config tests
**Then** tests verify both present and absent `agent_mesh` sections parse correctly
**And** tests verify default values (enabled=false, max_depth=5) when the section is absent

### Story 2.2: Implement depth guard check in executor

As a platform operator,
I want the executor to enforce recursion depth limits before executing agent steps,
So that infinite agent-to-agent recursion is prevented and runaway agent chains fail fast.

**Acceptance Criteria:**

**Given** the `PULSE_AGENT_DEPTH` environment variable is not set
**When** `check_depth_guard(config)` is called
**Then** the function reads the env var (defaulting to 0), computes `next = current + 1`, and returns `Ok(1)`

**Given** `PULSE_AGENT_DEPTH` is set to `"4"` and `max_depth` is 5
**When** `check_depth_guard(config)` is called
**Then** the function returns `Ok(5)` (next = 4 + 1 = 5, which equals max, so allowed)

**Given** `PULSE_AGENT_DEPTH` is set to `"5"` and `max_depth` is 5
**When** `check_depth_guard(config)` is called
**Then** the function returns `Err(WitPluginError::invalid_input("agent mesh depth limit exceeded: 6 > 5"))`

**Given** `PULSE_AGENT_DEPTH` is set to a non-numeric value like `"abc"`
**When** `check_depth_guard(config)` is called
**Then** the function treats it as 0 and returns `Ok(1)`

**Given** the depth guard implementation exists
**When** performance is measured
**Then** the check completes in <1ms (NFR-SDK-1: single env var read + integer comparison)

### Story 2.3: Inject agent mesh config into JSON-RPC parameters

As a workflow designer,
I want the executor to automatically enrich JSON-RPC calls with agent mesh configuration when `mesh_enabled: true` is set on a step,
So that provider-claude-code receives the agent identity, MCP server config, and environment variables needed for inter-agent invocation.

**Acceptance Criteria:**

**Given** a workflow step has `mesh_enabled: true` in its config and an `agent_name` field
**When** the executor builds JSON-RPC parameters for provider-claude-code
**Then** the parameters include `"agent_name"` with the value from the step config (e.g., `"bmad/architect"`)
**And** the parameters include `"mcp_config"` with a JSON object containing `mcpServers.pulse-agents` pointing to the plugin binary with `--mcp-mode` arg
**And** the parameters include `"env_vars"` with `PULSE_AGENT_DEPTH` set to the incremented depth value and `PULSE_AGENT_NAME` set to the agent name

**Given** a workflow step does not have `mesh_enabled: true`
**When** the executor builds JSON-RPC parameters
**Then** the parameters do not include `agent_name`, `mcp_config`, or `env_vars` fields (conditional insertion, no nulls)

**Given** mesh config injection is implemented
**When** `check_depth_guard()` is called before the JSON-RPC call
**Then** the depth guard runs before mesh config is built, and a depth violation prevents the RPC call entirely

**Given** the `build_mesh_config()` helper function exists
**When** it constructs the MCP config
**Then** it uses workspace config to resolve the plugin binary path for the `command` field
**And** the `env` section of the MCP server config includes the incremented `PULSE_AGENT_DEPTH`

---

## Epic 3: Tool Calling Support and Unified LLM Types

Adopt SDK `CompletionRequest`/`CompletionResponse` types internally and support forwarding tool definitions from workflow step config to LLM providers. After this epic, workflow designers can specify which tools an agent step can use, and the executor uses type-safe SDK LLM types for all request construction and response parsing.

### Story 3.1: Adopt SDK LLM types for internal request and response construction

As a plugin developer,
I want the executor to use `CompletionRequest`, `CompletionResponse`, `ChatMessage`, and related SDK types internally,
So that LLM interactions have compile-time type safety and are compatible with future direct LLM provider invocation.

**Acceptance Criteria:**

**Given** the `pulse_plugin_sdk::types::llm` module provides `CompletionRequest`, `CompletionResponse`, `ChatMessage`, `TokenUsage`, `ResponseChoice`, `ToolDef`, and `ToolCall`
**When** the executor is refactored to use these types
**Then** `CompletionRequest` is used to construct LLM request payloads (model, messages, temperature, max_tokens, tools)
**And** `ChatMessage` is used for conversation history instead of raw strings
**And** `CompletionResponse` is used to parse LLM responses where applicable
**And** these types are serialized to JSON for the JSON-RPC transport via serde

**Given** the executor uses SDK types internally
**When** JSON-RPC communication with provider-claude-code occurs
**Then** the existing JSON-RPC transport continues to work without modification
**And** the SDK types serialize/deserialize transparently through the JSON-RPC layer

**Given** breaking changes to executor types are acceptable (NFR-SDK-5)
**When** existing executor function signatures are updated
**Then** internal types that previously used ad-hoc structs or raw JSON are replaced with SDK equivalents
**And** `cargo build` compiles successfully with no type errors

### Story 3.2: Forward tool definitions from step config to LLM provider

As a workflow designer,
I want to specify a `tools` field in my workflow step configuration that gets forwarded to the LLM provider,
So that agent steps can use specific tools (like `run_tests` or `read_file`) during their execution.

**Acceptance Criteria:**

**Given** a workflow step YAML contains a `tools` array with tool definitions (each having `name`, `description`, `parameters`)
**When** the executor parses the step config
**Then** each tool definition is deserialized into a `ToolDef` from `pulse_plugin_sdk::types::tools`
**And** the `ToolDef` objects are serialized and included in the JSON-RPC `parameters` under a `"tools"` key

**Given** a workflow step does not have a `tools` field
**When** the executor builds JSON-RPC parameters
**Then** no `"tools"` key is included in the parameters (conditional insertion, no empty array)

**Given** a step config includes tools like:
```yaml
tools:
  - name: "run_tests"
    description: "Run the project test suite"
    parameters: { "type": "object", "properties": {} }
```
**When** the JSON-RPC parameters are built
**Then** the `"tools"` parameter contains a JSON array with one object matching the `ToolDef` serialization format
**And** the tool's `name`, `description`, and `parameters` fields are preserved exactly

**Given** both `mesh_enabled: true` and `tools` are present on the same step
**When** the executor builds parameters
**Then** both mesh config fields (`agent_name`, `mcp_config`, `env_vars`) and `tools` are present in the parameters without conflict

---

## Epic 4: Multi-Agent Session Deliberation

Implement a new `session` step type in `src/session.rs` that orchestrates multi-turn conversations between 2+ agents with configurable activation and convergence strategies. After this epic, workflow designers can create architecture review or code review sessions where agents debate trade-offs across multiple turns until convergence.

### Story 4.1: Define session configuration types and parsing

As a workflow designer,
I want to define `session` steps in workflow YAML with participants, activation strategies, and convergence settings,
So that the executor can parse and validate multi-agent session configurations.

**Acceptance Criteria:**

**Given** a new `src/session.rs` module is created
**When** session config types are defined
**Then** `SessionConfig` struct contains: `participants: Vec<SessionParticipant>`, `convergence: ConvergenceConfig`, `system_prompt: Option<String>`, `context_from: Vec<String>`
**And** `SessionParticipant` struct contains: `agent: String`, `activation: ActivationStrategy`
**And** `ActivationStrategy` enum has variants: `EveryTurn`, `WhenMentioned`, `OnTag(String)`, `KeywordMatch(Vec<String>)`
**And** `ConvergenceConfig` struct contains: `strategy: ConvergenceStrategy`, `max_turns: u32`
**And** `ConvergenceStrategy` enum has variants: `FixedTurns`, `Unanimous`, `Stagnation(u32)`
**And** all types derive `Debug, Clone, Deserialize`

**Given** a workflow YAML step with `type: session` and the session config from the architecture spec
**When** the step config is parsed
**Then** the `SessionConfig` is correctly deserialized with 2 participants (bmad/architect with every_turn, bmad/qa with every_turn), convergence (fixed_turns, max_turns=4), and a system_prompt

**Given** default values are defined
**When** `activation` is omitted in YAML
**Then** it defaults to `EveryTurn`
**And** when `max_turns` is omitted it defaults to 4

### Story 4.2: Implement session turn loop with activation evaluation

As a workflow designer,
I want the session executor to orchestrate multi-turn conversations where each participant speaks according to their activation strategy,
So that agents engage in structured deliberation with controlled participation.

**Acceptance Criteria:**

**Given** a `SessionConfig` with 2 participants both using `EveryTurn` activation and `fixed_turns` convergence with `max_turns: 4`
**When** `execute_session_step()` is called
**Then** the function runs a turn loop, and each turn calls provider-claude-code for each active participant
**And** the conversation history is maintained as `Vec<ChatMessage>` (SDK type)
**And** each agent's response is appended to the conversation history before the next participant speaks
**And** all participants see the full conversation history on each turn

**Given** a participant has `activation: WhenMentioned`
**When** the previous message in the conversation does not contain `@agent_name`
**Then** the participant is skipped for that turn
**And** when the previous message contains `@bmad/qa`, the bmad/qa participant is activated

**Given** a participant has `activation: KeywordMatch(["security", "vulnerability"])`
**When** the previous message contains the word "security"
**Then** the participant is activated for that turn

**Given** the depth guard has been checked at session start
**When** individual agent calls are made within the session
**Then** the depth guard is not re-checked per turn (architecture rule: checked once at session start)

### Story 4.3: Implement convergence evaluation and session output

As a workflow designer,
I want sessions to end based on configurable convergence conditions and produce a structured output,
So that multi-agent deliberations terminate predictably and their results flow to downstream steps.

**Acceptance Criteria:**

**Given** a session with `convergence: { strategy: fixed_turns, max_turns: 4 }`
**When** 4 complete rounds have been executed (all participants spoke in each round)
**Then** the session loop terminates after the 4th round

**Given** a session with `convergence: { strategy: unanimous }`
**When** all agents' most recent responses contain agreement signals (e.g., "I agree", "approved", "no objections")
**Then** the session loop terminates early before reaching max_turns

**Given** a session with `convergence: { strategy: stagnation, max_turns: 10 }` and stagnation threshold of 2
**When** the last 2 rounds produce responses that do not introduce new points
**Then** the session loop terminates due to stagnation detection

**Given** a session has completed (by any convergence strategy)
**When** `build_session_output()` is called
**Then** the output includes the full conversation transcript as a formatted string
**And** the output includes the total number of turns executed
**And** the output includes the convergence reason (e.g., "fixed_turns reached", "unanimous agreement", "stagnation detected")
**And** the return type matches `Result<(StepOutput, Option<String>), WitPluginError>`

**Given** convergence evaluation runs after each complete round
**When** performance is measured
**Then** evaluation completes in <10ms per turn (NFR-SDK-4)

### Story 4.4: Integrate session step type into executor dispatch

As a workflow designer,
I want the executor to recognize `type: session` steps and dispatch them to the session module,
So that session steps work alongside existing `agent` and other step types in workflow DAGs.

**Acceptance Criteria:**

**Given** the executor's `execute_step()` function handles multiple step types
**When** a step with `type: session` is encountered
**Then** the executor dispatches to `session::execute_session_step()` with the step definition, prior outputs, template variables, plugins directory, and workspace config

**Given** a session step has `context_from: [plan_step]`
**When** the session is initialized
**Then** the output from the `plan_step` is assembled and injected as the first `ChatMessage::user()` in the conversation history

**Given** a session step completes successfully
**When** the result flows to downstream steps
**Then** the `StepOutput` is stored in the outputs map and accessible via `context_from` from subsequent steps

**Given** a session step fails (e.g., provider-claude-code returns an error)
**When** the error is propagated
**Then** the error is mapped to `WitPluginError` and the session terminates with the error
**And** partial conversation state is not persisted (clean failure)

---

## Epic 5: Agent Mesh ACL Generation and Pack Actions

Generate `agents.yaml` ACL configuration from the BMAD agent registry and expose agent management through pack actions (`generate-agents-yaml`, `list-agents`). After this epic, the agent mesh ACL is automatically derived from authoritative agent definitions, preventing drift between the plugin registry and mesh configuration.

### Story 5.1: Implement generate-agents-yaml pack action

As a platform operator,
I want a `generate-agents-yaml` action that produces an `agents.yaml` ACL configuration file from the BMAD agent registry,
So that agent mesh access control is automatically derived from authoritative definitions and cannot drift.

**Acceptance Criteria:**

**Given** the `BmadAgentRegistry` contains all 9 agents with ACL rules defined in the architecture
**When** the `generate-agents-yaml` action is invoked via pack executor
**Then** an `agents.yaml` file is written to `config/agents.yaml` (or the path from workspace config `agents_yaml_path`)
**And** the file uses `BTreeMap` for deterministic alphabetical key ordering
**And** the file begins with a header comment: `# Generated by plugin-coding-pack. Do not edit manually.`

**Given** the generated `agents.yaml` is produced
**When** the content is inspected
**Then** each agent entry contains: `description`, `model` (Claude model string), `max_turns`, `max_budget_usd`, `timeout_secs`, `can_invoke` (list of agents), `can_respond_to` (list of agents), `allowed_tools` (list of tool names)
**And** the `can_invoke` ACL matches architecture rules: all agents can invoke `bmad/developer`; `bmad/architect` can also invoke `bmad/analyst` and `bmad/ux-designer`; `bmad/qa` can also invoke `bmad/developer`; `bmad/quick-dev` has empty `can_invoke`
**And** the `can_respond_to` ACL matches architecture rules: all agents can respond to `bmad/pm` and `bmad/sm`

**Given** an existing `agents.yaml` already exists
**When** `generate-agents-yaml` is run
**Then** the existing file is overwritten (idempotent generation, no merge)

**Given** generation performance is measured
**When** all 9 agents are processed
**Then** the operation completes in <50ms (NFR-SDK-2)

### Story 5.2: Implement list-agents pack action

As a platform operator,
I want a `list-agents` action that returns all registered BMAD agents with their metadata,
So that I can query available agents and their capabilities through the pack interface.

**Acceptance Criteria:**

**Given** the `BmadAgentRegistry` is available to `src/pack.rs`
**When** the `list-agents` action is invoked
**Then** the response includes a JSON array of all 9 agents
**And** each entry contains: `name`, `description`, `model_tier`, `skills`, `tools`
**And** the list is deterministically ordered by agent name

**Given** the `list-agents` action is added to `src/pack.rs`
**When** the pack input parser encounters `{"action": "list-agents"}`
**Then** the action is recognized and dispatched to the list-agents handler
**And** the result is returned as a `StepResult` with the agent list in the output data

**Given** the `CodingPackInput` enum in `src/pack.rs` is extended
**When** new variants `GenerateAgentsYaml` and `ListAgents` are added
**Then** existing actions continue to work without modification
**And** unknown actions continue to return appropriate error messages

### Story 5.3: Define ACL rules as constants in agent registry

As a plugin developer,
I want ACL rules (can_invoke, can_respond_to) co-located with agent definitions in `src/agents.rs`,
So that access control is authoritative, testable, and cannot drift from the registry.

**Acceptance Criteria:**

**Given** the `BmadAgentRegistry` exists in `src/agents.rs`
**When** ACL rule functions or constants are added
**Then** each agent has an associated `can_invoke: Vec<String>` computed from architecture rules
**And** each agent has an associated `can_respond_to: Vec<String>` computed from architecture rules
**And** `bmad/architect` can invoke: `bmad/developer`, `bmad/analyst`, `bmad/ux-designer`
**And** `bmad/qa` can invoke: `bmad/developer`
**And** `bmad/quick-dev` can invoke: (empty list)
**And** all other agents can invoke: `bmad/developer`
**And** all agents can respond to: `bmad/pm`, `bmad/sm`

**Given** ACL rules are defined
**When** unit tests verify the ACL
**Then** a test confirms each agent's `can_invoke` and `can_respond_to` lists match the architecture specification
**And** a test confirms `bmad/quick-dev` has an empty `can_invoke` list

---

## Epic 6: Module Wiring, Validation, and Workflow Integration

Wire all new modules into `src/lib.rs`, extend `src/validator.rs` to validate session step configs and agents.yaml schemas, and update workflow YAML templates to use new step types and mesh features. After this epic, the full SDK integration is complete, validated, and ready for use in production workflows.

### Story 6.1: Wire new modules into crate root and plugin trait

As a plugin developer,
I want all new modules (`agents`, `session`) declared in `src/lib.rs` and the `AgentDefinitionProvider` trait exposed through the plugin,
So that all new functionality is accessible to the Pulse platform and can be loaded at runtime.

**Acceptance Criteria:**

**Given** `src/agents.rs` and `src/session.rs` exist as modules
**When** `src/lib.rs` is updated
**Then** `pub mod agents;` and `pub mod session;` are added to the module declarations
**And** `BmadAgentRegistry` is re-exported or accessible from the crate root

**Given** the `CodingPackPlugin` needs to expose `AgentDefinitionProvider`
**When** the trait is wired
**Then** `CodingPackPlugin` delegates to `BmadAgentRegistry` for agent definition queries (composition, not direct implementation on `CodingPackPlugin`)
**And** the plugin can be queried for agent definitions through the SDK interface

**Given** all modules are wired
**When** `cargo build` is run
**Then** the crate compiles successfully as both `cdylib` and `rlib`
**And** `cargo clippy -- -D warnings` passes with no warnings
**And** `cargo fmt --check` reports no formatting issues

### Story 6.2: Extend validator for session step configs and agents.yaml

As a platform operator,
I want the validator to catch invalid session step configurations and malformed agents.yaml files at validation time,
So that misconfigurations are detected early rather than causing runtime failures.

**Acceptance Criteria:**

**Given** `src/validator.rs` handles workflow validation
**When** a workflow contains a `type: session` step
**Then** the validator checks that `participants` is non-empty and has at least 2 entries
**And** the validator checks that each participant's `agent` name is a valid `bmad/` prefixed name
**And** the validator checks that `convergence.strategy` is one of `fixed_turns`, `unanimous`, or `stagnation`
**And** the validator checks that `convergence.max_turns` is greater than 0

**Given** a session step config is missing required fields
**When** validation is run
**Then** the validator returns a descriptive error like "session step 'architecture_review' requires at least 2 participants"

**Given** the `agents.yaml` file exists at the configured path
**When** validation includes agents.yaml checks
**Then** the validator verifies each agent entry has required fields: `description`, `can_invoke`, `can_respond_to`
**And** the validator verifies all agent names in `can_invoke` and `can_respond_to` reference agents that exist in the registry
**And** invalid references produce descriptive errors

### Story 6.3: Update workflow YAML templates with session and mesh features

As a workflow designer,
I want the existing workflow templates updated to demonstrate session steps and mesh-enabled agent steps,
So that I have working examples of the new capabilities and can compose my own workflows.

**Acceptance Criteria:**

**Given** `config/workflows/coding-feature-dev.yaml` exists
**When** it is updated for SDK integration
**Then** it includes at least one `type: session` step for architecture review (e.g., bmad/architect + bmad/qa deliberation with fixed_turns convergence)
**And** the session step uses `context_from` to receive output from a prior planning step

**Given** `config/workflows/coding-review.yaml` exists
**When** it is updated for SDK integration
**Then** it is converted to use the `session` step type for multi-agent code review
**And** it includes appropriate activation and convergence settings

**Given** existing workflow templates include agent steps
**When** mesh features are added
**Then** at least one agent step demonstrates `mesh_enabled: true` with an `agent_name` field
**And** at least one step demonstrates the `tools` configuration with a sample tool definition

**Given** all updated workflows are in place
**When** the validator is run against each workflow
**Then** all workflows pass validation without errors
