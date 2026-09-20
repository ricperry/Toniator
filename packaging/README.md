# Toniator desktop packages

Download the x86_64 AppImage or Flatpak from the
[0.3.2 prerelease](https://github.com/ricperry/Toniator/releases/tag/v0.3.2).
Both include the GUI and headless CLI. Packages, checksums, and build provenance
are release attachments; `dist/` is a local output directory, not tracked source.

## Install or run

### AppImage

```sh
chmod +x Toniator-0.3.2-x86_64.AppImage
./Toniator-0.3.2-x86_64.AppImage
```

If FUSE mounting is unavailable:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 ./Toniator-0.3.2-x86_64.AppImage
```

Use `--cli --help` to invoke the bundled CLI. The AppImage bundles GTK,
dependent libraries, image loaders, icons, and schemas. Host glibc and graphics
drivers remain system-provided. It requires **glibc 2.39 or newer**. Fedora
44/Wayland is the tested development environment; broad distribution compatibility
is not established.

### Flatpak

```sh
flatpak install --user ./Toniator-0.3.2-x86_64.flatpak
flatpak run io.github.ricperry.Toniator
```

The bundle resolves the GNOME 50 runtime through Flathub and may download that
runtime. This application bundle is distributed on GitHub, not listed on Flathub,
and does not provide an automatic-update feed. The CLI is available with:

```sh
flatpak run --command=toniator io.github.ricperry.Toniator --help
```

### Application ID change

Version 0.3.2 uses **io.github.ricperry.Toniator** for the GTK application,
Flatpak, desktop entry, and icons. Version 0.3.1 and earlier used
`com.sbdd.Toniator`. Flatpak treats the new ID as a separate application;
it does not automatically update the old installation or migrate its data.

New private data: `~/.var/app/io.github.ricperry.Toniator/`.
Old private data: `~/.var/app/com.sbdd.Toniator/`.

The release does not move or delete either directory. Keep the old installation
until you have recovered personal Patterns and settings you need. Project files
can be opened from their existing locations if their document format is supported.
Native and AppImage personal data paths are unchanged.

## Permissions and verification

Flatpak grants Wayland, fallback X11, shared IPC, and graphics-device access.
File pickers use the desktop portal; there is no blanket home or network access.
CLI file paths must be accessible inside the sandbox. Personal data is separate
from native/AppImage data. Both packages follow system light/dark preference
through the Settings portal, with native GTK settings as fallback.

Download `SHA256SUMS` with the package, then run:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Private Sway startup checks provide automated launch evidence, not exhaustive
GNOME/Mutter or portal acceptance. See [known issues](../ISSUES.md).

## Rebuild locally

Prerequisites: Python 3, Flatpak, GNOME SDK and Platform 50, Rust/rustup 1.94+,
Cargo dependencies cached for Cargo.lock, binutils, tar, and network access for
pinned packaging tools and media source downloads. The SDK provides GTK,
dav1d, and blueprint-compiler. The scripts do not install apps or dependencies.

```sh
flatpak install flathub org.gnome.Sdk//50 org.gnome.Platform//50
cargo fetch --locked
python packaging/media.py
python packaging/build.py
python packaging/appimage.py
```

`media.py` verifies pinned FFmpeg, SVT-AV1, dav1d, and libvpx sources, then builds
private software media tools. A subsequent unchanged recipe can reuse those
verified tools. The bundles include original corresponding sources, recipes,
and logs; see [media notices](MEDIA-NOTICES.md). Packaged Toniator uses these
tools beside its executable, without a host FFmpeg fallback.

`build.py` compiles locked/offline Rust sources inside the SDK, stages desktop
metadata, and builds the Flatpak. `appimage.py` bundles those SDK-built binaries
and libraries using checksum-pinned appimagetool. Previous staging directories
are retained under `target/packaging/`.

Outputs: `dist/Toniator-0.3.2-x86_64.AppImage`,
`dist/Toniator-0.3.2-x86_64.flatpak`, `dist/build-info.json`, and `dist/SHA256SUMS`.
Build provenance records the source commit, runtime/SDK identities, and icon
hashes. This is a repeatable procedure, not a bit-for-bit reproducibility claim.
Follow [release instructions](../docs/RELEASING.md) to publish.

## Application icon

`assets/icon-final.svg` is the approved editable source. Packages copy
`assets/appicon.svg` unchanged and include `assets/appicon.png` at 512×512.
The AppImage also uses the PNG as its root thumbnail. After an icon edit:

```sh
cp assets/icon-final.svg assets/appicon.svg
inkscape assets/icon-final.svg --export-type=png --export-width=512 --export-height=512 --export-filename=assets/appicon.png
```
