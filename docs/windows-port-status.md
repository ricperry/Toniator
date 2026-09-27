# Windows/Fedora port status

Updated 2026-09-27. The native GUI sequence-video export workflow remains incomplete verification. The Windows and Fedora checks recorded below pass for their stated scope. No user acceptance is claimed.

See [native Windows build notes](windows-build.md) for the pinned toolchain, setup commands, and local build helpers.

## Builds, storage, and portability

The Windows GUI and CLI build natively for `x86_64-pc-windows-msvc` with Rust 1.94.1 and the user-local GTK SDK. The latest app build passed with both WSL distributions stopped. The app uses GLib's Windows data, config, and state roots; native UI Automation and screenshot evidence show the recent-files startup warning is resolved. This root change and the Pattern preview correction add or change no interactive controls.

Windows storage checks pass: 15 native leaf tests and 36 focused IO tests cover private staging, ACL/identity retention, directory flushes, ownership-checked cleanup, collision and sync failures, bounded enumeration, read-only capacity queries, and held-reader replacement. Replacement uses the documented `FileRenameInformationEx` POSIX semantics with no weaker fallback. These filesystem guarantees are evidenced on NTFS and should not be generalized to other filesystems. Sequence durability order remains file sync, frame rename, progress update, manifest replacement, then directory sync. Published video survives a later fatal sync error; Presets keep their post-publication durability-warning policy. Evidence is under `target/validation/windows-port/io-storage/` and `media-integration/`.

Final Fedora verification used a canonical source archive after the Windows GLib-root update and passed both locked product builds, architecture validation, storage/IO, sampling (9 tests), engine (8), CLI (2, including SIGTERM and help), wizard (2), and focused strict Clippy checks for IO, sampling, engine, CLI, and app. The run is recorded in `target/validation/windows-port/final-fedora/fedora-verify-canonical.log`. From an ext4 checkout on Fedora 44 x86_64, run:

```bash
sudo bash scripts/fedora/install-prerequisites.sh
rustup-init -y --no-modify-path --default-toolchain none
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
rustup toolchain install "$(bash scripts/fedora/rust-channel.sh)" --profile minimal --component clippy --component rustfmt
bash scripts/fedora/verify.sh
```

The passing snapshot includes `gawk` and the Rustup path setup. The later explicit `tar`, `gzip`, and `zstd` cache prerequisites received package and syntax checks without a second full-suite run. The Fedora Rustup package provides `/usr/bin/rustup-init`.

Native Windows media checks pass for sampling (9), engine (8), and CLI (3, including cooperative Ctrl+Break cancellation). Windows and Fedora CLI output for both immutable raster/vector inputs has matching native RGBA and rendered SVG structure; the text/font caveat remains. Projects created on either OS reopen on the other. Supplied video checks cover six 64x64 frames at 30000/1001 fps: PNG frames, FFV1, and AV1 are verified, and decoded FFV1 frames match the PNG frames exactly. Exact CLI evidence is under `native-cli/still-20260926-200453/` and `native-cli/media-20260926-200750/`.

Document presets round-trip across Windows and Fedora. Fedora loads a Windows GUI preset, saves/reloads semantic equality, and emits output with SHA-256 `3dd12260e5d2af74d9fb7fa1b241c1e2fcebbd7cc931927f1c147ee8a2b3c49c`; Windows loads that Fedora-written preset and also saves/reloads semantic equality. The two directions are recorded in `final-fedora/linux-preset-roundtrip.log` and `final-fedora/windows-preset-roundtrip.log`.

## Native GUI evidence and limits

