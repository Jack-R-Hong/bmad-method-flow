# Story 25.2: Implement Depth Guard Check in Executor

**Epic:** 25 — Agent Mesh Safety Guards
**Story ID:** 25.2
**Status:** ready-for-dev

## Story

As a platform operator,
I want the executor to enforce recursion depth limits before executing agent steps,
so that infinite agent-to-agent recursion is prevented and runaway agent chains fail fast.

## Acceptance Criteria

1. **Given** the `PULSE_AGENT_DEPTH` environment variable is not set, **When** `check_depth_guard(config)` is called, **Then** the function reads the env var (defaulting to 0), computes `next = current + 1`, and returns `Ok(1)`.
2. **Given** `PULSE_AGENT_DEPTH` is set to `"4"` and `max_depth` is 5, **When** `check_depth_guard(config)` is called, **Then** the function returns `Ok(5)` (next = 4 + 1 = 5, which equals max, so allowed).
3. **Given** `PULSE_AGENT_DEPTH` is set to `"5"` and `max_depth` is 5, **When** `check_depth_guard(config)` is called, **Then** the function returns `Err(WitPluginError::invalid_input("agent mesh depth limit exceeded: 6 > 5"))`.
4. **Given** `PULSE_AGENT_DEPTH` is set to a non-numeric value like `"abc"`, **When** `check_depth_guard(config)` is called, **Then** the function treats it as 0 and returns `Ok(1)`.
5. **Given** the depth guard implementation exists, **When** performance is measured, **Then** the check completes in <1ms (NFR-AD-6: single env var read + integer comparison).
6. **Given** the depth guard is integrated into `execute_step()` in `src/executor.rs`, **When** an agent step is about to dispatch, **Then** `check_depth_guard()` is called first, and a depth violation prevents the dispatch entirely.

## Tasks / Subtasks

