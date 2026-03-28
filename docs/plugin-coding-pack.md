# plugin-coding-pack

Pulse meta-plugin: coding pack orchestrator — validates, loads, and coordinates bmad-method + provider-claude-code + git-worktree plugins into a self-bootstrapping AI development system.

## Overview

`plugin-coding-pack` is the orchestration layer for the Pulse coding plugin pack. It coordinates 5 native plugins through 9 workflow pipelines to deliver AI-driven software development — from feature design to code review to self-evolution.

The defining capability is **self-bootstrapping**: the plugins develop themselves through the same workflows they power.

```
┌─────────────────────────────────────────────────┐
│              plugin-coding-pack                  │
│                (orchestrator)                     │
│                                                   │
│  ┌─────────────┐  ┌──────────────────┐           │
│  │ bmad-method │  │ provider-claude-  │           │
│  │  (required) │  │   code (required) │           │
│  │ 10 AI agents│  │ Claude Code CLI   │           │
│  └─────────────┘  └──────────────────┘           │
│  ┌─────────────┐  ┌──────────────────┐           │
│  │  git-ops    │  │ plugin-git-       │           │
│  │  (optional) │  │  worktree (opt)   │           │
│  └─────────────┘  └──────────────────┘           │
└─────────────────────────────────────────────────┘
```

## Quick Start

```bash
# From coding-pack directory
# Build the plugin
cargo build --release

# Install binary
cp target/release/plugin-coding-pack config/plugins/

# Validate setup
PULSE_DB_PATH=sqlite:pulse.db?mode=rwc \
  pulse registry validate --config ./config

# Run a workflow
PULSE_DB_PATH=sqlite:pulse.db?mode=rwc \
  pulse run coding-quick-dev --config ./config \
  -i '{"input": "Add input validation to the login endpoint"}'
```

## Plugins

| Plugin | Executor Name | Type | Description |
|--------|--------------|------|-------------|
| `bmad-method` | `bmad-method` | Required | BMAD AI team — 10 specialized agents (architect, dev, pm, qa, sm, etc.) |
| `provider-claude-code` | `provider-claude-code` | Required | Spawns Claude Code CLI for AI code generation |
| `plugin-git-ops` | `git-ops` | Optional | Git operations — commit, push, branch, PR creation |
| `plugin-git-worktree` | `plugin-git-worktree` | Optional | Git worktree isolation for parallel development |
| `plugin-coding-pack` | `plugin-coding-pack` | Self | Pack orchestrator — validate, list, status actions |

### BMAD Agent Roster

The `bmad-method` plugin provides 10 AI agents, each with a distinct persona:

| Agent | Name | Role | Used In Workflows |
|-------|------|------|-------------------|
| `bmad/architect` | Winston | System Architect | feature-dev, bug-fix, refactor, review, bootstrap |
| `bmad/dev` | Amelia | Developer | story-dev |
| `bmad/pm` | John | Product Manager | story preparation |
| `bmad/qa` | Quinn | QA Engineer | all review steps |
| `bmad/sm` | Bob | Scrum Master | story-dev, review synthesis |
| `bmad/quick-flow-solo-dev` | Barry | Quick Flow Specialist | quick-dev |
| `bmad/analyst` | Mary | Business Analyst | research workflows |
| `bmad/ux-designer` | Sally | UX Designer | UX workflows |
| `bmad/tech-writer` | Paige | Technical Writer | documentation |

## Workflows

### Coding Workflows (6)

#### `coding-feature-dev` — Full Feature Development
```
architect_design ──→ create_worktree ──→ dev_implement ──→ qa_review ──→ git_commit
  bmad/architect    plugin-git-worktree   claude-code      bmad/qa       git add
```
5 steps. Architect designs component breakdown and API contracts, then Claude Code implements with TDD, QA reviews for quality and edge cases.

#### `coding-story-dev` — Story-Driven Development
```
prepare_story ──→ technical_design ──→ create_worktree ──→ implement ──→ qa_review ──→ git_commit
  bmad/sm         bmad/architect      git-worktree       claude-code   bmad/qa       git add
```
6 steps. Scrum Master prepares story with acceptance criteria (Given/When/Then), architect designs, developer implements with TDD against ACs.

#### `coding-quick-dev` — Rapid Development
```
quick_spec ──→ implement ──→ git_commit
  bmad/barry   claude-code   git add
```
3 steps. Minimum ceremony. Quick Flow Solo Dev creates lean spec, Claude Code implements immediately.

#### `coding-bug-fix` — Bug Fix
```
analyze_bug ──→ implement_fix ──→ edge_case_review ──→ git_commit
bmad/architect   claude-code       bmad/qa            git add
```
4 steps. Root cause analysis (not just symptoms), fix with regression tests, edge case validation.

