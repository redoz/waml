#!/usr/bin/env pwsh
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./scripts/run-native.ps1 [path-to-fixture]
#        ./scripts/run-native.ps1 -Empty       # no bundle -> start screen
#        ./scripts/run-native.ps1 -Optimized   # release build (optimized)
param(
    [Parameter(Position = 0)]
    [string]$Fixture,
    [switch]$Empty,
    [switch]$Optimized,
    # Per-agent window marker: badge text and wash colour, so several
    # concurrently-running editors can be told apart by eye.
    [string]$Title,
    [string]$Color
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

# --release swaps in the optimized profile / separate target dir; thread it
# through both the explicit build and the run so they stay in lockstep.
# [string[]] keeps the single-element case an array; a bare @(...) unwraps to a
# scalar that @-splats character-by-character (a stray '-' cargo then rejects).
[string[]]$profileArgs = if ($Optimized) { '--release' } else { @() }

[string[]]$markArgs = @()
if ($Title) { $markArgs += @('--title', $Title) }
if ($Color) { $markArgs += @('--color', $Color) }

# A still-running instance holds the target exe lock, so cargo would relink
# against a stale binary (or fail) and the new window would show old code.
# Kill only OUR stragglers -- the instance built from THIS checkout's exe -- so
# a run in one worktree leaves other worktrees' (and the main checkout's)
# windows alone. Match on the exact exe path this script is about to relink.
$profileDir = if ($Optimized) { 'release' } else { 'debug' }
$exePath = [IO.Path]::GetFullPath((Join-Path $root "target/$profileDir/waml-editor.exe"))
Get-Process waml-editor -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $exePath) } |
    Stop-Process -Force
cargo build -p waml-editor --bin waml-editor @profileArgs
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

if ($Empty) {
    cargo run -p waml-editor --bin waml-editor @profileArgs -- @markArgs
}
else {
    if (-not $Fixture) { $Fixture = 'crates/waml-editor/tests/fixtures/mini' }
    cargo run -p waml-editor --bin waml-editor @profileArgs -- $Fixture @markArgs
}
