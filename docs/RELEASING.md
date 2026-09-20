# Publishing a GitHub release

Release binaries belong on GitHub Releases. `dist/` and local archives are
ignored and must not be added to Git. The source tag identifies the committed
source used to build the packages. This does not submit the app to Flathub.

## Prepare

1. Review source, icon exports, README, desktop metadata, and release notes.
   Keep all nine first-party Cargo versions, Cargo.lock, AppStream release,
   and package filenames consistent. Use a new version for changed packages;
   never move an existing published tag.
2. Run focused checks for affected behavior and commit the release source.
   Do not include local planning, research, or archived material.
3. Build both packages from that commit using the
   [packaging guide](../packaging/README.md). Verify `dist/build-info.json`
   names the source commit and has an empty packaging Rust diff.
4. Check packaged CLI versions, icons, desktop metadata, relevant output,
   and GUI startup. Run `sha256sum -c SHA256SUMS` from `dist/` after both
   bundles finish. Keep local evidence under `target/validation/`.

## Publish

After publication is authorized, push the source and a new annotated tag.
For version 0.3.2:

```sh
git push origin main
git tag -a v0.3.2 -m 'Toniator 0.3.2'
git push origin refs/tags/v0.3.2
gh release create v0.3.2 \
  dist/Toniator-0.3.2-x86_64.AppImage \
  dist/Toniator-0.3.2-x86_64.flatpak \
  dist/SHA256SUMS dist/build-info.json \
  --verify-tag --draft --prerelease \
  --title 'Toniator 0.3.2 — New application identity and icon' \
  --notes-file docs/releases/v0.3.2.md
```

Review the draft and attachments, then publish:

```sh
gh release edit v0.3.2 --draft=false
```

Download all four published assets into a fresh directory, verify their hashes,
and confirm the release tag resolves to the intended source commit. Retain
prerelease status until a stable release is explicitly chosen.

The same process can be performed through GitHub's Releases page by selecting
the pushed tag, attaching the four files, and using the versioned release notes.
Historical releases retain their original assets and identifiers.
