#!/usr/bin/env bash
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./scripts/run-native.sh [-o|--optimized] [path-to-fixture]
#        -o / --optimized   release build (optimized)
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

profile_args=()
fixture=""
for arg in "$@"; do
    case "$arg" in
        -o | --optimized) profile_args+=(--release) ;;
        *) fixture="$arg" ;;
    esac
done
fixture="${fixture:-crates/waml-editor/tests/fixtures/mini}"

cd "$root"
cargo run -p waml-editor --bin waml-editor "${profile_args[@]}" -- "$fixture"
