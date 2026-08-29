# Stage 21B — Pattern Wizard and Personal Library

Status: **approved decision-complete plan; Gates 21B-1 and 21B-2 complete and
accepted; Gates 21B-3 and 21B-4 planned and not begun** (2026-08-28). This plan is
subordinate to the protected project specifications, especially the Addendum,
and does not by itself authorize later gates or publication.

Stage 21B follows the accepted Stage 21A checkpoint and the accepted headless
Curve Motif prerequisite. It is one product stage with four separately
reviewed gates, each requiring its own acceptance transition and checkpoint.
The parent owns those transitions. A gate does not authorize the next gate
automatically.

## Authority and boundaries

The domain and headless pipeline remain authoritative. The wizard, GTK, CLI,
gallery, thumbnails, and preset labels project validated typed recipes and
descriptors; they must not create a second pattern model, renderer branch,
topology algorithm, or cache identity. The existing canonical geometry,
source sampling, paint, preview, PNG, and SVG paths remain the shared output
boundary.

The accepted headless Curve Motif prerequisite owns one embedded validated
authored open path, one-guide Along Guides cadence, continuous per-row motif
paths, odd-row mirror/phase composition, and source-driven variable-width
stroke realization. It is not a wizard editor or a reusable closed-shape
store. The Gate 21B-1 registry entry exposes that authority as the
seventeenth built-in, and accepted Gate 21B-2 now projects it in the gallery.

Current-only persistence is document schema v7 and preset format v4; the
container remains v1. Preset-v4 embeds motif geometry and layout. Obsolete v6
documents and v3 presets reject without adapters. The Gate 21B-1 resource and
library formats are version 1. The explicit filesystem writer boundary is one
process at a time: ordinary external changes are detected and refused, but
the library does not claim a multi-process transactional CAS.

The exact implementation checkpoint for Gate 21B-1 is `f77998c`. It includes
registry v3 with 17 built-ins, the `curve-motif-rows` entry displayed as
**Curve Motif** in **Guides**, layered immutable built-in/personal catalog
projection, personal resource storage, safe filesystem operations, the
frontend exhaustiveness correction, focused and independent verification, and
private-Sway smoke evidence. It does not enable **Edit Pattern**.

The exact implementation checkpoint for Gate 21B-2 is `63fd9fb`, user-accepted
on 2026-08-28. All first-party crates declare version `0.2.0`, and
`toniator --version` reports `toniator 0.2.0`.

## Gate sequence

### Gate 21B-1 — Storage and Registry Foundation

**Complete and user-accepted at `f77998c` on 2026-08-28.**

The current-only personal resource format stores authored resources with stable
IDs, strict JSON, bounded regular-file reads, no-follow opening, atomic writes,
and fingerprints. The default data root is `$XDG_DATA_HOME/Toniator`, with
`presets/`, `shapes/`, `motifs/`, `thumbnails/`, and `.trash/`; the versioned
library configuration is under `$XDG_CONFIG_HOME/Toniator`. Candidate roots are
validated before activation or configuration publication.

The library supports safe create/update, duplicate, rename, trash, undo, root
switching, stale-write refusal, warnings, and type-qualified thumbnails.
Paired trash/undo moves roll back on ordinary second-step failure and sync the
affected directories. Personal entries that are malformed, obsolete, or
conflict with a built-in are isolated with nonfatal warnings; built-ins cannot
be overwritten, renamed, or deleted. The catalog keeps built-ins first and
orders valid personal entries by name. Presets, closed shapes, and open motifs
have distinct resource kinds and storage directories. Preset-v4 records embed
the geometry they use, so evaluating or sharing a preset never depends on an
external shape or motif file.

Verification passed for the focused domain, registry, current preset-v4,
personal-library, Curve Motif persistence/evaluation, app compile correction,
strict Clippy, formatting, architecture, diff, rotated registry evaluation,
and private-Sway/apply/Advanced Settings smoke checks, with independent
rereview. The 28-case Curve Motif intrinsic performance/memory evidence
remains valid because this gate did not change evaluator, renderer, or motif
geometry authority. The exact post-primary thumbnail-rename fault branch has
no deterministic fault injector; this is a recorded test gap, not a claim of
multi-process transactional behavior.

### Gate 21B-2 — Wizard Shell and Gallery

**Complete and user-accepted at `63fd9fb` on 2026-08-28.**

