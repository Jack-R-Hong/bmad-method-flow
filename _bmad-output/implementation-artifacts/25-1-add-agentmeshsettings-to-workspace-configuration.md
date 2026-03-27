# Story 25.1: Add AgentMeshSettings to Workspace Configuration

**Epic:** 25 — Agent Mesh Safety Guards
**Story ID:** 25.1
**Status:** done

## Story

As a platform operator,
I want an optional `agent_mesh` section in workspace configuration,
so that I can control mesh behavior (enable/disable, max depth, agents.yaml path) per workspace without breaking existing configs.

## Acceptance Criteria

1. **Given** `src/workspace.rs` contains the existing `WorkspaceConfig` struct, **When** `AgentMeshSettings` is added as a new struct, **Then** it has fields: `enabled: bool` (default false), `max_depth: u32` (default 5), `agents_yaml_path: Option<String>` (default None), derives `Debug, Clone, Default, Deserialize`, and uses `#[serde(default = "default_max_depth")]` for `max_depth` and `#[serde(default)]` for `enabled`.
2. **Given** `AgentMeshSettings` exists, **When** `WorkspaceConfig` is extended with an `agent_mesh` field, **Then** the field uses `#[serde(default)]` so existing `config/config.yaml` files without an `agent_mesh` section continue to parse without errors.
3. **Given** a config YAML with `agent_mesh: { enabled: true, max_depth: 3 }`, **When** parsed, **Then** the values are correctly read (enabled=true, max_depth=3, agents_yaml_path=None).
4. **Given** the config deserialization is tested, **When** `cargo test` runs workspace config tests, **Then** tests verify both present and absent `agent_mesh` sections parse correctly, and default values (enabled=false, max_depth=5) when the section is absent.
5. **Given** `ConfigYaml` (the raw YAML struct), **When** it is extended, **Then** it includes an `agent_mesh: Option<AgentMeshSettings>` field with `#[serde(default)]`.

## Tasks / Subtasks

