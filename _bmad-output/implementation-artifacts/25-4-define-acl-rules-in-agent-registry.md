# Story 25.4: Define ACL Rules in Agent Registry

**Epic:** 25 — Agent Mesh Safety Guards
**Story ID:** 25.4
**Status:** ready-for-dev

## Story

As a plugin developer,
I want ACL rules (`can_invoke`, `can_respond_to`) co-located with agent definitions in `src/agent_registry.rs`,
so that access control is authoritative, testable, and cannot drift from the registry.

## Acceptance Criteria

1. **Given** `BmadAgentRegistry` exists in `src/agent_registry.rs`, **When** `get_can_invoke(agent_name)` and `get_can_respond_to(agent_name)` methods are added, **Then** each returns a `Vec<String>` of agent names per the architecture rules.
2. **Given** ACL rules are defined, **When** `bmad/architect` ACL is queried, **Then** `can_invoke` returns `["bmad/analyst", "bmad/developer", "bmad/ux-designer"]` (alphabetically sorted) and `can_respond_to` returns `["bmad/pm", "bmad/sm"]`.
3. **Given** ACL rules are defined, **When** `bmad/qa` ACL is queried, **Then** `can_invoke` returns `["bmad/developer"]`.
4. **Given** ACL rules are defined, **When** `bmad/quick-dev` ACL is queried, **Then** `can_invoke` returns `[]` (empty -- self-contained, no mesh invocation).
5. **Given** ACL rules are defined, **When** any other agent's ACL is queried (developer, pm, sm, tech-writer, ux-designer, analyst), **Then** `can_invoke` returns `["bmad/developer"]` and `can_respond_to` returns `["bmad/pm", "bmad/sm"]`.
6. **Given** an `AgentAcl` struct is defined with `can_invoke: Vec<String>` and `can_respond_to: Vec<String>`, **When** `get_acl(agent_name)` is called, **Then** it returns the complete ACL for the agent.
7. **Given** unit tests are added, **When** `cargo test` runs, **Then** tests verify each agent's ACL matches the architecture specification, `bmad/quick-dev` has an empty `can_invoke` list, and all agents have `bmad/pm` and `bmad/sm` in `can_respond_to`.

## Tasks / Subtasks

