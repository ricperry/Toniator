[CmdletBinding()]
param(
    [switch] $Apply,
    [string] $RecipeRoot,
    [string] $WorkRoot
)

$ErrorActionPreference = 'Stop'
$patchRoot = Join-Path $PSScriptRoot 'gvsbuild-patches'

if ([string]::IsNullOrWhiteSpace($RecipeRoot)) {
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw 'LOCALAPPDATA is not set; pass -RecipeRoot to the gvsbuild site-packages directory.'
    }
    $RecipeRoot = Join-Path $env:LOCALAPPDATA 'Toniator\tools\windows-x64-msvc\python\gvsbuild-2026.8.0\Lib\site-packages'
}
$RecipeRoot = [IO.Path]::GetFullPath($RecipeRoot)

if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    $WorkRoot = [IO.Path]::GetTempPath()
}
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)
if (-not (Test-Path -LiteralPath $WorkRoot -PathType Container)) {
    throw "Scratch parent does not exist: $WorkRoot"
}

$expected = @{
    Librsvg = @{
        RelativePath = 'gvsbuild\projects\librsvg.py'
        Before       = 'CAC5DDE74DB823AFB480E7997F1A306924542F8519641D0CBF4C9E46D8CEE64C'
        AfterFirst   = '885E705E9D1527471A85FB7E0E37755A2E9EAD5F59DF9250747FA4B3B7D74613'
        After        = 'C201101F7E476813C2AA893B58030CD328DF675285DAA60A030B9BB43CA98F9F'
    }
    Gtk = @{
        RelativePath = 'gvsbuild\projects\gtk.py'
        Before       = '609A92C9C0A5A7A0ABA86828E0CB102E567C6519BAEE4338A134273EEEEDB7F1'
        After        = 'D1D84A31CD6634A3743F1FC96657AE4B646D788A5B85E5AB78D4832C2F2007A2'
    }
}

function Get-FileSha256([string] $Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToUpperInvariant()
}

function Invoke-GitPatch([string] $StageRoot, [string] $PatchName) {
    $patchPath = Join-Path $patchRoot $PatchName
    if (-not (Test-Path -LiteralPath $patchPath -PathType Leaf)) {
        throw "Missing checked-in patch: $patchPath"
    }
    & git -C $StageRoot apply --check --ignore-space-change -p1 $patchPath
    if ($LASTEXITCODE -ne 0) {
        throw "Patch context check failed: $PatchName"
    }
    & git -C $StageRoot apply --ignore-space-change -p1 $patchPath
    if ($LASTEXITCODE -ne 0) {
        throw "Patch application failed: $PatchName"
    }
}

$live = @{}
foreach ($name in @('Librsvg', 'Gtk')) {
    $path = Join-Path $RecipeRoot $expected[$name].RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Missing gvsbuild recipe: $path"
    }
    $hash = Get-FileSha256 $path
    $live[$name] = @{ Path = $path; Hash = $hash }
}

$validLibrsvg = @($expected.Librsvg.Before, $expected.Librsvg.AfterFirst, $expected.Librsvg.After)
if ($live.Librsvg.Hash -notin $validLibrsvg) {
    throw "Unexpected librsvg.py SHA-256 $($live.Librsvg.Hash); expected one of the pinned patch states."
}
if ($live.Gtk.Hash -notin @($expected.Gtk.Before, $expected.Gtk.After)) {
    throw "Unexpected gtk.py SHA-256 $($live.Gtk.Hash); expected the pinned pre- or post-patch recipe."
}

$stageRoot = Join-Path $WorkRoot ('toniator-gvsbuild-patches-' + [guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stageRoot) | Out-Null
foreach ($name in @('Librsvg', 'Gtk')) {
    $relative = $expected[$name].RelativePath
    $stagePath = Join-Path $stageRoot $relative
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($stagePath)) | Out-Null
    Copy-Item -LiteralPath $live[$name].Path -Destination $stagePath
}

& git -c core.autocrlf=false -C $stageRoot init --quiet
if ($LASTEXITCODE -ne 0) {
    throw 'Could not initialize the isolated patch-check directory.'
}

$stagedLibrsvg = Join-Path $stageRoot $expected.Librsvg.RelativePath
if ($live.Librsvg.Hash -eq $expected.Librsvg.Before) {
    Invoke-GitPatch $stageRoot '01-librsvg-pin-cargo-c.patch'
    $hash = Get-FileSha256 $stagedLibrsvg
    if ($hash -ne $expected.Librsvg.AfterFirst) {
        throw "cargo-c patch produced unexpected SHA-256 $hash"
    }
}
if ($live.Librsvg.Hash -ne $expected.Librsvg.After) {
    Invoke-GitPatch $stageRoot '02-librsvg-cargo-path.patch'
    $hash = Get-FileSha256 $stagedLibrsvg
    if ($hash -ne $expected.Librsvg.After) {
        throw "librsvg Cargo-path patch produced unexpected SHA-256 $hash"
    }
}

$stagedGtk = Join-Path $stageRoot $expected.Gtk.RelativePath
if ($live.Gtk.Hash -eq $expected.Gtk.Before) {
    Invoke-GitPatch $stageRoot '03-gtk-static-accesskit.patch'
    $hash = Get-FileSha256 $stagedGtk
    if ($hash -ne $expected.Gtk.After) {
        throw "GTK AccessKit patch produced unexpected SHA-256 $hash"
    }
}

if ($Apply) {
    Copy-Item -LiteralPath $stagedLibrsvg -Destination $live.Librsvg.Path -Force
    Copy-Item -LiteralPath $stagedGtk -Destination $live.Gtk.Path -Force
    foreach ($name in @('Librsvg', 'Gtk')) {
        $hash = Get-FileSha256 $live[$name].Path
        if ($hash -ne $expected[$name].After) {
            throw "Post-write verification failed for $($expected[$name].RelativePath): $hash"
        }
    }
    Write-Output "Applied and verified the pinned gvsbuild recipe patches in $RecipeRoot"
} else {
    Write-Output 'Patch replay validated in an isolated copy; installed recipes were not changed.'
    Write-Output 'Pass -Apply to write the verified post-patch recipes to RecipeRoot.'
}
Write-Output "Scratch evidence: $stageRoot"
