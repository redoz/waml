#!/usr/bin/env bash
# Full build from a clean checkout: install deps, compile the Rust core to
# inlined wasm, then build every workspace package. Run from anywhere.
#
#   ./build.sh            install + wasm + build
#   ./build.sh --test     also run `pnpm -r test` at the end
#   ./build.sh --lint     also run `pnpm lint`
set -euo pipefail
cd "$(dirname "$0")"

need() { command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found on PATH ($2)" >&2; exit 1; }; }
need pnpm      "https://pnpm.io/installation"
need wasm-pack "cargo install wasm-pack"

run_test=0; run_lint=0
for arg in "$@"; do
  case "$arg" in
    --test) run_test=1 ;;
    --lint) run_lint=1 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

echo "==> pnpm install"
pnpm install --frozen-lockfile

echo "==> build wasm"
pnpm build:wasm

echo "==> build packages"
pnpm build

[ "$run_lint" = 1 ] && { echo "==> lint"; pnpm lint; }
[ "$run_test" = 1 ] && { echo "==> test"; pnpm -r test; }

echo "==> done"
