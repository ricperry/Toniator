# TON-010 Stage 4.5: baseline restoration and framework demonstrability

Stage 4.5 is a new review gate inserted after the technically accepted Stage 4
and before Weighted Voronoi. It does not reimplement, invalidate, supersede,
or downgrade Stage 4. It addresses a shape-editing UI regression introduced
during TON-013 and the missing testing presets, fixtures, and manual
demonstrations that were not previously required by the TON-010 tracker.

Stage 5 Weighted Voronoi is blocked until the user explicitly accepts Stage
4.5D. No 4.5 substage starts automatically after another; each substage stops
with a report, parent review, cached evidence, and an explicit user approval
before the next handoff.

## Independent substage gates

### 4.5A — Historical audit and demonstrability plan

Read-only comparison with pre-TON-013 behavior. Produce:

* a complete inventory of shape-editor workflow features lost or changed by the
  TON-013 Blueprint migration;
* owning current and historical files/symbols for each finding;
* a testing-preset matrix covering authority, schema parameters,
  save/reopen, undo/redo, adapters, CMYK/RGB, and preview/PNG/SVG behavior;
* bounded restoration and artifact acceptance criteria.

Stop and request approval. No implementation begins in 4.5A.

#### 4.5A current audit result — 2026-07-28

The read-only comparison found the core shape-editing interaction engine in
both the pre-TON-013 and current Rust paths: rendering, anchor/handle editing,
insertion, deletion, keyboard movement, Escape/cancel, validation, Done, and
undo/redo. The verified change is the static ownership/resource boundary:
Shapes/Curves surfaces moved into GtkBuilder/Blueprint while the dialog and
its drawing/gesture/keyboard controllers remain Rust-owned. Current shape
paths read and write authoritative `Document.pattern_state`.

No user-visible loss is proven by source comparison alone. Current
Blueprint/GResource realization, focus/accessibility, and manual workflow
behavior remain the bounded 4.5B target. The full inventory and matrix are
cached in `.codex-work/evidence/ton-010-stage-4.5a-historical-audit-f9c138c-dirty.md`.
4.5A was approved before 4.5B began; 4.5B is now accepted and 4.5C1 has
completed its first bounded substage.

### 4.5B — Shape-editor restoration

Restore the complete shape-editing workflow in Blueprint and reconnect its
existing behavior. Add GTK regression coverage and visual artifacts for the
restored workflow. Preserve the Stage 4 authority rule: the editor reads and
writes authoritative `Document.pattern_state`, and any Shapes adapter remains
transient and derived.

#### 4.5B current result — 2026-07-28

The existing shape-editor algorithms and local edit state were preserved. The
shipping Blueprint → GResource → `gtk::Builder::from_resource` path now has a
realized GTK regression covering entry discovery/activation, dialog focus,
canvas accessibility metadata, representative insertion, Done, undo/redo,
reopen, Cancel, return to Shapes, and the shipping narrow `OverlaySplitView`
branch. The verified fixes were modal-map canvas focus loss and missing canvas
accessible name/description/tooltip. No duplicate control or algorithm rewrite
was introduced.

#### 4.5B correction — Regular Polygon sides control — 2026-07-28

The existing `web_polygon_sides` control was present in the current Blueprint,
GResource, schema descriptor, and authoritative mutation callback, but its
Blueprint row container was not explicitly retained and synchronized. The
shipping row could therefore remain hidden after selecting Regular Polygon.
`src/ui.rs` now retains the production row and synchronizes its visibility,
label, and spin button from authoritative pattern state. Realized GTK coverage
proves the default square value (4), integer values 3–6, shared and per-target
authoritative edits, contradictory-adapter resistance, and hiding for Circle,
User Defined, and mixed marks. No shape algorithms, editor state, presets,
fixtures, or obsolete-format behavior were changed.

Inspected artifacts are recorded in
`.codex-work/evidence/ton-010-stage-4.5b-shape-editor-f9c138c-dirty.md` and
under `test-artifacts/ton-010-stage-4.5b/`. Automated validation passes; human
GNOME/Wayland pointer, keyboard, and compositor-resized narrow-layout review
remains outstanding. The user accepted 4.5B on 2026-07-28. Its correction
evidence is cached in
`.codex-work/evidence/ton-010-stage-4.5b-polygon-sides-f9c138c-dirty.md`.

4.5B is closed; do not reopen it while 4.5C proceeds.

### 4.5C — Testing presets and observable fixtures

