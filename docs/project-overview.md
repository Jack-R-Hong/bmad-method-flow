# plugin-coding-pack — Project Overview

> Generated: 2026-03-27 | Scan Level: quick | Mode: full_rescan

## Executive Summary

`plugin-coding-pack` is a Rust-based meta-plugin for the [Pulse](https://github.com/pulsate-labs/pulse) platform that orchestrates AI-driven software development workflows. It coordinates 7 plugins (bmad-method, provider-claude-code, git-ops, git-worktree, git-pr, plugin-memory) through 11 workflow pipelines, enabling a complete development lifecycle from feature design to code review — all powered by BMAD methodology AI agents.

Key capabilities since initial release:
- **Workflow Execution Engine** — DAG-based step dispatch with retry loops, parallel execution, quality gates, and context propagation
- **Scrum Board** — Full Kanban board with JSON-based persistence for tracking epics, stories, and task assignments
- **Tool Provisioning** — MCP-style tool provider exposing plugin capabilities to LLMs
- **Config Injection** — Dynamic provider configuration via template rendering
- **Agent Discovery** — Workspace-based agent definition discovery from BMAD skill definitions
- **Self-bootstrapping** — The plugins develop themselves through the same workflows they power

## Project Identity

| Property | Value |
|----------|-------|
| **Package Name** | `plugin-coding-pack` |
| **Version** | 0.1.0 |
| **License** | MIT OR Apache-2.0 |
| **Author** | Jack |
| **Rust Edition** | 2021 (MSRV 1.85) |
| **Repository Type** | Monolith |
| **Project Type** | Library / Plugin |

## Technology Stack

| Category | Technology | Version | Purpose |
|----------|-----------|---------|---------|
| Language | Rust | 2021 edition, MSRV 1.85 | Core implementation |
| SDK | pulse-plugin-sdk | local path | Plugin trait implementations |
| Serialization | serde + serde_json + serde_yaml | 1.0 / 1.0 / 0.9 | JSON/YAML config and I/O |
| Logging | tracing | 0.1 | Structured logging |
| HTTP | reqwest | 0.12 (non-wasm) | HTTP client for external service calls |
| Async | tokio + async-trait | 1.40 / 0.1 (non-wasm) | Async runtime and trait abstractions |
| WASM Bindings | wit-bindgen | 0.53 | WASM component model (wasm32 target) |
| Database | SQLite | — | Runtime state (pulse.db) |
| Memory | GitNexus | npm latest | Knowledge graph / code indexing |
| Test Harness | pulse-plugin-test | local path | WASM harness for integration tests |
| Test Utilities | tempfile | 3 | Temporary file handling in tests |

## Architecture Type

**Plugin-based orchestrator with WASM Component Model**

The plugin implements four Pulse SDK traits:
- `PluginLifecycle` — Health checks and dependency declarations
- `StepExecutorPlugin` — Action dispatch (30+ actions across pack, board, workflow, and tool domains)
- `DashboardExtensionPlugin` — Dashboard pages, API routes, and display customizations
- `AgentDefinitionProvider` — Workspace-based agent discovery and definition loading

## Coordinated Plugins

| Plugin | Type | Required |
|--------|------|----------|
| bmad-method | BMAD AI agent framework (10 agents) | Yes |
| provider-claude-code | Claude Code CLI integration | Yes |
| plugin-git-ops | Git operations (commit, branch, merge) | Optional |
| plugin-git-worktree | Git worktree isolation | Optional |
| plugin-git-pr | Git PR creation (shell script) | Optional |
| plugin-memory | Knowledge graph (GitNexus/Greptile) | Optional |

## Workflow Categories

| Category | Count | Workflows | Description |
|----------|-------|-----------|-------------|
| Coding | 7 | quick-dev, feature-dev, story-dev, bug-fix, refactor, review, parallel-review | Standard development workflows |
| Bootstrap | 3 | plugin, rebuild, cycle | Self-evolution workflows |
| Utility | 1 | memory-index | Knowledge graph re-indexing |

## Dashboard Pages

| Page | Layout | Description |
|------|--------|-------------|
| Overview | detail | Pack health, workflows, plugins at a glance |
| Task Board | board (Kanban) | Task assignments by status with swimlanes |
| Epics | table | All epics with stories and progress tracking |
| Workflows | table | Browse and manage all workflows |
| AI Agents | table | BMAD agent roster and roles |
| Pack Status | detail | Validation results, plugin health |
| Execute | form | Trigger workflow execution |
| Logs | stream | Real-time SSE execution events |
| Workflow Detail | detail | Step pipeline, execution history |
| Assignment Detail | detail | Task checklist, comments |
| Epic Detail | detail | Stories breakdown and progress |

## AI Team (BMAD Agents)

| ID | Name | Role |
|----|------|------|
| bmad/architect | Winston | System Architect |
| bmad/dev | Amelia | Developer |
| bmad/pm | John | Product Manager |
| bmad/qa | Quinn | QA Engineer |
| bmad/sm | Bob | Scrum Master |
| bmad/quick-flow-solo-dev | Barry | Quick Development Expert |
| bmad/analyst | Mary | Business Analyst |
| bmad/ux-designer | Sally | UX Designer |
| bmad/tech-writer | Paige | Technical Writer |
