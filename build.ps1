#!/usr/bin/env pwsh
# Full build from a clean checkout: install deps, compile the Rust core to
# inlined wasm, then build every workspace package. Run from anywhere.
#
#   ./build.ps1            install + wasm + build
#   ./build.ps1 -Test      also run `pnpm -r test` at the end
#   ./build.ps1 -Lint      also run `pnpm lint`
[CmdletBinding()]
param(
    [switch]$Test,
    [switch]$Lint
)
$ErrorActionPreference = "Stop"
Set-Location -Path $PSScriptRoot

function Need($cmd, $hint) {
    if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {
        Write-Error "'$cmd' not found on PATH ($hint)"
    }
}
Need pnpm      "https://pnpm.io/installation"
Need wasm-pack "cargo install wasm-pack"

function Run($label, [scriptblock]$block) {
    Write-Host "==> $label"
    & $block
    if ($LASTEXITCODE -ne 0) { Write-Error "$label failed (exit $LASTEXITCODE)" }
}

Run "pnpm install"    { pnpm install --frozen-lockfile }
Run "build wasm"      { pnpm build:wasm }
Run "build packages"  { pnpm build }
if ($Lint) { Run "lint" { pnpm lint } }
if ($Test) { Run "test" { pnpm -r test } }

Write-Host "==> done"
