# TON-010 Pattern / Channel Control Boundary

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Timestamp: 2026-08-01
- Producing agent: `/root`
- Scope: present pattern selection and structural editing separately from channel modulation controls without changing the authoritative recipe/rendering boundary.

## Working-tree boundary

The checkout was already dirty with TON-010 implementation, generated assets, presets, documentation, and an unrelated `ISSUES.md` draft. This substage changed only the current UI/native recipe files and its evidence/cache records; no reset, clean, commit, or push was performed.

## Verified implementation

- The visible Pattern Settings section now exposes one `Pattern Preset` dropdown (`Grid / Marks`, `Curves`, `Weighted Cells`, and the selected `Custom Pattern`) plus Load Preset, Save Preset, and Edit Pattern actions. The old Shapes/Curves/Weighted Voronoi toggle strip and Legacy Crosshatch action are hidden from the normal surface.
- Pattern Settings is placed before Channel Settings in the realized inspector. Channel Settings now owns the channel scope, per-channel controls, and the existing channel-targeted treatment panels. The output model remains the only visible RGB/CMYK mode selector.
- Pattern Editor remains local-draft based and cancel-safe. Its structural controls now include X/Y grid modes and spacing, explicit connected/disconnected geometry choices, jitter, and a persisted curve-function dropdown (Sine, Square Wave, Spiral). The selected curve function is a typed recipe parameter consumed by the bounded native editor lattice operation.
- Channel Settings exposes both a unified random seed for all included channels and individual per-channel seed controls; the unified edit is one undoable authoritative pattern-state edit and synchronizes embedded recipe output-channel values.
- Editing context and labels use creator-facing `Grid Pattern` / `Curve Pattern` language rather than presenting those recipes as application modes. Compatibility callbacks remain available for existing fixtures and documents but are not visible as mode buttons.

## Commands and results

- `cargo test --locked --no-fail-fast` — passed after the unified-seed addition: 251 library tests, 53 binary/UI tests, 0 doc tests.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo check --locked --release` — passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `cargo fmt --all && cargo check --locked` — passed.
- `timeout 30s cargo run --locked -- --demo --show-controls --screenshot /tmp/toniator-ton010-pattern-controls-20260801.png` — real GTK launch completed without Adwaita critical output; screenshot visibly showed Pattern Settings above Channel Settings, the Pattern Preset dropdown, and channel controls.

## Unresolved uncertainty / manual gate

Manual GNOME/Wayland acceptance is still required: open Edit Pattern, change curve function and connected/disconnected geometry, Apply, re-enter and confirm persistence, then choose a channel scope and verify density/fill/rotation/sampler/seed changes alter the canvas. Per-channel selection of different recipe definitions, broader Gaussian/blue-white-pink/Poisson site constructors, and a dedicated modal curve-shape authoring dialog are not yet implemented by this substage.

## Invalidation

Invalidate this entry if the inspector hierarchy, Pattern Editor draft schema, or Shapes editor operation changes, if HEAD changes without reconciling the dirty files, or if a manual GNOME/Wayland check disproves the screenshot/runtime observations.
