param(
    [switch] $Update,
    [double] $MaxChangedPixelRatio = 0.001
)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$editorExe = [IO.Path]::GetFullPath((Join-Path $root 'target/debug/waml-editor.exe'))
$manifest = @(
    @{ Source='docs/waml/use-cases/views/editor-workflows.md'; Title='Editor Workflows'; Baseline='crates/waml-editor/tests/screenshots/use-case/editor-workflows.png'; Slug='use-case-editor-workflows' },
    @{ Source='docs/waml/use-cases/views/browser-and-publishing-workflows.md'; Title='Browser and Publishing Workflows'; Baseline='crates/waml-editor/tests/screenshots/use-case/browser-and-publishing-workflows.png'; Slug='use-case-browser-workflows' },
    @{ Source='docs/waml/use-cases/views/tooling-workflows.md'; Title='Tooling Workflows'; Baseline='crates/waml-editor/tests/screenshots/use-case/tooling-workflows.png'; Slug='use-case-tooling-workflows' }
)

Add-Type -AssemblyName System.Drawing.Common

function Get-PixelBytes([System.Drawing.Bitmap] $bitmap) {
    $rect = [Drawing.Rectangle]::new(0, 0, $bitmap.Width, $bitmap.Height)
    $data = $bitmap.LockBits($rect, [Drawing.Imaging.ImageLockMode]::ReadOnly, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    try {
        $bytes = [byte[]]::new([Math]::Abs($data.Stride) * $bitmap.Height)
        [Runtime.InteropServices.Marshal]::Copy($data.Scan0, $bytes, 0, $bytes.Length)
        return $bytes
    }
    finally { $bitmap.UnlockBits($data) }
}

function Compare-Png([string] $expectedPath, [string] $actualPath) {
    $expected = [Drawing.Bitmap]::new($expectedPath)
    $actual = [Drawing.Bitmap]::new($actualPath)
    try {
        if ($expected.Width -ne $actual.Width -or $expected.Height -ne $actual.Height) {
            throw "dimension mismatch: expected $($expected.Width)x$($expected.Height), got $($actual.Width)x$($actual.Height)"
        }
        $left = Get-PixelBytes $expected
        $right = Get-PixelBytes $actual
        $changed = 0
        for ($i = 0; $i -lt $left.Length; $i += 4) {
            if ($left[$i] -ne $right[$i] -or $left[$i + 1] -ne $right[$i + 1] -or $left[$i + 2] -ne $right[$i + 2] -or $left[$i + 3] -ne $right[$i + 3]) { $changed++ }
        }
        $ratio = $changed / ($expected.Width * $expected.Height)
        if ($ratio -gt $MaxChangedPixelRatio) {
            throw "changed-pixel ratio $ratio exceeds $MaxChangedPixelRatio"
        }
        return $ratio
    }
    finally { $expected.Dispose(); $actual.Dispose() }
}

foreach ($entry in $manifest) {
    $source = Join-Path $root $entry.Source
    $baseline = Join-Path $root $entry.Baseline
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "missing source document: $source" }
    $sourceText = Get-Content -LiteralPath $source -Raw
    if ($sourceText -notmatch '(?m)^type:\s+uml\.UseCaseDiagram\s*$' -or $sourceText -notmatch "(?m)^title:\s+$([regex]::Escape($entry.Title))\s*$") {
        throw "source does not declare the expected active use-case diagram '$($entry.Title)': $source"
    }
    if (-not $Update -and -not (Test-Path -LiteralPath $baseline -PathType Leaf)) { throw "missing baseline: $baseline" }

    $quotedTitle = '"' + $entry.Title + '"'
    $arguments = "-NoProfile -File run.ps1 docs/waml -DebugBuild -Diagram $quotedTitle -Title $($entry.Slug)"
    $launcher = Start-Process -FilePath pwsh -ArgumentList $arguments -WorkingDirectory $root -WindowStyle Hidden -PassThru
    $app = $null
    try {
        $deadline = [DateTime]::UtcNow.AddSeconds(60)
        do {
            Start-Sleep -Milliseconds 250
            $app = Get-Process waml-editor -ErrorAction SilentlyContinue |
                Where-Object { $_.Path -and ([IO.Path]::GetFullPath($_.Path) -ieq $editorExe) -and $_.MainWindowHandle -ne 0 } |
                Select-Object -First 1
        } while (-not $app -and [DateTime]::UtcNow -lt $deadline)
        if (-not $app) { throw "editor window did not open for '$($entry.Title)'" }
        # The first presented frame can contain text while GPU-backed linework
        # is still compiling. Capture only after the native scene has settled.
        Start-Sleep -Seconds 15
        $capture = Join-Path ([IO.Path]::GetTempPath()) ("waml-$($entry.Slug)-$PID.png")
        & pwsh -NoProfile -File (Join-Path $root 'scripts/capture-window.ps1') -Out $capture -ProcessId $app.Id
        if ($LASTEXITCODE -ne 0) { throw "capture failed for '$($entry.Title)'" }
        if ($Update) {
            Copy-Item -LiteralPath $capture -Destination $baseline -Force
            Write-Host "updated $($entry.Baseline)"
        }
        else {
            $ratio = Compare-Png $baseline $capture
            Write-Host "ok $($entry.Title) changed-pixel-ratio=$ratio"
        }
        Remove-Item -LiteralPath $capture -ErrorAction SilentlyContinue
    }
    finally {
        if ($app) { Stop-Process -Id $app.Id -Force -ErrorAction SilentlyContinue }
        Stop-Process -Id $launcher.Id -Force -ErrorAction SilentlyContinue
    }
}
