<#
.SYNOPSIS
    Set the waml-editor UI theme in ~/.waml/editor.json without touching recents.

.DESCRIPTION
    Reads the existing editor.json (schema v1: { version, recents, theme }),
    sets the "theme" field, and writes it back pretty-printed. Preserves any
    recents already stored. Creates ~/.waml and the file if absent.

    Matches config.rs: ThemeMode serializes lowercase ("light" | "dark"),
    theme field is #[serde(default)] so older files load as light.

.EXAMPLE
    ./config.ps1 -Theme dark
    ./config.ps1 -Theme light
    ./config.ps1            # prints the current theme
#>
param(
    [ValidateSet('light', 'dark')]
    [string]$Theme
)

$ErrorActionPreference = 'Stop'

$dir  = Join-Path $HOME '.waml'
$file = Join-Path $dir 'editor.json'

# Load existing config, or start a fresh v1 shell.
if (Test-Path $file) {
    $config = Get-Content -Raw -Path $file | ConvertFrom-Json
} else {
    $config = [pscustomobject]@{ version = 1; recents = @(); theme = 'light' }
}

# No -Theme: just report the current value and exit.
if (-not $Theme) {
    $current = if ($config.PSObject.Properties.Name -contains 'theme') { $config.theme } else { 'light' }
    Write-Host "current theme: $current"
    return
}

# Ensure the field exists (older files predate it), then set it.
if ($config.PSObject.Properties.Name -notcontains 'theme') {
    $config | Add-Member -NotePropertyName theme -NotePropertyValue $Theme
} else {
    $config.theme = $Theme
}
$config.version = 1

New-Item -ItemType Directory -Force -Path $dir | Out-Null
$config | ConvertTo-Json -Depth 10 | Set-Content -Path $file -Encoding utf8

Write-Host "theme set to '$Theme' in $file"
Write-Host "relaunch waml-editor to apply."
