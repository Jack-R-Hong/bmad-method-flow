# Story 23.3: Create coding-pr-fix Workflow Template

Status: ready-for-dev

## Story

As a workflow designer,
I want a `coding-pr-fix.yaml` workflow template that reads review feedback, applies fixes, and pushes to the PR branch,
So that review-driven fixes follow a standard, tested workflow pattern.

## Acceptance Criteria

1. **Given** a new `config/workflows/coding-pr-fix.yaml` is created, **When** the workflow is inspected, **Then** it contains these steps in order:
   1. `read_review` -- function step calling `build_fix_context()` via the `plugin-coding-pack` executor to fetch review comments for `{{pr_number}}`
   2. `checkout_branch` -- function step checking out the existing PR branch `{{pr_branch}}` (not creating a new branch)
   3. `implement_fix` -- agent step (executor: `provider-claude-code`) with review context injected via `context_from: [read_review]`, system prompt instructing the agent to address each review comment
   4. `run_tests` -- function step running `cargo test 2>&1 || npm test 2>&1 || echo 'no test runner found'` with `timeout_seconds: 120`
   5. `git_commit` -- function step committing with message `fix: address review feedback for PR #{{pr_number}}`
   6. `git_push` -- function step pushing to the existing PR branch via `plugin-git-pr push`
   And the workflow declares `requires: [provider-claude-code, plugin-git-ops]`.

2. **Given** the `implement_fix` step, **When** the `run_tests` step fails, **Then** the `implement_fix` step is retried with `retry.max_attempts: 2` and `retry.on_failure_of: run_tests` (matching the pattern in `coding-bug-fix.yaml`).

3. **Given** the workflow uses `{{pr_number}}` and `{{pr_branch}}` template variables, **When** triggered with these variables populated, **Then** all steps operate on the correct PR branch (the `checkout_branch` step switches to the existing branch, not creating a new one).

4. **Given** the workflow is added to `config/workflows/`, **When** `validate-workflows` action is run, **Then** it passes validation without errors.

5. **Given** the `read_review` step, **When** executed, **Then** it calls the pack action `build-fix-context` with payload `{"pr_number": {{pr_number}}}` and produces structured fix context that the `implement_fix` step reads via `context_from`.

## Tasks / Subtasks

- [ ] Task 1: Create `config/workflows/coding-pr-fix.yaml` (AC: 1, 2, 3, 5)
  - [ ] 1.1 Add workflow metadata: `name: coding-pr-fix`, `version: 1`, `description` explaining the PR fix workflow
  - [ ] 1.2 Add `requires` block listing `provider-claude-code`, `bmad-method`, `plugin-git-ops`
  - [ ] 1.3 Add Step 1 `read_review`: function step, executor `plugin-coding-pack`, command invokes `build-fix-context` action with `{{pr_number}}`
  - [ ] 1.4 Add Step 2 `checkout_branch`: function step, executor `git-ops`, command `git checkout {{pr_branch}}` (existing branch)
  - [ ] 1.5 Add Step 3 `implement_fix`: agent step, executor `provider-claude-code`, with `context_from: [read_review]`, system prompt instructing fix of each review comment, retry config `max_attempts: 2` on `on_failure_of: run_tests`
  - [ ] 1.6 Add Step 4 `run_tests`: function step, command `bash -c "cargo test 2>&1 || npm test 2>&1 || echo 'no test runner found'"`, `timeout_seconds: 120`
  - [ ] 1.7 Add Step 5 `git_commit`: function step, executor `git-ops`, command `git add -A && git commit -m "fix: address review feedback for PR #{{pr_number}}"`
  - [ ] 1.8 Add Step 6 `git_push`: function step, executor `plugin-git-ops`, command `plugin-git-pr push`, `timeout_seconds: 30`