The implemented GTK/Blueprint/GResource wizard shell is modal and transient,
with a visual gallery, family breadcrumb, canonical private preview,
**Use as is**, **Edit**, Back, Cancel, and Review/Apply. The gallery and
compact main-window dropdown use the same layered catalog. All 17 built-ins
are cards and support **Use as is**. **Edit** is available for exactly
`curve-motif-rows`, `one-guide-lines`, `even-random-circles`, and
`round-spiral-line`; other cards explain why editing is unavailable. This gate
enables the existing accessibly-disabled **Edit Pattern** control.

Opening the wizard captures its ALL/document or named-channel target and seeds
the private draft from the compact dropdown candidate without first applying
that candidate. **Use as is** materializes the candidate into the draft and
moves to Review. Apply is disabled for an invalid draft or an exact no-op, but
not merely because a newer private preview is still pending. The latest preview
ticket alone may publish; the last successful preview remains visible while a
replacement runs, and Apply/export continue to use full document data.

The private preview uses a 256-pixel-longest-edge source proxy and a 512×512
output with latest-ticket/last-success semantics. Curve Motif uses the exact
stored density-10 canonical SVG presentation adopted for the 17 built-in icon
assets; only personal entries use the synthetic thumbnail fallback. ALL and
named-channel drafts retain the specified delta-reset/named behavior, and one
Apply publishes one history transition. Private Sway/AT-SPI/grim verification
and focused tests cover the shell, gallery, accessible names/state, wide and
narrow layout, and canonical preview. This automated Sway/wlroots evidence is
not manual GNOME/Mutter acceptance; the narrow keyboard-restoration probe
remains inconclusive after WayVNC capture-buffer errors.

One writer owned `crates/toniator-app`, its Blueprint/GResource composition,
and focused app tests for this gate. The Stage 20S source and exact 17 built-in
SVG icons, including the corrected Curve Motif density-10 icon, were adopted
only within this gate; no other asset work was pulled forward. The normal wide
layout presents gallery and preview side by side, while a narrow window stacks
the same semantic groups without changing navigation or draft state.

### Gate 21B-3 — Complete Editing and Nested Editors

**Planned; not begun.** Complete capability/descriptor-driven Edit coverage
for all 17 cards, including structural reconstruction and ordered outputs.
Integrate nested Guide, Shape, and Motif editors with endpoint/seam rules,
reusable-resource insertion, explicit copy-versus-shared behavior, local
undo/redo, and focus restoration. A private draft remains isolated until one
Apply. Independent geometry, history, and UX review runs before the gate stops
for acceptance.

Recipe pages are grouped by authoritative capability and descriptor families,
not by card name. The Curve Motif path is one-guide layout, row spacing and
Along Guides cadence, motif editing, independent odd-row mirror/phase,
source-driven thickness, then Review. Its copy explains that each row has one
connected centerline and that artistically meaningful zero source response may
dissolve portions of the visible stroke.

The shared authored-path canvas serves Guide, Shape, and Motif modes. Nested
history is local; **Apply to Pattern** contributes one parent-draft change and
Cancel restores the exact parent draft. Editing a multiply used personal
resource defaults to **Edit a Copy**, while **Edit All Uses** is explicit.
Guide endpoints remain on the left and right frame edges with numeric parity
and a corner guard. Motif endpoints are pinned to opposite tile-edge centers
and cannot be moved or deleted; interior nodes and Bezier handles remain
editable. New motifs begin in **Smooth direction** mode, linking terminal
handle directions but not their lengths; **Corner** unlinks them. Existing
geometry is never silently smoothed, and no separate seam-mode flag is
persisted when the geometry already determines the result.

### Gate 21B-4 — Personal Management and Final Verification

**Planned; not begun.** Add user-facing save/update, copy, rename, trash,
undo, configurable-root switching, external refresh, conflict handling,
thumbnail behavior, warnings, accessibility, and keyboard workflows. Built-ins
remain immutable. Saving a user preset and applying a pattern remain separate
decisions.

Run the final persistence, filesystem, concurrency, performance, memory, and
UX verification, including both immutable source assets at intrinsic
dimensions, before stopping for acceptance. Final durable documentation is
updated only from verified implementation state.

Personal preset IDs use `user-<uuid>` and remain stable across rename; file
identity follows the stable ID rather than display text. Names are
case-insensitively unique within the combined gallery and within each resource
kind. Duplicate and **Save a Copy** allocate a fresh ID. Delete moves files to
`.trash`, Undo restores them, and no automatic purge policy is introduced.
Built-ins offer **Save as New** only. Personal presets offer confirmed **Save
Changes**, **Save a Copy**, Rename, and Delete; none of those actions implicitly
applies the pattern. A fingerprint mismatch refuses overwrite and offers
Reload or Save a Copy. Switching the configured library root activates the new
root without moving or deleting the old one.

