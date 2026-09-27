param([string]$CargoExecutable = 'cargo')

$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    $manifest = Get-Content -LiteralPath 'Cargo.toml' -Raw
    if ($manifest -notmatch '(?m)^exclude = \["ToniatorLegacy"\]\r?$') { throw 'workspace must exclude ToniatorLegacy' }
    $metadataText = & $CargoExecutable metadata --format-version 1 --no-deps --locked
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
    $metadata = ($metadataText -join "`n") | ConvertFrom-Json
    $expected = @('toniator-app','toniator-cli','toniator-domain','toniator-engine','toniator-geometry','toniator-io','toniator-patterns','toniator-render','toniator-sampling','toniator-windows-fs')
    $actual = @($metadata.packages | Where-Object { $metadata.workspace_members -contains $_.id } | ForEach-Object { $_.name } | Sort-Object)
    if (($actual -join ',') -ne ($expected -join ',')) { throw 'workspace members do not match the core and Windows infrastructure crate set' }
    $allowed = @(
        'toniator-geometry:toniator-domain',
        'toniator-sampling:toniator-domain','toniator-sampling:toniator-geometry','toniator-sampling:toniator-windows-fs',
        'toniator-patterns:toniator-domain','toniator-patterns:toniator-geometry','toniator-patterns:toniator-sampling',
        'toniator-render:toniator-domain','toniator-render:toniator-geometry',
        'toniator-io:toniator-domain','toniator-io:toniator-windows-fs',
        'toniator-engine:toniator-domain','toniator-engine:toniator-sampling','toniator-engine:toniator-patterns','toniator-engine:toniator-render','toniator-engine:toniator-io',
        'toniator-cli:toniator-domain','toniator-cli:toniator-engine','toniator-cli:toniator-io',
        'toniator-app:toniator-domain','toniator-app:toniator-geometry','toniator-app:toniator-engine','toniator-app:toniator-io','toniator-app:toniator-patterns'
    )
    foreach ($package in $metadata.packages) {
        if ($metadata.workspace_members -notcontains $package.id) { continue }
        foreach ($dependency in $package.dependencies) {
            if ($null -ne $dependency.path -and $allowed -notcontains ($package.name + ':' + $dependency.name)) {
                throw ('forbidden workspace dependency: ' + $package.name + ' -> ' + $dependency.name)
            }
        }
    }
    & rg -n -i --glob '*.rs' --glob 'Cargo.toml' --glob '!crates/toniator-app/**' '(^|[^[:alnum:]_])(gtk4?|libadwaita|adw)([^[:alnum:]_]|$)' crates
    if ($LASTEXITCODE -eq 0) { throw 'GTK/libadwaita is restricted to toniator-app' }
    if ($LASTEXITCODE -ne 1) { throw 'GTK dependency search failed' }
    & rg -n -i --glob '*.rs' --glob 'Cargo.toml' '(toniator_sampling|toniator_patterns|resvg|usvg|tiny_skia|gtk4?|libadwaita)' crates/toniator-render
    if ($LASTEXITCODE -eq 0) { throw 'toniator-render must consume canonical geometry without sampling, patterns, or GTK' }
    if ($LASTEXITCODE -ne 1) { throw 'render dependency search failed' }
    & rg -n --glob '*.rs' '(DocumentSession|apply_command|DocumentCommand|&mut[[:space:]]+Document)' crates/toniator-render
    if ($LASTEXITCODE -eq 0) { throw 'toniator-render must not own writable document state' }
    if ($LASTEXITCODE -ne 1) { throw 'render authority search failed' }
    $guidance = @('AGENTS.md','.codex/agents','.agents/skills') | Where-Object { Test-Path -LiteralPath $_ }
    if ($guidance.Count -gt 0) {
        & rg -n -i 'TON-010|Stage[[:space:]]*4\.5|4\.5[A-D]' @guidance
        if ($LASTEXITCODE -eq 0) { throw 'obsolete TON-010 or Stage 4.5 workflow remains active' }
        if ($LASTEXITCODE -ne 1) { throw 'local workflow search failed' }
    }
    Write-Output 'architecture validation passed'
} finally {
    Pop-Location
}
