# Installing Toniator

Download **one** of the x86_64 Linux packages from the
[0.4.6 prerelease](https://github.com/ricperry/Toniator/releases/tag/v0.4.6).
Both include the graphical app and CLI, with application ID
`io.github.ricperry.Toniator`.

**Upgrading to 0.4.6:** ordinary `.toniator` projects require current schema 19.
Older projects, including the 0.4.5 gallery projects, are rejected. Keep 0.4.5 to
reopen those projects, or open their source artwork in 0.4.6 and create a new
project. Schema 19 projects from the unpublished app remain valid even if that
app displayed version 0.4.5. Supported older source-free Document Presets retain deterministic
conversion with their original bytes preserved. See [format details](USING_TONIATOR.md#046-project-and-preset-formats).

**Upgrading from 0.3.1 or earlier:** the Flatpak ID changed from
`com.sbdd.Toniator`. The new Flatpak installs separately and uses a new private
data directory. Existing settings and personal Patterns are not moved or deleted.
Your `.toniator` projects remain ordinary files and can be opened from their
existing locations if they use a supported format. Keep the old installation
until you have recovered any settings or personal Patterns you need.

## AppImage

In the folder containing your download:

```bash
chmod +x Toniator-0.4.6-x86_64.AppImage
./Toniator-0.4.6-x86_64.AppImage
```

The AppImage's recorded minimum glibc version is in the attached
`build-info.json`. Fedora 44/Wayland is the tested development environment;
broad distribution compatibility is not established. If FUSE mounting is
unavailable, run it with:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 ./Toniator-0.4.6-x86_64.AppImage
```

## Flatpak

With Flatpak installed, run:

```bash
flatpak install --user ./Toniator-0.4.6-x86_64.flatpak
flatpak run io.github.ricperry.Toniator
```

The bundle uses the shared GNOME 50 runtime, which Flatpak may download during
installation. It is distributed here on GitHub; it is not a Flathub listing.
Flatpak keeps its own settings, recent files, and personal Patterns under
`~/.var/app/io.github.ricperry.Toniator/`, separate from a native or AppImage installation.

Download `SHA256SUMS` alongside the packages to verify their integrity with
`sha256sum --ignore-missing -c SHA256SUMS`. See the
[packaging guide](../packaging/README.md) for permissions and build details.

## Windows builds

Windows packages are distributed separately; check the
[release assets](https://github.com/ricperry/Toniator/releases) for their matching
versions. The [Windows build instructions](windows-build.md) and
[0.4.5 handoff](releases/v0.4.5-windows-handoff.md) retain their historical scope.
This Linux release does not establish native Windows verification of 0.4.6.
A 0.4.6 Windows package requires a separate native build and verification from
the exact peeled `v0.4.6` source commit, with its own package provenance.
The checked-in packaging scripts still describe the historical 0.3.2 debug ZIP;
they require a separately verified recipe for a current release package.

The [historical Windows development release](https://github.com/ricperry/Toniator/releases/tag/v0.3.2-windows-dev.20260927)
is an unsigned portable 0.3.2 build with no installer. Its video tools require the
optional `Setup-Media.ps1` download. Extract the complete ZIP, double-click
`Toniator.vbs` for the GUI, or run `Toniator-CLI.cmd` from a terminal. The guide
records tested behavior and known limits.

Return to the [first artwork walkthrough](../README.md#make-your-first-artwork).