- [ ] Define `AgentAcl` struct in `src/agent_registry.rs` (AC: #6)
  - [ ] Fields: `pub can_invoke: Vec<String>`, `pub can_respond_to: Vec<String>`
  - [ ] Derive `Debug, Clone, PartialEq`
- [ ] Implement `get_acl()` method on `BmadAgentRegistry` (AC: #1, #2, #3, #4, #5, #6)
  - [ ] Signature: `pub fn get_acl(&self, agent_name: &str) -> AgentAcl`
  - [ ] Match on agent_name to determine `can_invoke` list
  - [ ] All agents get `can_respond_to: vec!["bmad/pm", "bmad/sm"]`
  - [ ] Unknown agents return empty `can_invoke` and standard `can_respond_to`
- [ ] Implement convenience methods (AC: #1)
  - [ ] `pub fn get_can_invoke(&self, agent_name: &str) -> Vec<String>` -- delegates to `get_acl`
  - [ ] `pub fn get_can_respond_to(&self, agent_name: &str) -> Vec<String>` -- delegates to `get_acl`
- [ ] Add unit tests (AC: #7)
  - [ ] Test: `bmad/architect` can_invoke = `["bmad/analyst", "bmad/developer", "bmad/ux-designer"]`
  - [ ] Test: `bmad/architect` can_respond_to = `["bmad/pm", "bmad/sm"]`
  - [ ] Test: `bmad/qa` can_invoke = `["bmad/developer"]`
  - [ ] Test: `bmad/quick-dev` can_invoke = `[]` (empty)
  - [ ] Test: `bmad/developer` can_invoke = `["bmad/developer"]` (default rule)
  - [ ] Test: `bmad/pm` can_invoke = `["bmad/developer"]` (default rule)
  - [ ] Test: `bmad/sm` can_invoke = `["bmad/developer"]` (default rule)
  - [ ] Test: `bmad/tech-writer` can_invoke = `["bmad/developer"]` (default rule)
  - [ ] Test: `bmad/ux-designer` can_invoke = `["bmad/developer"]` (default rule)
  - [ ] Test: `bmad/analyst` can_invoke = `["bmad/developer"]` (default rule)
  - [ ] Test: all 9 agents have `["bmad/pm", "bmad/sm"]` in can_respond_to
  - [ ] Test: unknown agent `"bmad/nonexistent"` returns empty can_invoke and standard can_respond_to
- [ ] Run `cargo clippy -- -D warnings` and `cargo fmt --check`

## Dev Notes

### Key File

`src/agent_registry.rs` -- this is the ONLY file modified in this story.

### No Dependencies on Other Stories

This story can be implemented independently of Stories 25.1-25.3. It only modifies the agent registry, which already exists and is fully functional.

### AgentAcl Struct

```rust
/// Access control list for an agent in the mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentAcl {
    /// Agent names this agent can invoke
    pub can_invoke: Vec<String>,
    /// Agent names this agent can respond to
    pub can_respond_to: Vec<String>,
}
```

### ACL Implementation Strategy

The ACL rules are static and architecture-defined. They do NOT depend on the CSV manifest or any runtime data. Implement as a pure function based on agent name matching:

```rust
impl BmadAgentRegistry {
    /// Get the full ACL for an agent by name.
    pub fn get_acl(&self, agent_name: &str) -> AgentAcl {
        let can_respond_to = vec![
            "bmad/pm".to_string(),
            "bmad/sm".to_string(),
        ];

        let can_invoke = match agent_name {
            "bmad/architect" => vec![
                "bmad/analyst".to_string(),
                "bmad/developer".to_string(),
                "bmad/ux-designer".to_string(),
            ],
            "bmad/qa" => vec![
                "bmad/developer".to_string(),
            ],
            "bmad/quick-dev" => vec![],
            _ => vec![
                "bmad/developer".to_string(),
            ],
        };

        AgentAcl { can_invoke, can_respond_to }
    }

    /// Get the list of agents this agent can invoke.
    pub fn get_can_invoke(&self, agent_name: &str) -> Vec<String> {
        self.get_acl(agent_name).can_invoke
    }

    /// Get the list of agents this agent can respond to.
    pub fn get_can_respond_to(&self, agent_name: &str) -> Vec<String> {
        self.get_acl(agent_name).can_respond_to
    }
}
```

### ACL Rules (Complete Mapping)

| Agent | can_invoke | can_respond_to |
|-------|-----------|---------------|
| `bmad/architect` | `bmad/analyst`, `bmad/developer`, `bmad/ux-designer` | `bmad/pm`, `bmad/sm` |
| `bmad/qa` | `bmad/developer` | `bmad/pm`, `bmad/sm` |
| `bmad/quick-dev` | (empty) | `bmad/pm`, `bmad/sm` |
| `bmad/developer` | `bmad/developer` | `bmad/pm`, `bmad/sm` |
| `bmad/pm` | `bmad/developer` | `bmad/pm`, `bmad/sm` |
| `bmad/sm` | `bmad/developer` | `bmad/pm`, `bmad/sm` |
| `bmad/tech-writer` | `bmad/developer` | `bmad/pm`, `bmad/sm` |
| `bmad/ux-designer` | `bmad/developer` | `bmad/pm`, `bmad/sm` |
| `bmad/analyst` | `bmad/developer` | `bmad/pm`, `bmad/sm` |

### Why get_acl() Does Not Require &self

The ACL rules are static and do not depend on the loaded agents from the CSV manifest. However, the method takes `&self` to be consistent with the other registry methods (`list_agents`, `get_agent`) and because Story 25.5 will call `self.get_acl()` while also accessing `self.agents`. If you prefer a pure function instead of a method, that works too, but the convenience methods `get_can_invoke` / `get_can_respond_to` should be on `BmadAgentRegistry` for the public API.

### Return Type for Unknown Agents

The `get_acl()` method handles unknown agents gracefully via the `_` match arm. It returns the default `can_invoke: ["bmad/developer"]` and standard `can_respond_to`. This is a deliberate design choice -- unknown agents get conservative defaults rather than errors.

### Anti-Patterns to Avoid

- Do NOT load ACL rules from a file or external source. They are architecture constants.
- Do NOT use `HashMap` for ACL storage. The `Vec<String>` values are small (1-3 items) and always returned sorted.
- Keep `can_invoke` lists alphabetically sorted for deterministic output.
- Do NOT add `Serialize` or `Deserialize` to `AgentAcl` -- it is a runtime-only type. Story 25.5 handles serialization to YAML using `BTreeMap`.

### Existing Code References

- `src/agent_registry.rs` lines 28-31: `BmadAgentRegistry` struct -- add methods here
- `src/agent_registry.rs` lines 34-157: `impl BmadAgentRegistry` block -- add `get_acl()`, `get_can_invoke()`, `get_can_respond_to()` here
- `src/agent_registry.rs` lines 209-353: existing `#[cfg(test)] mod tests` block -- add ACL tests here
- `src/agent_registry.rs` line 14: `AgentEntry` struct (private) -- `AgentAcl` should be `pub` since Story 25.5 uses it

### Overlap with Epic 16

This story covers the same scope as Story 16.3 from the SDK Integration epics. The implementation is identical. If Story 16.3 has already been implemented, this story becomes a validation task. If not, implement per these specs.

### References

- [Source: epics-auto-dev-loop.md#Story 25.4] -- acceptance criteria
- [Source: epics.md#Story 5.3] -- original SDK integration version
- [Source: architecture.md] -- ACL rules for all 9 agents

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