- [x] Add `AgentMeshSettings` struct to `src/workspace.rs` (AC: #1)
  - [x] Define struct with `enabled: bool`, `max_depth: u32`, `agents_yaml_path: Option<String>`
  - [x] Add `#[derive(Debug, Clone, Deserialize)]` (implement `Default` manually)
  - [x] Add `#[serde(default)]` on `enabled`
  - [x] Add `#[serde(default = "default_max_depth")]` on `max_depth`
  - [x] Add `#[serde(default)]` on `agents_yaml_path`
  - [x] Add free function `fn default_max_depth() -> u32 { 5 }`
  - [x] Implement `Default` manually: `enabled: false, max_depth: 5, agents_yaml_path: None`
- [x] Extend `WorkspaceConfig` with `agent_mesh` field (AC: #2)
  - [x] Add `pub agent_mesh: AgentMeshSettings` field to `WorkspaceConfig`
  - [x] Wire into `from_base_dir()`: extract `yaml.agent_mesh` and use `.unwrap_or_default()` to pass to `WorkspaceConfig`
  - [x] Set `agent_mesh: AgentMeshSettings::default()` in `default_for()` fallback
- [x] Extend `ConfigYaml` with `agent_mesh` field (AC: #5)
  - [x] Add `#[serde(default)] agent_mesh: Option<AgentMeshSettings>` field to `ConfigYaml`
- [x] Add unit tests (AC: #3, #4)
  - [x] Test: config YAML without `agent_mesh` section parses successfully, `agent_mesh` has default values
  - [x] Test: config YAML with `agent_mesh: { enabled: true, max_depth: 3 }` parses correctly
  - [x] Test: config YAML with `agent_mesh: {}` parses with defaults (enabled=false, max_depth=5)
  - [x] Test: config YAML with `agent_mesh: { enabled: true, max_depth: 3, agents_yaml_path: "custom/agents.yaml" }` parses all three fields
  - [x] Test: existing backward-compatibility test (`config_yaml_backward_compatible`) still passes
- [x] Run `cargo clippy -- -D warnings` and `cargo fmt --check`

## Dev Notes

### Key File

`src/workspace.rs` -- this is the ONLY file modified in this story.

### How WorkspaceConfig Deserialization Works

The `WorkspaceConfig` struct is NOT deserialized directly from YAML. Instead, `ConfigYaml` is the raw YAML struct that gets deserialized via `serde_yaml::from_str::<ConfigYaml>()`, and then its fields are manually mapped into `WorkspaceConfig` inside the `from_base_dir()` method. Follow the existing pattern for how `defaults`, `workflows`, `auto_dev`, and `memory` fields are handled:

```rust
// In from_base_dir() — existing pattern for auto_dev:
auto_dev: yaml.auto_dev.unwrap_or_default(),

// Follow same pattern for agent_mesh:
agent_mesh: yaml.agent_mesh.unwrap_or_default(),
```

### AgentMeshSettings Struct Design

```rust
/// Agent mesh settings for multi-agent invocation.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentMeshSettings {
    /// Whether agent mesh is enabled for this workspace
    #[serde(default)]
    pub enabled: bool,
    /// Maximum recursion depth for agent-to-agent invocation
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    /// Path to agents.yaml ACL file (relative to workspace base_dir)
    #[serde(default)]
    pub agents_yaml_path: Option<String>,
}

fn default_max_depth() -> u32 {
    5
}

impl Default for AgentMeshSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_depth: default_max_depth(),
            agents_yaml_path: None,
        }
    }
}
```

The `Default` derive alone would set `max_depth` to 0 (the u32 default), but the serde `default_max_depth()` function returns 5. Implement `Default` manually so that both code-path (`Default::default()`) and serde-path produce consistent `max_depth: 5`.

### WorkspaceConfig Field Type

Use `pub agent_mesh: AgentMeshSettings` (not `Option<AgentMeshSettings>`) on `WorkspaceConfig`. The `ConfigYaml` side uses `Option<AgentMeshSettings>`, and `from_base_dir()` unwraps with `.unwrap_or_default()`. This makes downstream access simpler -- consumers can write `config.agent_mesh.max_depth` directly without unwrapping an Option. This follows the same pattern as `auto_dev: AutoDevConfig`.

### Anti-Patterns to Avoid

- Do NOT use `#[serde(deny_unknown_fields)]` on any config struct -- forward-compatible parsing is required.
- Do NOT derive `Serialize` on `AgentMeshSettings` -- it is a config-only type (read from YAML, never written). Story 25.5 will serialize agent data using `BTreeMap`, not this struct.
- Do NOT use `HashMap` anywhere in serialized output.
- Do NOT add `unwrap()` or `expect()` in non-test code.

### Testing Notes

- Add tests in the existing `#[cfg(test)] mod tests` block at the bottom of `src/workspace.rs`.
- Follow the exact pattern of existing tests like `config_yaml_parsing` and `test_auto_dev_config_parsed_custom_values`.
- Test that the existing `config_yaml_backward_compatible` test continues to pass (it validates configs without new fields still parse).

### Existing Code References

- `src/workspace.rs` lines 61-90: `AutoDevConfig` struct and its `Default` impl -- follow this exact pattern for `AgentMeshSettings`.
- `src/workspace.rs` lines 92-116: `ConfigYaml` struct -- add `agent_mesh` field here.
- `src/workspace.rs` lines 121-165: `from_base_dir()` method -- wire in the new field following how `auto_dev` is handled (line 158).
- `src/workspace.rs` lines 168-178: `default_for()` method -- add the default value here.

### Overlap with Epic 13

This story covers the same scope as Story 13.1 from the SDK Integration epics (`epics.md`). Epic 13 was designed for the full SDK trait integration phase, while Epic 25 implements the same concepts for the auto-dev loop completeness. If Story 13.1 has already been implemented, this story becomes a validation-only task (verify the implementation matches these specs). If not, implement per these specs -- they are authoritative.

### References

- [Source: epics-auto-dev-loop.md#Story 25.1] -- acceptance criteria
- [Source: epics.md#Story 2.1] -- original SDK integration version of same story
- [Source: architecture.md#"Workspace Config Extension Pattern"] -- new config sections added as optional fields with `#[serde(default)]`

## Dev Agent Record

### Agent Model Used
Claude Opus 4.6 (1M context)

### Debug Log References
N/A

### Completion Notes List
- Added AgentMeshSettings struct with enabled, max_depth, agents_yaml_path fields
- Manual Default impl ensures max_depth=5 matches serde default_max_depth()
- Wired into WorkspaceConfig (non-Option), ConfigYaml (Option), from_base_dir(), default_for()
- All 24 workspace tests pass including 5 new agent_mesh tests
- cargo clippy -- -D warnings passes clean

### File List
- src/workspace.rs
