# plugin-coding-pack — Documentation Index

> Generated: 2026-03-21 | Scan: quick | Mode: initial_scan

## Project Overview

- **Type:** Monolith — Rust plugin library (cdylib + rlib + binary)
- **Primary Language:** Rust (Edition 2021, MSRV 1.85)
- **Architecture:** Plugin-based orchestrator with WASM Component Model
- **Platform:** [Pulse](https://github.com/pulsate-labs/pulse) plugin ecosystem

## Quick Reference

- **Tech Stack:** Rust + pulse-plugin-sdk + serde + wit-bindgen (WASM) + tracing + SQLite
- **Entry Points:** `src/lib.rs` (library), `src/main.rs` (binary)
- **Architecture Pattern:** Plugin orchestrator — coordinates 5 sibling plugins through 9 workflow pipelines
- **BMAD Agents:** 9 AI team members (Architect, Developer, PM, QA, SM, Quick Dev, Analyst, UX Designer, Tech Writer)

## Generated Documentation

- [Project Overview](./project-overview.md) — Executive summary, tech stack, coordinated plugins, workflow categories, AI team roster
- [Architecture](./architecture.md) — Plugin traits, action dispatch, dashboard extension, workflow system, configuration, dependencies, testing strategy
- [Source Tree Analysis](./source-tree-analysis.md) — Annotated directory structure, critical folders, entry points, key configuration files
- [Development Guide](./development-guide.md) — Prerequisites, environment setup, build commands, workflow execution, plugin management, testing
- [Technical Reference](./plugin-coding-pack.md) — Detailed technical documentation of the plugin internals

## Existing Documentation

- [README.md](../README.md) — Project README with installation, usage examples, and workflow overview (Chinese/English)

## Getting Started

```bash
# 1. Prerequisites: Rust 1.85+, Pulse CLI, Claude Code CLI, Git 2.20+
# 2. Build and install
./install.sh

# 3. Set environment
export PULSE_DB_PATH=sqlite:pulse.db?mode=rwc

# 4. Validate setup
pulse registry validate --config ./config

# 5. Run a workflow
pulse run coding-quick-dev --config ./config \
  -i '{"input": "your task description"}'
```