Status: Active — beginning with a bounded current-format matrix and fixture
foundation. C1 and C2A are complete; C2A is paused for user feedback before
C2B. Later 4.5C work remains paused until each substage is reviewed.

Create visibly distinct current-format presets and fixtures proving
authoritative pattern state, schema parameters, persistence, undo/redo,
transient adapters, CMYK/RGB transitions, and preview/PNG/SVG behavior.
Update current definitions and reject obsolete schemas under the project-wide
no-backwards-compatibility policy. Do not add migration or obsolete-format
opening behavior.

Stop for parent review and user approval.

#### 4.5C1 completion record — 2026-07-28

Added the current-format bundled fixtures `Polygon Six` (Shapes, shared
Regular Polygon with six sides) and `Motif Ladder` (Curves, manual flipped
motif arrangement). Both load through the real production preset path and
persist selection and typed schema parameters only in `Document.pattern_state`.
Focused and full validation pass; no visual/export artifacts or later C2/C3
behavior was started. Parent evidence is cached in
`.codex-work/evidence/ton-010-stage-4.5c1-parent-review-f9c138c-dirty.md`.

Pause for user feedback before the next C2 substage.

#### 4.5C2A completion record — 2026-07-28

The C1 fixtures now exercise current-document save/reopen and authoritative
undo/redo through the production preset, editor, persistence, and loader paths.
Shapes and Curves typed edits survive reopen, while undo/redo restores and
reapplies `Document.pattern_state`; saved documents contain no serialized
`render`. Parent validation passes with 140 library and 46 binary/UI tests.
Contradictory adapters and CMYK/RGB transitions remain C2B; preview/PNG/SVG
parity remains C3. Evidence is cached in
`.codex-work/evidence/ton-010-stage-4.5c2a-parent-review-f9c138c-dirty.md`.

At the time of this record, C2B was paused for user feedback; the user later
accepted the correction and authorized C2B-1.

#### 4.5C2 PNG export-background correction — 2026-07-28

The supplied PNG's transparency was consistent with the saved
`ExportBackground::None`; the renderer correctly uses the saved document
background and excludes Preview Surface. The PNG dialog now discloses the
effective saved value or explicitly identifies an override, including
accessible text. Focused PNG/UI coverage and the full 140+47 suite pass. Live
review of the new wording remains for the user. Evidence is cached in
`.codex-work/evidence/ton-010-stage-4.5c2-png-export-background-parent-review-f9c138c-dirty.md`.

Pause for user feedback before C2B.

#### 4.5C2 current-work acceptance — 2026-07-28

The user accepted the current export-background behavior and organization. The
saved `Document.appearance.export_background` remains authoritative, with
explicit transparent `None`, visible saved RGBA for `Color`, and no coupling to
Preview Surface. This clears the bounded authoring/dialog correction and
advances 4.5C2B; it does not claim that the Output section's organization is
final UX.

#### 4.5C2B-1 completion record — 2026-07-28

Parent review accepted the bounded contradictory-adapter proof through the
production `Polygon Six` and `Motif Ladder` fixtures, renderer, save/reopen,
undo/redo, and shipping selector transitions. `Document.pattern_state` remains
the authority and `RenderVariant` remains a derived, non-persisted legacy
executor. The audit found and corrected a Crosshatch entry leak that selected
its source from `Document.render`; it now reads authoritative selection and
typed settings. The remaining adapter inventory and removal boundaries are in
`.codex-work/evidence/ton-010-stage-4.5c2b1-adapter-authority-f9c138c-dirty.md`.

Parent validation passed the focused C2B-1 tests, full locked suite (143
library, 48 binary/UI, 0 doc tests), formatting, and diff checks; the writer
also passed locked all-targets check and strict Clippy. Manual GNOME/Wayland
and screen-reader acceptance remains unclaimed. The user subsequently approved
this handoff and authorized C2B-2 CMYK/RGB transition coverage.

#### 4.5C2B-2 — CMYK/RGB transition coverage

Beginning with C2B2-A, parent review found no CMYK/RGB cache-authority defect.
A production regression now proves that output-model transitions and inactive CMYK/RGB caches
retain authoritative pattern selection and typed parameters, ignore
contradictory transient adapters, preserve each model's Preview Surface and
export-background state, and remain correct through rendering, save/reopen,
undo/redo, and the shipping controls. The active and inactive `render` fields
remain derived compatibility adapters. Validation passes with 144 library,
48 binary/UI, and 0 doc tests plus formatting, all-targets check, strict
Clippy, and diff checks. No manual GTK or screen-reader acceptance is claimed.
Stop here for explicit approval before C2B2-B, the remaining realized GTK
transition surface before C2C.

