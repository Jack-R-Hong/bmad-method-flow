# Story 6.2: Coding Workflow Memory Step Integration

Status: done

## Story

As a developer,
I want all coding workflows to automatically query the knowledge graph before implementation and assess risk before commit,
So that AI agents have codebase-aware context and changes are validated against the dependency graph.

## Acceptance Criteria

1. All 6 coding workflows declare `plugin: plugin-memory` with `optional: true` in `requires`
2. Each workflow adds `memory_context` (or `memory_impact`) as the first step, querying the knowledge graph with `{{input}}`
3. Architect / planning agent steps receive memory context via `context_from: [memory_context]`
4. Agent system prompts include guidance to use knowledge graph context when available
5. `memory_detect_changes` step runs after QA review / regression check, before git commit
6. `memory_reindex` step runs after git commit with `--preserve-embeddings`
7. All memory steps are marked `optional: true` — workflows succeed without them
8. `coding-review` workflow feeds memory context to both parallel review branches
9. All workflow YAML files pass `validate-workflows` action without errors

## Tasks / Subtasks

- [x] Task 1: Update `coding-feature-dev.yaml` (AC: 1, 2, 3, 5, 6, 7)
  - [x] Add `plugin-memory` optional requirement
  - [x] Add `memory_context` step (first, query, optional)
  - [x] Wire `context_from: [memory_context]` to architect step
  - [x] Add `memory_detect_changes` after QA review
  - [x] Add `memory_reindex` after git commit
- [x] Task 2: Update `coding-bug-fix.yaml` (AC: 1, 2, 3, 4, 5, 6, 7)
  - [x] Add `memory_context` step feeding into `analyze_bug`
  - [x] Update architect system prompt with knowledge graph guidance
  - [x] Add `memory_detect_changes` and `memory_reindex`
- [x] Task 3: Update `coding-quick-dev.yaml` (AC: 1, 2, 3, 6, 7)
  - [x] Add `memory_context` step feeding into `quick_spec`
  - [x] Add `memory_reindex` after commit (skip detect-changes for quick workflow)
- [x] Task 4: Update `coding-refactor.yaml` (AC: 1, 2, 3, 4, 5, 6, 7)
  - [x] Add `memory_impact` step (blast radius is critical for refactoring)
  - [x] Update architect system prompt with blast radius guidance
  - [x] Add `memory_detect_changes` and `memory_reindex`
- [x] Task 5: Update `coding-story-dev.yaml` (AC: 1, 2, 3, 6, 7)
  - [x] Add `memory_context` step feeding into `prepare_story`
  - [x] Add `memory_detect_changes` and `memory_reindex`
- [x] Task 6: Update `coding-review.yaml` (AC: 1, 2, 4, 7, 8)
  - [x] Add `memory_context` step
  - [x] Both parallel review steps depend on and receive `memory_context`
  - [x] Update review system prompts with knowledge graph guidance
- [x] Task 7: Validate all workflows (AC: 9)
  - [x] Run `validate-workflows` action — all 10 workflows pass

## Dev Notes

- Memory steps use `type: function` with `executor: plugin-memory`
- The `optional: true` flag on steps ensures graceful degradation when plugin-memory is absent
- `memory_context` / `memory_impact` use `plugin-memory query "{{input}}"` — the provider decides how to handle the query
- `memory_detect_changes` uses `plugin-memory detect-changes` — maps git diff to affected processes
- `memory_reindex` uses `plugin-memory reindex .` — incremental re-index preserving embeddings
- Quick-dev workflow skips `memory_detect_changes` to maintain minimal ceremony philosophy

### Workflow Step Injection Pattern

Each coding workflow follows this pattern:

```
[memory_context/impact] → [architect/plan] → ... → [QA/review]
                                                        ↓
                                              [memory_detect_changes]
                                                        ↓
                                                   [git_commit]
                                                        ↓
                                                 [memory_reindex]
```

### File List

- `config/workflows/coding-feature-dev.yaml` — MODIFIED: +3 memory steps, +optional plugin-memory require
- `config/workflows/coding-bug-fix.yaml` — MODIFIED: +3 memory steps, updated architect prompt
- `config/workflows/coding-quick-dev.yaml` — MODIFIED: +2 memory steps (context + reindex)
- `config/workflows/coding-refactor.yaml` — MODIFIED: +3 memory steps, updated architect prompt
- `config/workflows/coding-story-dev.yaml` — MODIFIED: +3 memory steps
- `config/workflows/coding-review.yaml` — MODIFIED: +1 memory step feeding both parallel branches

### References

- [Source: architecture.md#Workflow Template Architecture] — step organization, conditional execution, plugin dependency
- [Source: epics.md#Epic 6] — FR46-FR53 requirements
- [Source: coding-feature-dev.yaml] — reference workflow structure for step injection pattern