- [ ] Task 2: Validate the workflow template (AC: 4)
  - [ ] 2.1 Run `cargo test` to ensure existing tests still pass (no regressions)
  - [ ] 2.2 Verify the workflow passes `validate-workflows` by checking that `validate_workflow_file()` accepts it
  - [ ] 2.3 Ensure all `depends_on` references are valid (each step depends on a previous step's ID)

- [ ] Task 3: Add `"pr-fix"` label routing in `auto_dev.rs` (AC: 1)
  - [ ] 3.1 Add a match arm in `resolve_workflow_id()`: `"pr-fix" => return "coding-pr-fix"` -- this allows board tasks with label `pr-fix` to be routed to this workflow
  - [ ] 3.2 Add unit test `test_resolve_workflow_from_label_pr_fix` verifying the new routing

- [ ] Task 4: Write unit tests (AC: 1, 4)
  - [ ] 4.1 Add test in `src/pack.rs` tests: `test_coding_pr_fix_workflow_listed` -- verify `list-workflows` includes `coding-pr-fix` after the YAML file is added
  - [ ] 4.2 Add test `test_coding_pr_fix_workflow_validates` -- load and validate the YAML file directly using `validator::validate_workflow_file()`

## Dev Notes

### Workflow YAML Pattern -- Follow `coding-bug-fix.yaml` Exactly

The closest existing workflow is `coding-bug-fix.yaml`. The PR fix workflow follows the same structure but is simpler (no memory steps, no QA review step) because the review feedback replaces the architect analysis step:

```yaml
name: coding-pr-fix
version: 1
description: "PR fix workflow: read review feedback -> implement fixes -> test -> commit -> push"
requires:
  - plugin: provider-claude-code
  - plugin: bmad-method
  - plugin: plugin-git-ops
steps:
  # Step 1: Read review feedback
  - id: read_review
    type: function
    depends_on: []
    executor: plugin-coding-pack
    config:
      command: ["plugin-coding-pack", "build-fix-context", "--pr-number", "{{pr_number}}"]
      timeout_seconds: 30

  # Step 2: Checkout the existing PR branch
  - id: checkout_branch
    type: function
    depends_on: [read_review]
    executor: git-ops
    config:
      command: ["git", "checkout", "{{pr_branch}}"]
      timeout_seconds: 15

  # Step 3: Implement fixes based on review feedback
  - id: implement_fix
    type: agent
    depends_on: [checkout_branch]
    executor: provider-claude-code
    config:
      model_tier: balanced
      system_prompt: |
        You are a senior developer addressing PR review feedback.
        You have been given structured review comments grouped by file.
        For each comment:
        1. Read the reviewer's feedback and the diff_hunk context
        2. Make the requested change in the correct file at the correct location
        3. If the feedback is unclear, make a reasonable interpretation
        Do NOT introduce unrelated changes. Only fix what reviewers requested.
        After all fixes, verify the code compiles.
      user_prompt_template: "Address PR review feedback for PR #{{pr_number}} on branch {{pr_branch}}"
      max_tokens: 8192
      context_from: [read_review]
      timeout_seconds: 600
      retry:
        max_attempts: 2
        on_failure_of: run_tests

  # Step 4: Run tests to verify fixes
  - id: run_tests
    type: function
    depends_on: [implement_fix]
    config:
      command: ["bash", "-c", "cargo test 2>&1 || npm test 2>&1 || echo 'no test runner found'"]
      timeout_seconds: 120

  # Step 5: Commit the fix
  - id: git_commit
    type: function
    depends_on: [run_tests]
    executor: git-ops
    config:
      command: ["bash", "-c", "git add -A && git commit -m \"fix: address review feedback for PR #{{pr_number}}\""]

  # Step 6: Push to the PR branch
  - id: git_push
    type: function
    depends_on: [git_commit]
    executor: plugin-git-ops
    config:
      command: ["plugin-git-pr", "push"]
      timeout_seconds: 30
```

### Template Variables

The workflow expects two template variables to be provided by the caller (Story 23.4's auto-dev integration):

- `{{pr_number}}` -- the GitHub PR number (e.g., `42`). Used by `read_review` step to fetch fix context and by `git_commit` step for the commit message.
- `{{pr_branch}}` -- the PR's head branch name (e.g., `auto-dev/story-23-1`). Used by `checkout_branch` step to switch to the correct branch.

These are populated by `auto_dev.rs` when it creates a PR fix task (Story 23.4).

### Step Dependencies Chain

```
read_review → checkout_branch → implement_fix → run_tests → git_commit → git_push
```

This is a linear pipeline. No parallel steps. The `implement_fix` step has a retry loop that goes back to `run_tests` on failure (up to 2 attempts).

### `read_review` Step: Pack Action Invocation

The `read_review` step calls the `build-fix-context` pack action added in Story 23.2. The executor is `plugin-coding-pack` which routes through `execute_action()` in `pack.rs`. The step output is the JSON `FixContext` struct, which `implement_fix` receives via `context_from: [read_review]`.

The exact command format depends on how the executor dispatches function steps. Based on existing workflow patterns (e.g., `coding-bug-fix.yaml` calling `plugin-memory query`), the command array invokes the plugin binary with subcommand and arguments.

### `checkout_branch` Step: Existing Branch Only

This step MUST use `git checkout {{pr_branch}}`, NOT `git checkout -b`. The branch already exists because the PR was created in a previous auto-dev cycle. If the branch does not exist locally, `git checkout` will automatically create a tracking branch from the remote.

### Retry Configuration

The retry config on `implement_fix` matches the existing pattern in `coding-bug-fix.yaml` (lines 53-55):

```yaml
retry:
  max_attempts: 2
  on_failure_of: run_tests
```

This means: if `run_tests` fails after `implement_fix` completes, re-run `implement_fix` (with test failure context) up to 2 times.

### Workflow Label Routing

Add `"pr-fix"` to the label-based routing in `auto_dev.rs::resolve_workflow_id()`:

```rust
for label in &assignment.labels {
    match label.as_str() {
        "story" => return "coding-story-dev",
        "bug" => return "coding-bug-fix",
        "refactor" => return "coding-refactor",
        "quick" => return "coding-quick-dev",
        "feature" => return "coding-feature-dev",
        "review" => return "coding-review",
        "pr-fix" => return "coding-pr-fix",  // NEW
        _ => {}
    }
}
```

### Anti-Patterns to Avoid

- **Do NOT** add memory steps (memory_context, memory_reindex) -- the PR fix workflow is lightweight and fast; memory indexing adds unnecessary latency for review fixes
- **Do NOT** add a QA review step -- the original PR already went through review; the fix only addresses specific feedback
- **Do NOT** add a `create_pr` step -- the PR already exists; we are pushing fix commits to the same branch
- **Do NOT** add a `create_worktree` step -- the PR branch already exists; use simple `git checkout`
- **Do NOT** use `git checkout -b` -- the branch must already exist (it was created when the original PR was made)
- **Do NOT** hardcode test commands specific to Rust -- use the multi-runner pattern from existing workflows: `cargo test || npm test || echo 'no test runner found'`

### Testing Strategy

**Validation testing**: The workflow validator (`validator::validate_workflow_file()`) checks:
- All step IDs are unique
- All `depends_on` references point to existing step IDs
- Required fields are present (id, type, config)
- Required plugins exist in the plugins directory

**Unit tests**:
- `test_resolve_workflow_from_label_pr_fix` in `auto_dev.rs` -- verify label routing
- `test_coding_pr_fix_workflow_validates` in `pack.rs` -- load YAML and run validator

**Manual verification**: After creating the file, run `cargo test` to ensure no regressions in `test_coding_pr_fix_workflow_listed` or other workflow-related tests.

### Files Created

- `config/workflows/coding-pr-fix.yaml` -- new workflow template file

### Files Modified

- `src/auto_dev.rs` -- add `"pr-fix" => return "coding-pr-fix"` in `resolve_workflow_id()` match block; add unit test

### No Rust Dependency Changes

This story only creates a YAML file and adds one match arm. No new crate dependencies.

### References

- [Source: _bmad-output/planning-artifacts/epics-auto-dev-loop.md, Epic 23, Story 23.3]
- [Source: config/workflows/coding-bug-fix.yaml -- closest workflow pattern, retry config]
- [Source: config/workflows/coding-story-dev.yaml -- worktree/branch workflow pattern]
- [Source: src/auto_dev.rs -- resolve_workflow_id() label routing]
- [Source: src/validator.rs -- workflow validation logic]

## Dev Agent Record

### Agent Model Used

### Debug Log References

### Completion Notes List

### File List