- [ ] Add `check_depth_guard()` function to `src/executor.rs` (AC: #1, #2, #3, #4)
  - [ ] Read `PULSE_AGENT_DEPTH` from `std::env::var()`, parse to `u32`, default to 0 on missing or parse failure
  - [ ] Compute `next = current + 1`
  - [ ] Read `max_depth` from `config.agent_mesh.max_depth` (direct field access since Story 25.1 makes it non-Option)
  - [ ] If `next > max`, return `Err(WitPluginError::invalid_input(format!("agent mesh depth limit exceeded: {} > {}", next, max)))`
  - [ ] Otherwise return `Ok(next)`
- [ ] Integrate depth guard into `execute_step()` (AC: #6)
  - [ ] Add `config: &WorkspaceConfig` parameter to `execute_step()` (requires updating the call site in `execute_workflow_steps()`)
  - [ ] Call `check_depth_guard(config)?` at the start of the `"agent"` match arm, before dispatching to `execute_agent_step()` or `execute_bmad_agent_step()`
  - [ ] Alternatively, call it inside `execute_agent_step()` itself -- choose the location that ensures it runs for ALL agent step variants
- [ ] Add unit tests for depth guard (AC: #1, #2, #3, #4, #5)
  - [ ] Test: no env var set -> returns Ok(1)
  - [ ] Test: env var = "4", max_depth = 5 -> returns Ok(5)
  - [ ] Test: env var = "5", max_depth = 5 -> returns Err (6 > 5)
  - [ ] Test: env var = "abc" -> returns Ok(1) (treated as 0)
  - [ ] Test: env var = "0" -> returns Ok(1)
  - [ ] Test: custom max_depth via AgentMeshSettings (e.g., max_depth=3) is respected
- [ ] Run `cargo clippy -- -D warnings` and `cargo fmt --check`

## Dev Notes

### Key File

`src/executor.rs` -- add the `check_depth_guard()` function and integrate it into the step execution flow.

### Dependency

Story 25.1 must be complete first -- `check_depth_guard` takes `&WorkspaceConfig` and accesses `config.agent_mesh.max_depth`.

### Exact Implementation

The architecture provides the reference implementation:

```rust
/// Check agent mesh recursion depth. Returns the next depth value if allowed.
pub(crate) fn check_depth_guard(config: &crate::workspace::WorkspaceConfig) -> Result<u32, WitPluginError> {
    let current: u32 = std::env::var("PULSE_AGENT_DEPTH")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let next = current + 1;
    let max = config.agent_mesh.max_depth;
    if next > max {
        return Err(WitPluginError::invalid_input(
            format!("agent mesh depth limit exceeded: {} > {}", next, max)
        ));
    }
    Ok(next)
}
```

Note: If Story 25.1 uses `Option<AgentMeshSettings>` instead of `AgentMeshSettings` on `WorkspaceConfig`, adjust to `config.agent_mesh.as_ref().map(|m| m.max_depth).unwrap_or(5)`.

### Integration Point in execute_step()

The `execute_step()` function at line 797 of `src/executor.rs` currently dispatches based on `step.step_type`. The depth guard should run for agent steps. The integration point:

```rust
fn execute_step(
    step: &StepDef,
    outputs: &HashMap<String, StepOutput>,
    template_vars: &HashMap<String, String>,
    plugins_dir: &Path,
    use_injection_pipeline: bool,
    config: &crate::workspace::WorkspaceConfig,  // NEW parameter
) -> Result<(StepOutput, Option<String>), WitPluginError> {
    let timeout_secs = step.config.as_ref()
        .and_then(|c| c.timeout_seconds)
        .unwrap_or(DEFAULT_TIMEOUT_SECS);

    match step.step_type.as_str() {
        "agent" => {
            // Depth guard: prevent infinite agent recursion
            if config.agent_mesh.enabled {
                check_depth_guard(config)?;
            }
            execute_agent_step(/* ... */)
        }
        // ...
    }
}
```

The depth guard only needs to run when `agent_mesh.enabled` is true. When mesh is disabled (the default), no depth check is needed. However, the epic AC says depth guard runs before dispatch regardless -- choose the interpretation that matches: if `enabled` is false, the check is a no-op/skipped.

### Updating the Call Site

The `execute_step()` call at line 247 in `execute_workflow_steps()` needs the `WorkspaceConfig` reference. The `execute_workflow_steps()` function already receives `plugins_dir` and `use_injection_pipeline` from `execute_workflow_with_config()`, but does NOT receive the full `WorkspaceConfig`. You have two options:

1. **Pass `&WorkspaceConfig` through**: Add it as a parameter to `execute_workflow_steps()` and down to `execute_step()`. This requires updating the function signature chain.
2. **Pass only the parts needed**: Since `check_depth_guard` only needs `agent_mesh.max_depth`, you could pass a `max_depth: u32` through. But this is less flexible for Story 25.3 which needs the full config.

**Recommendation**: Pass `&WorkspaceConfig` through to `execute_step()`. Story 25.3 will need it anyway for mesh config injection. This avoids a second refactor.

### Testing Notes

- Use `std::env::set_var()` and `std::env::remove_var()` in tests. These are process-global and NOT thread-safe. The project does not use the `serial_test` crate.
- Create a helper function that builds a `WorkspaceConfig` with a specific `max_depth` for testing.
- Add tests in the existing `#[cfg(test)] mod tests` block at the bottom of `src/executor.rs`.
- The function should be `pub(crate)` so it can be tested from the module's own tests.

### Anti-Patterns to Avoid

- Do NOT use `unwrap()` or `expect()` in the production `check_depth_guard` function. The `.ok().and_then().unwrap_or()` chain is the correct safe pattern.
- Do NOT skip the depth guard for any agent step type (bmad-method or direct).
- Do NOT read any file or do network I/O in the depth guard -- it must be O(1).

### Existing Code References

- `src/executor.rs` line 1: already imports `WitPluginError`
- `src/executor.rs` line 797: `execute_step()` function -- the integration point
- `src/executor.rs` line 247: call site of `execute_step()` in `execute_workflow_steps()`
- `src/executor.rs` line 172: `execute_workflow_steps()` function signature -- needs `&WorkspaceConfig` parameter added
- `src/workspace.rs`: `WorkspaceConfig` struct with `agent_mesh` field (from Story 25.1)

### Overlap with Epic 13

This story covers the same scope as Story 13.2 from the SDK Integration epics. Epic 13.2 only implemented and tested the function in isolation without integration into `execute_step()`. This story (25.2) additionally requires wiring the depth guard into the execution flow (AC #6). If Story 13.2 has been implemented, this story adds the integration step. If not, implement both the function and integration per these specs.

### References

- [Source: epics-auto-dev-loop.md#Story 25.2] -- acceptance criteria
- [Source: epics.md#Story 2.2] -- original SDK integration version
- [Source: architecture.md#Decision 3] -- exact implementation
- [Source: architecture.md#NFR-SDK-1] -- <1ms performance requirement

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
