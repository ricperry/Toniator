# Toniator desktop packages

Download the AppImage or Flatpak from the
[v0.3.1 GitHub prerelease](https://github.com/ricperry/Toniator/releases/tag/v0.3.1).
Both packages, checksums and build provenance were published on 2026-09-13.
For maintainers, [release instructions](../docs/RELEASING.md) cover publishing
the two bundles and their checksums together under one version tag.

Local x86_64 packages use **com.sbdd.Toniator** as the
application/desktop ID. The internal GResource prefix and project/Pattern formats
stay unchanged. Both formats include the GUI and headless `toniator` CLI.

Both packages use the author's exported `assets/appicon.svg` as the scalable
desktop icon and include `assets/appicon.png` at 512×512. The AppImage also uses
the PNG as its root icon/thumbnail. Builds copy these exports unchanged and record
their hashes; `assets/ToniatorIcon.svg` remains the editable Inkscape source.

## Install or run

From the repository directory, run the AppImage directly:

```sh
chmod +x dist/Toniator-0.3.1-x86_64.AppImage
./dist/Toniator-0.3.1-x86_64.AppImage
```

It needs no application installation. If FUSE mounting is unavailable:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/Toniator-0.3.1-x86_64.AppImage
```

The AppImage bundles GTK, its dependent libraries, image loaders, icons, and
schemas. Host glibc and graphics drivers remain system-provided. This build
requires **glibc 2.39 or newer** and was tested on Fedora 44/Wayland; broader
distribution compatibility is not claimed. `--cli --help` invokes the bundled CLI.

Install the Flatpak bundle for your user account:

```sh
flatpak install --user ./dist/Toniator-0.3.1-x86_64.flatpak
flatpak run com.sbdd.Toniator
```

Flatpak resolves the GNOME 50 runtime through the included Flathub runtime
repository hint. It may need to download that shared runtime on another machine.
Use `flatpak run --command=toniator com.sbdd.Toniator --help` for the CLI.
These are local bundles, not a Flathub submission or an automatic-update feed.

Flatpak grants Wayland, fallback X11, shared IPC, and graphics-device access.
File pickers use the desktop portal; there is no blanket home access or network
permission. Its personal Patterns/configuration/history are separate from native
or AppImage data, under `~/.var/app/com.sbdd.Toniator/`. Existing native personal
Patterns are not copied or moved by installing the bundle. Project files remain
portable between formats. Choose a custom Pattern folder explicitly if needed.
Both packages read light/dark preference through the desktop Settings portal,
including live changes; native GNOME/GTK settings provide the fallback.

## Rebuild locally

The current checkout builds **0.3.1**, producing
`dist/Toniator-0.3.1-x86_64.AppImage` and `dist/Toniator-0.3.1-x86_64.flatpak`.
The download/install examples above refer to the published 0.3.1 prerelease.
Building locally does not publish or replace its release assets.

Prerequisites: Python 3, Flatpak, GNOME SDK and Platform 50, Rust/rustup with
Rust 1.94 or newer, Cargo dependencies cached for Cargo.lock, binutils, tar, and
network access for the pinned AppImage packaging tool/runtime. The SDK supplies
GTK, dav1d, and blueprint-compiler. The scripts do not install dependencies or apps.

If the SDK/runtime are missing, install them through your configured Flathub remote:

```sh
flatpak install flathub org.gnome.Sdk//50 org.gnome.Platform//50
cargo fetch --locked
python packaging/media.py
python packaging/build.py
python packaging/appimage.py
```

The media script verifies pinned FFmpeg, SVT-AV1, dav1d and libvpx sources and
builds software tools offline in the SDK. Both packages carry these private tools,
their original corresponding sources, recipe and build logs. See
[media provenance and notices](MEDIA-NOTICES.md). Packaged Toniator resolves the
tools beside its executable, without host FFmpeg fallback.

The build script compiles locked/offline sources inside the GNOME SDK using the
host's installed Rust toolchain, stages desktop metadata, exports a local OSTree
repository, and writes the Flatpak bundle. The second bundles SDK-built binaries
and libraries and runs checksum-pinned appimagetool 1.9.1. Previous generated
staging trees are retained under `target/packaging/` on rebuild. No source checkout,
user library, installed app, Git state, or remote repository is modified.

Outputs are in `dist/`: the two bundles, `build-info.json` with source/runtime
provenance, and `SHA256SUMS`. Verify them with `sha256sum -c SHA256SUMS` from dist.
The user-requested 0.3.0 acceptance checkpoint includes these four files. Both
package files use Git LFS; after cloning, run `git lfs pull` to download their
contents. Build provenance points to the preceding accepted source checkpoint.
Repository inclusion does not publish a GitHub release.
The AppImage runtime comes from AppImage's official type2-runtime release; the
final artifact checksum records the actual assembled file. This is a reproducible
build procedure, not a claim of bit-for-bit reproducible archives across SDK updates.

## Development media verification

### Published 0.3.1 packages

Both bundles use optimized, stripped release binaries built inside GNOME SDK50
with locked/offline dependencies and private media tools from source checkpoint
`4e59a5d7394155869d26fb983c5d1fbe8dcfc7a4`. dist/build-info.json records that source,
an empty Rust packaging diff, SDK/runtime identities, icon hashes and checksums.
The AppImage is 117729784 bytes and requires glibc2.39; Flatpak is 62528352 bytes.
The pinned media recipe verifies bundled executables and corresponding sources.

Current verification under target/validation/release-0.3.1/ exercises the actual
AppImage and an isolated Flatpak installation. Both CLIs report0.3.1; their
immutable PNG/SVG outputs and current source-mapping project renders match byte
for byte. Two supplied video frames survive FFV1 encoding/decoding exactly and
AV1 export succeeds with host FFmpeg/ffprobe names masked. Native raster output,
SVG inspection and mapped project output were visually inspected without flattening.
Extracted AppImage icons match tracked exports; the executable is stripped.

Private GTK runs ui-run-20260912-235710-234531 and
ui-run-20260912-235807-235509 verify AppImage preview/keyboard Feature size editing
and Flatpak preview/channel selection. Screenshots were inspected; no GTK critical
or Rust panic occurred. The AppImage wrapper logs expected termination at restart.
Flatpak numeric keyboard automation could not verify focus and is not a successful
edit witness. Temporary private-bus/file permissions apply only to the harness;
shipping permissions are unchanged. The private session is stopped. Checksums pass.
These are automated Sway checks, not new GNOME/Mutter or portal acceptance.

### Earlier 0.3.0 media verification

Both locally built 0.3.0 packages pass the media checks recorded in
`target/validation/stage22-packaged-media/1788671372198098377/`. The actual AppImage
and an isolated test-installation Flatpak render identical native 1024×1024 PNG
and 900×620 SVG outputs. All ten supplied 1080×1920 video frames round-trip through
FFV1 exactly, as do two frames with fractional alpha and hidden RGB. Optional
AV1/WebM encoding passes with an explicit opaque matte. Native artifacts are
visually inspected; AV1 is not claimed lossless.

Private-tool selection is tested independently: moving-media work succeeds with
host tool names masked, while relocating the packaged CLI without its private
tools fails without publishing output. Media dependency logs contain only SDK
system/compression libraries. No host codec extension is copied into the bundles.
The test Flatpak adds scoped file access for CLI fixtures; production permissions
are unchanged. Desktop portal-grant persistence and GNOME/Mutter behavior remain
separate acceptance checks. Those 0.3.0 artifacts were subsequently published.

## Verification for the published 0.2.0 build

- Installed the actual Flatpak bundle into an isolated test installation;
  both packaged CLIs report version 0.2.0.
- Both packages render the immutable PNG at 1024×1024 and SVG at 900×620;
  the corresponding exports are byte-identical. Native PNG and a rasterized SVG
  were visually inspected. The source SVG's font caveat remains in assets/README.md.
- Private GTK workflow checks cover startup, native Open, preview completion,
  dirty Close Cancel/Save, saved-project Recent Files, reopening, clean Close,
  and Exit. Parent inspected startup, preview, and saved-project screenshots.
- A private Settings portal verifies initial dark preference and live light/dark
  changes; a read-only call through normal Flatpak permissions also reads the actual
  host's dark preference. The portal-value test and strict app Clippy pass.
- Desktop/AppStream metadata validate. No product controls or domain semantics
  were changed for automation. The author's SVG and PNG package icons are included.

Evidence/scripts are in `target/validation/packaging/` and the indexed private
bundles. These are automated Sway/wlroots tests, not human GNOME/Mutter acceptance.
File-dialog testing uses `GDK_DEBUG=no-portals` and temporary test permissions to
keep dialogs/accessibility inside the private compositor. The shipping Flatpak
retains normal portal permissions. Native fallback chooser logs include GVFS/mount
warnings in that isolated environment; actual GNOME portal file dialogs were not
visually tested. No app crash or failed document operation occurred.

References: [Flatpak bundles](https://docs.flatpak.org/en/latest/single-file-bundles.html),
[sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html),
[Settings portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html),
[AppImage packaging](https://docs.appimage.org/packaging-guide/from-source/native-binaries.html).
