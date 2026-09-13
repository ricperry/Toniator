# Toniator 0.3 review repair plan

The user authorized implementation of this reviewed three-gate plan on
2026-09-12. Input: `Toniator-0.3.0-notes.md`. Each gate stops for review before
the next begins. Existing unrelated work and the post-acceptance Export menu
split remain intact. Implementation approval does not authorize commits,
packages, publication, or acceptance.

## Gate 1 — Reliable editing and construction admission

- ALL assigns the entered effective scalar value to every compatible channel
  and output at the selected endpoint. Mixed values are explicitly shown.
  Preserve unrelated values, the opposite initialized endpoint, easing, and
  atomic Undo. This supersedes relative-average ALL editing in Stage 22–23.
- Fix response-bias propagation and pending text at Advanced Apply. Present
  each effective output control once; output subgroups are needed only for
  multiple outputs.
- Require successful native-size construction before Wizard Apply, including
  gallery candidates. Preserve bounded Spiral construction; `segment_limit`
  is an internal geometry guard, not a missing document field. Point a failed
  Spiral to Revolutions and keep Apply disabled until the current draft passes.
- Call unconstrained scatter Random. Retain Even/Clustered parameters when
  switching styles within the editing session, including Apply/reopen and
  selected-channel copy-on-edit. Cancel discards private changes; a new recipe,
  project/preset, or main history navigation invalidates remembered context.

Verification: RGB/CMYK mixed and repeated channel edits, Start/End and easing,
paired bounds, private Apply/Cancel, Undo/Redo and persistence; actual native
PNG/SVG/video-frame output; valid and excessive Spirals; Scatter round trip;
GTK semantic actions/readback, screenshots, keyboard path, and process logs.

## Gate 2 — Source response and control organization

Implement levels (black/white points), gamma, centered contrast, per-channel
source assignment/mapping, and small-response suppression. Curves graph editing
is deferred. Advanced focuses on source interpretation. Promote X/Y offset
beside Rotation in the main inspector and use artist-facing Feature size and
Coverage terminology. Keep minimum fill editable and use a 0–1 default range
except documented technical requirements. Persist and animate the eligible
numeric controls through existing document/endpoint authority. Normal controls
edit the selected Start or End; do not introduce duplicate endpoint panels.

Verification: tonal ramps and endpoint interpolation, exact mappings and
suppression boundaries, source/preview/PNG/SVG agreement, save/reopen/Undo,
mixed-channel applicability, and private GTK presentation and interaction.

## Gate 3 — Consistent placement and explicit export timing

The user's subsequent responsibility-boundary update reopens the source-binding
portion of this gate. Pattern recipes own source dependence and geometric
construction, but no source-component selection or source-response shaping. Main-window Advanced owns
independent weighting and fill-response settings in document/channel
state, defaulting to matching components. Applying or editing a Pattern preserves
both bindings and response settings. The user's further clarification also moves
weighting strength/curve/inversion/gain/bias and tonal shaping to Advanced;
the existing fill-source controls remain independent. A weighted recipe retains
only its source-dependence switch and geometric construction/fill bounds. This
supersedes the earlier wizard-owned Default/explicit weighting-source choice.
Verification must prove Green-driven weighting with Red-driven fill, independent
edits and correct cache invalidation, ALL assignment, pattern reuse, Undo/Redo,
current-format persistence and absence of source selectors from the wizard.

Make rotation and X/Y offsets work across all Pattern families. Artwork is
sampled at the transformed pattern positions. Source-weighted placement consumes
the channel's independent Advanced weighting mapping and tonal adjustments;
its canonical default is the matching color component. Explicit Luminance remains
available for monochrome artwork. This supersedes the earlier raw-component
Default behavior; do not silently share a channel-independent weighted site set.

Retain separate Export image… and Export video actions. Expose video duration,
frame count, and rate using one exact project timing authority. Video sources
initialize timing from their source. Timing edits in Video Export update saved
document timing when accepted; cancellation discards pending edits. Preserve
PNG sequences, FFV1/Matroska masters, optional AV1/WebM sharing, prompted/default
directories, and cancellation-safe publication. No scrubbing or GPU work.