#### 4.5C2B-2B — Shipping UI transition coverage

Complete and parent-reviewed. The actual Blueprint/GResource `AppUi` surface
was exercised for CMYK/RGB switching. Selector and parameter controls remain
bound to authoritative `Document.pattern_state`; contradictory adapters cannot
alter visible state; and Preview Surface and Export Background remain distinct.
The change is test-only in `src/ui.rs`; no shipping UI defect was found.

Parent validation passes the focused realized GTK test, the full locked suite
(144 library, 48 binary/UI, 0 doc tests), locked all-targets check, strict
Clippy, formatting, and diff checks. No manual GNOME/Wayland or screen-reader
acceptance is claimed. Evidence:
`.codex-work/evidence/ton-010-stage-4.5c2b2b-realized-output-authority-f9c138c-dirty.md`.
Pause here for explicit approval before C2C.

#### 4.5C3-A — Preview/PNG parity fixtures

Complete and parent-reviewed. Use the current-format `Polygon Six` and `Motif Ladder` fixtures
through the production preview and PNG paths. Prove that preview and PNG
consume the same authoritative pattern output, preserve transparency and saved
Export Background semantics, remain deterministic, and ignore contradictory
transient adapters. Create inspectable artifacts and bounded regression
coverage. Stop for parent review before C3-B SVG parity.

The production parity regression proves that transparent preview output and
PNG share the same authoritative pattern pixels; Preview Surface changes only
preview, while saved Export Background changes only document PNG composition.
Contradictory active/inactive adapters cannot alter either output. Four
inspectable artifacts are available under
`test-artifacts/ton-010-stage-4.5c3a/`. Validation passes with 145 library,
48 binary/UI, and 0 doc tests plus locked all-targets check, strict Clippy,
formatting, and diff checks. No SVG or manual desktop/screen-reader acceptance
is claimed. C3-B is now the active handoff.

#### 4.5C3-B — SVG parity and editable artifacts

In progress. Use the same current-format fixtures through the production SVG
exporter. Prove that SVG and preview/PNG share authoritative geometry and
presentation semantics, preserve editable semantic grouping and valid compound
paths/masks, retain transparency and Export Background behavior, and ignore
contradictory adapters. Produce inspectable SVG artifacts and parity
regressions, then proceed to 4.5D as authorized.

Complete and parent-reviewed. The focused regression proves authoritative SVG
geometry, deterministic read-only projection, editable groups/path IDs, cubic
paths, clipping, transparency, Export Background composition, Preview Surface
separation, and contradictory active/inactive adapter resistance for both C1
fixtures. SVG artifacts are under
`test-artifacts/ton-010-stage-4.5c3b/`. The writer hit an external usage-limit
blocker before returning its report; the partial work was preserved and the
parent completed validation and evidence review. Full validation passes with
146 library and 48 binary/UI tests. No manual desktop or screen-reader
acceptance is claimed.

### 4.5D — Integrated readiness review

Complete and parent-reviewed. The parent reconciled 4.5A through C3-B,
inspected the restored shape-editor, preview/PNG, and SVG artifacts, and
completed the integrated authority, persistence, adapter, CMYK/RGB,
presentation, and current-schema review. Final validation passes 146 library,
48 binary/UI, and 0 doc tests plus all-targets check, strict Clippy, formatting,
and diff checks. The C3-B writer usage-limit blocker was preserved and resolved
through parent review without discarding valid work or overlapping reassignment.
Evidence:
`.codex-work/evidence/ton-010-stage-4.5d-integrated-readiness-parent-review-f9c138c-dirty.md`.

No human GNOME/Wayland click-through or screen-reader acceptance is claimed;
realized GTK regression coverage and visual artifact inspection are complete.
Stage 5 remains untouched and requires explicit user approval.

## Orchestration contract

Each 4.5 substage uses a small, bounded assignment with named deliverables,
acceptance checks, and a safe handoff point. Only one writing subagent may be
active at a time. A progressing subagent is not interrupted or replaced merely
because elapsed time has passed; if it genuinely blocks, its completed work
and concrete blocker are preserved before any reassignment. Subagent reports
are evidence; the parent integrates, reviews, and accepts the stage.
