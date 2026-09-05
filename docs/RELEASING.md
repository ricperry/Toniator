# Publishing a GitHub release

A GitHub release attaches downloadable files to a Git tag. Toniator's AppImage
and Flatpak are two assets of the same release; binaries stay out of Git history.
The tag identifies the source used to build them. This does not submit the app
to Flathub or provide a Flatpak automatic-update repository.

## Prepare

1. Review the intended source, README, icons, packaging metadata, and version.
   Commit only the release's explicit paths. Preserve unrelated local work.
2. Ensure the Cargo package versions, desktop AppStream release, and versioned
   output names in the packaging scripts agree. The current recipes target 0.2.0.
3. Run the focused checks for changed behavior, then rebuild the two packages
   from the committed source using [the packaging guide](../packaging/README.md).
   Verify `dist/build-info.json` names that commit and has no packaging Rust diff.
4. Inspect the packaged icons and exercise the actual bundles. Verify checksums
   with `sha256sum -c SHA256SUMS` from `dist/`. Review release notes and limitations.

## Publish with GitHub CLI

The following example assumes the source is committed on `main`, the release
files are verified, and publication is authorized. Authenticate with `gh auth
login` if needed. Never move an existing published tag to a different commit.

```bash
git push origin main
git tag -a v0.2.0 -m 'Toniator 0.2.0'
git push origin refs/tags/v0.2.0
gh release create v0.2.0 \
  dist/Toniator-0.2.0-x86_64.AppImage \
  dist/Toniator-0.2.0-x86_64.flatpak \
  dist/SHA256SUMS dist/build-info.json \
  --verify-tag --draft --prerelease \
  --title 'Toniator 0.2.0' --notes-file docs/releases/v0.2.0.md
```

Review the draft and its four attachments, then publish:

```bash
gh release edit v0.2.0 --draft=false
gh release view v0.2.0 --web
```

Use a new version/tag for a later release and update the paths and notes
accordingly. Keep pre-release status until a stable release is explicitly chosen.
Download the published assets into a fresh directory and check their hashes.

## Publish through the website

Open the repository's **Releases** page, select **Draft a new release**, choose
the pushed version tag, enter a title and release notes, and attach the two
packages plus `SHA256SUMS` and `build-info.json`. Mark it as a pre-release while
Toniator is in development, review the draft, then select **Publish release**.
