# plugin-coding-pack — Project Overview

## Executive Summary

`plugin-coding-pack` is a Rust-based meta-plugin for the [Pulse](https://github.com/pulsate-labs/pulse) platform that orchestrates AI-driven software development workflows. It coordinates 5 native plugins (bmad-method, provider-claude-code, git-ops, git-worktree, plugin-memory) through 9 workflow pipelines, enabling a complete development lifecycle from feature design to code review — all powered by BMAD methodology AI agents.

The defining capability is **self-bootstrapping**: the plugins develop themselves through the same workflows they power.

## Project Identity

| Property | Value |
|----------|-------|
| **Package Name** | `plugin-coding-pack` |
| **Version** | 0.1.0 |
| **License** | MIT OR Apache-2.0 |
| **Author** | Jack |
| **Rust Edition** | 2021 (MSRV 1.85) |
| **Repository Type** | Monolith |
| **Project Type** | CLI / Plugin |

## Technology Stack

| Category | Technology | Version | Purpose |
|----------|-----------|---------|---------|
| Language | Rust | 2021 edition, MSRV 1.85 | Core implementation |
| SDK | pulse-plugin-sdk | local path | Plugin trait implementations |
| Serialization | serde + serde_json + serde_yaml | 1.0 / 1.0 / 0.9 | JSON/YAML config and I/O |
| Logging | tracing | 0.1 | Structured logging |
| WASM Bindings | wit-bindgen | 0.53 | WASM component model (wasm32 target) |
| Database | SQLite | — | Runtime state (pulse.db) |
| Test Harness | pulse-plugin-test | local path | WASM harness for integration tests |
| Test Runtime | tokio | 1.40 | Async test runtime |

## Architecture Type

**Plugin Architecture with WASM Component Model**

The plugin implements three Pulse SDK traits:
- `PluginLifecycle` — Health checks and dependency declarations
- `StepExecutorPlugin` — Action dispatch (validate-pack, list-workflows, list-plugins, status)
- `DashboardExtensionPlugin` — Dashboard pages, API routes, and display customizations

## Coordinated Plugins

| Plugin | Type | Required |
|--------|------|----------|
| bmad-method | BMAD AI agent framework (10 agents) | Yes |
| provider-claude-code | Claude Code CLI integration | Yes |
| plugin-git-worktree | Git worktree isolation | Optional |
| plugin-git-ops | Git operations | Optional |
| plugin-memory | Knowledge graph (GitNexus/Greptile) | Optional |

## Workflow Categories

| Category | Workflows | Description |
|----------|-----------|-------------|
| Coding | 6 (quick-dev, feature-dev, story-dev, bug-fix, refactor, review) | Standard development workflows |
| Bootstrap | 3 (plugin, rebuild, cycle) | Self-evolution workflows for developing the plugins themselves |

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
