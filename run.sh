#!/usr/bin/env bash
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./run.sh [-d|--debug] [path-to-fixture]
#        release build (optimized) by default; -d / --debug opts out
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

release=1
fixture=""
for arg in "$@"; do
    case "$arg" in
        -d | --debug) release=0 ;;
        # -o/--optimized kept for existing callers; now a no-op.
        -o | --optimized) ;;
        *) fixture="$arg" ;;
    esac
done
profile_args=()
[ "$release" -eq 1 ] && profile_args+=(--release)
fixture="${fixture:-crates/waml-editor/tests/fixtures/mini}"

cd "$root"
cargo run -p waml-editor --bin waml-editor "${profile_args[@]}" -- "$fixture"
