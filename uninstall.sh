#!/usr/bin/env bash
# uninstall.sh — Remove plugin-coding-pack binaries, workflows, and dashboard extensions
#
# Usage:
#   ./uninstall.sh              # Full uninstall (interactive confirmation)
#   ./uninstall.sh --dashboard  # Remove dashboard extensions only
#   ./uninstall.sh --plugins    # Remove plugin binaries only
#   ./uninstall.sh --yes        # Skip confirmation prompt
#
set -euo pipefail

# ── Paths ────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="$SCRIPT_DIR"
DASHBOARD_DIR="$(cd "$PLUGIN_DIR/../../pulse-dashboard" 2>/dev/null && pwd || true)"

DEST="$PLUGIN_DIR/config/plugins"
WORKFLOWS_DIR="$PLUGIN_DIR/config/workflows"

# Plugin binaries installed by this pack
PLUGIN_BINS=(
  plugin-coding-pack
  bmad-method
  provider-claude-code
  plugin-git-ops
  plugin-git-worktree
  plugin-memory
)

# Workflow files owned by this pack
WORKFLOW_FILES=(
  coding-quick-dev.yaml
  coding-feature-dev.yaml
  coding-story-dev.yaml
  coding-bug-fix.yaml
  coding-refactor.yaml
  coding-review.yaml
  coding-memory-index.yaml
  bootstrap-plugin.yaml
  bootstrap-rebuild.yaml
  bootstrap-cycle.yaml
)

# Dashboard files synced by install.sh
declare -A DASHBOARD_FILES=(
  ["stores/codingPack.svelte.ts"]="lib/stores/codingPack.svelte.ts"
  ["components/CodingPackStatus.svelte"]="lib/components/workflow/CodingPackStatus.svelte"
  ["components/ExecuteWorkflowDialog.svelte"]="lib/components/workflow/ExecuteWorkflowDialog.svelte"
  ["routes/coding-pack/+page.svelte"]="routes/coding-pack/+page.svelte"
)

# Dashboard directories to remove entirely (plugin-specific routes)
DASHBOARD_DIRS_TO_REMOVE=(
  "routes/plugins/plugin-coding-pack"
)

# Dashboard files that import from codingPack store and need patching
# Format: "file_path|old_import_line|replacement"
DASHBOARD_PATCH_IMPORTS=(
  "lib/components/insights/LLMChatView.svelte|import { WORKFLOWS, AGENTS } from '\$lib/stores/codingPack.svelte';|const WORKFLOWS: any[] = []; const AGENTS: any[] = [];"
  "lib/components/TaskDetail.svelte|import { WORKFLOWS } from '\$lib/stores/codingPack.svelte';|const WORKFLOWS: any[] = [];"
)

# Sidebar entry to remove
SIDEBAR_FILE="lib/components/navigation/Sidebar.svelte"
SIDEBAR_CODING_PACK_LINE="plugin-coding-pack"

# ── Helpers ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}>>>${NC} $*"; }
ok()    { echo -e "${GREEN}  RM${NC} $*"; }
skip()  { echo -e "${YELLOW}  --${NC} $*"; }
header(){ echo -e "\n${BOLD}── $* ──${NC}"; }

# ── Parse args ───────────────────────────────────────────────────────────────

DO_PLUGINS=true
DO_DASHBOARD=true
DO_WORKFLOWS=true
AUTO_YES=false

for arg in "$@"; do
  case "$arg" in
    --dashboard)  DO_PLUGINS=false; DO_WORKFLOWS=false ;;
    --plugins)    DO_DASHBOARD=false; DO_WORKFLOWS=false ;;
    --yes|-y)     AUTO_YES=true ;;
    -h|--help)
      echo "Usage: ./uninstall.sh [--dashboard] [--plugins] [--yes] [-h|--help]"
      echo ""
      echo "  (no args)      Full uninstall: plugins + workflows + dashboard"
      echo "  --dashboard    Remove dashboard extensions only"
      echo "  --plugins      Remove plugin binaries only"
      echo "  --yes, -y      Skip confirmation prompt"
      echo "  -h, --help     Show this help"
      exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      exit 1
      ;;
  esac
