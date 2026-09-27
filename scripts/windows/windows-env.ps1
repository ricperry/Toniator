function Get-ToniatorWindowsPaths {
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw 'LOCALAPPDATA is unavailable; Toniator cannot locate its native Windows tools.'
    }

    $repositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
    $toolsRoot = Join-Path $env:LOCALAPPDATA 'Toniator\tools\windows-x64-msvc'
    $gtkRoot = Join-Path $toolsRoot 'gvsbuild\gtk\x64\release'
    $pythonRoot = Join-Path $toolsRoot 'python'
    $toolchainFile = Join-Path $repositoryRoot 'rust-toolchain.toml'
    $toolchainMatch = [regex]::Match((Get-Content -LiteralPath $toolchainFile -Raw), '(?m)^\s*channel\s*=\s*"(?<channel>[^"]+)"')
    if (-not $toolchainMatch.Success) {
        throw "Could not read the Rust channel from $toolchainFile."
    }

    $programFilesX86 = ${env:ProgramFiles(x86)}
    if ([string]::IsNullOrWhiteSpace($programFilesX86)) {
        throw 'ProgramFiles(x86) is unavailable; Toniator cannot locate the Windows SDK or Visual Studio Installer.'
    }

    return [pscustomobject]@{
        RepositoryRoot = $repositoryRoot
        ToolsRoot = $toolsRoot
        CargoHome = Join-Path $toolsRoot 'cargo'
        RustupHome = Join-Path $toolsRoot 'rustup'
        Cargo = Join-Path $toolsRoot 'cargo\bin\cargo.exe'
        Rustup = Join-Path $toolsRoot 'cargo\bin\rustup.exe'
        RustToolchain = $toolchainMatch.Groups['channel'].Value + '-x86_64-pc-windows-msvc'
        Target = 'x86_64-pc-windows-msvc'
        ProductBuildRoot = Join-Path $repositoryRoot 'target\validation\windows-port\gtk-sdk\product-build'
        AppExecutable = Join-Path $repositoryRoot 'target\validation\windows-port\gtk-sdk\product-build\x86_64-pc-windows-msvc\debug\toniator-app.exe'
        CliExecutable = Join-Path $repositoryRoot 'target\validation\windows-port\gtk-sdk\product-build\x86_64-pc-windows-msvc\debug\toniator.exe'
        GtkRoot = $gtkRoot
        GtkBin = Join-Path $gtkRoot 'bin'
        GtkLib = Join-Path $gtkRoot 'lib'
        GtkInclude = Join-Path $gtkRoot 'include'
        GtkSchemas = Join-Path $gtkRoot 'share\glib-2.0\schemas'
        GtkTypelibs = Join-Path $gtkRoot 'lib\girepository-1.0'
        PkgConfig = Join-Path $gtkRoot 'bin\pkg-config.exe'
        PkgConfigPath = Join-Path $gtkRoot 'lib\pkgconfig'
        BlueprintScripts = Join-Path $pythonRoot 'blueprint-0.22.2\Scripts'
        BlueprintCompiler = Join-Path $pythonRoot 'blueprint-0.22.2\Scripts\blueprint-compiler.exe'
        Python = Join-Path $pythonRoot 'gvsbuild-2026.8.0\Scripts\python.exe'
        FfmpegBin = Join-Path $toolsRoot 'media\ffmpeg-8.1.2-full_build\bin'
        Ffmpeg = Join-Path $toolsRoot 'media\ffmpeg-8.1.2-full_build\bin\ffmpeg.exe'
        VsWhere = Join-Path $programFilesX86 'Microsoft Visual Studio\Installer\vswhere.exe'
        VsInstallRoot = $null
        VsDevCmd = $null
        WindowsSdkVersion = '10.0.22621.0'
        WindowsSdkRoot = Join-Path $programFilesX86 'Windows Kits\10'
        WindowsSdkInclude = Join-Path $programFilesX86 'Windows Kits\10\Include\10.0.22621.0'
        WindowsSdkLib = Join-Path $programFilesX86 'Windows Kits\10\Lib\10.0.22621.0'
    }
}

function Get-ToniatorCleanPath {
    param([string]$PathValue)

    $entries = @($PathValue -split [regex]::Escape([System.IO.Path]::PathSeparator) | Where-Object {
        -not [string]::IsNullOrWhiteSpace($_) -and
        $_ -notmatch '(?i)(?:^|[\\/])(?:msys|msys64|mingw32|mingw64)(?:[\\/]|$)'
    })
    return ($entries -join [System.IO.Path]::PathSeparator)
}

