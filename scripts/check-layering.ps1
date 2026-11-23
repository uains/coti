$ErrorActionPreference = 'Stop'

# AC-09: non-UI crates must not depend on GPUI.
$crates = @('md-core', 'md-document', 'md-markdown', 'md-editor', 'md-layout')

foreach ($crate in $crates) {
    $tree = cargo tree -p $crate --edges normal --prefix none
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    if ($tree -match '(?m)^gpui(\s|$)') {
        Write-Error "layering violation: $crate depends on gpui"
        exit 1
    }
}

Write-Host 'layering: OK (no GPUI below md-render)'