Verification: transformed sampling and per-channel registration across families,
shared/per-channel weighted position witnesses, still/video timeline consistency,
export cancellation, saved timing, native outputs, and GTK controls/readback.

## Gate 1 implementation record

The user accepted Gate 1, including its review corrections, on 2026-09-12.
Implementation remains in the working tree; a local checkpoint is pending.
Acceptance closeout changes documentation only and reuses the recorded
verification. Gates 2 and 3 had not begun at that acceptance handoff.
The authorized Addendum amendment defines absolute ALL scalar assignments.
No schema migration or format revision is introduced. Inactive Scatter payloads
are runtime editing memory; `.toniator` continues storing only the active recipe.

The wizard checks both source endpoints using canonical evaluation at native
geometry scale with a discarded 128-pixel raster. Validation is cancellable and
bound to wizard epoch, exact document, revision, and generation. It does not
claim to certify every intermediate video frame or arbitrary future canvas.
Gallery Review and Apply opens Review; Apply Pattern publishes after the check.

A live regression also exposed a panic when replacing a Pattern after a
channel output-response edit. Temporary neutral recipe construction now prunes
foreign output deltas before descriptor projection; final command reset and
remapping semantics remain authoritative.

Evidence and final verification are recorded in the Gate 1 evidence report
under `.codex-work/evidence/`. Subsequent Gate 2 acceptance is recorded below;
Gate 3 has not begun.

### User review clarification, 2026-09-12

ALL is an explicit assignment target for every changed compatible Pattern
setting, including structural choices. Such edits replace that setting's channel
overrides/deltas without replacing untouched source mappings or other settings.
The user specifically confirms that explicit Scatter-style overrides are replaced.
Default artwork spacing selects each channel's matching source component; explicit
component choices remain available. Construction success must not claim readiness
when unchanged, incomplete or otherwise unpublishable draft state disables Apply.

These corrections are implemented and verified in the working tree. Typed ALL
assignments cover compatible descriptor, structural recipe, output-order/filter,
and nested authored-resource edits, preserving untouched settings and compatible
End values. Focused tests include unlinked document bases, heterogeneous channels,
multiple nested edits and atomic Undo. Default remains contextual until channel
evaluation; explicit component choices and the current flat mapping file shape
remain supported. Current correction evidence is recorded separately in
`.codex-work/evidence/review-0.3-gate1-corrections.md`; Gate 1 is user-accepted.

## Gate 2 implementation and acceptance record

The user accepted Gate 2 on 2026-09-12. Implementation remains in the working
tree; no checkpoint or release is created. Gate 3 remains unstarted.

Source mapping now carries black/white levels, gamma, centered contrast and
response cutoff. Response order is inversion, levels, gamma, centered contrast,
existing gain/bias, then source-alpha association for non-alpha components.
Cutoff acts on the completed sampled response; zero disables it and equality
is retained. Marks, strokes and regions suppress below-cutoff contributions
before positive minima, retaining geometry/paint alignment. Cache keys include
all five fields. Bundled response defaults span 0–1.

The five fields join the existing endpoint authority (25 scalar fields total),
including ALL assignment, pending paired levels, atomic Undo, easing and reset.
Black remains strictly below white throughout the transition, checked against
the exact piecewise easing polynomials rather than endpoint ranges alone.
Document schema 9 and document-Preset envelope 2 store the new fields; the
media container stays at 2 and standalone Pattern format at 4. Obsolete formats
are rejected; no user fixtures are migrated.

Advanced retains source interpretation and paint controls. The main inspector
uses Feature size and Coverage, exposes editable minima and X/Y beside Rotation,
and groups controls by output when a named channel has multiple outputs. Normal
controls edit the selected frame; there are no duplicate endpoint panels.

