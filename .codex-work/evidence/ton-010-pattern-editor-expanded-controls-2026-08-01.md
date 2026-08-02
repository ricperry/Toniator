# TON-010 expanded Pattern Editor controls

Date: 2026-08-01
Repository: /home/ricperry1/projects/Toniator
Git HEAD: 262c7e857446ded100d4a90fd23d651e52460665 (dirty worktree preserved)

## Implemented

- Reopening an embedded custom pattern now reads the embedded instance values
  instead of projecting back to default legacy Shapes settings.
- The tracked modal draft exposes X/Y grid modes, X/Y curve bend controls,
  independent X/Y spacing, point-definition mode, C/M/Y/K point samplers,
  unified/per-channel seed scope, unified and per-channel seeds, and jitter.
- Custom definitions switch to the versioned
  shapes.lattice-placement-editor operation. Uniform and source-weighted
  samplers, curved grid offsets, axis spacing, and deterministic jitter affect
  canonical mark placement.
- Repeated execution of a weighted curved-grid custom recipe is deterministic.

## Verification

- cargo test --locked: 249 library tests, 51 binary/UI tests, and doc tests
  passed.
- cargo clippy --locked --all-targets -- -D warnings passed.
- cargo fmt --all -- --check passed.
- Bounded real GTK --demo --show-controls --screenshot launch completed and
  wrote /tmp/toniator-ton010-pattern-editor-expanded-20260801.png.

## Boundary

The modal currently exposes numeric curve-bend controls; an interactive
freehand X/Y curve editor and continuous full-curve path output remain a
separate editor/geometry substage. The current Full Curves point-definition
choice is persisted and routed through deterministic point placement, but the
Shapes mark output remains discrete marks.
