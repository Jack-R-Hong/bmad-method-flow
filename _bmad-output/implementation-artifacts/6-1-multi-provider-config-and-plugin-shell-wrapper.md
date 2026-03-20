# Story 6.1: Multi-Provider Config & Plugin Shell Wrapper

Status: done

## Story

As a workflow designer,
I want plugin-memory to read `memory.provider` from `config/config.yaml` and dispatch commands to the configured backend,
So that teams can switch between GitNexus, Greptile, or disable memory entirely without changing workflows.

## Acceptance Criteria

1. `config/config.yaml` has a `memory` section with `provider` (gitnexus|greptile|none), `auto_reindex` (bool), and provider-specific subsections
2. `plugin-memory` shell script reads config via `yq` or `python3` YAML parser, falls back to defaults if neither available
3. Provider `gitnexus`: dispatches to `npx -y <package>` with configurable package spec (`memory.gitnexus.package`)
4. Provider `greptile`: dispatches to Greptile REST API v2 with API key from env var and remote from config
5. Provider `none`: all commands return `{"status":"skipped"}` with exit 0
6. Unknown provider: returns `{"status":"error"}` with exit 1
7. `plugin-memory health` returns provider-specific health JSON
8. `plugin-memory info` returns metadata JSON including `provider` and `auto_reindex`
9. `PULSE_CONFIG` env var overrides default config path
10. Config defaults: `provider: gitnexus`, `auto_reindex: true`

## Tasks / Subtasks

- [x] Task 1: Add `memory` section to `config/config.yaml` (AC: 1, 10)
  - [x] Add `provider`, `auto_reindex`, `gitnexus` subsection, `greptile` subsection
  - [x] Document available providers and their settings as YAML comments
- [x] Task 2: Implement config reader in shell (AC: 2, 9)
  - [x] `read_yaml_value()` function with `yq` primary, `python3` fallback
  - [x] Support `PULSE_CONFIG` env var override
  - [x] Default values when config key is missing
- [x] Task 3: Implement GitNexus provider dispatch (AC: 3)
  - [x] `gitnexus_exec()` function handling: index, reindex, impact, context, query, detect-changes, rename, mcp, health
  - [x] Configurable package spec via `memory.gitnexus.package`
- [x] Task 4: Implement Greptile provider dispatch (AC: 4)
  - [x] `greptile_exec()` function handling: index, query, context, impact, detect-changes, health
  - [x] API key from env var (`memory.greptile.api_key_env`), remote from config
  - [x] Unsupported commands (rename, mcp) return clear error
- [x] Task 5: Implement `none` provider (AC: 5, 6)
  - [x] All commands return skipped JSON with exit 0
  - [x] Unknown provider returns error JSON with exit 1
- [x] Task 6: Implement `info` and `help` commands (AC: 7, 8)
  - [x] `info` returns JSON with provider, auto_reindex, commands list
  - [x] `help` shows active provider and config reference

## Dev Notes

- Shell script lives at `config/plugins/plugin-memory` (executable, no extension)
- Config parsing uses `yq` (preferred) or `python3` with PyYAML as fallback — no hard dependency on either
- All provider functions use `exec` to replace the shell process on the final command (no unnecessary subshells)
- Greptile API: `POST /v2/repositories` for index, `POST /v2/query` for all query-type operations
- GitNexus commands map 1:1 to `npx gitnexus <command>` CLI

### File List

- `config/config.yaml` — MODIFIED: added `memory` section with provider config
- `config/plugins/plugin-memory` — REWRITTEN: multi-provider dispatcher with config reader
- `plugin-packs/coding.toml` — MODIFIED: added `memory.provider` and `memory.auto_reindex` defaults

### References

- [Source: GitNexus README] — CLI commands, MCP tools, analysis pipeline
- [Source: Greptile API docs] — POST /v2/repositories, POST /v2/query endpoints
- [Source: config/config.yaml] — Pulse framework configuration structure