Focused tests cover source mapping and alpha order, suppression boundaries and
paint alignment, all scalar/easing combinations, level tangencies and Hold
transitions, cache reuse/invalidation, current-format persistence, ALL, Apply,
Undo/Redo and endpoint isolation. App/CLI builds, scoped strict Clippy and
architecture checks pass. Native PNG/SVG artifacts and private GTK screenshots
were inspected. The GTK easing-selection attempt has an explicit focus-related
evidence limitation; no successful selection is claimed. See
`.codex-work/evidence/review-0.3-gate2.md` for exact checks, artifacts and logs.
Acceptance is the user's decision, separate from automated Sway evidence.

## Gate 3 implementation and delivered-work acceptance record

The user accepted the gate on 2026-09-12 after the export-timing handoff.
The delivered export-timing slice is accepted. The user subsequently settled the
placement decisions and explicitly accepted their verified implementation and
the source-consumer correction with “Accept the remaining Gate 3 work.” Implementation
remains in the working tree, with no checkpoint or release created.

The export-timing slice is implemented in `temporal_export.rs`. Editable exact
frame rate and duration project a read-only frame count through existing media
timing authority. Video import already initializes project timing from source
metadata. Empty optional first/last fields export the whole selected duration;
explicit subsets retain the prior export interval behavior. A no-op preserves
absolute frame numbering and the original timing representation.

Export applies valid timing to document history as one undoable change, then
exports that accepted snapshot. Close/X before export discards pending edits.
Cancelling a running export cancels output work; its already accepted timing
remains a normal undoable document edit. Input validation and static exporter
checks run before history changes, including existing-output rejection. Stale
workspace, active worker and recovery guards remain authoritative.

Focused tests, app/CLI build, strict Clippy, architecture checks, native output
inspection and private GTK checks cover this slice. Evidence is recorded in
`.codex-work/evidence/review-0.3-gate3-timing.md`. Acceptance closeout reuses this
verification and changes documentation only. Subsequent placement verification
is recorded below.
No schema, codec, package or publication change is introduced.

The placement audit confirms transforms and transformed-position sampling in
non-artwork-weighted families. The user's subsequent clarification permits
artwork-weighted transforms with a warning about offsetting source weighting
alignment; neutral Rotation/X/Y restores alignment. The protected Addendum is
amended accordingly. The user chooses recalculation of weighting after transforming
candidate positions. The later source-consumer correction supersedes recipe-owned
Default with independently configured channel mappings, initially matching each
receiving component. Domain rotation
suppression, command rejection and recipe-change pruning are removed. The native
inspector reports source alignment guidance for non-neutral weighted targets,
including ALL and the selected Start/End frame, and retains it through preview
status updates. Neutral controls clear the notice. Focused domain, pattern and
app tests, strict checks, native PNG/SVG inspection and private GTK action/readback
pass. See `.codex-work/evidence/review-0.3-gate3-placement.md` for evidence and
limits. The completed implementation is now explicitly user-accepted.

## Source-consumer correction verification (2026-09-12)

Implemented the superseding responsibility boundary: the recipe has only the
weighted choice and geometric settings; channel state owns independent fill and
weighting source mappings, tone, and weighting strength/curve. Advanced exposes
the two groups; the Wizard excludes both consumers. Pattern replacement and ALL
recipe edits preserve source choices. Weighting numeric fields follow normal
ALL and Start/End editing, including atomic paired levels and continuous validity.

Current-only persistence is document 10, document-Preset 3, Pattern 5, container 2.
Focused tests cover Green weighting/Red fill, cache identity, inactive retention,
recipe replacement, history, endpoints, source-free Presets and project-as-Preset
validation against both immutable inputs. Native PNG/SVG and private GTK controls
were inspected; production strict Clippy, app/CLI compilation and architecture
checks pass. See `.codex-work/evidence/review-0.3-gate3-source-consumers.md` for
commands, controls, harness corrections and verification limits. The user
explicitly accepts this remaining Gate 3 work on 2026-09-12. All three gates
are accepted awaiting checkpoint; the 0.3.1 implementation/version-update goal
is fulfilled. The subsequent release-preparation request creates source checkpoint
`4e59a5d` and verified optimized AppImage/Flatpak packages. The user subsequently
authorizes pushing main and publishing v0.3.1 on 2026-09-13. Artifact checkpoint
`a5e22fa` is tagged and published; packaging/README.md records the artifact checks.
