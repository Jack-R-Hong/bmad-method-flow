# Source Tree Analysis — plugin-coding-pack

> Generated: 2026-03-27 | Scan Level: quick | Mode: full_rescan

## Directory Structure

```
bmad-method-flow/                    # Project root
├── src/                             # Rust source — core plugin logic (13 modules)
│   ├── lib.rs                       #   Library entry point (cdylib + rlib) — plugin trait impl
│   ├── main.rs                      #   Binary entry point — CLI for standalone execution
│   ├── pack.rs                      #   Pack orchestration — plugin coordination & management
│   ├── validator.rs                 #   Pack validation — checks plugin readiness
│   ├── executor.rs                  #   Workflow execution engine — DAG-based step dispatch
│   ├── board.rs                     #   Scrum/Kanban board actions — epic/story/task operations
│   ├── board_store.rs               #   Board data persistence — JSON-based CRUD store
│   ├── tool_provider.rs             #   Tool provisioning — MCP-style tool interface
│   ├── config_injector.rs           #   Config injection — provider config template rendering
│   ├── workspace.rs                 #   Workspace detection — project root and config discovery
│   ├── agent_registry.rs            #   Agent discovery — workspace-based agent definitions
│   ├── test_parser.rs               #   Test result parsing — structured test output extraction
│   └── util.rs                      #   Shared utilities
│
├── config/                          # Pulse runtime configuration
│   ├── config.yaml                  #   Main config (db_path, log_level, memory provider)
│   ├── plugins/                     #   Plugin binaries directory (deploy target)
│   │   ├── plugin-coding-pack       #     This plugin (built binary)
│   │   ├── provider-claude-code     #     Claude Code LLM provider
│   │   ├── bmad-method              #     BMAD methodology plugin
│   │   ├── plugin-git-ops           #     Git operations plugin
│   │   ├── plugin-git-worktree      #     Git worktree management
│   │   ├── plugin-git-pr            #     Git PR creation (shell script)
│   │   └── plugin-memory            #     Memory/knowledge graph (shell script)
│   ├── workflows/                   #   Workflow YAML definitions (source of truth)
│   │   ├── coding-quick-dev.yaml    #     Quick development (3 steps)
│   │   ├── coding-feature-dev.yaml  #     Full feature development (5 steps)
│   │   ├── coding-story-dev.yaml    #     Story-driven development (6 steps)
│   │   ├── coding-bug-fix.yaml      #     Bug fix workflow (4 steps)
│   │   ├── coding-refactor.yaml     #     Refactoring workflow (4 steps)
│   │   ├── coding-review.yaml       #     Code review workflow (3 steps)
│   │   ├── coding-parallel-review.yaml  # Parallel code review (multi-reviewer)
│   │   ├── coding-memory-index.yaml #     Memory re-indexing workflow
│   │   ├── bootstrap-plugin.yaml    #     Self-development: single plugin (5 steps)
│   │   ├── bootstrap-rebuild.yaml   #     Self-development: rebuild all (3 steps)
│   │   └── bootstrap-cycle.yaml     #     Self-development: full cycle (8 steps)
│   ├── provider-configs/            #   LLM provider configuration templates
│   │   ├── _default/                #     Default templates (AGENT.md, RULE.md, SKILL.md)
│   │   └── claude-code/             #     Claude Code-specific overrides
│   └── config/workflows/...         #   Deployed workflow copies (install.sh output)
│
├── dashboard/                       # Pulse dashboard extension (JSON manifest-driven)
│   ├── manifest.json                #   Dashboard page definitions (11 pages)
│   ├── display-customizations.json  #   UI display overrides
│   ├── mock-responses/              #   Mock API responses for dashboard testing
│   │   ├── status.json              #     Pack health status
│   │   ├── workflows-list.json      #     Workflow listing
│   │   ├── workflow-detail.json     #     Single workflow details
│   │   ├── agents-list.json         #     AI agent roster
│   │   ├── board-data.json          #     Task board Kanban data
│   │   └── board-filters.json       #     Board filter definitions
│   └── tests/                       #   Dashboard extension tests (TypeScript)
│       ├── coding-pack.test.ts      #     Pack overview page tests
│       ├── execute-workflow.test.ts  #     Workflow execution tests
│       ├── scrum-board.test.ts      #     Scrum board basic tests
│       ├── scrum-board-detail.test.ts   # Card detail popup tests
│       ├── scrum-board-filters.test.ts  # Board filtering tests
│       ├── atdd-scrum-board.test.ts #     Acceptance-driven board tests
│       ├── board-tools-e2e.test.ts  #     Board tools end-to-end tests
│       └── helpers.ts               #     Test helper utilities
│
├── tests/                           # Rust integration & E2E tests
│   ├── registration_tests.rs        #   Plugin registration and action dispatch tests
│   ├── e2e_tests.rs                 #   End-to-end plugin integration tests
│   ├── e2e_executor_tests.rs        #   Workflow executor E2E tests (23+ tests)
│   ├── e2e/                         #   E2E test module
│   │   └── mod.rs                   #     E2E test harness
│   └── fixtures/                    #   Test fixtures
│       ├── mock-plugins/            #     Mock plugin executables (bmad-method, etc.)
│       ├── sample-project/          #     Sample Rust project for testing
│       └── workflows/               #     Test workflow YAML definitions (15 files)
│
├── plugin-packs/                    # Plugin pack definitions
│   └── coding.toml                  #   Coding pack manifest (7 plugins, 11 workflows)
│
├── docs/                            # Project documentation (project_knowledge)
│   ├── index.md                     #   Master documentation index
│   ├── project-overview.md          #   Executive summary
│   ├── architecture.md              #   Architecture documentation
│   ├── source-tree-analysis.md      #   (this file)
│   ├── development-guide.md         #   Development guide
│   ├── plugin-coding-pack.md        #   Detailed technical reference
│   └── project-scan-report.json     #   Scan workflow state
│
├── _bmad/                           # BMAD methodology framework (installed module)
│   ├── bmm/                         #   Core BMAD agents, workflows, teams
│   │   └── config.yaml              #   BMM module config (user prefs, output paths)
│   ├── core/                        #   Shared skills and tasks
│   └── tea/                         #   Test architecture module
│
├── _bmad-output/                    # BMAD workflow output artifacts
│   ├── planning-artifacts/          #   PRD, architecture, UX design outputs
│   │   └── prd.md                   #   Product Requirements Document
│   └── implementation-artifacts/    #   21 implementation story files
│
├── .claude/                         # Claude Code configuration and skills
│
├── Cargo.toml                       # Rust package manifest
├── Cargo.lock                       # Dependency lock file
├── README.md                        # Project README (Chinese/English)
├── install.sh                       # Plugin installation script
├── uninstall.sh                     # Plugin uninstallation script
└── pulse.db                         # SQLite runtime database
```

