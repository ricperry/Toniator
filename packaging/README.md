# Toniator desktop packages

Download the AppImage or Flatpak from the
[v0.2.0 GitHub release](https://github.com/ricperry/Toniator/releases/tag/v0.2.0).
For maintainers, [release instructions](../docs/RELEASING.md) cover publishing
the two bundles and their checksums together under one version tag.

Local x86_64 packages of version **0.2.0** use **com.sbdd.Toniator** as the
application/desktop ID. The internal GResource prefix and project/Pattern formats
stay unchanged. Both formats include the GUI and headless `toniator` CLI.

Both packages use the author's exported `assets/appicon.svg` as the scalable
desktop icon and include `assets/appicon.png` at 512×512. The AppImage also uses
the PNG as its root icon/thumbnail. Builds copy these exports unchanged and record
their hashes; `assets/ToniatorIcon.svg` remains the editable Inkscape source.

## Install or run

From the repository directory, run the AppImage directly:

```sh
chmod +x dist/Toniator-0.2.0-x86_64.AppImage
./dist/Toniator-0.2.0-x86_64.AppImage
```

It needs no application installation. If FUSE mounting is unavailable:

```sh
APPIMAGE_EXTRACT_AND_RUN=1 ./dist/Toniator-0.2.0-x86_64.AppImage
```

The AppImage bundles GTK, its dependent libraries, image loaders, icons, and
schemas. Host glibc and graphics drivers remain system-provided. This build
requires **glibc 2.39 or newer** and was tested on Fedora 44/Wayland; broader
distribution compatibility is not claimed. `--cli --help` invokes the bundled CLI.

Install the Flatpak bundle for your user account:

```sh
flatpak install --user ./dist/Toniator-0.2.0-x86_64.flatpak
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

Prerequisites: Python 3, Flatpak, GNOME SDK and Platform 50, Rust/rustup with
Rust 1.94 or newer, Cargo dependencies cached for Cargo.lock, binutils, tar, and
network access for the pinned AppImage packaging tool/runtime. The SDK supplies
GTK, dav1d, and blueprint-compiler. The scripts do not install dependencies or apps.

If the SDK/runtime are missing, install them through your configured Flathub remote:

```sh
flatpak install flathub org.gnome.Sdk//50 org.gnome.Platform//50
cargo fetch --locked
python packaging/build.py
python packaging/appimage.py
```

The first script compiles locked/offline sources inside the GNOME SDK using the
host's installed Rust toolchain, stages desktop metadata, exports a local OSTree
repository, and writes the Flatpak bundle. The second bundles SDK-built binaries
and libraries and runs checksum-pinned appimagetool 1.9.1. Previous generated
staging trees are retained under `target/packaging/` on rebuild. No source checkout,
user library, installed app, Git state, or remote repository is modified.

Outputs are in `dist/`: the two bundles, `build-info.json` with source/runtime
provenance, and `SHA256SUMS`. Verify them with `sha256sum -c SHA256SUMS` from dist.
The AppImage runtime comes from AppImage's official type2-runtime release; the
final artifact checksum records the actual assembled file. This is a reproducible
build procedure, not a claim of bit-for-bit reproducible archives across SDK updates.

## Verification for this build

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
