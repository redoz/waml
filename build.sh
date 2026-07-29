#!/usr/bin/env bash
# Full native Rust and VS Code build from a clean checkout. Run from anywhere.
#
#   ./build.sh            install + build
#   ./build.sh --test     also run Rust and VS Code tests
#   ./build.sh --lint     also run Rust clippy and VS Code lint
set -euo pipefail
cd "$(dirname "$0")"

need() { command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found on PATH ($2)" >&2; exit 1; }; }
need pnpm      "https://pnpm.io/installation"
need cargo     "https://rustup.rs"

run_test=0; run_lint=0
for arg in "$@"; do
  case "$arg" in
    --test) run_test=1 ;;
    --lint) run_lint=1 ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

ext=editors/vscode

echo "==> pnpm install"
pnpm -C "$ext" install --frozen-lockfile

echo "==> Rust build"
cargo build --workspace

echo "==> VS Code build"
pnpm -C "$ext" build

[ "$run_test" = 1 ] && { echo "==> Rust test"; cargo test --workspace; echo "==> VS Code test"; pnpm -C "$ext" test; }
[ "$run_lint" = 1 ] && { echo "==> Rust clippy"; cargo clippy --workspace --all-targets --all-features -- -D warnings; echo "==> VS Code lint"; pnpm -C "$ext" lint; }

echo "==> done"
