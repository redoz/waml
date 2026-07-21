#!/usr/bin/env pwsh
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./scripts/run-native.ps1 [path-to-fixture]
#        ./scripts/run-native.ps1 -Empty   # no bundle -> start screen
param(
    [Parameter(Position = 0)]
    [string]$Fixture,
    [switch]$Empty
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# A still-running instance holds the target exe lock, so cargo would relink
# against a stale binary (or fail) and the new window would show old code.
# Kill any stragglers, then build explicitly so compile errors surface here
# instead of as a window that never opens.
Get-Process waml-editor -ErrorAction SilentlyContinue | Stop-Process -Force
cargo build -p waml-editor --bin waml-editor
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Empty) {
    cargo run -p waml-editor --bin waml-editor
}
else {
    if (-not $Fixture) { $Fixture = 'crates/waml-editor/tests/fixtures/mini' }
    cargo run -p waml-editor --bin waml-editor -- $Fixture
}
