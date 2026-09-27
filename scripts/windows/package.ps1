[CmdletBinding()]
param(
    [string]$OutputRoot,
    [ValidatePattern('^[A-Fa-f0-9]{64}$')]
    [string]$ExpectedAppSha256 = 'A5B4DC85FCB51FDB3435056B20CE189D4FE2C9C4E0179A31CBB5476F2ED92446',
    [ValidatePattern('^[A-Fa-f0-9]{64}$')]
    [string]$ExpectedCliSha256 = '2DED48A66DCD56B393039C8C66B1419729852FEC28D692D703B7AE143068A7B1',
    [ValidatePattern('^[A-Fa-f0-9]{40}$')]
    [string]$CheckpointCommit
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-env.ps1')
$paths = Get-ToniatorWindowsPaths
if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $OutputRoot = Join-Path $paths.RepositoryRoot 'target\validation\windows-port\releases\windows-dev-20260927'
}
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
$packageName = 'Toniator-0.3.2-windows-x64-dev-20260927'
$packageRoot = Join-Path $OutputRoot 'package'
$sourceRoot = Join-Path $OutputRoot 'sources'
$packageZip = Join-Path $OutputRoot "$packageName.zip"
$sourceZip = Join-Path $OutputRoot "$packageName-sources.zip"
$metadataPath = Join-Path $packageRoot 'PACKAGE-MANIFEST.json'
$sourceMetadataPath = Join-Path $sourceRoot 'SOURCE-MANIFEST.json'
$templateRoot = Join-Path $PSScriptRoot 'release'
$utf8 = New-Object Text.UTF8Encoding($false)

# Writes stable UTF-8 JSON without PowerShell's platform-dependent text encoding.
function Write-PackageJson {
    param([object]$Value, [string]$Path)
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 12) + "`n", $utf8)
}