function New-ToniatorProcessStartInfo {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string]$Arguments = '',
        [Parameter(Mandatory = $true)][System.Collections.IDictionary]$Environment,
        [string]$WorkingDirectory = $PWD.Path,
        [switch]$CaptureOutput,
        [switch]$CreateNoWindow
    )

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = $FilePath
    $startInfo.Arguments = $Arguments
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $CreateNoWindow.IsPresent
    if ($CaptureOutput) {
        $startInfo.RedirectStandardOutput = $true
        $startInfo.RedirectStandardError = $true
    }

    $environmentNames = @($startInfo.EnvironmentVariables.Keys)
    foreach ($name in $environmentNames) {
        if ([string]::Equals([string]$name, 'PATH', [System.StringComparison]::OrdinalIgnoreCase) -or
            [string]$name -match '^(?i:MSYSTEM|MINGW_PREFIX|MSYS2_PATH_TYPE)$') {
            $startInfo.EnvironmentVariables.Remove([string]$name)
        }
    }
    foreach ($name in $Environment.Keys) {
        if ($null -ne $Environment[$name]) {
            $startInfo.EnvironmentVariables[[string]$name] = [string]$Environment[$name]
        }
    }

    return $startInfo
}

function Invoke-ToniatorCapturedProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string]$Arguments,
        [Parameter(Mandatory = $true)][System.Collections.IDictionary]$Environment,
        [string]$WorkingDirectory = $PWD.Path
    )

    $startInfo = New-ToniatorProcessStartInfo -FilePath $FilePath -Arguments $Arguments -Environment $Environment -WorkingDirectory $WorkingDirectory -CaptureOutput -CreateNoWindow
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw "Windows did not start $FilePath."
        }
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            StdOut = $stdoutTask.Result
            StdErr = $stderrTask.Result
        }
    }
    finally {
        $process.Dispose()
    }
}

function Get-ToniatorEnvironmentFromSetOutput {
    param([string]$Output)

    $environment = [hashtable]::new([System.StringComparer]::OrdinalIgnoreCase)
    foreach ($line in ($Output -split "`r?`n")) {
        $separator = $line.IndexOf('=')
        if ($separator -le 0) {
            continue
        }

        $name = $line.Substring(0, $separator)
        if ($name -match '^[A-Za-z_][A-Za-z0-9_]*$') {
            $environment[$name] = $line.Substring($separator + 1)
        }
    }
    return $environment
}

