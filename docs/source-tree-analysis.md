# Source Tree Analysis — plugin-coding-pack

> Generated: 2026-03-21 | Scan Level: quick

## Directory Structure

```
bmad-method-flow/                    # Project root
├── src/                             # Rust source — core plugin logic
│   ├── lib.rs                       #   Library entry point (cdylib + rlib)
│   ├── main.rs                      #   Binary entry point
│   ├── pack.rs                      #   Pack orchestration — plugin coordination
│   ├── validator.rs                 #   Pack validation — checks plugin readiness
│   └── util.rs                      #   Shared utilities
│
├── config/                          # Pulse runtime configuration
│   ├── config.yaml                  #   Main config (db_path, log_level, memory provider)
│   ├── plugins/                     #   Plugin binaries directory (deploy target)
│   └── config/workflows/            #   Workflow YAML definitions
│       ├── coding-quick-dev.yaml    #     Quick development (3 steps)
│       ├── coding-feature-dev.yaml  #     Full feature development (5 steps)
│       ├── coding-story-dev.yaml    #     Story-driven development (6 steps)
│       ├── coding-bug-fix.yaml      #     Bug fix workflow (4 steps)
│       ├── coding-refactor.yaml     #     Refactoring workflow (4 steps)
│       ├── coding-review.yaml       #     Code review workflow (3 steps)
│       ├── bootstrap-plugin.yaml    #     Self-development: single plugin (5 steps)
│       ├── bootstrap-rebuild.yaml   #     Self-development: rebuild all (3 steps)
│       └── bootstrap-cycle.yaml     #     Self-development: full cycle (8 steps)
│
├── dashboard/                       # Pulse dashboard extension (JSON manifest-driven)
│   ├── manifest.json                #   Dashboard page definitions (6 pages)
│   ├── display-customizations.json  #   UI display overrides
│   ├── mock-responses/              #   Mock API responses for dashboard testing
│   │   ├── status.json
│   │   ├── workflows-list.json
│   │   ├── workflow-detail.json
│   │   └── agents-list.json
│   └── tests/                       #   Dashboard extension tests (TypeScript)
│       ├── coding-pack.test.ts
│       ├── execute-workflow.test.ts
│       └── helpers.ts
│
├── plugin-packs/                    # Plugin pack definitions
│   └── coding.toml                  #   Coding pack manifest (5 plugins, 9 workflows)
│
├── docs/                            # Project documentation (project_knowledge)
│   ├── project-overview.md
│   ├── architecture.md
│   ├── source-tree-analysis.md      #   (this file)
│   ├── development-guide.md
│   ├── plugin-coding-pack.md        #   Detailed technical reference
│   └── project-scan-report.json     #   Scan workflow state
│
├── _bmad/                           # BMAD methodology framework (installed module)
│   ├── bmm/                         #   Core BMAD agents, workflows, teams
│   │   └── config.yaml              #   BMM module config (user prefs, output paths)
│   ├── core/                        #   Shared skills and tasks
│   │   └── config.yaml
│   ├── tea/                         #   Test architecture module
│   │   └── config.yaml
│   └── _config/                     #   BMAD configuration and agent customizations
│
├── _bmad-output/                    # BMAD workflow output artifacts
│   ├── planning-artifacts/          #   Architecture, PRD, UX design outputs
│   └── implementation-artifacts/    #   Sprint status, code generation outputs
│
├── .claude/                         # Claude Code configuration and skills
│
├── Cargo.toml                       # Rust package manifest
├── Cargo.lock                       # Dependency lock file (269 packages)
├── README.md                        # Project README (Chinese/English)
├── install.sh                       # Plugin installation script
├── uninstall.sh                     # Plugin uninstallation script
├── pulse.db                         # SQLite runtime database
└── .gitnexus/                       # GitNexus knowledge graph index
```

## Critical Folders

| Folder | Purpose | Key Files |
|--------|---------|-----------|
| `src/` | Core Rust plugin — all business logic | `lib.rs` (library entry), `main.rs` (binary entry), `pack.rs` (orchestration), `validator.rs` (validation) |
| `config/config/workflows/` | Pulse workflow definitions — defines the step pipelines for all 9 workflows | 9 YAML files |
| `dashboard/` | Dashboard UI extension — 6 SDK-rendered pages defined via JSON manifest | `manifest.json`, mock responses, tests |
| `plugin-packs/` | Pack manifest — declares required plugins, workflows, prerequisites, and dashboard config | `coding.toml` |
| `config/` | Runtime configuration — database path, logging, memory/knowledge graph provider | `config.yaml` |

## Entry Points

| Entry Point | Type | Purpose |
|-------------|------|---------|
| `src/lib.rs` | Library (cdylib + rlib) | Plugin interface — exposes pack actions (status, validate-pack, list-workflows, list-plugins) |
| `src/main.rs` | Binary | Standalone execution entry point |
| `install.sh` | Script | Builds and installs all plugin binaries + dashboard extension |
| `uninstall.sh` | Script | Removes installed plugin binaries and dashboard artifacts |

## Key Configuration Files

| File | Format | Purpose |
|------|--------|---------|
| `Cargo.toml` | TOML | Rust package manifest, dependencies, build targets |
| `config/config.yaml` | YAML | Runtime config (DB, logging, plugin dir, memory provider) |
| `plugin-packs/coding.toml` | TOML | Pack manifest (plugins, workflows, prerequisites, dashboard) |
| `dashboard/manifest.json` | JSON | Dashboard page definitions and layouts |

## Notable Patterns

- **No custom frontend code**: Dashboard is entirely JSON-manifest-driven, rendered by Pulse SDK
- **WASM target support**: `wit-bindgen` dependency for `wasm32` target — plugin can run as WASM component
- **Self-bootstrapping**: Bootstrap workflows allow the plugin to develop and rebuild itself
- **Nested workflow duplication**: `config/config/workflows/config/workflows/` contains duplicated workflow files (likely a copy artifact to investigate)
