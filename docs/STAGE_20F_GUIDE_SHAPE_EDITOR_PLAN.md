# Stage 20F — Guide and Shape Editor Exposure

Status: **Complete at commit
`7117e24b8c9e2e723c3c23e7e9050dc71277d15c`**, implemented from clean
checkpoint `df5dcf5d36ba264c898bc434df3910c54f69480a` (2026-08-14) and accepted by
the user on 2026-08-21.

This bounded contract exposes the accepted Stage 20C–20E2 authored structures,
curved guides, repetition, and closed-shape marks through provisional private
GTK authored-resource editors. It does not change evaluation, persistence, cache, renderer,
or canonical-output semantics. The normative specification remains protected,
and `docs/GREENFIELD_REWRITE_PLAN.md` remains the parent roadmap.

## Authority and public interfaces

### Typed authored-structure uses

`toniator-domain` adds `AuthoredStructureUse::{Guide, Mark}` and
`Document::authored_structure_uses`. A use identifies the stable channel,
definition, and guide or output-layer reference site needed to retarget one
selected use. Results follow deterministic document, mechanism, and output
order. The projection contains no presentation labels, ordinals, prompts, or
other UI policy.

### Private-draft squash

`DocumentHistory` gains an immutable draft-root snapshot and
`squash_draft(&DocumentHistory)`. A draft history created for editing retains
the source document and source revision that formed its base. Squash:

- rejects a draft whose base no longer equals the current main document and
  revision, without changing either history;
- compares the draft's current document, so undone draft entries do not publish;
- treats a draft equal to its base as an unchanged no-op;
- validates the final document before publication;
- publishes the final draft as exactly one main revision and one main undo entry;
- clears the main redo branch exactly as an ordinary successful main edit does;
- leaves resource identifiers as the results of the individual draft commands
  and never synthesizes identifiers during squash; and
- returns a summary with affected channels in deterministic document order and
  the strongest invalidation across the net document change.

The draft remains a normal `DocumentHistory` for typed commands and local undo/
redo. Squash is the only new main-history transition; GTK never installs or
mutates a `Document` directly.

### Reusable `CurvePath` editing

`toniator-geometry::CurvePath` adds validated immutable operations that return a
complete replacement or a stable error, never a partial path:

- move one anchor while retaining the two adjacent cubic control vectors
  relative to that anchor;
- move an addressed cubic control point;
- convert line to cubic with one-third-chord controls, or cubic to line;
- insert a node on a line exactly or on a cubic by exact De Casteljau split;
- delete one node while retaining at least two nodes; open endpoint deletion
  removes its adjacent segment, while interior and closed deletion reconnect
  the retained neighbours; and
- fit that reconnection with fixed samples and equal-arc targets, retained outer
  directions, nonnegative least-squares handle magnitudes, deterministic
  singular fallback, finite/work-limit checks, and no partial result.

The bounded fit independently reimplements the reviewed Bezziator behavior in
Rust. No Bezziator document model, React/Paper.js code, dependency, or dirty
file is copied or imported. The fit must be deterministic, retain required
endpoints and outer directions, and have sampled residual no worse than a
straight reconnection.

## Artist workflow

The existing small control-only modal becomes two adaptive purpose-specific
presentations, each with one private authoritative draft and the complete
canonical draft preview:

- **Grid Pattern Editor** exposes only open **Guide paths**, their construction
  canvas, and Grid-owned guide prototype, arc, repetition, placement, and
  layout descriptors; and
- **Mark Editor** exposes only closed **Mark shapes**, their construction
  canvas, and mark-owned prototype, shape, orientation, fill, and rotation
  descriptors.

Both presentations contain:

- stable presentation ordinals and concise use summaries, never raw IDs;
- a pan/zoom construction canvas with anchors, cubic handles, selected-segment
  highlighting, and screen-space hit testing;
- numeric X/Y controls that expose the same selected geometry operations to
  keyboard and assistive-technology users; and
- persistent Apply and Cancel actions with a horizontal spacious layout that
  switches to a vertically stacked resource-list/canvas layout before a narrow
  window can reduce the canvas to a sliver.