#### `coding-refactor` — Safe Refactoring
```
plan_refactor ──→ execute_refactor ──→ regression_check ──→ git_commit
bmad/architect    claude-code          bmad/qa             git add
```
4 steps. Incremental migration plan (each step leaves code working), QA verifies behavioral equivalence.

#### `coding-review` — Multi-Layer Code Review
```
adversarial_review ──┐
                     ├──→ review_synthesis
edge_case_review ────┘       bmad/sm
  bmad/architect
  bmad/qa
```
3 steps. Two reviews run **in parallel** (adversarial + edge-case hunting), then synthesized into prioritized findings (critical/high/medium/low).

### Bootstrap Workflows (3) — Self-Development

#### `bootstrap-plugin` — Develop a Single Plugin
```
architect_plan ──→ implement ──→ build_verify ──→ qa_review ──→ git_commit
 bmad/architect   claude-code    cargo test      bmad/qa       git add
```
5 steps. Plan → implement → test → review → commit.

#### `bootstrap-rebuild` — Rebuild All Plugins
```
build_all ──→ install_plugins ──→ validate
cargo build     cp binaries       pulse validate
```
3 steps. Rebuild all 4 sibling plugins, copy binaries, validate registry.

#### `bootstrap-cycle` — Full Self-Evolution Loop
```
plan ──→ implement ──→ test ──→ review ──→ rebuild ──→ install ──→ validate ──→ commit
bmad     claude-code   cargo   bmad/qa    cargo build   hot-swap   pulse val    git add
```
8 steps. The complete self-referential development cycle:
1. **plan** — Architect designs change with awareness of circular dependencies
2. **implement** — Claude Code implements following the plan
3. **test** — `cargo test` verifies implementation (16+ tests)
4. **review** — QA reviews for safety in self-bootstrapping context
5. **rebuild** — `cargo build --release` all 4 sibling plugins
6. **install** — Hot-swap: remove old binaries, copy new ones
7. **validate** — `pulse registry validate` confirms 9 workflows valid
8. **commit** — `git add -A` stages all changes

Runs in ~12 seconds end-to-end.

## Plugin Actions

The `plugin-coding-pack` executor exposes pack management actions:

### `validate-pack`
```json
{"action": "validate-pack"}
```
Checks required/optional plugins exist and counts workflow files.

**Response:**
```json
{
  "valid": true,
  "plugins_ok": 3,
  "workflows_found": 9,
  "issues": []
}
```

### `list-workflows`
```json
{"action": "list-workflows"}
```
Returns sorted list of all registered workflow YAML files.

### `list-plugins`
```json
{"action": "list-plugins"}
```
Returns installed plugin binaries with name, size, and executable status.

### `status`
```json
{"action": "status"}
```
Aggregates validation + workflows + plugins into a single health report.

## Architecture

### Project Structure

```
coding-pack/
├── Cargo.toml                  # Rust package manifest
├── src/
│   ├── lib.rs                  # CodingPackPlugin (PluginLifecycle + StepExecutorPlugin)
│   ├── main.rs                 # Native binary entry point (stdio JSON-RPC)
│   ├── pack.rs                 # Pack actions: validate, list, status
│   └── validator.rs            # Workflow YAML validation
├── config/
│   ├── config.yaml             # Pulse runtime config (db, log, plugin_dir)
│   ├── plugins/                # 5 native plugin binaries (~6MB total)
│   └── workflows/              # 9 workflow YAML definitions
├── plugin-packs/
│   └── coding.toml             # Pack declaration with dependencies & defaults
├── _bmad/                      # BMAD AI team system
│   ├── core/                   # 12 core skills
│   ├── bmm/                    # 24 BMM workflow skills + 10 agents
│   └── tea/                    # 9 Test Architecture Enterprise skills
└── _bmad-output/               # Generated artifacts
```

### Execution Model

```
Workflow Step
├── type: "agent"
│   ├── Resolve executor (bmad-method, provider-claude-code)
│   ├── Load system_prompt from step config
│   ├── Inject context_from previous steps
│   ├── Execute with model_tier (fast / balanced)
│   ├── Apply timeout_seconds (default: none — always set explicitly)
│   └── Return JSON result
├── type: "function"
│   ├── Execute command array via shell
│   ├── Apply timeout_seconds
│   └── Return stdout/stderr
└── depends_on: [step_ids]
    └── Wait for dependencies before starting
```

### Model Tiers

Agent steps use `model_tier` to select LLM parameters:

| Tier | Use Case | Typical Mapping | Token Budget |
|------|----------|-----------------|--------------|
| `fast` | Quick specs, lightweight decisions | Smaller/faster model (e.g. Haiku) | Lower `max_tokens` (1024–2048) |
| `balanced` | Design, implementation, review | Standard model (e.g. Sonnet) | Standard `max_tokens` (4096–8192) |

