# TON-012 final closeout evidence

- Repository: `/home/ricperry1/projects/Toniator`
- Feature branch before closeout commit: `refactor/ton-012-artwork-pipeline`
- Pre-closeout HEAD: `4161635d90ee81421ffa1f2dc52e2a381d18c6d7`
- Acceptance: user accepted the Stage 5 manual gate in the closeout request.

## Final verified architecture

- `Document.artwork_pipeline` is the authoritative source, alpha, output,
  assignment, and semantic-channel state.
- Prepared sources and resolved channel fields feed Shapes, Curves, preview,
  PNG, and SVG.
- CMYK and RGB channels are semantic identities; RGB Curves contain Red,
  Green, and Blue only.
- Crosshatch remains the temporary progressive K/C/M/Y compatibility boundary.
- Preview Surface is model-specific display state; Export Background is explicit
  export state.
- Current v6 projects and v4 presets are supported; obsolete pre-release
  snapshots are rejected rather than migrated.

## Verification

- 117 library tests passed.
- 43 binary/UI tests passed.
- Strict Clippy with all features passed.
- Locked release build, formatting, diff checks, desktop-file validation, and
  AppStream validation passed.
- Final coredump check found no Toniator coredumps; `coredumpctl` returns 1
  when the result set is empty, which is expected for this no-coredump result.
- The user reported/accepted normal exit and no GTK critical during the manual
  Stage 5 gate.

## Scope and deferred work

- Retained legacy facade adapters remain for legacy-settings callers; current
  document render/export paths use semantic pipeline entrypoints.
- TON-014 Source-Sampled Mark Colors is Planned and deferred.
- TON-008 remaining non-pipeline work, TON-009 DTF, TON-010 pattern framework,
  TON-011 Advanced Pattern Mixing, and TON-013 GtkBuilder/Cambalache remain
  separate planned work.
- No later issue implementation was started.
