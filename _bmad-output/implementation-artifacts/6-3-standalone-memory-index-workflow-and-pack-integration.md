# Story 6.3: Standalone Memory Index Workflow & Pack Integration

Status: done

## Story

As a developer,
I want a `coding-memory-index` workflow for initial codebase indexing and the memory plugin registered in the pack manifest,
So that I can bootstrap the knowledge graph and install it as part of the coding pack.

## Acceptance Criteria

1. `config/workflows/coding-memory-index.yaml` exists with `requires: [plugin: plugin-memory]`
2. Step 1 (`index_codebase`) runs `plugin-memory index .` with 300s timeout
3. Step 2 (`verify_index`) runs `plugin-memory health` with 30s timeout, depends on step 1
4. `plugin-packs/coding.toml` lists `plugin-memory` as optional plugin
5. `coding-memory-index.yaml` is included in the pack's workflow list
6. `npx` is listed in pack prerequisites with check command and install hint
7. `src/lib.rs` declares `plugin-memory` as optional dependency with `version_req: ">=0.1.0"`
8. `src/pack.rs` includes `plugin-memory` in optional plugins check
9. `validate-pack` reports missing plugin-memory as non-blocking
10. `config.defaults` in `coding.toml` includes `memory.provider` and `memory.auto_reindex` defaults
11. All 26 existing tests continue to pass after changes

## Tasks / Subtasks

- [x] Task 1: Create `coding-memory-index.yaml` workflow (AC: 1, 2, 3)
  - [x] Define 2-step workflow: index → verify
  - [x] Set appropriate timeouts (300s for index, 30s for health)
- [x] Task 2: Update `plugin-packs/coding.toml` (AC: 4, 5, 6, 10)
  - [x] Add `plugin-memory` to `[plugins]` section with `optional = true`
  - [x] Add `coding-memory-index.yaml` to `[workflows].include`
  - [x] Add `npx` to `[prerequisites]`
  - [x] Add `memory.provider` and `memory.auto_reindex` to `[config.defaults]`
- [x] Task 3: Update `src/lib.rs` plugin dependencies (AC: 7)
  - [x] Add `PluginDependency` for `plugin-memory` with `optional: true`
- [x] Task 4: Update `src/pack.rs` optional plugins (AC: 8, 9)
  - [x] Add `plugin-memory` to `optional_plugins` array
- [x] Task 5: Verify existing tests (AC: 11)
  - [x] Run `cargo test` — 26 tests pass
  - [x] Run `cargo build` — compiles successfully

## Dev Notes

- `plugin-memory` is a shell script (not a compiled binary), placed at `config/plugins/plugin-memory`
- The pack manifest uses `source: "local:config/plugins/plugin-memory"` since it's bundled with this plugin
- This story is primarily configuration/integration — no new Rust code beyond dependency declarations
- The `coding-memory-index` workflow is the entry point for users: run once to bootstrap the knowledge graph

### File List

- `config/workflows/coding-memory-index.yaml` — NEW: 2-step index + verify workflow
- `config/plugins/plugin-memory` — NEW: multi-provider shell wrapper (created in Story 6.1)
- `plugin-packs/coding.toml` — MODIFIED: +plugin-memory, +workflow, +prerequisite, +defaults
- `src/lib.rs` — MODIFIED: +plugin-memory optional dependency
- `src/pack.rs` — MODIFIED: +plugin-memory in optional_plugins

### References

- [Source: plugin-packs/coding.toml] — pack manifest format for plugin and workflow registration
- [Source: src/lib.rs#get_info] — PluginDependency declaration pattern
- [Source: src/pack.rs#validate_pack_value] — optional plugin validation pattern
