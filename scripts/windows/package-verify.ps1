[CmdletBinding()]
param([string]$OutputRoot)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-env.ps1')
$paths = Get-ToniatorWindowsPaths
if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    $OutputRoot = Join-Path $paths.RepositoryRoot 'target\validation\windows-port\releases\windows-dev-20260927'
}
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)
$verificationRoot = Join-Path $OutputRoot ('smoke-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-ffff'))
# Construct Unicode without relying on the host's script-file code page.
$extractedRoot = Join-Path $verificationRoot ('Portable Toniator ' + [char]0x03A9 + ' with spaces')
New-Item -ItemType Directory -Force -Path $extractedRoot | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::ExtractToDirectory((Join-Path $OutputRoot 'Toniator-0.3.2-windows-x64-dev-20260927.zip'), $extractedRoot)
$manifest = Get-Content -LiteralPath (Join-Path $extractedRoot 'PACKAGE-MANIFEST.json') -Raw | ConvertFrom-Json
foreach ($file in $manifest.files) {
    if ((Get-FileHash -LiteralPath (Join-Path $extractedRoot $file.path)).Hash -ne $file.sha256) {
        throw "Extracted package checksum mismatch: $($file.path)"
    }
}
$environment = @{
    PATH = (Join-Path $extractedRoot 'bin') + ";$env:SystemRoot\System32;$env:SystemRoot"
    XDG_DATA_DIRS = Join-Path $extractedRoot 'share'
    GSETTINGS_SCHEMA_DIR = Join-Path $extractedRoot 'share\glib-2.0\schemas'
    GI_TYPELIB_PATH = Join-Path $extractedRoot 'lib\girepository-1.0'
    GDK_PIXBUF_MODULE_FILE = Join-Path $extractedRoot 'lib\gdk-pixbuf-2.0\2.10.0\loaders.cache'
    FONTCONFIG_PATH = Join-Path $extractedRoot 'etc\fonts'
    FONTCONFIG_FILE = Join-Path $extractedRoot 'etc\fonts\fonts.conf'
    GTK_PATH = ''
    GTK_MODULES = ''
    GIO_EXTRA_MODULES = ''
}
$modules = @{}
$results = [Collections.Generic.List[object]]::new()

# Captures actual package processes without opening helper console windows.
function Invoke-PackagedSmoke {
    param([string]$File, [string[]]$Arguments, [string]$Name)
    $quoted = @($Arguments | ForEach-Object { ConvertTo-ToniatorCommandLineArgument $_ }) -join ' '
    $start = New-ToniatorProcessStartInfo -FilePath $File -Arguments $quoted -Environment $environment -WorkingDirectory $verificationRoot -CaptureOutput -CreateNoWindow
    $process = New-Object Diagnostics.Process
    $process.StartInfo = $start
    try {
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $deadline = [DateTime]::UtcNow.AddSeconds(60)
        while (-not $process.HasExited) {
            try {
                foreach ($module in $process.Modules) { $modules[$module.FileName] = $true }
            } catch [ComponentModel.Win32Exception] { }
            if ([DateTime]::UtcNow -gt $deadline) {
                $process.Kill()
                throw "Owned package smoke process timed out: $Name"
            }
            [void]$process.WaitForExit(20)
        }
        $process.WaitForExit()
        [IO.File]::WriteAllText((Join-Path $verificationRoot "$Name.stdout.log"), $stdout.Result)
        [IO.File]::WriteAllText((Join-Path $verificationRoot "$Name.stderr.log"), $stderr.Result)
        $results.Add([ordered]@{ check = $Name; exitCode = $process.ExitCode; arguments = $Arguments })
        if ($process.ExitCode -ne 0) { throw "Packaged $Name failed ($($process.ExitCode)): $($stderr.Result)" }
    } finally { $process.Dispose() }
}

