# plugin-coding-pack — Project Overview

> Generated: 2026-03-28 | Scan Level: exhaustive | Mode: full_rescan

## Executive Summary

plugin-coding-pack is a Rust meta-plugin for the Pulse ecosystem that orchestrates AI-driven software development workflows. It coordinates five sibling plugins (bmad-method, provider-claude-code, git-worktree, memory, and board) and exposes 9 BMAD methodology agents as first-class entities with persona injection, skill-based routing, and LLM tool integration.

The plugin serves as the central coordination layer between:
- The Pulse platform (task management, workflow execution, dashboard)
- BMAD methodology agents (architect, developer, QA, PM, etc.)
- LLM providers (Claude Code via provider-claude-code)
- Development infrastructure (git worktrees, GitHub issues, PR feedback)

## Project Identity

| Field | Value |
|-------|-------|
| Package Name | plugin-coding-pack |
| Version | 0.1.0 |
| Language | Rust (Edition 2021, MSRV 1.85) |
| License | MIT OR Apache-2.0 |
| Author | Jack |
| Repository Type | Monolith |
| Project Type | Library (cdylib + rlib + binary) |
| Crate Types | cdylib (dynamic plugin), rlib (Rust lib), bin (CLI) |

## Tech Stack Summary

| Category | Technology |
|----------|-----------|
| Core | Rust 2021 + pulse-plugin-sdk |
| Serialization | serde + serde_json + serde_yaml |
| Async | tokio 1.40 |
| HTTP | reqwest 0.12 (blocking) |
| WASM | wit-bindgen 0.53 |
| Logging | tracing 0.1 |
| Storage | SQLite |
| Testing | cargo test + Playwright ^1.52.0 |
| Methodology | BMAD v6.2.0 |

## Key Capabilities

### 1. Plugin Orchestration
Validates, loads, and coordinates 5+ sibling plugins. Health checks verify binary existence and executability.

### 2. BMAD Agent System (9 Agents)
Loads agent personas from CSV manifest. Each agent has: display name, title, role, identity, communication style, principles, and capabilities/skills. Agents are injected into LLM requests via ConfigInjector and discoverable via AgentDefinitionProvider.

### 3. Workflow Engine Integration
12 YAML workflow definitions covering: feature dev, bug fix, refactor, review, story dev, PR fix, memory indexing, and bootstrap operations. Workflows are validated for structure, DAG cycles, and plugin dependencies.

### 4. Auto-Dev Loop
Autonomous development cycle: picks tasks from the board by label, routes to appropriate workflow, executes, validates with tests, and updates the board. Configurable retries, validation gates, and polling intervals.

### 5. Dashboard Extension
7 dashboard pages, 21+ API endpoints, 4 display customizations providing real-time visibility into pack health, workflow execution, agent activity, and sprint progress.

### 6. LLM Tool Integration
6 tools exposed to LLMs: pack validation, workflow listing, plugin listing, data queries, data mutations, and auto-dev task pickup. Sensitivity levels enforced (Low/Medium/High).

### 7. Platform Plugin Delegation
Thin bridge layer delegates to platform plugins for: workflow execution, GitHub issue sync, PR feedback loops, worktree management, and scheduled triggers.

## Architecture Type

Plugin architecture with capability-based registration. Three HookPoint capabilities registered with Pulse's PluginRegistry:
1. **ConfigInjector** (BmadAgentInjector) — Per-agent persona injection into LLM system prompts
2. **ToolProvider** (BmadToolProvider) — Pack operations as LLM-callable tools
3. **AgentDefinitionProvider** (BmadAgentRegistry) — Agent discovery, skill routing, ACL

## Codebase Metrics

| Metric | Value |
|--------|-------|
| Source modules | 11 |
| Source LOC | 5,137 |
| Test LOC | ~6,156 (Rust + TypeScript) |
| Pack actions | 18 |
| JSON-RPC methods | 16 |
| LLM tools | 6 |
| BMAD agents | 9 |
| Dashboard pages | 7 |
| API endpoints | 21+ |
| Workflow definitions | 12 |
| Workflow test fixtures | 15 |

## Links to Detailed Documentation

- [Architecture](./architecture.md) — Component details, data flow, WASM support
- [Source Tree Analysis](./source-tree-analysis.md) — Annotated directory structure
- [Development Guide](./development-guide.md) — Build, test, and run instructions
- [Plugin Coding Pack Reference](./plugin-coding-pack.md) — Additional reference
