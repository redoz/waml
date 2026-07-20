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
if ($Empty) {
    cargo run -p waml-editor --bin waml-editor
}
else {
    if (-not $Fixture) { $Fixture = 'crates/waml-editor/tests/fixtures/mini' }
    cargo run -p waml-editor --bin waml-editor -- $Fixture
}