done

# ── Preview what will be removed ─────────────────────────────────────────────

echo -e "${BOLD}plugin-coding-pack uninstaller${NC}"
echo ""
echo "This will remove:"

if [[ "$DO_PLUGINS" == true ]]; then
  count=0
  for bin in "${PLUGIN_BINS[@]}"; do
    [[ -f "$DEST/$bin" ]] && count=$((count + 1))
  done
  echo "  - $count plugin binary(ies) from config/plugins/"
fi

if [[ "$DO_WORKFLOWS" == true ]]; then
  count=0
  for wf in "${WORKFLOW_FILES[@]}"; do
    [[ -f "$WORKFLOWS_DIR/$wf" ]] && count=$((count + 1))
  done
  echo "  - $count workflow file(s) from config/workflows/"
fi

if [[ "$DO_DASHBOARD" == true ]]; then
  # Resolve dashboard dir
  if [[ -z "$DASHBOARD_DIR" || ! -d "$DASHBOARD_DIR/src" ]]; then
    for candidate in \
      "$PLUGIN_DIR/../../pulse-dashboard" \
      "$HOME/Document/pulse-dashboard" \
      "$HOME/pulse-dashboard"; do
      if [[ -d "$candidate/src" ]]; then
        DASHBOARD_DIR="$(cd "$candidate" && pwd)"
        break
      fi
    done
  fi

  if [[ -n "$DASHBOARD_DIR" && -d "$DASHBOARD_DIR/src" ]]; then
    count=0
    for dst_rel in "${DASHBOARD_FILES[@]}"; do
      [[ -f "$DASHBOARD_DIR/src/$dst_rel" ]] && count=$((count + 1))
    done
    echo "  - $count dashboard file(s) from pulse-dashboard/src/"
    echo "  - plugin-assets/plugin-coding-pack/ directory"
  else
    echo "  - (dashboard not found, skipping)"
  fi
fi

echo ""

# ── Confirm ──────────────────────────────────────────────────────────────────

if [[ "$AUTO_YES" == false ]]; then
  read -rp "Proceed? [y/N] " answer
  case "$answer" in
    [yY]|[yY][eE][sS]) ;;
    *)
      echo "Aborted."
      exit 0
      ;;
  esac
fi

# ── Step 1: Remove plugin binaries ───────────────────────────────────────────

if [[ "$DO_PLUGINS" == true ]]; then
  header "Removing plugin binaries"

  for bin in "${PLUGIN_BINS[@]}"; do
    target="$DEST/$bin"
    if [[ -f "$target" ]]; then
      rm "$target"
      ok "$target"
    else
      skip "$bin (not found)"
    fi
  done
fi

# ── Step 2: Remove workflow files ────────────────────────────────────────────

if [[ "$DO_WORKFLOWS" == true ]]; then
  header "Removing workflow files"

  for wf in "${WORKFLOW_FILES[@]}"; do
    target="$WORKFLOWS_DIR/$wf"
    if [[ -f "$target" ]]; then
      rm "$target"
      ok "$target"
    else
      skip "$wf (not found)"
    fi
  done

  # Remove workflows dir if empty
  if [[ -d "$WORKFLOWS_DIR" ]] && [[ -z "$(ls -A "$WORKFLOWS_DIR" 2>/dev/null)" ]]; then
    rmdir "$WORKFLOWS_DIR"
    ok "$WORKFLOWS_DIR/ (empty, removed)"
  fi
fi

# ── Step 3: Remove dashboard extensions ──────────────────────────────────────