The actual model mapping is configured via `provider-claude-code.default_model` in `plugin-packs/coding.toml`. The tier serves as a hint to the provider plugin — it may adjust model selection, temperature, or token limits accordingly.

### Template Variables

Workflow steps use `{{variable}}` template syntax for dynamic values:

| Variable | Source | Used In |
|----------|--------|---------|
| `{{input}}` | User-provided workflow input | All `user_prompt_template` fields |
| `{{branch_name}}` | Generated by architect/SM agent in a prior step output | `coding-feature-dev`, `coding-story-dev` (worktree creation) |

Variables are resolved at step execution time. `context_from` injects prior step results, enabling multi-step context propagation.

### Error Handling & Recovery

| Scenario | Behavior |
|----------|----------|
| Agent step timeout | Step fails with timeout error; workflow halts at that step |
| Agent returns invalid JSON | Downstream `context_from` steps receive raw text; may fail on parse |
| Function step exits non-zero | Step fails; dependent steps are skipped |
| Required plugin missing | `validate-pack` reports error; workflow refuses to start |
| Build failure in bootstrap | `rebuild` step exits non-zero; `install` and `validate` steps are skipped |

**Recovery strategy:** Workflows do not auto-retry or rollback. On failure, the workflow stops at the failed step. The user should inspect the error, fix the root cause, and re-run the workflow. Bootstrap workflows use build isolation (separate `target/` dirs) and atomic binary swaps to minimize partial-failure risk.

### Self-Bootstrap Safety

The bootstrap-cycle handles the paradox of modifying running code:

1. **Build isolation** — Plugins compile in their own `target/` directories
2. **Atomic swap** — `rm` old binary then `cp` new one (avoids "text file busy")
3. **Post-swap validation** — `pulse registry validate` confirms integrity
4. **QA awareness** — Review step explicitly checks: "Will this change break the currently running workflow?"

## Configuration

### config/config.yaml
```yaml
db_path: "pulse.db"
log_level: "info"
plugin_dir: "config/plugins"
```

### Environment Variables
```bash
PULSE_DB_PATH=sqlite:pulse.db?mode=rwc  # SQLite with auto-create
PULSE_LLM_PROVIDER=anthropic             # Optional: for non-plugin agent steps
PULSE_LLM_MODEL=claude-sonnet-4-6        # Optional: model override
```

### Plugin Pack Defaults (coding.toml)
```toml
[config.defaults]
"provider-claude-code.default_model" = "sonnet"
"provider-claude-code.max_budget_usd" = 10.00
"bmad-method.communication_language" = "English"
"bmad-method.user_name" = "Jack"
```

## Building

```bash
# Build plugin
cargo build --release

# Run tests (16 tests)
cargo test

# Build all sibling plugins
for d in provider-claude-code git-ops git-worktree bmad-method; do
  (cd ../$d && cargo build --release)
done

# Install all to config/plugins/
DEST=config/plugins
cp ../provider-claude-code/target/release/provider-claude-code $DEST/
cp ../git-ops/target/release/plugin-git-ops $DEST/
cp ../git-worktree/target/release/plugin-git-worktree $DEST/
cp ../bmad-method/target/release/bmad-method $DEST/
cp target/release/plugin-coding-pack $DEST/
```

## Tests

```
$ cargo test
running 16 tests

lib::tests::
  plugin_health_check_returns_true ........... ok
  plugin_info_has_correct_name ............... ok
  plugin_info_declares_dependencies .......... ok
  probe_returns_ok ........................... ok
  execute_validate_pack_returns_success ...... ok
  execute_list_workflows_returns_success ..... ok
  execute_list_plugins_returns_success ....... ok
  execute_unknown_action_returns_error ....... ok
  execute_missing_input_returns_error ........ ok

pack::tests::
  validate_pack_returns_valid_json ........... ok
  list_workflows_returns_valid_json .......... ok
  list_plugins_returns_valid_json ............ ok
  unknown_action_returns_not_found ........... ok

validator::tests::
  valid_workflow_passes ...................... ok
  missing_name_fails ........................ ok
  missing_steps_fails ....................... ok

test result: ok. 16 passed; 0 failed
```

## BMAD Skills Inventory

The pack includes 47 skills across three modules:

| Module | Skills | Focus |
|--------|--------|-------|
| **core** (v6.2.0) | 12 | Elicitation, brainstorming, distillation, editorial review, party mode |
| **bmm** (v6.2.0) | 26 | Product briefs, PRD, architecture, epics, sprint planning, code review, story dev |
| **tea** (v1.7.0) | 9 | Test design, ATDD, automation, CI pipeline, NFR assessment, traceability |

## License

MIT OR Apache-2.0
