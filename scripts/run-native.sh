#!/usr/bin/env bash
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./scripts/run-native.sh [path-to-fixture]
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="${1:-crates/waml-editor/tests/fixtures/mini}"
cd "$root"
cargo run -p waml-editor -- "$fixture"
