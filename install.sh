#!/usr/bin/env bash
# install.sh — Build plugin-coding-pack + install dashboard extensions
#
# Usage:
#   ./install.sh              # Full install: build + plugins + dashboard
#   ./install.sh --skip-build # Skip cargo build, only sync binaries + dashboard
#   ./install.sh --dashboard  # Dashboard sync only
#
set -euo pipefail

# ── Paths ────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLUGIN_DIR="$SCRIPT_DIR"
SIBLINGS_DIR="$(dirname "$PLUGIN_DIR")"
PULSE_DIR="$(cd "$PLUGIN_DIR/../../pulse" && pwd 2>/dev/null || true)"
DASHBOARD_DIR="$(cd "$PLUGIN_DIR/../../pulse-dashboard" 2>/dev/null && pwd || true)"

DEST="$PLUGIN_DIR/config/plugins"

# Sibling plugins to build
SIBLINGS=(provider-claude-code git-ops git-worktree bmad-method)

# Dashboard file mapping: source (relative to dashboard/) → destination (relative to DASHBOARD_DIR/src/)
declare -A DASHBOARD_FILES=(
  ["stores/codingPack.svelte.ts"]="lib/stores/codingPack.svelte.ts"
  ["components/CodingPackStatus.svelte"]="lib/components/workflow/CodingPackStatus.svelte"
  ["components/ExecuteWorkflowDialog.svelte"]="lib/components/workflow/ExecuteWorkflowDialog.svelte"
  ["routes/coding-pack/+page.svelte"]="routes/coding-pack/+page.svelte"
)

# ── Helpers ──────────────────────────────────────────────────────────────────

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}>>>${NC} $*"; }
ok()    { echo -e "${GREEN}  OK${NC} $*"; }
warn()  { echo -e "${YELLOW}  !!${NC} $*"; }
fail()  { echo -e "${RED}  FAIL${NC} $*"; }
header(){ echo -e "\n${BOLD}── $* ──${NC}"; }

# ── Parse args ───────────────────────────────────────────────────────────────

SKIP_BUILD=false
DASHBOARD_ONLY=false

for arg in "$@"; do
  case "$arg" in
    --skip-build)    SKIP_BUILD=true ;;
    --dashboard)     DASHBOARD_ONLY=true; SKIP_BUILD=true ;;
    -h|--help)
      echo "Usage: ./install.sh [--skip-build] [--dashboard] [-h|--help]"
      echo ""
      echo "  (no args)      Full install: build all plugins + sync dashboard"
      echo "  --skip-build   Skip cargo build, sync existing binaries + dashboard"
      echo "  --dashboard    Dashboard sync only (no plugin install)"
      echo "  -h, --help     Show this help"
      exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      exit 1
      ;;
  esac
done

# ── Step 1: Build ────────────────────────────────────────────────────────────

if [[ "$SKIP_BUILD" == false ]]; then
  header "Building plugins"

  info "Building plugin-coding-pack..."
  (cd "$PLUGIN_DIR" && cargo build --release 2>&1) && ok "plugin-coding-pack" || { fail "plugin-coding-pack"; exit 1; }

  for name in "${SIBLINGS[@]}"; do
    sibling_path="$SIBLINGS_DIR/$name"
    if [[ -d "$sibling_path" ]]; then
      info "Building $name..."
      (cd "$sibling_path" && cargo build --release 2>&1) && ok "$name" || warn "$name (build failed, skipping)"
    else
      warn "$name — directory not found at $sibling_path, skipping"
    fi
  done
fi

# ── Step 2: Install plugin binaries ─────────────────────────────────────────

if [[ "$DASHBOARD_ONLY" == false ]]; then
  header "Installing plugin binaries"

  mkdir -p "$DEST"

  # Orchestrator binary
  src="$PLUGIN_DIR/target/release/plugin-coding-pack"
  if [[ -f "$src" ]]; then
    cp "$src" "$DEST/"
    ok "plugin-coding-pack → $DEST/"
  else
    warn "plugin-coding-pack binary not found (run without --skip-build)"
  fi

  # Sibling binaries
  declare -A BIN_NAMES=(
    [provider-claude-code]="provider-claude-code"
    [git-ops]="plugin-git-ops"
    [git-worktree]="plugin-git-worktree"
    [bmad-method]="bmad-method"
  )

  for name in "${SIBLINGS[@]}"; do
    bin="${BIN_NAMES[$name]}"
    src="$SIBLINGS_DIR/$name/target/release/$bin"
    if [[ -f "$src" ]]; then
      cp "$src" "$DEST/"
      ok "$bin → $DEST/"
    else
      warn "$bin not found at $src"
    fi
  done

  # plugin-memory (script, not a Rust binary)
  memory_src="$PLUGIN_DIR/config/plugins/plugin-memory"
  if [[ -f "$memory_src" ]]; then
    ok "plugin-memory (already in place)"
  else
    warn "plugin-memory not found"
  fi

  # Install workflows via pulse if available
  if command -v pulse &>/dev/null; then
    info "Registering pack with pulse..."
    (cd "$PLUGIN_DIR" && PULSE_DB_PATH=sqlite:pulse.db?mode=rwc pulse plugin install-pack coding --pack-dir plugin-packs 2>&1) \
      && ok "pulse plugin install-pack coding" \
      || warn "pulse install-pack failed (plugins still copied manually)"
  fi