## Critical Folders

| Folder | Purpose | Key Files |
|--------|---------|-----------|
| `src/` | Core Rust plugin — all business logic (13 modules, ~315K source) | `lib.rs` (entry), `executor.rs` (workflow engine), `board.rs` + `board_store.rs` (scrum board), `tool_provider.rs` (MCP tools), `pack.rs` (orchestration) |
| `config/workflows/` | Pulse workflow definitions — defines the step pipelines for all 11 workflows | 11 YAML files covering coding, review, and bootstrap workflows |
| `dashboard/` | Dashboard UI extension — 11 SDK-rendered pages defined via JSON manifest | `manifest.json`, 6 mock responses, 8 test files |
| `tests/` | Integration and E2E test suite | 3 Rust test files, 15 workflow fixtures, mock plugins |
| `config/provider-configs/` | LLM provider configuration templates | Default templates for AGENT.md, RULE.md, SKILL.md |

## Entry Points

| Entry Point | Type | Purpose |
|-------------|------|---------|
| `src/lib.rs` | Library (cdylib + rlib) | Plugin interface — exposes all actions (pack management, workflow execution, board operations, tool provisioning) |
| `src/main.rs` | Binary | Standalone execution — CLI for direct workflow and board operations |
| `install.sh` | Script | Builds and installs all plugin binaries + dashboard extension |
| `uninstall.sh` | Script | Removes installed plugin binaries and dashboard artifacts |

## Key Configuration Files

| File | Format | Purpose |
|------|--------|---------|
| `Cargo.toml` | TOML | Rust package manifest, dependencies, build targets |
| `config/config.yaml` | YAML | Runtime config (DB, logging, plugin dir, memory provider) |
| `plugin-packs/coding.toml` | TOML | Pack manifest (7 plugins, 11 workflows, dashboard, prerequisites) |
| `dashboard/manifest.json` | JSON | Dashboard page definitions (11 pages) and layouts |
| `config/provider-configs/_default/*.md` | Markdown | Default provider config templates (AGENT, RULE, SKILL) |

## Notable Patterns

- **No custom frontend code**: Dashboard is entirely JSON-manifest-driven, rendered by Pulse SDK
- **WASM target support**: `wit-bindgen` dependency for `wasm32` target — plugin can run as WASM component
- **Self-bootstrapping**: Bootstrap workflows allow the plugin to develop and rebuild itself
- **Board system**: Full Scrum/Kanban board with JSON-based persistence (board_store.rs) for tracking epics, stories, and tasks
- **Tool provisioning**: MCP-style tool provider (tool_provider.rs) for exposing plugin capabilities to LLMs
- **Config injection**: Dynamic config injection (config_injector.rs) into provider configurations via templates
- **Agent discovery**: Workspace-based agent definition discovery (agent_registry.rs) from BMAD skill definitions
- **Nested workflow copies**: `config/config/workflows/config/workflows/` contains deployed workflow copies from install.sh