The inspector exposes separate **Edit guide paths…** and **Edit mark shapes…**
actions. Each action describes itself to assistive technology as editing the
authored resources used by the selected channel in a private draft. These are
provisional technical exposure points, not the final Pattern Wizard entry
workflow.
There is no New mark action in Grid Pattern Editor, no New guide action in Mark
Editor, and no Grid/Random family switch in either modal.

Purpose fixes topology. Guide paths and future motif resources stay open; mark
shapes stay closed; every structure retains at least two nodes. There is no kind
toggle, path breaking, multi-resource splitting, node reordering, holes,
multiple contours, freehand fitting, snapping, or import.

### Construction and geometry edits

New construction places line-connected nodes. Enter completes an open guide.
Enter, or clicking the first node, completes a closed mark with an explicit
closing segment. Escape cancels incomplete construction without applying a
draft command or changing draft undo/redo.

For an ordinary selected Grid channel, a new closed mark promotes exactly that
channel's circular output to the authored closed-shape output in the same
private add-and-attach history entry; linked channels retain their old output.
For an already generic guide, an exact selected guide dimension is captured
before drawing. For the ordinary straight Grid, Grid Pattern Editor presents a
modal confirmation before drawing; confirmation stores only a pending private
intent. Completion atomically adds the open resource and retargets only the
selected channel to a fresh generic `GuideDimensions` plus `AlongGuideSites`
definition that preserves coverage and supported mark-output semantics. This
`GuideCustomAlongLayout` convenience remains for Stage 20F compatibility and
is a planned Stage 20S migration: the Pattern Wizard will attach a guide only
to an artist-selected guide use. No
construction, definition, or resource is published before successful
completion, and any failure retains the local construction without an orphan.

Pointer dragging is local preview state until release, when it applies one typed
`ReplaceAuthoredStructure`. Numeric edits commit on Enter or focus leave.
Arrow keys nudge the selected point by one document unit, Shift by ten units,
and Control by one tenth unit.

Selected-segment actions are **Make curve**, **Make line**, and **Insert node**.
Pointer insertion uses the selected segment position clamped to
`0.05..=0.95`; keyboard insertion uses `0.5`. Delete applies the bounded
reconnection operation and is refused at the two-node minimum. Each committed
topology or geometry edit is one draft undo step.

### Shared resources

Before the first mutation of a resource with more than one typed use, the
editor discloses all uses and requires one choice:

- **Edit all uses** retains the shared resource ID; or
- **Make a copy for this use** duplicates the selected resource and atomically
  retargets only the selected guide or mark inside the private draft.

The copy path uses existing `DuplicateAuthoredStructure` plus the existing
selected-copy definition semantics and typed retarget commands. Its grouped
draft action is one user-visible undo step; allocation and retarget failure are
atomic. The disclosure is required only for the first mutation of that resource
within the current editor draft.

### Apply, Cancel, and preview state

**Apply** is enabled exactly when the current draft differs from its immutable
base and has neither an incomplete construction nor local-invalid edit. Draft
preview work may still be pending. Successful Apply squashes the latest valid
draft into main history as one undo step, closes the editor, updates content-
based dirty state, and schedules main preview. A stale or invalid squash leaves
the editor, main history, and draft history intact.

**Cancel** and titlebar close preserve the existing dirty-discard confirmation.
Disabled **Save as Preset…** is unchanged.

Both main and draft preview surfaces expose a visible spinner, status text, and
accessible name **Preview updating** while their newest request is pending. The
last successful image remains visible. Pending is cleared only by the matching
terminal success or failure, explicit cancellation, workspace replacement, or
draft close. A stale completion cannot clear a newer pending request. Fractional
progress is excluded.

## Scope and paths

One `desktop_implementer` owns all executable and test changes. Allowed tracked
paths are:

- `crates/toniator-domain/src/**` and focused tests under
  `crates/toniator-domain/tests/**`;
- `crates/toniator-geometry/src/**` and focused tests under
  `crates/toniator-geometry/tests/**`;
- `crates/toniator-app/Cargo.toml`, `build.rs`, `resources/**`, `src/**`, and
  focused app tests;
- `Cargo.lock` only for the permitted existing-workspace geometry dependency;
- `scripts/validate_architecture.sh` only to allow
  `toniator-app -> toniator-geometry`;
