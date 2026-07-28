# TON-010 Stage 4.5A — parent-reviewed historical audit and demonstrability plan

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `codebase_explorer` / `019faaa2-8d61-7ef2-b178-2c430a87489d`
- Parent reviewer: orchestrator
- Scope: read-only audit; no 4.5B/4.5C/4.5D or Stage 5 work started

## Historical findings

Comparison of pre-TON-013 `546ea4c`, TON-013 checkpoints `ec908d4` and
`f9c138c`, and the current dirty checkout found no proven loss of the core
shape-editing algorithms. The following remain present in current `src/ui.rs`
and were also present before TON-013:

- Bézier shape rendering and closed-path editing;
- anchor selection, anchor translation with handle carry, independent incoming
  and outgoing handle edits;
- double-click curve insertion using cubic subdivision;
- anchor deletion with a three-anchor minimum;
- arrow-key movement in 0.005 increments;
- Delete/Backspace and Escape handling;
- local dialog editing with Cancel/Escape and validated Done commit;
- existing model-level undo/redo and focused shape geometry tests.

Verified TON-013/current changes are ownership and packaging changes:

- static Shapes/Curves surfaces moved from Rust construction into
  GtkBuilder/Blueprint resources;
- the current dirty worktree deletes legacy XML resources and uses Blueprint
  compilation plus GResource registration;
- the shape-editor dialog, drawing area, gestures, keyboard controller, and
  commit/cancel behavior remain programmatic;
- post-Stage-4 shape reads/writes use authoritative `pattern_state` and
  `set_shape_settings`, rather than the transient adapter.

The reported regression is therefore not proven as a lost algorithm by this
read-only comparison. The concrete unresolved regression boundary is current
Blueprint/GResource runtime realization and observable workflow behavior,
including whether any static surface, focus/accessibility relation, or action
path was omitted during the resource transition. This is the bounded target
for 4.5B; no compatibility migration is proposed.

## Lost/changed-feature inventory

| Surface | Pre-TON-013 | Current state | 4.5B check |
|---|---|---|---|
| Shape treatment controls | Rust-built in `build_editor_view` | Blueprint objects in `resources/toniator-window.blp` | Realized object IDs, visibility, labels, and callbacks |
| Mark/mixed/polygon controls | Rust-built | Blueprint static surface, Rust models/callbacks | Shape mode and dynamic visibility in GTK |
| User-defined mark action | Rust button and callback | Blueprint `web_edit_shape`, Rust callback/help | Open action, focus, help, and authority path |
| Shape canvas/dialog | Rust-owned in both checkpoints | Still Rust-owned | Open, draw, focus, and modal behavior |
| Anchor/handle gestures | Present before TON-013 | Present in `connect_shape_editor_click` and drag controller | Edit and redraw visibly |
| Insert/delete/keyboard | Present before TON-013 | Present in key/click controllers | Double-click, Delete, arrows, Escape |
| Cancel/Done/undo | Present before TON-013 | Present with authoritative commit | Cancel leaves no edit; Done is one undoable edit |
| Resource packaging | Checked-in GtkBuilder XML | Blueprint compiler + GResource in dirty tree | Build/load current resources and screenshot |
| Static help/accessibility | XML labels plus runtime help | Blueprint labels/help hosts plus Rust descriptors | Inspect names, descriptions, focus order |

## 4.5C/4.5D demonstrability matrix

| Requirement | Existing evidence | Required observable example/check | Gap |
|---|---|---|---|
| Authoritative selection and typed schema | `PatternDocumentState`, registry, Stage 4 tests | Shapes/Curves selection and typed parameters drive controls | No consolidated visible preset |
| Shape persistence | persistence/preset loaders | Save/reopen multi-anchor custom shape and compare handles | No dedicated current-format fixture |
| Undo/redo/cancel | model and geometry tests | Commit, undo/redo, then cancel a second edit | No focused end-to-end GTK workflow test |
| Adapter projection/contradiction | Stage 4 model/UI tests | Contradict adapter and compare UI/render/export authority | No observable artifact |
| CMYK/RGB | pipeline/UI tests | Paired visibly distinct CMYK/RGB shape artifacts | Missing paired fixture |
| Shapes/Curves transitions | transition tests | Switch Shapes → Curves → Shapes and inspect state/artifacts | Missing visual pair |
| Preview/PNG/SVG parity | broad canonical/export tests | Same custom shape compared across all three outputs | Missing shape-editor-specific fixture |
| Current-format examples | bundled current presets | Visibly distinct current schema presets/fixtures | 4.5C work |
| Manual workflow | automated realized GTK and historical artifacts | Edit, insert/delete, cancel, reopen, apply, undo/redo, export | Current Blueprint/Wayland review absent |

Existing artifact/demo inputs include `--edit-shape`, `--curved-shape`,
`--independent-shapes`, `--screenshot`, `--save-document`, `--save-treatment`,
`--export-svg`, `--export-png`, preview/export-background options, and the
`curved_shape_fixture`/`install_*_shape_fixture` helpers.

## Proposed bounded acceptance checks

4.5B must restore and realize the complete current Blueprint shape surface,
preserve the programmatic editor interactions and authority path, add realized
GTK coverage for open/edit/insert/delete/cancel/apply, and produce a current
visual artifact. 4.5C must add only current-format, visibly distinct presets
and fixtures covering every matrix row, with obsolete schemas rejected.
4.5D must run the full automated suite, perform manual Fedora GNOME/Wayland
workflow review, inspect focus/accessibility and artifacts, reconcile evidence,
and stop for explicit approval before Stage 5.

## Verification

- Explorer read-only comparison and report reviewed by parent.
- `blueprint-compiler lint -r syntax` passed for all three current `.blp`
  resources.
- `cargo check --locked --all-targets` passed.
- `git diff --check` passed.
- No screenshots, presets, fixtures, implementation files, or manual workflow
  artifacts were created in 4.5A.

## Uncertainty and invalidation

Current Blueprint/GResource GTK realization, current screenshots, and the
precise user-visible regression remain unverified until 4.5B/4.5D. Invalidate
this record after changes to `src/ui.rs`, `src/model.rs`, `build.rs`,
`resources/*`, presets/fixtures, persistence/export code, Git HEAD, or the
dirty worktree baseline.

