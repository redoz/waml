param(
    [string]$Root = (Join-Path $PSScriptRoot '..')
)

$ErrorActionPreference = 'Stop'
$expected = @(
    'editor-workflows.png',
    'browser-and-publishing-workflows.png',
    'tooling-workflows.png'
)
$directory = Join-Path $Root 'crates/waml-editor/tests/screenshots/use-case'

foreach ($name in $expected) {
    $path = Join-Path $directory $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "missing use-case screenshot: $path"
    }
    $bytes = [IO.File]::ReadAllBytes($path)
    if ($bytes.Length -lt 24 -or $bytes[0] -ne 0x89 -or $bytes[1] -ne 0x50 -or $bytes[2] -ne 0x4e -or $bytes[3] -ne 0x47) {
        throw "not a PNG: $path"
    }
    $width = ([int]$bytes[16] -shl 24) -bor ([int]$bytes[17] -shl 16) -bor ([int]$bytes[18] -shl 8) -bor [int]$bytes[19]
    $height = ([int]$bytes[20] -shl 24) -bor ([int]$bytes[21] -shl 16) -bor ([int]$bytes[22] -shl 8) -bor [int]$bytes[23]
    if ($width -ne 1280 -or $height -ne 840) {
        throw "unexpected screenshot dimensions for ${name}: ${width}x${height}; expected 1280x840"
    }
    if ($bytes.Length -lt 10000) {
        throw "screenshot is unexpectedly small and may be blank: $path"
    }
    Write-Host "ok $name ${width}x${height}"
}