- `docs/STAGE_20F_GUIDE_SHAPE_EDITOR_PLAN.md`, the Stage 20F paragraphs in
  `docs/GREENFIELD_REWRITE_PLAN.md` and `ProgressTracker.md`;
- `.codex-work/semantic-map/USAGE_EVALUATION.md` and Stage 20F checkout-aware
  evidence; and
- derived validation output under `target/validation/stage-20f/` and private
  UI evidence under `.codex-work/evidence/`.

No other tracked file was allowed during the Stage 20F implementation without
stopping for a material contract review. In particular, the Stage 20F code did
not change `Project Specification/**`, `ToniatorLegacy/**`, baseline assets,
IO DTOs, document or preset versions, engine/pattern/render/CLI
implementation, evaluator/cache contracts, or canonical output semantics. A
later user-authorized protected-specification revision defines the future
Stage 20G effective-pattern authority; it does not expand this stage's code.

Every touched non-trivial Rust function, method, and test receives literal
present-tense `///` documentation with relevant authority, invariant, bounds,
side-effect, and error conditions. No compatibility path for obsolete schemas
or presets is introduced.

## Focused verification

### Geometry

Tests cover exact line and cubic insertion, closed-seam insertion, anchor and
control movement, line/cubic conversion, open endpoint/interior and closed
deletion, the two-node minimum, finite/work-limit failures, deterministic fit,
retained endpoints/directions, and fit residual no worse than straight
reconnection.

### Domain

Tests cover deterministic typed use projection; unchanged, stale, and invalid
draft squash; exactly one main revision and one undo entry; affected-channel and
strongest-invalidation aggregation; redo branching; exact resource IDs; undone
draft entries; and grouped duplicate-and-retarget behavior for guide and mark
uses.

### Application

Widget-independent app tests cover purpose filtering, ordinary default mark
promotion, explicit default-guide transition, selected-channel isolation, open
and closed creation, node selection and focus, numeric/pointer parity,
insertion/deletion/conversion, shared-resource choice, incomplete-input
isolation, Apply/Cancel/stale Apply, one-step main undo, and main/draft pending
success, failure, stale completion, cancellation, workspace replacement, and
close behavior.

Run only focused Stage 20F tests plus directly relevant current Stage 20C, 20D,
and 20E2 witnesses. Then run focused package checks, strict affected-package
Clippy, `scripts/validate_architecture.sh`, `git diff --check`, allowlist review,
and protected-path/asset hash checks. Do not sweep obsolete workspace tests or
regenerate historical validation directories.

## Canonical and private-Wayland evidence

Exercise both immutable source artworks at intrinsic dimensions through edited
current-v3 documents. Save and reopen with existing IO, then validate and render
native RGBA PNG plus editable structural SVG under
`target/validation/stage-20f/`. Preserve live text structurally and use
same-process determinism rather than font-sensitive SVG raster hashes. Verify
the baseline asset bytes and the documented HolidayMugs checksum remain exact.

Use the private headless Sway skill for an affected-path run. Collect AT-SPI
structure, numeric values, status, and modal focus; WayVNC pointer and keyboard
gestures; before/after grim screenshots; automation events; logs; and an
evidence bundle. Cover open-guide and closed-mark creation, cubic handles,
insertion/deletion, shared copy/edit-all, Apply, Cancel, main undo, pending and
failure states, narrow layout, and accessible numeric controls. Stop the private
session at handoff.

Automated Sway/wlroots evidence is not human GNOME Shell/Mutter acceptance.
Remaining user review owns pointer feel, focus traversal, visual clarity, and
compositor-specific behavior.

## Review and stop gate

After the one writer completes, run parallel read-only test and UX reviews.
Confirmed findings return to the same writer for bounded repair, followed only
by affected verification and focused re-review.

The implementation stopped uncommitted at **Implemented awaiting review** for
independent test and UX review. Confirmed findings returned to the same writer,
focused repair verification and re-review passed, and the user accepted Stage
20F on 2026-08-21. The accepted implementation is checkpointed at
`7117e24b8c9e2e723c3c23e7e9050dc71277d15c`. This completion does not authorize
publication or Stage 20G implementation.
