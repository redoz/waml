#!/usr/bin/env pwsh
# Full native Rust and VS Code build from a clean checkout. Run from anywhere.
#
#   ./build.ps1            install + build
#   ./build.ps1 -Test      also run Rust and VS Code tests
#   ./build.ps1 -Lint      also run Rust clippy and VS Code lint
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
Need cargo     "https://rustup.rs"

function Run($label, [scriptblock]$block) {
    Write-Host "==> $label"
    & $block
    if ($LASTEXITCODE -ne 0) { Write-Error "$label failed (exit $LASTEXITCODE)" }
}

Run "pnpm install"   { pnpm install --frozen-lockfile }
Run "Rust build"     { cargo build --workspace }
Run "VS Code build"  { pnpm build }
if ($Test) {
    Run "Rust test" { cargo test --workspace }
    Run "VS Code test" { pnpm test }
}
if ($Lint) {
    Run "Rust clippy" { cargo clippy --workspace --all-targets --all-features -- -D warnings }
    Run "VS Code lint" { pnpm lint }
}

Write-Host "==> done"