if [[ "$DO_DASHBOARD" == true ]]; then
  header "Removing dashboard extensions"

  if [[ -z "$DASHBOARD_DIR" || ! -d "$DASHBOARD_DIR/src" ]]; then
    skip "pulse-dashboard not found, nothing to remove"
  else
    DST_BASE="$DASHBOARD_DIR/src"

    for dst_rel in "${DASHBOARD_FILES[@]}"; do
      target="$DST_BASE/$dst_rel"
      if [[ -f "$target" ]]; then
        rm "$target"
        ok "$target"
      else
        skip "$dst_rel (not found)"
      fi
    done

    # Remove empty routes/coding-pack/ directory
    coding_route="$DST_BASE/routes/coding-pack"
    if [[ -d "$coding_route" ]] && [[ -z "$(ls -A "$coding_route" 2>/dev/null)" ]]; then
      rmdir "$coding_route"
      ok "$coding_route/ (empty, removed)"
    fi

    # Remove plugin-assets
    assets_dir="$DASHBOARD_DIR/plugin-assets/plugin-coding-pack"
    if [[ -d "$assets_dir" ]]; then
      rm -rf "$assets_dir"
      ok "$assets_dir/"
    fi

    # Remove plugin-assets/ parent if empty
    assets_parent="$DASHBOARD_DIR/plugin-assets"
    if [[ -d "$assets_parent" ]] && [[ -z "$(ls -A "$assets_parent" 2>/dev/null)" ]]; then
      rmdir "$assets_parent"
      ok "$assets_parent/ (empty, removed)"
    fi

    # ── Step 3b: Remove plugin-specific route directories ─────────────────
    header "Removing plugin route directories"
    for dir_rel in "${DASHBOARD_DIRS_TO_REMOVE[@]}"; do
      target="$DST_BASE/$dir_rel"
      if [[ -d "$target" ]]; then
        rm -rf "$target"
        ok "$target/"
      else
        skip "$dir_rel/ (not found)"
      fi
    done

    # ── Step 3c: Patch dashboard files that import from codingPack store ──
    header "Patching dashboard imports"
    for entry in "${DASHBOARD_PATCH_IMPORTS[@]}"; do
      IFS='|' read -r file_rel old_pattern replacement <<< "$entry"
      target="$DST_BASE/$file_rel"
      if [[ -f "$target" ]] && grep -qF "$old_pattern" "$target"; then
        sed -i "s|$(printf '%s' "$old_pattern" | sed 's/[&/\]/\\&/g')|$(printf '%s' "$replacement" | sed 's/[&/\]/\\&/g')|g" "$target"
        ok "patched $file_rel"
      else
        skip "$file_rel (not found or already patched)"
      fi
    done

    # ── Step 3d: Remove Coding Pack sidebar entry ─────────────────────────
    header "Cleaning sidebar"
    sidebar_target="$DST_BASE/$SIDEBAR_FILE"
    if [[ -f "$sidebar_target" ]] && grep -q "$SIDEBAR_CODING_PACK_LINE" "$sidebar_target"; then
      sed -i "/$SIDEBAR_CODING_PACK_LINE/d" "$sidebar_target"
      # Also remove the now-unused Package import if present
      sed -i '/^[[:space:]]*Package,$/d' "$sidebar_target"
      ok "removed Coding Pack from sidebar"
    else
      skip "sidebar (not found or already clean)"
    fi
  fi
fi

# ── Step 4: Clean database (optional) ────────────────────────────────────────

header "Database"

DB_FILE="$PLUGIN_DIR/pulse.db"
if [[ -f "$DB_FILE" ]]; then
  echo -e "  SQLite database remains at: ${CYAN}$DB_FILE${NC}"
  echo "  To remove execution history:  rm pulse.db pulse.db-shm pulse.db-wal"
else
  skip "No database file found"
fi

# ── Step 5: Clean build artifacts (optional) ─────────────────────────────────

header "Build artifacts"

if [[ -d "$PLUGIN_DIR/target" ]]; then
  echo -e "  Build cache remains at: ${CYAN}$PLUGIN_DIR/target/${NC}"
  echo "  To free disk space:  cargo clean"
else
  skip "No build artifacts found"
fi

# ── Done ─────────────────────────────────────────────────────────────────────

echo ""
echo -e "${GREEN}${BOLD}Uninstall complete.${NC}"
echo ""
echo "To reinstall:  ./install.sh"