## Product contract

The wizard is a modal transient private draft. It starts from the current
dropdown candidate without applying it to the document. **Cancel** discards
the draft; closing a dirty draft asks for confirmation. **Apply** validates and
publishes exactly one history transition to the invoking document or selected
channel target. A pending preview never changes document state and stale work
cannot replace the current private preview.

The same layered registry serves the compact dropdown and gallery. Built-in
and personal resources are data, not behavior selectors. Pages and controls
come only from the validated capability projection and active property
descriptors. The UI never dispatches on preset names, family labels, or
thumbnail identity.

For document-level **ALL** replacement, the existing contract remains exact:
ordinary base edits preserve compatible pattern-relative channel deltas, while
loading/applying a preset replaces the base recipe and resets those deltas.
Named-channel editing targets the selected channel, creates a selected-copy or
fresh resource set as specified by the effective-resource rule, and prunes only
incompatible deltas. One Undo restores the exact previous intent.

Guide, Shape, and Motif editors are nested private drafts. They preserve exact
open-path endpoints and seam continuity, reject invalid topology or resource
references before acceptance, and return focus to their invoking control.
Reusable insertion makes copy/shared ownership explicit; no editor may mutate
a built-in or an unrelated resource implicitly.

Validation rejects malformed references, invalid or wrong-kind resources,
nonfinite layout values, and incompatible reconstructed controls before draft
publication. Recipe replacement always succeeds at the document boundary by
pruning only deltas that the replacement makes invalid; Undo restores the exact
prior recipe, resources, and deltas.

The Gate 21B-2 preview target is fixed at a 256-longest-edge source proxy and
512×512 output. This is a private-preview fidelity decision, not a new
document Density authority. Production preview, PNG, and SVG continue to use
the ordinary canonical pipeline.

## Verification and exclusions

Retain both immutable inputs: native PNG at 1024×1024 and native SVG at
900×620, inspecting native RGB and alpha separately. Use an isolated release
matrix, 30 quiescent cycles, settled RSS thresholds of 15% and 32 MiB, and a
Curve Motif comparison threshold of 15% RSS and 25% cold time. Performance
measurements are diagnostic only and never become cache or persistence
identity. No application-authored creative upper limit may be introduced;
checked arithmetic, fallible growth, bounded cancellation, and machine
representation checks remain mandatory.

The focused matrix covers all 17 built-in identities and reconstruction,
layered ordering, preset-v4/resource-v1 round trips and rejection, IDs/names,
traversal and symlink refusal, permissions, atomic replacement, stale writes,
root switching, trash/undo, and thumbnail failures. UI gates cover both entry
targets, candidate seeding, ALL reset versus ordinary-delta preservation,
selected-channel copies, page paths, exact Undo/Redo, dirty close, Save versus
Apply, warning/focus/keyboard/accessibility behavior, motif endpoint and seam
rules, deterministic latest-ticket preview publication, and narrow/wide
layouts. The final release matrix evaluates all 17 recipes against both
immutable sources; evaluator RSS is measured separately from exporters, and
native export artifacts are inspected directly. A monotonic-retention result,
settled RSS growth over either 15% or 32 MiB after 30 open/edit/cancel cycles,
Curve Motif RSS growth over 15%, or cold-time regression over 25% requires
investigation rather than an application-authored ceiling.

The following are outside Stage 21B: cloud synchronization, a preset database,
preset import/export, multiple mounted libraries, recovery drafts, Stage 22
media/sequence work, TSP routing, wraparound endpoints, aligned curved-guide
sampling, new topology mechanisms, renderer-specific branches, compatibility
adapters, and any unapproved icon or asset adoption. GTK/Blueprint work,
personal management UI, and reusable closed-shape storage are not retroactively
part of Gate 21B-1.

## Acceptance and Git gates

Each gate uses one writer, focused verification, independent review, and a
parent-owned acceptance transition. Update `ProgressTracker.md` only to the
verified status: Gate 21B-1 is Complete at `f77998c`; Gate 21B-2 is Complete
at `63fd9fb`; Gates 21B-3 and 21B-4 are Planned/not begun; Stage 21B overall
remains In progress. Do not mark the whole stage accepted at Gate 21B-2.

Do not commit, push, publish, or begin the next gate without its explicit
authorization. At closeout, inspect the exact milestone diff, preserve
unrelated worktree changes, run applicable format/check/Clippy/architecture/
diff gates, and record documentation evidence under `.codex-work/`.
