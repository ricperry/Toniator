# Publishing a GitHub release

Release binaries belong on GitHub Releases. `dist/` and local archives are
ignored and must not be added to Git. The source tag identifies the committed
source used to build the packages. This does not submit the app to Flathub.

## Prepare

1. Review source, icon exports, README, desktop metadata, and release notes.
   Keep all ten first-party Cargo versions, Cargo.lock, AppStream release,
   and package filenames consistent. Use a new version for changed packages;
   never move an existing published tag.
2. Run focused checks for affected behavior and commit the release source.
   Do not include local planning, research, or archived material.
3. Preserve byte copies of earlier `dist/` artifacts and shared metadata before
   rebuilding. Build both packages from that commit using the
   [packaging guide](../packaging/README.md). Verify `dist/build-info.json`
   names the source commit and has an empty packaging Rust diff. Do not use
   `--skip-build` for a release. Finish Flatpak and AppImage before the final
   `packaging/check-artifacts.py` check and checksum verification.
4. Check packaged CLI versions, icons, desktop metadata, relevant output,
   and GUI startup. Verify the JPEG XL dependency notices and provenance are
   present under `share/licenses/io.github.ricperry.Toniator/jpegxl/` in both
   bundles, and exercise still/animation decoding from each package.
   Run `sha256sum -c SHA256SUMS` from `dist/` after both
   bundles finish. Keep local evidence under `target/validation/`.

The existing Windows development ZIP is a separately versioned 0.3.2 package
built from its verified debug binaries. Do not attach or relabel it as a 0.4.6
Windows build; a current Windows package needs its own native build and checks.

## Publish

After publication is authorized, push the source and a new annotated tag.
For version 0.4.6:

The prior 0.4.5 release notes remain available at
[docs/releases/v0.4.5.md](releases/v0.4.5.md).

```sh
git push origin main
git tag -a v0.4.6 -m 'Toniator 0.4.6'
git push origin refs/tags/v0.4.6
gh release create v0.4.6 \
  dist/Toniator-0.4.6-x86_64.AppImage \
  dist/Toniator-0.4.6-x86_64.flatpak \
  dist/SHA256SUMS dist/build-info.json \
  --verify-tag --draft --prerelease \
  --title 'Toniator 0.4.6 — Artistic controls and background preview' \
  --notes-file docs/releases/v0.4.6.md
```

Review the draft and attachments, then publish:

```sh
gh release edit v0.4.6 --draft=false
```

Download all four published assets into a fresh directory, verify their hashes,
and confirm the release tag resolves to the intended source commit. Retain
prerelease status until a stable release is explicitly chosen.

The same process can be performed through GitHub's Releases page by selecting
the pushed tag, attaching the four files, and using the versioned release notes.
Historical releases retain their original assets and identifiers.

## Append a Windows build later

The tracked [0.4.5 Windows handoff](releases/v0.4.5-windows-handoff.md) is
historical; do not use its source/version commands for 0.4.6. A later 0.4.6 build
must use exactly the peeled `v0.4.6` source commit and compare it with that Linux
release's `build-info.json`; do not move the tag or relabel the old debug ZIP.
Native Windows release build, portable-package smoke checks and complete notices
are required before attachment. Current Windows packaging scripts remain tied
to the historical 0.3.2 debug package and are not current release automation.
If a source fix is needed, stop for a new checkpoint/version decision.

Append distinct versioned Windows assets to the existing release, with
`WINDOWS-SHA256SUMS` and `Windows-build-info.json`. Never replace Linux
`SHA256SUMS`, `build-info.json`, AppImage or Flatpak assets; do not use upload
`--clobber`. Preserve historical releases and prerelease status.
