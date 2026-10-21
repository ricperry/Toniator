# Pattern Wizard regression checks

From the repository root:

```sh
bash scripts/test-pattern-wizard
bash scripts/test-pattern-wizard --gtk
```

The first command runs focused model, preview, and construction checks. The
second also exercises production GTK controls and asynchronous workers in a
private headless Sway session, leaving your desktop untouched. It requires the
dependencies listed by `.agents/skills/gtk-wayland-debug/scripts/preflight`.
Use `--gtk-only` to repeat just the widget workflow after a UI change.

Coverage includes:

- Every bundled Pattern's neutral preview, using both immutable source assets.
- All and named-channel materialization, private edits, Apply, and Undo.
- Shared and Mixed effective Feature size readback, including main-panel 0.1.
- Increasing geometry counts and changed pixels at finer Feature sizes.
- Clustered to Poisson selection with relaxation and density settings retained.
- Invalid zero input, correction, Back/Next, superseded preview requests, and
  Cancel while rendering, including Keep editing and Discard confirmation.
- Current-revision construction admission, stale completion rejection, and
  parametric construction failures.
- Scatter ordering, cancellation, and reserved effort fields that must not
  impose normal rendering limits.

The GTK workflow saves raw screenshots under
`target/validation/pattern-wizard-suite/`. Its deadlines fail the test; they
never reduce the requested density or disable relaxation. The final Review
check uses native document geometry, independently of the small preview.
The 90-second Review watchdog detects stalls; it is not an acceptable artist
latency target. Fine Poisson with weighted Lloyd currently takes about 50 seconds
in that check and remains tracked in [TON-003](../ISSUES.md#ton-003--fine-pattern-sizes-have-slow-full-resolution-previews).

For native-size profiling of the reported Poisson/weighted-Lloyd recipe:

```sh
cargo test -p toniator-app wizard_workflow_tests::poisson_native_profile \
  -- --ignored --exact --nocapture
```

`TONIATOR_TEST_FEATURE_SIZE` changes this diagnostic's size (default `0.1`).
Performance timings depend on hardware. Passing construction checks does not
establish acceptable interactive latency or artistic quality. The private GTK
run also does not replace GNOME/Mutter or human visual acceptance.

## 0.4.6 artistic geometry evidence

The scoped 2026-10-19 [Phase 5 report](../target/validation/artistic-geometry-20261019/phase5/report.md)
records the exact focused commands and private GTK witnesses for the new controls;
the general script above is not a claim that it exercises this entire milestone.
Check tone applicability, positive validation, proportional resolved help and
retained Wizard input; candidate versus final connection limits, tree loop state,
minimum distinct-point counts, Apply/Undo/Redo/Cancel and save/reopen. Follow
[the native control contract](GTK_ACCESSIBILITY.md#unpublished-artistic-geometry-controls)
for roles, state, labels, keyboard and semantic readbacks. Do not replace Text
entries with invented finite ranges in automation.

Preparation evidence distinguishes the initial all-14 run (13 passes and Skinny
Curve failure) from the corrected Skinny Curve retry, and the initial affected
gallery run (four passes and tree failure) from its corrected tree retry. It is
not one final-source all-14 refit. Twenty-eight current both-source reopens have
complete authority and explicitly empty preparation plans. Eight final intrinsic
PNG/SVG fixtures use genuine current fitting, separate from earlier synthetic
visual/performance fixtures. Original shipped archives remain preserved.

Advanced Apply automatically prepares missing active/baseline fits before one
history publication. Verify Cancel/failed/stale rejection separately from success;
manual Recalibrate is not a required preliminary action. Live rapid edits establish
coalescing/current acceptance; the focused test establishes stale-ticket rejection.
No package/workspace sweep or old-artifact regeneration is required for this scope.

## 0.4.6 coverage-intent evidence

The coverage fix accepted for 0.4.6 uses the checkout-local
[Stage C verification](../target/validation/offset-coverage-20261019/implementation/stage-c/verification.json)
and [sequential roundtrip follow-up](../target/validation/offset-coverage-20261019/implementation/stage-c-roundtrip/report.md),
rather than an assertion that the general Wizard script covers the milestone.
The follow-up carries one evolving document through zero, reported placement,
combined per-channel X/Y/rotation and restored zero. Eight groups retain strict
saved numeric/profile/binding and All/named/nonempty End responses with no new
fits, exact cold/warm pixels and restored-zero pixels.

The existing **Revolutions** control retains its visible name/hierarchy, text role,
value, enabled state and activation action. Typed recipe policy and descriptors
supply truthful Cover-minimum versus Fixed-finite help. Private AT-SPI evidence
checks the label relation, description, semantic action/readback and committed
keyboard edits 6→5; screenshots and logs cover customization, Apply, placement,
Undo/Redo and save/reopen. Final release-binary screenshots cover the corrected
Round Spiral and the formerly failing combined-ray project. No new control or
parallel accessibility schema is introduced. These are private Sway checks and
parent image inspection, not human GNOME/Mutter or physical-print acceptance.

Supported source-free Preset tests preserve original bytes and authored responses,
convert absent former policy to Fixed, retain explicit generic Cover, drop known
obsolete companions and genuinely prepare the new producer. Unknown profiles and
old ordinary project schemas reject. The corrected current-format reported-project
copy is an explicit offline evidence derivation, not a general project migration.
