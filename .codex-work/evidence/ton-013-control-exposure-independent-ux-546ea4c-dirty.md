# TON-013 control-exposure independent UX review

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Working tree: dirty; review was read-only and preserved existing application, resource, documentation, evidence, and artifact changes.
- Verdict: correction required before accepting the control-exposure stage.

## Verified findings

The Builder boundary is genuine: Source, Output, Appearance, treatment buttons/actions, and the three treatment-stack hosts load from `ToniatorEditorControls.ui` and bind into the live inspector without duplicate visible controls. The inspector order is Source -> Output -> Channel Settings -> Appearance -> Treatment, with Source and Output expanded by default and later groups collapsed. The aggregate-vs-real-channel boundary remains sound: Treatment Editing Scope is the only visible treatment-recipient selector, hidden legacy selectors remain projections, and aggregate context is separate from real semantic channels.

Two corrections are required:

1. The new static visual labels for Artwork Source, Source Alpha, Output Model, Channel Assignment, and Active Channel lack explicit accessible names or `labelled-by` relations. Add relations/names and a realized regression test.
2. The Basic/native panel's visible Sampling Detail, Coverage, Contrast, and Screen Angle rows are still assembled in Rust, but current documentation describes only Shapes/Curves/Motif rows as remaining Rust-built. Either move those native row shells to Builder or explicitly include them in the boundary. Since they are stable visible controls and the user wants maximum practical exposure, move their shells.

The supplied screenshot confirms Source/Output and the beginning of Channel Settings, but does not show Appearance or Treatment chrome below the fold. A follow-up expanded capture and focus/accessibility check are required after correction.

## Inspected paths and invalidation

Inspected `resources/ui/ToniatorEditorControls.ui`, `ToniatorInspector.ui`, channel/aggregate resources, `Toniator.cmb`, `src/ui.rs` Builder construction and synchronization, TON-013 docs/issues, and `test-artifacts/ton-013/control-exposure-stage.png`. Invalidate after changes to those files, GTK/libadwaita accessibility behavior, the artifact, Git HEAD, or dirty-worktree assumptions.
