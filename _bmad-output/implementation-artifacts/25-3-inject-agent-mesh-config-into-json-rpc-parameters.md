# Story 25.3: Inject Agent Mesh Config into JSON-RPC Parameters

**Epic:** 25 — Agent Mesh Safety Guards
**Story ID:** 25.3
**Status:** done

## Story

As a workflow designer,
I want the executor to automatically enrich JSON-RPC calls with mesh configuration when `mesh_enabled: true` is set,
so that provider-claude-code receives agent identity, MCP server config, and environment variables for inter-agent invocation.

## Acceptance Criteria

1. **Given** a workflow step has `mesh_enabled: true` and an `agent_name` field in its config, **When** the executor builds JSON-RPC parameters for provider-claude-code, **Then** the parameters include `"agent_name"` with the value from the step config, `"mcp_config"` with a JSON object containing `mcpServers.pulse-agents` pointing to the plugin binary with `--mcp-mode` arg, and `"env_vars"` with `PULSE_AGENT_DEPTH` set to the incremented depth value and `PULSE_AGENT_NAME` set to the agent name.
2. **Given** a workflow step does not have `mesh_enabled: true`, **When** the executor builds JSON-RPC parameters, **Then** no `agent_name`, `mcp_config`, or `env_vars` fields are included (conditional insertion, no nulls).
3. **Given** the `build_mesh_config()` helper function exists, **When** it constructs the MCP config, **Then** it uses workspace config to resolve the plugin binary path for the `command` field, and the `env` section of the MCP server config includes the incremented `PULSE_AGENT_DEPTH`.
4. **Given** `check_depth_guard()` is called before mesh config injection, **When** depth is exceeded, **Then** the mesh config is never built and the step fails immediately.
5. **Given** a step has `mesh_enabled: true` but no `agent_name`, **When** the executor builds parameters, **Then** an `Err(WitPluginError::invalid_input(...))` is returned indicating agent_name is required when mesh is enabled.

## Tasks / Subtasks