fi

# ── Step 3: Dashboard extensions ─────────────────────────────────────────────

header "Syncing dashboard extensions"

if [[ -z "$DASHBOARD_DIR" || ! -d "$DASHBOARD_DIR/src" ]]; then
  # Try common locations
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

if [[ -z "$DASHBOARD_DIR" || ! -d "$DASHBOARD_DIR/src" ]]; then
  warn "pulse-dashboard not found. Skipping dashboard sync."
  warn "Set PULSE_DASHBOARD_DIR env var or ensure ~/Document/pulse-dashboard exists."
  echo ""
  echo "Dashboard files are in: $PLUGIN_DIR/dashboard/"
  echo "Copy them manually to your pulse-dashboard/src/ directory."
else
  info "Dashboard directory: $DASHBOARD_DIR"
  SRC_BASE="$PLUGIN_DIR/dashboard"
  DST_BASE="$DASHBOARD_DIR/src"

  synced=0
  for src_rel in "${!DASHBOARD_FILES[@]}"; do
    dst_rel="${DASHBOARD_FILES[$src_rel]}"
    src_file="$SRC_BASE/$src_rel"
    dst_file="$DST_BASE/$dst_rel"

    if [[ ! -f "$src_file" ]]; then
      warn "Source missing: $src_rel"
      continue
    fi

    # Create destination directory if needed
    mkdir -p "$(dirname "$dst_file")"

    # Check if files differ
    if [[ -f "$dst_file" ]] && diff -q "$src_file" "$dst_file" &>/dev/null; then
      ok "$src_rel (up to date)"
    else
      cp "$src_file" "$dst_file"
      ok "$src_rel → $dst_rel"
      synced=$((synced + 1))
    fi
  done

  # Also sync manifest + display customizations for reference
  for meta in manifest.json display-customizations.json; do
    if [[ -f "$SRC_BASE/$meta" ]]; then
      mkdir -p "$DST_BASE/../plugin-assets/plugin-coding-pack"
      cp "$SRC_BASE/$meta" "$DST_BASE/../plugin-assets/plugin-coding-pack/$meta"
      ok "$meta → plugin-assets/"
    fi
  done

  if [[ $synced -gt 0 ]]; then
    info "$synced dashboard file(s) updated"
  else
    info "All dashboard files already up to date"
  fi
fi

# ── Step 4: Validate ─────────────────────────────────────────────────────────

header "Validation"

missing=0
for bin in plugin-coding-pack bmad-method provider-claude-code plugin-git-ops plugin-git-worktree; do
  if [[ -f "$DEST/$bin" ]]; then
    ok "$bin"
  else
    warn "$bin missing"
    ((missing++))
  fi
done

if command -v pulse &>/dev/null; then
  info "Running pulse registry validate..."
  (cd "$PLUGIN_DIR" && PULSE_DB_PATH=sqlite:pulse.db?mode=rwc pulse registry validate --config ./config 2>&1) \
    && ok "registry validate passed" \
    || warn "registry validate had issues"
fi

echo ""
if [[ $missing -eq 0 ]]; then
  echo -e "${GREEN}${BOLD}Installation complete.${NC}"
else
  echo -e "${YELLOW}${BOLD}Installation complete with $missing missing plugin(s).${NC}"
fi

echo ""
echo "Quick start:"
echo "  export PULSE_DB_PATH=sqlite:pulse.db?mode=rwc"
echo "  pulse run coding-quick-dev --config ./config -i '{\"input\": \"your task\"}'"
if [[ -n "$DASHBOARD_DIR" && -d "$DASHBOARD_DIR/src" ]]; then
  echo ""
  echo "Dashboard:"
  echo "  cd $DASHBOARD_DIR && npm run dev"
fi
