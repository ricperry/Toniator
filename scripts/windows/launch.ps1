[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$ProjectPath
)

$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
    throw 'LOCALAPPDATA is unavailable; Toniator cannot locate its native Windows runtime.'
}

$repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$toolRoot = Join-Path $env:LOCALAPPDATA 'Toniator\tools\windows-x64-msvc'
$gtkRoot = Join-Path $toolRoot 'gvsbuild\gtk\x64\release'
$gtkBin = Join-Path $gtkRoot 'bin'
$schemaDirectory = Join-Path $gtkRoot 'share\glib-2.0\schemas'
$typelibDirectory = Join-Path $gtkRoot 'lib\girepository-1.0'
$ffmpegBin = Join-Path $toolRoot 'media\ffmpeg-8.1.2-full_build\bin'
$executable = Join-Path $repositoryRoot 'target\validation\windows-port\gtk-sdk\product-build\x86_64-pc-windows-msvc\debug\toniator-app.exe'

foreach ($requiredPath in @($gtkBin, $schemaDirectory, $typelibDirectory, $ffmpegBin, $executable)) {
    if (-not (Test-Path -LiteralPath $requiredPath)) {
        throw "Required Toniator runtime path does not exist: $requiredPath"
    }
}

if (-not [string]::IsNullOrEmpty($ProjectPath) -and -not (Test-Path -LiteralPath $ProjectPath -PathType Leaf)) {
    throw "Project file does not exist: $ProjectPath"
}

$startInfo = New-Object System.Diagnostics.ProcessStartInfo
$startInfo.FileName = $executable
$startInfo.WorkingDirectory = (Get-Location).Path
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true

if (-not [string]::IsNullOrEmpty($ProjectPath)) {
    $startInfo.Arguments = '"' + $ProjectPath + '"'
}

# Normalize Path/PATH in the child only, preferring the exact PATH spelling and
# keeping the caller's path intact after Toniator's GTK and FFmpeg directories.
$inheritedEnvironment = [System.Environment]::GetEnvironmentVariables()
$pathNames = @($inheritedEnvironment.Keys | Where-Object {
    [string]::Equals([string]$_, 'PATH', [System.StringComparison]::OrdinalIgnoreCase)
})
$preferredPathName = $pathNames | Where-Object {
    [string]::Equals([string]$_, 'PATH', [System.StringComparison]::Ordinal)
} | Select-Object -First 1
if ($null -eq $preferredPathName -and $pathNames.Count -gt 0) {
    $preferredPathName = $pathNames[0]
}
$callerPath = if ($null -ne $preferredPathName) { [string]$inheritedEnvironment[$preferredPathName] } else { '' }

$childPathNames = @($startInfo.EnvironmentVariables.Keys | Where-Object {
    [string]::Equals([string]$_, 'PATH', [System.StringComparison]::OrdinalIgnoreCase)
})
foreach ($pathName in $childPathNames) {
    $startInfo.EnvironmentVariables.Remove([string]$pathName)
}
$pathEntries = @($gtkBin, $ffmpegBin)
if (-not [string]::IsNullOrEmpty($callerPath)) {
    $pathEntries += $callerPath
}
$startInfo.EnvironmentVariables['PATH'] = $pathEntries -join [System.IO.Path]::PathSeparator
$startInfo.EnvironmentVariables['GTK_A11Y'] = 'accesskit'
$startInfo.EnvironmentVariables['GSETTINGS_SCHEMA_DIR'] = $schemaDirectory
$startInfo.EnvironmentVariables['GI_TYPELIB_PATH'] = $typelibDirectory

$process = [System.Diagnostics.Process]::Start($startInfo)
if ($null -eq $process) {
    throw 'Windows did not start the Toniator application.'
}
$processId = $process.Id
$process.Dispose()
Write-Output "Started Toniator (PID $processId): $executable"
