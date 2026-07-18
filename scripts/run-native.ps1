#!/usr/bin/env pwsh
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./scripts/run-native.ps1 [path-to-fixture]
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$fixture = if ($args.Count -ge 1) { $args[0] } else { 'crates/waml-editor/tests/fixtures/mini' }
Set-Location $root
cargo run -p waml-editor -- $fixture