The reported Clustered connections preview stall after setting Feature size to 0.5 is repaired in shared geometry. Exact nearest-neighbor search now uses balanced spatial pruning without changing authored density, guard sites, distance/tie rules, or geometry fingerprints. Native CMYK 64x64 reproductions finish in 43.4 seconds for the raster input and 39.7 seconds for the SVG input. The rebuilt native GUI displays the completed 0.5 preview. Sixteen focused adjacency tests pass on both Windows and Fedora; 1,920 native comparisons against the previous implementation match exactly. Native engine cache/cancellation checks, focused strict Clippy, formatting, and locked GUI/CLI builds pass. Evidence is under `target/validation/windows-port/preview-stall-20260927/`. Progress still advances per output, with roughly ten-second gaps on this reproduction. No interactive controls change.

The actual Windows app was inspected with Windows UI Automation, keyboard input, process logs, and screenshots. The workflow in `native-gui/workflows-20260926-35444/` verifies a keyboard Feature Size edit from 3.125 to 6.25, semantic Undo/Redo readback, Unicode Save As and Recent Files reopening, PNG/SVG export parity on both immutable samples, document preset save/load/apply/undo, personal Pattern persistence and application, and FFV1 video export/cancellation. The cancellation destination is absent as required. Source-color-alpha Pattern previews now have valid review state for raster ALL, vector ALL, and vector named Source color; Apply works and document Undo restores the clean project. Screenshots and semantic trees are in `native-gui/wizard-fix-20260926/`.

Two GTK/provider limitations remain recorded. UI Automation `Value.SetValue` is a no-op for the tested GtkEntry, while keyboard editing works. After video cancellation the screenshot shows Stopped, but UI Automation still reads Encoding; the capture/provider cause is not established, so terminal UIA status correctness is not claimed. Production Clippy passes; test Clippy still reports two unrelated existing warnings in `startup.rs` and `temporal_preview.rs`.

GUI sequence export is still pending. The native foreground guard blocked keyboard injection, and a follow-up accessibility-only attempt found no UIA `InvokePattern` on Export video. No keys were sent in that attempt. Its record is `native-gui/sequence-20260926/README.txt`; existing CLI sequence checks and GUI FFV1/cancellation evidence are separate.

The hosted CI workflow is prepared but has not run. The native Windows build is independent of WSL. Passing platform checks do not record user acceptance.

## Windows development package

The Windows executable and live window now use the approved Toniator icon.
Native PE icon extraction and all three `WM_GETICON` requests show the actual
artwork. No interactive application controls change.

The portable development recipe copies the verified debug GUI/CLI, GTK runtime,
icons, schemas, and release CRT without rebuilding. It includes original notices
and a matching source bundle, and supports a final checkpoint SHA in generated
metadata. The package is unsigned and has no installer. FFmpeg is not bundled;
explicit checksum-verified upstream media setup is included. See
[Windows build/package notes](windows-build.md#portable-windows-development-prerelease).

Native staging-package launch with a Windows-only PATH opens a project, updates
the preview, and displays controls and icons. Non-Windows loaded DLLs are under
the package directory, including the CRT. An automated shutdown sequence
involving the main and welcome windows produced an AccessKit unknown-child panic in one
check. Closing Welcome through its own semantic Close action creates an Untitled
workspace; closing that main window then exits normally with code 0, recorded by
a retained native process handle. This is separate from the failing automated
sequence and is not a claim of full stability or clean-machine certification.

The extracted ZIP passes native CLI help, its command launcher, payload hashes,
and real PNG renders of both hash-checked canonical artwork inputs from a path
with spaces and Unicode using package/Windows-only PATH entries. Recorded loaded
modules use only those locations. Explicit media setup passes with the retained
hash-matching Gyan archive and the downloaded, verified standalone 7zr fallback;
both extracted media tools run with a Windows-only PATH. The source ZIP retains
344 observed Rust dependency archives with matching lockfile checksums. Final
checkpoint metadata is injected only after the authorized parent commit.

The final extracted package also passes second-instance project forwarding from
a Unicode path: the second process exits 0, the original window loads and renders
the requested project, and normal main-window closure exits 0 with empty stderr.
The package includes GLib's session-bus and spawn helpers; no SDK paths are needed.
