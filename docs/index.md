# plugin-coding-pack — Documentation Index

> Generated: 2026-03-28 | Scan Level: exhaustive | Mode: full_rescan

## Project Overview

- **Type:** Monolith — Rust Plugin Library (cdylib/rlib + binary)
- **Primary Language:** Rust (Edition 2021, MSRV 1.85)
- **Architecture:** Plugin architecture with capability-based registration
- **Package:** plugin-coding-pack v0.1.0

## Quick Reference

- **Tech Stack:** Rust + pulse-plugin-sdk + serde + tokio + reqwest + wit-bindgen + tracing + SQLite
- **Entry Points:** `src/lib.rs` (library), `src/main.rs` (binary)
- **Architecture Pattern:** Meta-plugin orchestrator with 3 HookPoint capabilities
- **Source Modules:** 11 files (5,137 LOC)
- **Test Coverage:** ~6,156 LOC across Rust unit/integration/e2e + Playwright
- **BMAD Agents:** 9 (analyst, architect, dev, pm, qa, quick-flow-solo-dev, sm, tech-writer, ux-designer)

## Generated Documentation

- [Project Overview](./project-overview.md) — Executive summary, identity, capabilities, metrics
- [Architecture](./architecture.md) — Component details, data flow, JSON-RPC dispatch, WASM support
- [Source Tree Analysis](./source-tree-analysis.md) — Annotated directory structure, entry points, dependency graph
- [Development Guide](./development-guide.md) — Prerequisites, build, test, run, configuration, dev patterns

## Existing Documentation

- [README](../README.md) — Project introduction and quick start (Chinese/English)
- [PRD](../_bmad-output/planning-artifacts/prd.md) — Product requirements document
- [Architecture (Planning)](../_bmad-output/planning-artifacts/architecture.md) — Original architecture design
- [Epics](../_bmad-output/planning-artifacts/epics.md) — Epic and story breakdown
- [Auto-Dev v2 PRD](../_bmad-output/planning-artifacts/prd-auto-dev-v2.md) — Auto-dev loop requirements
- [Config Injection PRD](../_bmad-output/planning-artifacts/prd-config-injection.md) — Config injection requirements

## Getting Started

1. **Understand the project** — Start with [Project Overview](./project-overview.md) for the big picture
2. **Explore the architecture** — Read [Architecture](./architecture.md) for component details and data flow
3. **Navigate the code** — Use [Source Tree Analysis](./source-tree-analysis.md) to find specific files
4. **Set up development** — Follow [Development Guide](./development-guide.md) for build and test instructions
5. **Plan new features** — Reference the architecture doc + planning artifacts for brownfield PRD work

### Quick Commands

```bash
# Build
cargo build

# Run all tests
cargo test

# Install all plugins
./install.sh

# Run dashboard E2E tests
cd dashboard && npx playwright test
```