function Initialize-ToniatorWindowsBuildEnvironment {
    $paths = Get-ToniatorWindowsPaths
    $requiredFiles = @(
        $paths.VsWhere,
        $paths.Cargo,
        $paths.Rustup,
        $paths.PkgConfig,
        $paths.BlueprintCompiler,
        $paths.Python,
        $paths.Ffmpeg
    )
    foreach ($file in $requiredFiles) {
        if (-not (Test-Path -LiteralPath $file -PathType Leaf)) {
            throw "Required Toniator Windows build tool does not exist: $file"
        }
    }

    $requiredDirectories = @(
        $paths.GtkSchemas,
        $paths.GtkTypelibs,
        $paths.PkgConfigPath,
        $paths.WindowsSdkInclude,
        $paths.WindowsSdkLib
    )
    foreach ($directory in $requiredDirectories) {
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) {
            throw "Required Toniator Windows build directory does not exist: $directory"
        }
    }

    $baseEnvironment = [hashtable]::new([System.StringComparer]::OrdinalIgnoreCase)
    $baseVariables = [System.Environment]::GetEnvironmentVariables()
    foreach ($name in $baseVariables.Keys) {
        $baseEnvironment[[string]$name] = [string]$baseVariables[$name]
    }
    $basePath = Get-ToniatorCleanPath -PathValue $baseEnvironment['PATH']
    $baseEnvironment['PATH'] = $basePath

    $vswhere = Invoke-ToniatorCapturedProcess -FilePath $paths.VsWhere -Arguments '-latest -version [17.0,18.0) -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath' -Environment $baseEnvironment -WorkingDirectory $paths.RepositoryRoot
    if ($vswhere.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($vswhere.StdOut)) {
        throw "vswhere could not locate Visual Studio 2022 with x64 MSVC tools. $($vswhere.StdErr)"
    }

    $paths.VsInstallRoot = $vswhere.StdOut.Trim()
    $paths.VsDevCmd = Join-Path $paths.VsInstallRoot 'Common7\Tools\VsDevCmd.bat'
    if (-not (Test-Path -LiteralPath $paths.VsDevCmd -PathType Leaf)) {
        throw "The Visual Studio 2022 installation has no VsDevCmd.bat: $($paths.VsDevCmd)"
    }

    $comspec = if (-not [string]::IsNullOrWhiteSpace($baseEnvironment['ComSpec'])) { $baseEnvironment['ComSpec'] } else { Join-Path $env:SystemRoot 'System32\cmd.exe' }
    $command = '""{0}" -no_logo -arch=x64 -host_arch=x64 -winsdk={1} >nul && set"' -f $paths.VsDevCmd, $paths.WindowsSdkVersion
    $visualStudio = Invoke-ToniatorCapturedProcess -FilePath $comspec -Arguments ('/d /s /c ' + $command) -Environment $baseEnvironment -WorkingDirectory $paths.RepositoryRoot
    if ($visualStudio.ExitCode -ne 0) {
        throw "VsDevCmd failed for x64 MSVC and Windows SDK $($paths.WindowsSdkVersion). $($visualStudio.StdErr)"
    }

    $environment = Get-ToniatorEnvironmentFromSetOutput -Output $visualStudio.StdOut
    $sdkVersion = ([string]$environment['WindowsSDKVersion']).TrimEnd('\')
    if ($sdkVersion -ne $paths.WindowsSdkVersion) {
        throw "VsDevCmd selected Windows SDK '$sdkVersion'; expected '$($paths.WindowsSdkVersion)'."
    }
    if ([string]::IsNullOrWhiteSpace([string]$environment['VCToolsInstallDir'])) {
        throw 'VsDevCmd did not initialize the x64 MSVC compiler environment.'
    }

    $environment['PATH'] = Get-ToniatorCleanPath -PathValue $environment['PATH']
    $environment['PATH'] = @(
        $paths.GtkBin,
        $paths.BlueprintScripts,
        $paths.FfmpegBin,
        (Join-Path $paths.CargoHome 'bin'),
        $environment['PATH']
    ) -join [System.IO.Path]::PathSeparator
    $environment['RUSTUP_HOME'] = $paths.RustupHome
    $environment['CARGO_HOME'] = $paths.CargoHome
    $environment['RUSTUP_TOOLCHAIN'] = $paths.RustToolchain
    $environment['CARGO_TARGET_DIR'] = $paths.ProductBuildRoot
    $environment['CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER'] = 'link.exe'
    $environment['PKG_CONFIG'] = $paths.PkgConfig
    $environment['PKG_CONFIG_PATH'] = $paths.PkgConfigPath
    $environment['LIB'] = $paths.GtkLib + [System.IO.Path]::PathSeparator + [string]$environment['LIB']
    $environment['INCLUDE'] = $paths.GtkInclude + [System.IO.Path]::PathSeparator + [string]$environment['INCLUDE']
    $environment['CC'] = 'cl'
    $environment['CXX'] = 'cl'
    $environment['GI_TYPELIB_PATH'] = $paths.GtkTypelibs
    $environment['GSETTINGS_SCHEMA_DIR'] = $paths.GtkSchemas
    $environment['TONIATOR_WINDOWS_PYTHON'] = $paths.Python

    $compiler = Resolve-ToniatorExecutable -Name 'cl.exe' -Path $environment['PATH']
    $linker = Resolve-ToniatorExecutable -Name 'link.exe' -Path $environment['PATH']
    if ([string]::IsNullOrWhiteSpace($compiler) -or [string]::IsNullOrWhiteSpace($linker)) {
        throw 'VsDevCmd did not place the x64 MSVC compiler and linker on PATH.'
    }
    if (-not $compiler.StartsWith([string]$environment['VCToolsInstallDir'], [System.StringComparison]::OrdinalIgnoreCase) -or
        -not $linker.StartsWith([string]$environment['VCToolsInstallDir'], [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Compiler or linker did not resolve from VCToolsInstallDir. cl=$compiler link=$linker"
    }

    return [pscustomobject]@{
        Paths = $paths
        Environment = $environment
        Compiler = $compiler
        Linker = $linker
    }
}

function Resolve-ToniatorExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Path
    )

    foreach ($directory in ($Path -split [regex]::Escape([System.IO.Path]::PathSeparator))) {
        if (-not [string]::IsNullOrWhiteSpace($directory)) {
            $candidate = Join-Path $directory $Name
            if (Test-Path -LiteralPath $candidate -PathType Leaf) {
                return $candidate
            }
        }
    }
    return $null
}

function ConvertTo-ToniatorCommandLineArgument {
    param([Parameter(Mandatory = $true)][AllowEmptyString()][string]$Value)

    $builder = New-Object System.Text.StringBuilder
    [void]$builder.Append('"')
    $slashes = 0
    foreach ($character in $Value.ToCharArray()) {
        if ($character -eq '\') {
            $slashes++
            continue
        }
        if ($character -eq '"') {
            [void]$builder.Append('\' * (2 * $slashes + 1))
            [void]$builder.Append('"')
            $slashes = 0
            continue
        }
        [void]$builder.Append('\' * $slashes)
        [void]$builder.Append($character)
        $slashes = 0
    }
    [void]$builder.Append('\' * (2 * $slashes))
    [void]$builder.Append('"')
    return $builder.ToString()
}

function Invoke-ToniatorProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [Parameter(Mandatory = $true)][System.Collections.IDictionary]$Environment,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory
    )

    $quotedArguments = @($Arguments | ForEach-Object { ConvertTo-ToniatorCommandLineArgument -Value $_ })
    $startInfo = New-ToniatorProcessStartInfo -FilePath $FilePath -Arguments ($quotedArguments -join ' ') -Environment $Environment -WorkingDirectory $WorkingDirectory
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw "Windows did not start $FilePath."
        }
        $process.WaitForExit()
        return $process.ExitCode
    }
    finally {
        $process.Dispose()
    }
}