# Copies retained inputs into a newly owned staging directory, never the SDK.
function Copy-PackageTree {
    param([string]$Source, [string]$Destination, [switch]$ExcludePythonCache)
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    foreach ($file in Get-ChildItem -LiteralPath $Source -Recurse -File) {
        if ($ExcludePythonCache -and $file.Extension -eq '.pyc') { continue }
        $relative = $file.FullName.Substring($Source.TrimEnd('\').Length + 1)
        $target = Join-Path $Destination $relative
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($target)) | Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $target
    }
}

# Records every shipped payload hash; a manifest never hashes itself.
function Get-PackageFileInventory {
    param([string]$Root, [string]$ExcludedName)
    return @(Get-ChildItem -LiteralPath $Root -Recurse -File | Sort-Object FullName | Where-Object {
        $_.FullName -ne (Join-Path $Root $ExcludedName)
    } | ForEach-Object {
        [ordered]@{
            path = $_.FullName.Substring($Root.Length + 1).Replace('\', '/')
            bytes = $_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    })
}

# Uses Microsoft's native import inspector and the existing hidden-process helper.
function Get-NativeImports {
    param([string]$File)
    $result = Invoke-ToniatorCapturedProcess -FilePath $dumpbin -Arguments ('/DEPENDENTS ' + (ConvertTo-ToniatorCommandLineArgument $File)) -Environment @{}
    if ($result.ExitCode -ne 0) { throw "Dependency inspection failed for $File : $($result.StdErr)" }
    return @([regex]::Matches($result.StdOut, '(?im)^\s+([a-z0-9_+.-]+\.dll)\s*$') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
}

# Extracts original legal notices using Windows' maintained archive reader.
function Copy-ArchiveNotices {
    param([string]$Archive, [string]$Destination)
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    $listed = Invoke-ToniatorCapturedProcess -FilePath $tar -Arguments ('-tvf ' + (ConvertTo-ToniatorCommandLineArgument $Archive)) -Environment @{}
    if ($listed.ExitCode -ne 0) { throw "Cannot list source notices: $Archive $($listed.StdErr)" }
    # Some upstream COPYING entries are symlinks. Extract the regular legal files,
    # including LICENSES targets, so Windows never needs symlink privileges.
    $members = @([regex]::Matches($listed.StdOut, '(?m)^-.* (?<member>[^\s]+)\r?$') | ForEach-Object {
        $_.Groups['member'].Value
    } | Where-Object {
        [IO.Path]::GetFileName($_) -match '(?i)^(LICENSE|COPYING|NOTICE|PATENTS|AUTHORS|FTL\.TXT|GPLv2\.TXT)' -or $_ -match '(?i)/(LICENSES|licenses)/'
    })
    foreach ($member in $members) {
        if ($member -match '(^|/)\.\.(/|$)|^/|^[A-Za-z]:') { throw "Unsafe archive member: $member" }
    }
    if ($members.Count -gt 0) {
        $arguments = @('-xf', $Archive, '-C', $Destination) + $members
        $quoted = @($arguments | ForEach-Object { ConvertTo-ToniatorCommandLineArgument $_ }) -join ' '
        $extracted = Invoke-ToniatorCapturedProcess -FilePath $tar -Arguments $quoted -Environment @{}
        if ($extracted.ExitCode -ne 0) { throw "Cannot extract source notices: $Archive $($extracted.StdErr)" }
    }
}

if (Test-Path -LiteralPath $packageRoot) {
    if ([string]::IsNullOrWhiteSpace($CheckpointCommit) -or -not (Test-Path -LiteralPath $metadataPath)) {
        throw "Package staging already exists at $packageRoot. Use a fresh -OutputRoot, or finalize this script's own draft with -CheckpointCommit."
    }
    $metadata = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json
    if ($metadata.packageName -ne $packageName) { throw 'This directory is not the expected package draft.' }
    foreach ($binary in $metadata.executables) {
        if ((Get-FileHash -LiteralPath (Join-Path $packageRoot $binary.path)).Hash -ne $binary.sha256) {
            throw "Draft executable changed: $($binary.path). No package was finalized."
        }
    }
} else {
    if (Test-Path -LiteralPath $sourceRoot) { throw "Source staging already exists: $sourceRoot" }
    foreach ($pair in @(@($paths.AppExecutable, $ExpectedAppSha256), @($paths.CliExecutable, $ExpectedCliSha256))) {
        if ((Get-FileHash -LiteralPath $pair[0]).Hash -ne $pair[1]) { throw "Verified executable hash mismatch: $($pair[0])" }
    }
    $vsRoot = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools'
    $dumpbin = Join-Path $vsRoot 'VC\Tools\MSVC\14.44.35207\bin\Hostx64\x64\dumpbin.exe'
    $crt = Join-Path $vsRoot 'VC\Redist\MSVC\14.44.35112\x64\Microsoft.VC143.CRT'
    $tar = Join-Path $env:SystemRoot 'System32\tar.exe'
    foreach ($file in @($dumpbin, $tar, (Join-Path $crt 'vcruntime140.dll'))) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Required native packaging input is absent: $file" }
    }
    New-Item -ItemType Directory -Force -Path $packageRoot, $sourceRoot, (Join-Path $packageRoot 'bin') | Out-Null
    $queue = [Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($paths.AppExecutable)
    $queue.Enqueue($paths.CliExecutable)
    # GLib loads these at runtime; PE imports alone cannot discover them.
    foreach ($helper in @('gdbus.exe', 'gspawn-win64-helper.exe', 'gspawn-win64-helper-console.exe')) {
        $queue.Enqueue((Join-Path $paths.GtkBin $helper))
    }
    $queue.Enqueue((Join-Path $paths.GtkRoot 'lib\gdk-pixbuf-2.0\2.10.0\loaders\pixbufloader_svg.dll'))
    $seen = @{}
    $dependencies = [Collections.Generic.List[object]]::new()
    while ($queue.Count -gt 0) {
        $file = $queue.Dequeue()
        $name = [IO.Path]::GetFileName($file)
        if ($seen.ContainsKey($name)) { continue }
        $seen[$name] = $true
        $relative = if ($name -eq 'pixbufloader_svg.dll') { 'lib\gdk-pixbuf-2.0\2.10.0\loaders\pixbufloader_svg.dll' } else { "bin\$name" }
        $target = Join-Path $packageRoot $relative
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($target)) | Out-Null
        Copy-Item -LiteralPath $file -Destination $target
        $imports = [Collections.Generic.List[object]]::new()
        foreach ($dependency in Get-NativeImports -File $file) {
            $sdkFile = Join-Path $paths.GtkBin $dependency
            $crtFile = Join-Path $crt $dependency
            if (Test-Path -LiteralPath $sdkFile -PathType Leaf) {
                $queue.Enqueue($sdkFile)
                $origin = 'bundled GTK SDK'
            } elseif (Test-Path -LiteralPath $crtFile -PathType Leaf) {
                $queue.Enqueue($crtFile)
                $origin = 'bundled Microsoft release redistributable'
            } elseif ($dependency -match '^(?i:api-ms-|ext-ms-)' -or (Test-Path -LiteralPath (Join-Path "$env:SystemRoot\System32" $dependency))) {
                $origin = 'Windows operating system (not redistributed)'
            } else { throw "Unresolved dependency: $name imports $dependency" }
            $imports.Add([ordered]@{ name = $dependency; origin = $origin })
        }
        $dependencies.Add([ordered]@{ path = $relative.Replace('\', '/'); imports = @($imports.ToArray()) })
    }
    Write-PackageJson -Value @($dependencies.ToArray()) -Path (Join-Path $packageRoot 'runtime-dependencies.json')
    foreach ($part in @('share\icons', 'share\locale', 'share\glib-2.0\schemas', 'share\gtk-4.0', 'share\fontconfig', 'etc\fonts', 'lib\girepository-1.0')) {
        Copy-PackageTree -Source (Join-Path $paths.GtkRoot $part) -Destination (Join-Path $packageRoot $part)
    }
    Copy-Item -LiteralPath (Join-Path $paths.GtkRoot 'lib\gdk-pixbuf-2.0\2.10.0\loaders.cache') -Destination (Join-Path $packageRoot 'lib\gdk-pixbuf-2.0\2.10.0\loaders.cache')
    foreach ($name in @('Toniator.vbs', 'Toniator-CLI.cmd', 'Setup-Media.ps1', 'README.txt', 'THIRD-PARTY-NOTICES.md', 'SOURCES.md')) {
        Copy-Item -LiteralPath (Join-Path $templateRoot $name) -Destination (Join-Path $packageRoot $name)
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $packageRoot 'notices\toniator'), (Join-Path $sourceRoot 'native'), (Join-Path $sourceRoot 'cargo'), (Join-Path $sourceRoot 'build-records') | Out-Null
    Copy-Item -LiteralPath (Join-Path $paths.RepositoryRoot 'LICENSE') -Destination (Join-Path $packageRoot 'notices\toniator\LICENSE')
    $nativeArchives = @(Get-ChildItem -LiteralPath (Join-Path $paths.ToolsRoot 'gvsbuild\src') -File | Where-Object { $_.Name -match '\.tar\.(gz|xz)$' })
    foreach ($archive in $nativeArchives) {
        Copy-Item -LiteralPath $archive.FullName -Destination (Join-Path $sourceRoot "native\$($archive.Name)")
        Copy-ArchiveNotices -Archive $archive.FullName -Destination (Join-Path $packageRoot "notices\native\$($archive.Name)")
    }
    $accesskitSource = Join-Path $paths.GtkRoot 'sources\accesskit-c-0c52a8ce2357bbeb927f90dc9a1c19c8ec1bd2c3'
    Copy-PackageTree -Source $accesskitSource -Destination (Join-Path $sourceRoot 'accesskit-c')
    New-Item -ItemType Directory -Force -Path (Join-Path $packageRoot 'notices\accesskit-c') | Out-Null
    Get-ChildItem -LiteralPath $accesskitSource -File | Where-Object { $_.Name -match '^(AUTHORS|COPYING|LICENSE)' } | Copy-Item -Destination (Join-Path $packageRoot 'notices\accesskit-c')
    $gvsSite = Join-Path $paths.ToolsRoot 'python\gvsbuild-2026.8.0\Lib\site-packages'
    Copy-PackageTree -Source (Join-Path $gvsSite 'gvsbuild') -Destination (Join-Path $sourceRoot 'gvsbuild\gvsbuild') -ExcludePythonCache
    Copy-PackageTree -Source (Join-Path $gvsSite 'gvsbuild-2026.8.0.dist-info') -Destination (Join-Path $sourceRoot 'gvsbuild\gvsbuild-2026.8.0.dist-info')
    Copy-PackageTree -Source (Join-Path $paths.GtkRoot 'share\licenses') -Destination (Join-Path $packageRoot 'notices\sdk-licenses')
    Copy-PackageTree -Source (Join-Path $paths.GtkRoot 'share\doc') -Destination (Join-Path $packageRoot 'notices\sdk-doc')
    $rustDocs = Join-Path $paths.RustupHome 'toolchains\1.94.1-x86_64-pc-windows-msvc\share\doc\rust'
    New-Item -ItemType Directory -Force -Path (Join-Path $packageRoot 'notices\rust-toolchain') | Out-Null
    Get-ChildItem -LiteralPath $rustDocs -File -Filter 'COPYRIGHT*.html' | Copy-Item -Destination (Join-Path $packageRoot 'notices\rust-toolchain')
    $depRoots = @(
        (Join-Path $paths.ProductBuildRoot 'x86_64-pc-windows-msvc\debug\deps'),
        (Join-Path $paths.ToolsRoot 'gvsbuild\accesskit-build\target'),
        (Join-Path $paths.ToolsRoot 'gvsbuild\build\x64\release\librsvg\_gvsbuild-meson\target')
    )
    $registrySources = @{}
    foreach ($depRoot in $depRoots) {
        foreach ($depfile in Get-ChildItem -LiteralPath $depRoot -Recurse -File -Filter '*.d') {
            foreach ($match in [regex]::Matches((Get-Content -LiteralPath $depfile.FullName -Raw), '(?<source>[A-Za-z]:[\\/][^\r\n:]*?registry[\\/]src[\\/][^\\/]+[\\/](?<crate>[^\\/ ]+))[\\/]')) {
                $registrySources[$match.Groups['crate'].Value] = $match.Groups['source'].Value
            }
        }
    }
    $cache = Join-Path $paths.CargoHome 'registry\cache\index.crates.io-1949cf8c6b5b557f'
    $crateChecksums = @{}
    foreach ($lock in @((Join-Path $paths.RepositoryRoot 'Cargo.lock'), (Join-Path $accesskitSource 'Cargo.lock'), (Join-Path $paths.ToolsRoot 'gvsbuild\build\x64\release\librsvg\Cargo.lock'))) {
        foreach ($entry in ((Get-Content -LiteralPath $lock -Raw) -split '(?m)^\[\[package\]\]\s*$')) {
            $name = [regex]::Match($entry, '(?m)^name = "([^"]+)"').Groups[1].Value
            $version = [regex]::Match($entry, '(?m)^version = "([^"]+)"').Groups[1].Value
            $checksum = [regex]::Match($entry, '(?m)^checksum = "([a-f0-9]{64})"').Groups[1].Value
            if ($checksum) { $crateChecksums["$name-$version"] = $checksum }
        }
    }
    foreach ($crate in ($registrySources.Keys | Sort-Object)) {
        $archive = Join-Path $cache "$crate.crate"
        $source = $registrySources[$crate]
        if (-not $crateChecksums.ContainsKey($crate) -or (Get-FileHash -LiteralPath $archive).Hash.ToLowerInvariant() -ne $crateChecksums[$crate]) { throw "Registry lockfile checksum mismatch: $crate" }
        Copy-Item -LiteralPath $archive -Destination (Join-Path $sourceRoot "cargo\$crate.crate")
        Copy-ArchiveNotices -Archive $archive -Destination (Join-Path $packageRoot "notices\rust-crates\$crate")
        Copy-Item -LiteralPath (Join-Path $source 'Cargo.toml') -Destination (Join-Path $packageRoot "notices\rust-crates\$crate\Cargo.toml")
    }
    $buildRecords = Join-Path $sourceRoot 'build-records'
    foreach ($project in Get-ChildItem -LiteralPath (Join-Path $paths.ToolsRoot 'gvsbuild\build\x64\release') -Directory) {
        $info = Join-Path $project.FullName '_gvsbuild-meson\meson-info\intro-buildoptions.json'
        if (Test-Path -LiteralPath $info) { Copy-Item -LiteralPath $info -Destination (Join-Path $buildRecords "$($project.Name)-buildoptions.json") }
    }
    Copy-Item -LiteralPath (Join-Path $paths.ToolsRoot 'gvsbuild\accesskit-build\meson-private\cmd_line.txt') -Destination (Join-Path $buildRecords 'accesskit-c-buildoptions.txt')
    Copy-Item -LiteralPath (Join-Path $paths.ToolsRoot 'gvsbuild\build\x64\release\librsvg\Cargo.lock') -Destination (Join-Path $buildRecords 'librsvg-Cargo.lock')
    Copy-Item -LiteralPath (Join-Path $templateRoot 'SOURCES.md') -Destination (Join-Path $sourceRoot 'SOURCES.md')
    $metadata = [pscustomobject][ordered]@{
        packageName = $packageName
        prereleaseTag = 'v0.3.2-windows-dev.20260927'
        developmentProfile = 'debug'
        unsigned = $true
        rustToolchain = '1.94.1-x86_64-pc-windows-msvc'
        gtkVersion = '4.22.4'
        checkpointCommit = $null
        checkpointState = 'pending parent checkpoint; not for publication'
        sourceRepository = 'https://github.com/ricperry/Toniator'
        mediaBundled = $false
        executables = @(Get-Item -LiteralPath (Join-Path $packageRoot 'bin\toniator-app.exe'), (Join-Path $packageRoot 'bin\toniator.exe') | ForEach-Object {
            [ordered]@{ path = "bin/$($_.Name)"; bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
        })
        files = @()
    }
}

# Finalization changes only this package's metadata/source artifacts, not executables.
if (-not [string]::IsNullOrWhiteSpace($CheckpointCommit)) {
    $git = (Get-Command git.exe -ErrorAction Stop).Source
    $resolved = Invoke-ToniatorCapturedProcess -FilePath $git -Arguments ('-C ' + (ConvertTo-ToniatorCommandLineArgument $paths.RepositoryRoot) + ' rev-parse ' + $CheckpointCommit) -Environment @{ PATH = $env:PATH }
    if ($resolved.ExitCode -ne 0 -or $resolved.StdOut.Trim() -ne $CheckpointCommit) { throw 'Checkpoint commit is unavailable or is not the exact full commit ID.' }
    $archivePath = Join-Path $sourceRoot 'repository-source.tar'
    $arguments = @('-c', 'core.autocrlf=false', '-C', $paths.RepositoryRoot, 'archive', '--format=tar', '--output', $archivePath, $CheckpointCommit)
    $archived = Invoke-ToniatorCapturedProcess -FilePath $git -Arguments (@($arguments | ForEach-Object { ConvertTo-ToniatorCommandLineArgument $_ }) -join ' ') -Environment @{ PATH = $env:PATH }
    if ($archived.ExitCode -ne 0) { throw "Checkpoint source archive failed: $($archived.StdErr)" }
    $metadata.checkpointCommit = $CheckpointCommit
    $metadata.checkpointState = 'parent checkpoint recorded; executable bytes unchanged'
}
$metadata.files = Get-PackageFileInventory -Root $packageRoot -ExcludedName 'PACKAGE-MANIFEST.json'
Write-PackageJson -Value $metadata -Path $metadataPath
Write-PackageJson -Value ([ordered]@{
    checkpointCommit = $metadata.checkpointCommit
    files = Get-PackageFileInventory -Root $sourceRoot -ExcludedName 'SOURCE-MANIFEST.json'
}) -Path $sourceMetadataPath
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
foreach ($pair in @(@($packageRoot, $packageZip), @($sourceRoot, $sourceZip))) {
    if (Test-Path -LiteralPath $pair[1]) {
        # Only previously generated ZIPs for the validated owned draft are replaced.
        if ([string]::IsNullOrWhiteSpace($CheckpointCommit)) { throw "ZIP already exists: $($pair[1])" }
        Remove-Item -LiteralPath $pair[1]
    }
    $zip = [IO.Compression.ZipFile]::Open($pair[1], [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($file in Get-ChildItem -LiteralPath $pair[0] -Recurse -File | Sort-Object FullName) {
            $entryName = $file.FullName.Substring($pair[0].Length + 1).Replace('\', '/')
            [void][IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file.FullName, $entryName, [IO.Compression.CompressionLevel]::Optimal)
        }
    } finally { $zip.Dispose() }
}
Write-PackageJson -Value @(Get-Item -LiteralPath $packageZip, $sourceZip | ForEach-Object {
    [ordered]@{ file = $_.Name; bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
}) -Path (Join-Path $OutputRoot 'release-assets.json')
Write-Output "Package: $packageZip"
Write-Output "Sources: $sourceZip"
Write-Output "Checkpoint state: $($metadata.checkpointState)"
