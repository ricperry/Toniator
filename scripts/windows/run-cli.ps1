[CmdletBinding()]
param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Arguments
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-env.ps1')

$paths = Get-ToniatorWindowsPaths
$requiredPaths = @($paths.CliExecutable, $paths.GtkBin, $paths.FfmpegBin)
foreach ($path in $requiredPaths) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Required Toniator CLI runtime path does not exist: $path"
    }
}

$inheritedPath = Get-ToniatorCleanPath -PathValue $env:PATH
$environment = [hashtable]::new([System.StringComparer]::OrdinalIgnoreCase)
$environment['PATH'] = @($paths.GtkBin, $paths.FfmpegBin, $inheritedPath) -join [System.IO.Path]::PathSeparator
if ($null -eq $Arguments -or $Arguments.Count -eq 0) {
    $Arguments = @('--help')
}

$exitCode = Invoke-ToniatorProcess -FilePath $paths.CliExecutable -Arguments $Arguments -Environment $environment -WorkingDirectory $PWD.Path
exit $exitCode