- [x] Add `mesh_enabled` and `agent_name` fields to `StepConfigDef` in `src/executor.rs` (AC: #1, #2)
  - [x] Add `#[serde(default)] pub mesh_enabled: bool` field
  - [x] Add `#[serde(default)] pub agent_name: Option<String>` field
- [x] Implement `build_mesh_config()` helper function (AC: #1, #3)
  - [x] Signature: `fn build_mesh_config(agent_name: &str, next_depth: u32, config: &WorkspaceConfig) -> serde_json::Map<String, serde_json::Value>`
  - [x] Build MCP server config: `mcpServers.pulse-agents` with `command` pointing to `config.plugins_dir.join("plugin-coding-pack")` and `args: ["--mcp-mode"]`
  - [x] Build env vars: `PULSE_AGENT_DEPTH` = next_depth as string, `PULSE_AGENT_NAME` = agent_name
  - [x] Return a JSON object with `agent_name`, `mcp_config`, `env_vars` keys
- [x] Integrate mesh config injection into agent step execution (AC: #1, #2, #4)
  - [x] In `execute_bmad_agent_step()`: after building the `parameters` map, check if `config.mesh_enabled` is true
  - [x] If mesh enabled, call `check_depth_guard()` first (Story 25.2), then call `build_mesh_config()`
  - [x] Insert `agent_name`, `mcp_config`, `env_vars` into the `parameters` map
  - [x] Do the same in `execute_direct_agent_step()` for non-bmad executors
  - [x] If mesh is NOT enabled, do not insert any mesh-related keys
- [x] Validate mesh_enabled requires agent_name (AC: #5)
  - [x] If `mesh_enabled` is true and `agent_name` is None, return an error
- [x] Add unit tests
  - [x] Test: `build_mesh_config()` produces correct JSON structure with all required fields
  - [x] Test: `build_mesh_config()` includes correct `PULSE_AGENT_DEPTH` value
  - [x] Test: mesh config is NOT injected when `mesh_enabled` is false (default)
  - [x] Test: mesh_enabled=true without agent_name returns error (deserialization verified)
- [x] Run `cargo clippy -- -D warnings` and `cargo fmt --check`

## Dev Notes

### Key File

`src/executor.rs` -- add `build_mesh_config()` function, extend `StepConfigDef`, and modify the parameter-building sections.

### Dependencies

- Story 25.1 must be complete (WorkspaceConfig with `agent_mesh` field)
- Story 25.2 must be complete (`check_depth_guard()` function exists and is wired in)

### StepConfigDef Extension

The `StepConfigDef` struct at line 49 of `src/executor.rs` needs two new optional fields:

```rust
#[derive(Debug, Deserialize)]
pub(crate) struct StepConfigDef {
    // ... existing fields ...

    /// When true, inject agent mesh configuration into JSON-RPC parameters
    #[serde(default)]
    pub mesh_enabled: bool,
    /// Agent name for mesh-enabled steps (e.g., "bmad/architect")
    #[serde(default)]
    pub agent_name: Option<String>,
}
```

### build_mesh_config() Implementation

```rust
/// Build mesh configuration for injection into JSON-RPC parameters.
/// Returns a map of key-value pairs to merge into the parameters object.
fn build_mesh_config(
    agent_name: &str,
    next_depth: u32,
    config: &crate::workspace::WorkspaceConfig,
) -> serde_json::Map<String, serde_json::Value> {
    let plugin_binary = config.plugins_dir.join("plugin-coding-pack");
    let binary_path = plugin_binary.to_string_lossy().to_string();

    let mcp_config = serde_json::json!({
        "mcpServers": {
            "pulse-agents": {
                "command": binary_path,
                "args": ["--mcp-mode"]
            }
        }
    });

    let env_vars = serde_json::json!({
        "PULSE_AGENT_DEPTH": next_depth.to_string(),
        "PULSE_AGENT_NAME": agent_name
    });

    let mut mesh = serde_json::Map::new();
    mesh.insert("agent_name".to_string(), serde_json::json!(agent_name));
    mesh.insert("mcp_config".to_string(), mcp_config);
    mesh.insert("env_vars".to_string(), env_vars);
    mesh
}
```

### Integration Points

There are two places where JSON-RPC parameters are built for agent steps:

1. **`execute_bmad_agent_step()`** (line ~909): builds `parameters` map starting at line ~1030, then constructs `claude_request` at line ~1058. Insert mesh config into `parameters` BEFORE the `claude_request` JSON is built.

2. **`execute_direct_agent_step()`** (line ~1087): builds `parameters` map starting at line ~1115. Same pattern -- inject mesh config before the request JSON is built.

The flow for mesh-enabled steps:

```
execute_step()
  -> check_depth_guard(config)?  // from Story 25.2 -- fail fast if depth exceeded
  -> execute_agent_step() or execute_bmad_agent_step()
    -> build parameters map
    -> if mesh_enabled: build_mesh_config() and merge into parameters
    -> build JSON-RPC request
    -> spawn_plugin_rpc()
```

### Conditional Insertion Pattern

Follow the existing pattern in the executor for conditional parameter insertion (no nulls):

```rust
// Existing pattern (line ~1035-1049):
if let Some(tokens) = config.max_tokens {
    parameters.insert("max_tokens".to_string(), serde_json::json!(tokens));
}

// New mesh pattern:
if step_config.mesh_enabled {
    let agent_name = step_config.agent_name.as_deref().ok_or_else(|| {
        WitPluginError::invalid_input(format!(
            "step '{}': mesh_enabled requires agent_name", step.id
        ))
    })?;
    let next_depth = check_depth_guard(ws_config)?;
    let mesh = build_mesh_config(agent_name, next_depth, ws_config);
    for (k, v) in mesh {
        parameters.insert(k, v);
    }
}
```

### Thread-Safety of WorkspaceConfig

The `execute_bmad_agent_step()` and `execute_direct_agent_step()` functions do not currently receive `&WorkspaceConfig`. Story 25.2 should have threaded it through to `execute_step()`. Verify that it reaches these inner functions. If not, pass it through the function chain:

- `execute_step()` -> `execute_agent_step()` -> `execute_bmad_agent_step()` / `execute_direct_agent_step()`

Each of these may need a new `config: &WorkspaceConfig` parameter added.

### Anti-Patterns to Avoid

- Do NOT insert `null` values for mesh fields when mesh is disabled. The entire block of mesh keys should be absent.
- Do NOT use `HashMap` for the mesh config map -- use `serde_json::Map` which preserves insertion order.
- Do NOT hardcode the plugin binary path -- resolve it from `config.plugins_dir`.
- Do NOT skip the depth guard when mesh is enabled. The sequence is always: depth guard first, then mesh config.

### Testing Notes

- Unit test `build_mesh_config()` directly by passing known values and asserting the JSON structure.
- For integration-level testing, build a `StepConfigDef` with `mesh_enabled: true` and verify the parameters map contains the expected keys.
- Do not test the full JSON-RPC round-trip (that requires a running provider-claude-code). Test the parameter construction only.

### Existing Code References

- `src/executor.rs` lines 49-71: `StepConfigDef` struct -- add `mesh_enabled` and `agent_name` fields
- `src/executor.rs` lines 1030-1056: parameter building in `execute_bmad_agent_step()` -- inject mesh config here
- `src/executor.rs` lines 1115-1130: parameter building in `execute_direct_agent_step()` -- inject mesh config here
- `src/executor.rs` lines 1058-1077: JSON-RPC request construction -- mesh params must be in `parameters` before this

### Overlap with Epic 13

This story covers the same scope as Story 13.3 from the SDK Integration epics (`epics.md`). The key difference: Epic 13.3 assumed the depth guard was only tested in isolation (Story 13.2 did not integrate it). Epic 25.3 assumes depth guard is already integrated (Story 25.2 wires it in). The mesh config injection logic is the same in both versions.

### References

- [Source: epics-auto-dev-loop.md#Story 25.3] -- acceptance criteria
- [Source: epics.md#Story 2.3] -- original SDK integration version
- [Source: architecture.md] -- MCP server config format with mcpServers.pulse-agents

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
N/A

### Completion Notes List
- Added mesh_enabled (bool) and agent_name (Option<String>) to StepConfigDef with #[serde(default)]
- Implemented build_mesh_config() returning serde_json::Map with agent_name, mcp_config, env_vars
- Integrated mesh injection into both execute_bmad_agent_step() and execute_direct_agent_step()
- Validation: mesh_enabled=true without agent_name returns WitPluginError::invalid_input
- Depth guard called inside mesh injection block (check_depth_guard before build_mesh_config)
- Renamed _workspace_config -> workspace_config in inner functions since it's now used
- 5 new unit tests plus updated all existing StepConfigDef struct literals with new fields
- All 272 tests pass, cargo clippy -- -D warnings clean

### File List
- src/executor.rs