$cli = Join-Path $extractedRoot 'bin\toniator.exe'
Invoke-PackagedSmoke -File $cli -Arguments @('--help') -Name 'help'
$cmd = Join-Path $env:SystemRoot 'System32\cmd.exe'
$wrapper = Invoke-ToniatorCapturedProcess -FilePath $cmd -Arguments ('/d /s /c ""' + (Join-Path $extractedRoot 'Toniator-CLI.cmd') + '" --help"') -Environment $environment -WorkingDirectory $verificationRoot
if ($wrapper.ExitCode -ne 0) { throw "CLI launcher failed: $($wrapper.StdErr)" }
[IO.File]::WriteAllText((Join-Path $verificationRoot 'cli-launcher-help.log'), $wrapper.StdOut + $wrapper.StdErr)
foreach ($sample in @('raster', 'vector')) {
    $extension = if ($sample -eq 'raster') { 'png' } else { 'svg' }
    $sampleInput = Join-Path $verificationRoot "$sample-sample.$extension"
    if ($sample -eq 'raster') {
        Copy-Item -LiteralPath (Join-Path $paths.RepositoryRoot 'assets\raster-sample.png') -Destination $sampleInput
        $expectedHash = '324AC232E319002A13FBCFAC46538CA5D7E8BA8A127EEA2EAF20E8DDB3ED2EF2'
    } else {
        # The Windows worktree may use CRLF. Restore canonical immutable blob bytes
        # only into validation output; never rewrite the project-wide input.
        $git = (Get-Command git.exe).Source
        $blob = Invoke-ToniatorCapturedProcess -FilePath $git -Arguments ('-C ' + (ConvertTo-ToniatorCommandLineArgument $paths.RepositoryRoot) + ' show HEAD:assets/vector-sample.svg') -Environment @{ PATH = $env:PATH }
        if ($blob.ExitCode -ne 0) { throw 'Cannot read canonical immutable SVG.' }
        [IO.File]::WriteAllText($sampleInput, $blob.StdOut, (New-Object Text.UTF8Encoding($false)))
        $expectedHash = '42EB5E23111A5DBAD66F2B1802A7CC06391C7EDE829B99EB28AEB1AC91596E2E'
    }
    if ((Get-FileHash -LiteralPath $sampleInput).Hash -ne $expectedHash) { throw "Immutable $sample input hash mismatch." }
    $arguments = @('render', '--input', $sampleInput, '--output', (Join-Path $verificationRoot "$sample.png"), '--channel-model', 'source-color-alpha', '--canvas', '64x64', '--density', '8', '--density-aspect', '1', '--rotation', '0', '--offset-x', '0', '--offset-y', '0', '--guard-steps', '1')
    Invoke-PackagedSmoke -File $cli -Arguments $arguments -Name "$sample-render"
}
$external = @($modules.Keys | Where-Object {
    -not $_.StartsWith($extractedRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
    -not $_.StartsWith($env:SystemRoot + '\', [StringComparison]::OrdinalIgnoreCase)
})
if ($external.Count -gt 0) { throw ('Unexpected external native modules: ' + ($external -join ', ')) }
[ordered]@{
    extractedRoot = $extractedRoot
    isolatedPath = $environment.PATH
    mediaBundled = Test-Path -LiteralPath (Join-Path $extractedRoot 'media')
    executableHashes = $manifest.executables
    inputHashes = @(Get-Item -LiteralPath (Join-Path $verificationRoot 'raster-sample.png'), (Join-Path $verificationRoot 'vector-sample.svg') | ForEach-Object {
        [ordered]@{ file = $_.Name; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
    })
    results = @($results.ToArray())
    loadedModules = @($modules.Keys | Sort-Object)
    outputHashes = @(Get-ChildItem -LiteralPath $verificationRoot -Filter '*.png' | ForEach-Object {
        [ordered]@{ file = $_.Name; bytes = $_.Length; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash }
    })
    scope = 'Native extracted-package CLI help and two real renders; static imports cover GUI closure. No clean-machine or GUI acceptance claim.'
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $verificationRoot 'result.json') -Encoding UTF8
Write-Output "Verified extracted CLI package: $extractedRoot"
Write-Output "Evidence: $verificationRoot"
