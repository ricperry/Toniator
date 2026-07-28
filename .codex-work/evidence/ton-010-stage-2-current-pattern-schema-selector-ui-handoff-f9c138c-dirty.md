# TON-010 Stage 2 current pattern-schema and selector UI handoff

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c`
- Checkout: dirty; preserve the existing TON-013 Blueprint migration and
  TON-010 Stage 1 changes.
- Producer: read-only `codebase_explorer` (Locke), reconciled by the parent
  on 2026-07-28.

## Verified boundary

- `src/pattern.rs` currently registers only `compat.shapes.v1` and
  `compat.curves.v1`. The registry has stable `PatternId`, family, output kind,
  and opaque versioned values, but no parameter descriptors yet.
- `Document.compatibility_pattern` is a persisted compatibility projection;
  `RenderVariant` remains the authoritative rendering state. Current document
  format is v7 and obsolete definitions are rejected per project policy.
- `src/ui.rs` currently wires the Shapes and Curves toggle buttons directly to
  `activate_shape_treatment` and `activate_curve_treatment`. Those callbacks
  restore or create the existing `RenderVariant` paths, while
  `sync_controls` owns visibility, terminology, mixed values, and deferred
  synchronization.
- `resources/toniator-window.blp` is the current static UI source. The
  existing treatment selector contains Shapes, Squares, Lines, Curves, and a
  hidden Legacy button; only Shapes and Curves have registry entries. Existing
  web/curve target rows are hidden/dynamic and must not become a second scope
  editor. `channel_scope` remains the sole semantic treatment-scope control.
- Blueprint owns static structure; Rust owns models, callbacks, visibility,
  help, drawing, and state. Channel and aggregate Blueprint files are hosts,
  not separate treatment editors.

## Stage 2 implementation boundary

Implement schema descriptors and registry-derived stable selection for the two
existing compatibility patterns only. Reuse the existing RenderVariant
callbacks, mixed-value handling, deferred synchronization, and one-undo
transition behavior. Generate applicable control visibility/help from the
schema without adding algorithms, new families, per-channel assignment, or
obsolete-format migration paths.

## Review and verification focus

Check metadata/schema exactness and duplicate keys; stable selector IDs;
Shapes/Curves setting restoration and inactive-state preservation; unknown or
unsupported definitions failing visibly; one undo on transitions; and GTK
behavior for repeated pattern/scope changes, invalid list positions,
reentrancy, accessibility, focus, narrow layout, and aggregate versus real
channel scope.
