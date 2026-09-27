[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-env.ps1')

$build = Initialize-ToniatorWindowsBuildEnvironment
$paths = $build.Paths
$environment = $build.Environment

Write-Output "Visual Studio: $($paths.VsInstallRoot)"
Write-Output "Compiler: $($build.Compiler)"
Write-Output "Linker: $($build.Linker)"
Write-Output "Rust toolchain: $($paths.RustToolchain)"
Write-Output "Windows SDK: $($paths.WindowsSdkVersion)"
Write-Output "GTK: $($paths.GtkRoot)"
Write-Output "Blueprint Compiler: $($paths.BlueprintCompiler)"
Write-Output "FFmpeg: $($paths.Ffmpeg)"
Write-Output "Cargo target directory: $($paths.ProductBuildRoot)"

$compilerVersion = Invoke-ToniatorCapturedProcess -FilePath $build.Compiler -Arguments '/Bv' -Environment $environment -WorkingDirectory $paths.RepositoryRoot
$linkerVersion = Invoke-ToniatorCapturedProcess -FilePath $build.Linker -Arguments '/?' -Environment $environment -WorkingDirectory $paths.RepositoryRoot
$compilerIdentity = @($compilerVersion.StdOut -split "`r?`n" | Where-Object { $_ -match 'Compiler Version|Optimizing Compiler Version' } | Select-Object -First 1)
$linkerIdentity = @($linkerVersion.StdOut -split "`r?`n" | Where-Object { $_ -match 'Linker Version' } | Select-Object -First 1)
if ($compilerIdentity.Count -gt 0) { Write-Output $compilerIdentity[0].Trim() }
if ($linkerIdentity.Count -gt 0) { Write-Output $linkerIdentity[0].Trim() }

$cargoArguments = @('build', '--locked', '--target', $paths.Target, '-p', 'toniator-app', '-p', 'toniator-cli')
$exitCode = Invoke-ToniatorProcess -FilePath $paths.Cargo -Arguments $cargoArguments -Environment $environment -WorkingDirectory $paths.RepositoryRoot
if ($exitCode -ne 0) {
    exit $exitCode
}

Write-Output "Built Toniator GUI and CLI for $($paths.Target)."
Write-Output "GUI: $($paths.AppExecutable)"
Write-Output "CLI: $($paths.CliExecutable)"
