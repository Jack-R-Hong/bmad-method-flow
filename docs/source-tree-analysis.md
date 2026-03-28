# Source Tree Analysis — plugin-coding-pack

> Generated: 2026-03-28 | Scan Level: exhaustive | Mode: full_rescan

## Directory Structure

```
plugin-coding-pack/
├── src/                          # Rust source (5,137 LOC across 11 modules)
│   ├── lib.rs                    # ★ Crate root — plugin metadata, register(), CodingPackPlugin struct
│   ├── main.rs                   # ★ Binary entry — JSON-RPC stdio adapter, 16-method dispatch
│   ├── pack.rs                   # ★ Core — 18 action dispatcher, data-query/mutate routing, agents.yaml generation
│   ├── workspace.rs              # Workspace config resolution (WorkspaceConfig, WorkflowFilter, AutoDevConfig, AgentMeshSettings)
│   ├── validator.rs              # Workflow YAML & agents.yaml structural validation with DAG cycle detection
│   ├── plugin_bridge.rs          # HTTP/RPC bridge to 5 platform plugins (auto-loop, issue-sync, feedback-loop, workspace-tracker)
│   ├── config_injector.rs        # BmadAgentInjector — CSV manifest → per-agent system prompt injection
│   ├── tool_provider.rs          # BmadToolProvider — 6 LLM-callable tools wrapping pack actions
│   ├── agent_registry.rs         # BmadAgentRegistry — agent discovery, skill routing, ACL rules
│   ├── pulse_api.rs              # Minimal Pulse task API client (get_task only)
│   └── util.rs                   # Utility: is_executable() permission check
│
├── config/                       # Runtime configuration
│   ├── config.yaml               # Main config: db_path, log_level, plugin_dir, memory settings
│   ├── auto-loop.yaml            # Auto-dev routing: label → workflow mapping, retries, poll interval
│   ├── plugins/                  # Plugin binary directory (installed by install.sh)
│   │   ├── bmad-method            # Required sibling plugin
│   │   ├── provider-claude-code   # Required sibling plugin
│   │   ├── plugin-git-ops         # Optional git operations
│   │   ├── plugin-git-worktree    # Optional worktree management
│   │   ├── plugin-memory          # Optional memory/knowledge graph
│   │   └── plugin-coding-pack     # This plugin's own binary
│   ├── workflows/                # YAML workflow definitions (12 workflows)
│   │   ├── coding-feature-dev.yaml
│   │   ├── coding-quick-dev.yaml
│   │   ├── coding-bug-fix.yaml
│   │   ├── coding-story-dev.yaml
│   │   ├── coding-refactor.yaml
│   │   ├── coding-review.yaml
│   │   ├── coding-parallel-review.yaml
│   │   ├── coding-memory-index.yaml
│   │   ├── coding-pr-fix.yaml
│   │   ├── bootstrap-plugin.yaml
│   │   ├── bootstrap-rebuild.yaml
│   │   └── bootstrap-cycle.yaml
│   └── provider-configs/         # Provider-specific config overrides
│       ├── _default/             # Default agent/rule/skill markdown templates
│       └── claude-code/          # Claude Code provider overrides
│
├── dashboard/                    # Dashboard test harness (Playwright)
│   ├── package.json              # Playwright ^1.52.0
│   ├── playwright.config.ts
│   ├── tsconfig.json
│   ├── display-customizations.json
│   ├── mock-responses/           # Mock JSON for dashboard endpoint testing
│   │   ├── status.json
│   │   ├── workflows-list.json
│   │   ├── workflow-detail.json
│   │   ├── agents-list.json
│   │   ├── board-data.json
│   │   └── board-filters.json
│   └── tests/                    # 7 Playwright test files
│       ├── helpers.ts
│       ├── coding-pack.test.ts
│       ├── execute-workflow.test.ts
│       ├── scrum-board.test.ts
│       ├── scrum-board-filters.test.ts
│       ├── scrum-board-detail.test.ts
│       └── atdd-scrum-board.test.ts
│
├── tests/                        # Rust integration tests (1,019 LOC)
│   ├── registration_tests.rs     # SDK PluginRegistry integration: register(), injection pipeline, tool dispatch
│   ├── e2e_tests.rs              # End-to-end workflow execution tests
│   ├── e2e_executor_tests.rs     # Executor-specific e2e tests
│   ├── e2e/
│   │   └── mod.rs                # E2E test utilities
│   └── fixtures/                 # Test data
│       ├── sample-project/       # Minimal Cargo.toml + lib.rs for testing
│       ├── mock-plugins/         # Stub plugin binaries
│       └── workflows/            # 15 test workflow YAMLs (happy/failure paths)
│
├── _bmad/                        # BMAD methodology data
│   └── _config/
│       └── agent-manifest.csv    # ★ Agent persona data for 9 BMAD agents
│
├── _bmad-output/                 # BMAD planning & implementation artifacts
│   ├── planning-artifacts/       # PRDs, architecture docs, epic breakdowns
│   └── implementation-artifacts/ # 70+ story implementation specs
│
├── docs/                         # Generated project documentation (this folder)
├── plugin-packs/                 # Pack distribution bundles
│
├── Cargo.toml                    # ★ Package manifest: plugin-coding-pack v0.1.0
├── Cargo.lock                    # Dependency lock file
├── README.md                     # Project readme (Chinese/English)
├── install.sh                    # Build + install script for all sibling plugins
├── uninstall.sh                  # Cleanup script
├── pulse.db                      # SQLite database (runtime data)
└── .gitignore
```

## Critical Folders

| Folder | Purpose | Key Files |
|--------|---------|-----------|
| `src/` | All Rust source code | `lib.rs` (entry), `pack.rs` (actions), `main.rs` (binary) |
| `config/plugins/` | Installed plugin binaries | `bmad-method`, `provider-claude-code` (required) |
| `config/workflows/` | YAML workflow definitions | 12 workflow files controlling dev pipelines |
| `_bmad/_config/` | Agent persona manifest | `agent-manifest.csv` — source of truth for 9 BMAD agents |
| `tests/` | Rust integration tests | `registration_tests.rs` (SDK integration) |
| `dashboard/tests/` | Playwright E2E tests | 7 test files covering dashboard endpoints |

## Entry Points

| Entry Point | File | Purpose |
|-------------|------|---------|
| Library crate | `src/lib.rs` | `register()` for server mode, `CodingPackPlugin` for WIT traits |
| Binary | `src/main.rs` | JSON-RPC stdio adapter with `dispatch_combined()` |
| Install | `install.sh` | Build and deploy all sibling plugins |
| Tests (Rust) | `cargo test` | Unit tests + integration tests |
| Tests (E2E) | `cd dashboard && npx playwright test` | Dashboard endpoint tests |

## Module Dependency Graph

```
lib.rs
├── pack (action dispatcher)
│   ├── validator (workflow/agents YAML validation)
│   ├── workspace (WorkspaceConfig resolution)
│   ├── plugin_bridge (platform plugin delegation)
│   ├── pulse_api (task API client)
│   └── util (is_executable)
├── config_injector (BmadAgentInjector — CSV → system prompts)
├── tool_provider (BmadToolProvider — pack actions as LLM tools)
│   └── pack (action execution)
└── agent_registry (BmadAgentRegistry — agent discovery + ACL)
    └── config_injector (CSV parsing utilities)

main.rs
├── lib (CodingPackPlugin, BmadAgentInjector, BmadToolProvider, BmadAgentRegistry)
└── pulse_plugin_sdk::dev_adapter (JSON-RPC stdio loop)
```
