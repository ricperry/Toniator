[CmdletBinding()]
param(
    # A retained upstream archive may be used for an offline FFmpeg download step.
    [string]$ArchivePath,
    [string]$SevenZipPath
)

$ErrorActionPreference = 'Stop'
$archiveHash = '0fff188997a499b5382e0f66e845d4556c48c54f0113ebed4853d556dbdd7059'
$archiveUrl = 'https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build.7z'
$extractorUrl = 'https://github.com/ip7z/7zip/releases/download/26.03/7zr.exe'
$extractorHash = 'ad4c82fadcbdf93c03b4fc440f300509c7d60c5c2f4d183e35d9d70d6957037d'
$mediaRoot = Join-Path $PSScriptRoot 'media'
$installRoot = Join-Path $mediaRoot 'ffmpeg-8.1.2-full_build'
if (Test-Path -LiteralPath $installRoot) {
    throw "Media already exists at $installRoot. Keep it or move it yourself before running setup again."
}
$downloadRoot = Join-Path $mediaRoot 'downloads'
New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

# Downloads are explicit, package-local, hash checked, and never installed globally.
function Get-VerifiedDownload {
    param([string]$Url, [string]$Path, [string]$Hash)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        Write-Host "Downloading $Url"
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Path
    }
    if ((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Hash) {
        throw "Checksum mismatch: $Path. No downloaded program has been run."
    }
}

if ([string]::IsNullOrWhiteSpace($ArchivePath)) {
    $ArchivePath = Join-Path $downloadRoot 'ffmpeg-8.1.2-full_build.7z'
    Get-VerifiedDownload -Url $archiveUrl -Path $ArchivePath -Hash $archiveHash
} else {
    $ArchivePath = (Resolve-Path -LiteralPath $ArchivePath).Path
    if ((Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant() -ne $archiveHash) {
        throw 'The supplied archive is not the pinned Gyan FFmpeg 8.1.2 full build.'
    }
}
if ([string]::IsNullOrWhiteSpace($SevenZipPath)) {
    $installed = Get-Command 7z.exe -ErrorAction SilentlyContinue
    if ($null -ne $installed) { $SevenZipPath = $installed.Source }
    elseif (Test-Path -LiteralPath "$env:ProgramFiles\7-Zip\7z.exe") {
        $SevenZipPath = "$env:ProgramFiles\7-Zip\7z.exe"
    } else {
        $SevenZipPath = Join-Path $downloadRoot '7zr-26.03.exe'
        Get-VerifiedDownload -Url $extractorUrl -Path $SevenZipPath -Hash $extractorHash
    }
}
$SevenZipPath = (Resolve-Path -LiteralPath $SevenZipPath).Path
$start = New-Object Diagnostics.ProcessStartInfo
$start.FileName = $SevenZipPath
$start.UseShellExecute = $false
$start.CreateNoWindow = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
# These fixed archive member names select only the two tools and upstream notices.
$start.Arguments = 'x "' + $ArchivePath + '" "-o' + $mediaRoot + '" -y ffmpeg-8.1.2-full_build/bin/ffmpeg.exe ffmpeg-8.1.2-full_build/bin/ffprobe.exe ffmpeg-8.1.2-full_build/LICENSE ffmpeg-8.1.2-full_build/README.txt'
$process = New-Object Diagnostics.Process
$process.StartInfo = $start
try {
    [void]$process.Start()
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw "Media extraction failed: $($stdout.Result) $($stderr.Result)" }
} finally { $process.Dispose() }
foreach ($tool in @('ffmpeg', 'ffprobe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $installRoot "bin\$tool.exe") -PathType Leaf)) {
        throw "Extraction did not produce $tool.exe."
    }
}
[ordered]@{
    installedUtc = [DateTime]::UtcNow.ToString('o')
    sourceUrl = $archiveUrl
    sourceArchiveSha256 = $archiveHash
    license = 'GPL-3.0; see upstream LICENSE and README.txt'
    tools = @(Get-ChildItem -LiteralPath (Join-Path $installRoot 'bin') -File | ForEach-Object {
        [ordered]@{ name = $_.Name; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash; bytes = $_.Length }
    })
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $mediaRoot 'media-install.json') -Encoding UTF8
Write-Host 'Media setup is complete. Restart Toniator using its packaged launcher.'
