#!/usr/bin/env bash
# One command: clean clone -> build -> full test matrix.
#
# Proves the repo works from nothing but git, cargo, node, and python --
# not "works on my machine". By default this clones a fresh copy into a
# temp directory and builds/tests THAT, so nothing in your existing working
# tree (uncommitted changes, stale build artifacts, local-only fixes) can
# make the result look better than what a judge cloning the repo actually
# gets.
#
# Usage:
#   ./scripts/verify.sh                 # clean clone of the current branch's HEAD commit
#   ./scripts/verify.sh --ref <branch>  # clean clone of a specific branch/tag/commit
#   ./scripts/verify.sh --in-place      # skip cloning, test the current working tree instead
#                                       # (faster, but not a from-nothing proof)
#   ./scripts/verify.sh --skip-python   # skip the channel-simulator Python matrix
#                                       # (skips the pip install step; useful with no network)
#
# Exit code 0 iff every step passed. On failure, the temp clone (if any) is
# left in place and its path is printed, so you can inspect what broke.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
REPO_ROOT="$(pwd)"
REPO_URL="$(git remote get-url origin 2>/dev/null || echo "$REPO_ROOT")"
REF="$(git rev-parse HEAD)"
IN_PLACE=0
SKIP_PYTHON=0

while [ $# -gt 0 ]; do
  case "$1" in
    --ref) REF="$2"; shift 2 ;;
    --in-place) IN_PLACE=1; shift ;;
    --skip-python) SKIP_PYTHON=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

STEPS_PASSED=()
STEPS_FAILED=()
run_step() {
  local name="$1"; shift
  echo ""
  echo "=== $name ==="
  echo "+ $*"
  if "$@"; then
    STEPS_PASSED+=("$name")
  else
    STEPS_FAILED+=("$name")
    echo ">>> FAILED: $name" >&2
  fi
}

if [ "$IN_PLACE" -eq 1 ]; then
  WORKDIR="$REPO_ROOT"
  echo "Testing in place: $WORKDIR (not a clean-clone proof)"
else
  WORKDIR="$(mktemp -d)"
  echo "Clean clone of $REPO_URL @ $REF into: $WORKDIR"
  if ! git clone --quiet "$REPO_URL" "$WORKDIR"; then
    echo "clone failed" >&2
    exit 1
  fi
  if ! git -C "$WORKDIR" checkout --quiet "$REF"; then
    echo "checkout of ref '$REF' failed" >&2
    exit 1
  fi
fi

cd "$WORKDIR"

run_step "Rust: release build (stegstr-cli)" \
  cargo build --release --bin stegstr-cli --manifest-path src-tauri/Cargo.toml

run_step "Rust: cargo test --release" \
  cargo test --release --manifest-path src-tauri/Cargo.toml

run_step "Rust: cargo clippy -- -D warnings" \
  cargo clippy --release --all-targets --manifest-path src-tauri/Cargo.toml -- -D warnings

if command -v npm >/dev/null 2>&1; then
  run_step "Frontend: npm install" npm install --no-audit --no-fund
  run_step "Frontend: npm test" npm test
else
  echo ""
  echo "=== Frontend tests skipped: npm not found ==="
  STEPS_FAILED+=("Frontend: npm not found")
fi

if [ "$SKIP_PYTHON" -eq 0 ]; then
  if command -v python >/dev/null 2>&1; then
    PY=python
  elif command -v python3 >/dev/null 2>&1; then
    PY=python3
  else
    PY=""
  fi

  if [ -n "$PY" ]; then
    pushd channel_simulator >/dev/null
    run_step "Python: pip install -r requirements.txt" \
      "$PY" -m pip install -q -r requirements.txt
    run_step "Python: generate realistic covers" \
      "$PY" gen_realistic_covers.py
    run_step "Python: generate extended covers" \
      "$PY" gen_extended_covers.py
    run_step "Python: run_matrix_rust_cli.py (45/45 expected)" \
      "$PY" run_matrix_rust_cli.py
    run_step "Python: run_matrix_realistic.py (prototype matrix)" \
      "$PY" run_matrix_realistic.py
    popd >/dev/null
  else
    echo ""
    echo "=== Python matrix skipped: no python/python3 found ==="
    STEPS_FAILED+=("Python: interpreter not found")
  fi
else
  echo ""
  echo "=== Python matrix skipped: --skip-python ==="
fi

echo ""
echo "==================== SUMMARY ===================="
for s in "${STEPS_PASSED[@]:-}"; do
  [ -n "$s" ] && echo "PASS  $s"
done
for s in "${STEPS_FAILED[@]:-}"; do
  [ -n "$s" ] && echo "FAIL  $s"
done
echo "${#STEPS_PASSED[@]} passed, ${#STEPS_FAILED[@]} failed"

if [ "$IN_PLACE" -eq 0 ]; then
  if [ "${#STEPS_FAILED[@]}" -eq 0 ]; then
    rm -rf "$WORKDIR"
  else
    echo "Clean clone left at: $WORKDIR (for inspection)"
  fi
fi

[ "${#STEPS_FAILED[@]}" -eq 0 ]
