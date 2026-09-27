# Native Windows development build

This is the verified native x64 MSVC development stack for Toniator. It builds the GUI and headless CLI from this checkout. The portable development package section below copies already verified debug executables; it does not build an installer or optimized release.

## Pinned tools and sources

| Component | Verified version and source |
| --- | --- |
| Rust | Rustup-managed `1.94.1-x86_64-pc-windows-msvc`, also pinned by [`rust-toolchain.toml`](../rust-toolchain.toml). The recorded installation used rustup 1.29.1. See the [Rust 1.94.1 release](https://blog.rust-lang.org/2026/03/26/1.94.1-release/) and [official Windows rustup instructions](https://rust-lang.github.io/rustup/installation/windows.html). |
| C/C++ toolchain | Visual Studio 2022 Build Tools 17.14.0, MSVC 14.44.35207, and Windows SDK 10.0.22621.0. Use the [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/vs/older-downloads/) installer with the x64 C++ tools and SDK component described in the [Rust MSVC prerequisite guide](https://rust-lang.github.io/rustup/devel/installation/windows-msvc.html). |
| Python | CPython 3.13.3 x64 at `C:\Python313`; see the [Python 3.13.3 release](https://www.python.org/downloads/release/python-3133/). Its base install is separate from the project virtual environments. |
| GTK | gvsbuild 2026.8.0 from [PyPI](https://pypi.org/project/gvsbuild/2026.8.0/) built GTK 4.22.4, `adwaita-icon-theme`, `dav1d`, and the CPython 3.13 PyGObject/PyCairo wheels. GTK's upstream source archive and SHA-256 are pinned by the [gvsbuild 2026.8.0 recipes](https://github.com/wingtk/gvsbuild/tree/2026.8.0). The verified wheels were PyGObject 3.56.3 and PyCairo 1.29.0. |
| Meson / Ninja | Meson 1.11.1 ([PyPI](https://pypi.org/project/meson/1.11.1/)) and Ninja 1.13.2 ([PyPI](https://pypi.org/project/ninja/1.13.2/)); installed explicitly because gvsbuild does not declare them. |
| AccessKit C | Version 0.18 from [commit `0c52a8ce2357bbeb927f90dc9a1c19c8ec1bd2c3`](https://github.com/AccessKit/accesskit-c/archive/0c52a8ce2357bbeb927f90dc9a1c19c8ec1bd2c3.zip). SHA-256 `F8EE82845DF57E2154F341288954A1618E1E70CD64ADB08F5E7EC01FF65135FE` was computed locally from the retained ZIP in the validation cache; it is not an upstream-published checksum. |
| Blueprint | `blueprint-compiler==0.22.2` in its own venv from [PyPI](https://pypi.org/project/blueprint-compiler/0.22.2/). |
| FFmpeg | Gyan.dev [`8.1.2-full_build.7z`](https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build.7z), with the [published SHA-256 sidecar](https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build.7z.sha256). The verified digest is `0fff188997a499b5382e0f66e845d4556c48c54f0113ebed4853d556dbdd7059`. |
| 7-Zip | Needed to extract Gyan's `.7z` archive; use the [official Windows x64 download](https://www.7-zip.org/). |
| MSYS2 | x64 MSYS2 under the user-local tools root, installed from [MSYS2's official installer](https://www.msys2.org/docs/installer/). The captured setup ran `pacman -Syu` twice; package repository state is rolling. |

The recorded rustup-init SHA-256 is `6F4BEF66261261FCB43131BE8720BAB817D403A09EDEC7455C371974B90BDB7E`; this is the workstation's setup receipt, not a publisher checksum. The official rustup-init URL below is rolling; the managed Rust toolchain is pinned, but the historical 1.29.1 installer binary was not retained. The retained setup records no Python installer or Blueprint wheel checksum. Visual Studio installer media and MSYS2 package snapshots were not retained. These missing receipts and the rolling MSYS2 repository mean the recipe pins tool versions and source patches, but does not claim byte-for-byte rebuilds of every bootstrap package.

## Provision the user-local SDK

Install the listed Visual Studio and Windows SDK components from Visual Studio Installer. Install the Python 3.13.3 x64 installer from the [official release page](https://www.python.org/downloads/release/python-3133/) at `C:\Python313`, with PATH integration disabled. Install MSYS2 x64 from its official installer at `%LOCALAPPDATA%\Toniator\tools\windows-x64-msvc\msys64`. In a PowerShell session, set the user-local locations and create the two isolated Python environments:

```powershell
$tools = Join-Path $env:LOCALAPPDATA 'Toniator\tools\windows-x64-msvc'
$python = 'C:\Python313\python.exe'
$gvsPython = Join-Path $tools 'python\gvsbuild-2026.8.0\Scripts\python.exe'
$blueprintPython = Join-Path $tools 'python\blueprint-0.22.2\Scripts\python.exe'

New-Item -ItemType Directory -Force -Path `
  (Join-Path $tools 'python'), (Join-Path $tools 'cargo'), `
  (Join-Path $tools 'rustup'), (Join-Path $tools 'gvsbuild'), `
  (Join-Path $tools 'media') | Out-Null
& $python -m venv (Join-Path $tools 'python\gvsbuild-2026.8.0')
& $python -m venv (Join-Path $tools 'python\blueprint-0.22.2')
& $gvsPython -m pip install 'gvsbuild==2026.8.0' 'meson==1.11.1' 'ninja==1.13.2'
& $blueprintPython -m pip install 'blueprint-compiler==0.22.2'
```

gvsbuild's package metadata does not declare Meson or Ninja, so install the versions observed in the working SDK explicitly; those executables are used for the AccessKit build below.

Download the x64 MSVC `rustup-init.exe` from the [official rustup instructions](https://rust-lang.github.io/rustup/installation/other.html), then install the repository's Rust toolchain into the same user-local prefix:

```powershell
$rustupInstaller = Join-Path $env:TEMP 'rustup-init.exe'
Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile $rustupInstaller
$env:RUSTUP_HOME = Join-Path $tools 'rustup'
$env:CARGO_HOME = Join-Path $tools 'cargo'
& $rustupInstaller -y --no-modify-path `
  --default-host x86_64-pc-windows-msvc `
  --default-toolchain 1.94.1-x86_64-pc-windows-msvc --profile default
```

Update the MSYS2 installation twice in the same user session, as the verified SDK setup did:

```powershell
$bash = Join-Path $tools 'msys64\usr\bin\bash.exe'
& $bash -l -c 'pacman --noconfirm -Syu'
& $bash -l -c 'pacman --noconfirm -Syu'
```

Download FFmpeg, verify Gyan's published digest before extraction, and unpack it to the exact directory used by the launchers. Install 7-Zip x64 first and make `7z.exe` available on the current process `PATH`:

```powershell
$ffmpegArchive = Join-Path $env:TEMP 'ffmpeg-8.1.2-full_build.7z'
$ffmpegRoot = Join-Path $tools 'media'
$ffmpegHash = '0fff188997a499b5382e0f66e845d4556c48c54f0113ebed4853d556dbdd7059'
Invoke-WebRequest -Uri 'https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build.7z' -OutFile $ffmpegArchive
if ((Get-FileHash -LiteralPath $ffmpegArchive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $ffmpegHash) {
  throw 'FFmpeg archive checksum does not match Gyan.dev.'
}
New-Item -ItemType Directory -Force -Path $ffmpegRoot | Out-Null
$sevenZip = (Get-Command 7z.exe -ErrorAction Stop).Source
& $sevenZip x $ffmpegArchive "-o$ffmpegRoot" -y
if ($LASTEXITCODE -ne 0) { throw 'FFmpeg extraction failed.' }
if (-not (Test-Path -LiteralPath (Join-Path $ffmpegRoot 'ffmpeg-8.1.2-full_build\bin\ffmpeg.exe') -PathType Leaf)) {
  throw 'FFmpeg archive did not produce the path expected by Toniator.'
}
```

Fetch and extract the AccessKit C archive into `$tools\gvsbuild\gtk\x64\release\sources`. Build and install its static archive from an x64 Visual Studio 2022 developer shell with Windows SDK 10.0.22621.0 selected. Then copy the identical archive to the `.lib` name required by the MSVC linker:

```powershell
$commit = '0c52a8ce2357bbeb927f90dc9a1c19c8ec1bd2c3'
$sourceArchive = Join-Path $env:TEMP "accesskit-c-$commit.zip"
$sourceUrl = "https://github.com/AccessKit/accesskit-c/archive/$commit.zip"
Invoke-WebRequest -Uri $sourceUrl -OutFile $sourceArchive
$sources = Join-Path $tools 'gvsbuild\gtk\x64\release\sources'
New-Item -ItemType Directory -Force -Path $sources | Out-Null
Expand-Archive -LiteralPath $sourceArchive -DestinationPath $sources
$accesskitSource = Join-Path $sources "accesskit-c-$commit"
$prefix = Join-Path $tools 'gvsbuild\gtk\x64\release'
$accesskitBuild = Join-Path $tools 'gvsbuild\accesskit-build'
$env:RUSTUP_HOME = Join-Path $tools 'rustup'
$env:CARGO_HOME = Join-Path $tools 'cargo'
$env:RUSTUP_TOOLCHAIN = '1.94.1-x86_64-pc-windows-msvc'
$env:CC = 'cl'
$env:CXX = 'cl'
$env:PATH = "$($tools)\cargo\bin;$($tools)\python\gvsbuild-2026.8.0\Scripts;$env:PATH"
Push-Location $accesskitSource
try {
  & "$($tools)\python\gvsbuild-2026.8.0\Scripts\meson.exe" setup $accesskitBuild `
    --prefix $prefix --libdir lib --buildtype release `
    -Ddefault_library=static -Dtriplet=x86_64-pc-windows-msvc
  if ($LASTEXITCODE -ne 0) { throw 'AccessKit Meson setup failed.' }
  & "$($tools)\python\gvsbuild-2026.8.0\Scripts\ninja.exe" -C $accesskitBuild
  if ($LASTEXITCODE -ne 0) { throw 'AccessKit build failed.' }
  & "$($tools)\python\gvsbuild-2026.8.0\Scripts\ninja.exe" -C $accesskitBuild install
  if ($LASTEXITCODE -ne 0) { throw 'AccessKit install failed.' }
} finally { Pop-Location }
Copy-Item -LiteralPath "$prefix\lib\accesskit-c-0.18.a" `
  -Destination "$prefix\lib\accesskit-c-0.18.lib" -Force
if ((Get-FileHash -LiteralPath "$prefix\lib\accesskit-c-0.18.a").Hash -ne `
    (Get-FileHash -LiteralPath "$prefix\lib\accesskit-c-0.18.lib").Hash) {
  throw 'AccessKit .a and .lib aliases do not match.'
}
```

The retained ZIP's local hash can be checked with `Get-FileHash`; because upstream does not publish that digest, the commit is the source pin. Apply the hash-checked gvsbuild recipe adjustments from the repository root. The first invocation only validates a dry-run; pass `-Apply` to update the venv's two recipe files:

```powershell
.\scripts\windows\apply-gvsbuild-patches.ps1
.\scripts\windows\apply-gvsbuild-patches.ps1 -Apply
```

The patches pin `cargo-c` to `0.10.23+cargo-0.97.1`, expose the user-local Cargo `bin` directory to librsvg's Meson build, and request GTK's AccessKit dependency as static so `Libs.private` (including `ntdll`) reaches the final link. The helper checks exact pre/post recipe SHA-256 values and applies into a scratch copy before writing. It is specific to gvsbuild 2026.8.0 and refuses unexpected recipe contents.

With the x64 MSVC developer environment active, set process-local build variables and run the captured gvsbuild recipe:

```powershell
$env:RUSTUP_HOME = Join-Path $tools 'rustup'
$env:CARGO_HOME = Join-Path $tools 'cargo'
$env:RUSTUP_TOOLCHAIN = '1.94.1-x86_64-pc-windows-msvc'
$env:CC = 'cl'
$env:CXX = 'cl'
$env:PKG_CONFIG_PATH = Join-Path $prefix 'lib\pkgconfig'
$env:PATH = "$($tools)\cargo\bin;$($tools)\python\gvsbuild-2026.8.0\Scripts;C:\Python313;$env:PATH"

& "$($tools)\python\gvsbuild-2026.8.0\Scripts\gvsbuild.exe" build `
  gtk4 adwaita-icon-theme dav1d pygobject pycairo `
  --build-dir (Join-Path $tools 'gvsbuild') --msys-dir (Join-Path $tools 'msys64') `
  --platform x64 --configuration release --vs-ver vs2022 `
  --vs-install-path 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools' `
  --win-sdk-ver sdk-22621 --enable-gi --py-wheel `
  --cargo-opts +1.94.1-x86_64-pc-windows-msvc `
  --extra-opts gtk4:-Daccesskit=enabled
```

Install the generated CPython 3.13 wheels into the Blueprint environment. GI imports also need the GTK DLL directory on the process `PATH` and the typelib directory in `GI_TYPELIB_PATH`; this same local setup passed the Blueprint/resource checks:

```powershell
$pygobjectWheel = Join-Path $prefix 'python\pygobject-3.56.3-cp313-cp313-win_amd64.whl'
$pycairoWheel = Join-Path $prefix 'python\pycairo-1.29.0-cp313-cp313-win_amd64.whl'
& $blueprintPython -m pip install --force-reinstall $pygobjectWheel $pycairoWheel
$env:PATH = "$($prefix)\bin;$($tools)\python\blueprint-0.22.2\Scripts;$env:PATH"
$env:GI_TYPELIB_PATH = Join-Path $prefix 'lib\girepository-1.0'
$env:GSETTINGS_SCHEMA_DIR = Join-Path $prefix 'share\glib-2.0\schemas'
& $blueprintPython -c "import cairo, gi; gi.require_version('Gtk', '4.0'); from gi.repository import Gtk; print(Gtk.get_major_version())"
```

`build.ps1` and the GUI/CLI launchers use this existing local SDK; they do not install it or alter global environment variables. MSYS2's package database can advance, so save an approved package snapshot separately if exact dependency replay matters.

## Build and run Toniator

The checked-in build helper discovers Visual Studio with `vswhere`, selects the x64 MSVC environment and SDK 10.0.22621.0, reads the Rust channel from `rust-toolchain.toml`, and passes GTK, Blueprint, and FFmpeg paths to Cargo in a child process:

```powershell
.\scripts\windows\build.ps1
```

Both products use `--locked --target x86_64-pc-windows-msvc` and write under `target/validation/windows-port/gtk-sdk/product-build/`. The CLI helper passes GTK and FFmpeg DLL paths only to its child process:

```powershell
.\scripts\windows\run-cli.ps1 --help
```

Launch the GUI with the GTK runtime, schemas, GI typelibs, and FFmpeg paths supplied to the child process:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/windows/launch.ps1
```

Add `-ProjectPath "C:\path\to\project.toniator"` to open a project. The app and CLI binaries are `...\x86_64-pc-windows-msvc\debug\toniator-app.exe` and `...\debug\toniator.exe` below the product-build directory.

## Verified behavior and limits

Both locked Windows products build natively with WSL stopped; the Windows build and runtime do not depend on WSL. Native CLI checks cover raster/vector output, Windows/Fedora project reopening, and supplied-video PNG, FFV1, and AV1 workflows. Native Windows UI Automation and screenshots cover startup, keyboard Feature Size editing with Undo/Redo readback, Unicode Save As/Recent Files reopening, raster and vector PNG/SVG export parity, document preset save/load/apply/undo, personal Pattern persistence and apply, and FFV1 export/cancellation. The GUI sequence-video export remains pending because the native foreground guard blocked keyboard input and the GTK menu item exposed no UIA InvokePattern. `Value.SetValue` remains a no-op for the tested GTK entry; cancellation's visible stopped state and UIA progress readback differ. See [Windows/Fedora port status](windows-port-status.md) for current evidence and caveats.

This setup does not provide an installer, signing, or a hosted CI result. The
native source port is accepted, but that does not verify the new 0.3.3 features
on Windows or provide a 0.3.3 binary package.

## Portable Windows development prerelease

`scripts/windows/package.ps1` packages the existing verified GUI and CLI without
rebuilding. It checks their supplied SHA-256 values, computes the native DLL
import closure, copies GTK resources and the release CRT, and produces the
unsigned `Toniator-0.3.2-windows-x64-dev-20260927.zip`. The companion source ZIP
contains retained native sources, original compiled Rust dependency archives
verified against lockfile checksums, AccessKit C, and patched gvsbuild recipes.
Original licenses and attribution are included in the binary package.

Choose a fresh staging root, then assemble and verify it:

```powershell
$releaseRoot = 'C:\path\to\fresh\release-staging'
.\scripts\windows\package.ps1 -OutputRoot $releaseRoot `
  -ExpectedAppSha256 '<verified GUI SHA-256>' `
  -ExpectedCliSha256 '<verified CLI SHA-256>'
.\scripts\windows\package-verify.ps1 -OutputRoot $releaseRoot
```

The verification helper extracts the ZIP into a path with spaces and Unicode,
checks every payload hash, runs CLI help and real renders of both canonical
immutable artwork inputs, and records loaded modules using only packaged/Windows
PATH entries. GUI verification remains a separate native application check.
These checks do not claim clean-machine certification.

After the authorized parent checkpoint commit, finalize only generated metadata
and archives, without rebuilding or changing tracked files:

```powershell
.\scripts\windows\package.ps1 -OutputRoot $releaseRoot `
  -CheckpointCommit '<full 40-character checkpoint commit>'
```

This adds the exact checkpoint source tar to the companion ZIP and records the
commit in `PACKAGE-MANIFEST.json`. Publish both ZIPs together. Draft metadata
explicitly says that its checkpoint is pending and is not for publication.

Recipients extract the entire binary ZIP and double-click `Toniator.vbs` for a
quiet GUI launch, or run `Toniator-CLI.cmd` from a terminal. Launchers resolve
package-relative paths and change only process environments. Windows Script Host
must be enabled for the double-click launcher.

Gyan's GPLv3 full FFmpeg build lacks a retained complete corresponding-source set,
so its binaries are not redistributed. Video workflows require the recipient to
run the explicit included `Setup-Media.ps1`, which downloads the pinned upstream
8.1.2 archive, verifies its published SHA-256, and extracts the tools and upstream
notices locally. If installed 7-Zip is unavailable, setup downloads hash-pinned
official standalone 7zr 26.03. Startup never downloads or installs anything.
See the package README and notices for exact sources, setup options, and limits.
