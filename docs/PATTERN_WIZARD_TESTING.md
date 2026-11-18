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
latency target. The historical September check took about 50 seconds and remains
part of [TON-003](../ISSUES.md#ton-003--fine-pattern-sizes-have-slow-full-resolution-previews)'s
history. The matched 2026-10-31 finer native evaluator instead measures
97.58 → 76.53 seconds under its exact saved calibrated workload. Different
recipe, build and timing boundaries must not be combined. See
[measured results and remaining costs](PERFORMANCE.md#2026-10-31-personal-gallery-and-dense-pattern-previews).

For a bounded current production Wizard projection of the Poisson/weighted-Lloyd
recipe, use the ignored diagnostic explicitly:

```sh
TONIATOR_TEST_FEATURE_SIZE=0.1 TONIATOR_PROFILE_SOURCE=vector-sample.svg \
  cargo test --release --locked --offline -p toniator-app \
  wizard_workflow_tests::poisson_native_profile \
  -- --ignored --exact --nocapture --test-threads=1
```

Without a calibrated fixture/native flag, this uses the actual Wizard neutral
projection at 128×128. It preserves the recipe's weighted Lloyd toggle, four
iterations and seed 19; it is not a full native Main/Review benchmark. Use
`raster-sample.png` to exercise the other immutable source.

The diagnostic supports these explicit controls:

| Environment variable | Meaning |
| --- | --- |
| `TONIATOR_TEST_FEATURE_SIZE` | Feature size; default 0.1. |
| `TONIATOR_PROFILE_SOURCE` | `vector-sample.svg` (default) or `raster-sample.png`. |
| `TONIATOR_PROFILE_CALIBRATED` | Existing genuine current project: validate source/base binding and reopen, retaining calibration; absent path: prepare through the production calibration path and save there. Separates calibration setup from evaluation. |
| `TONIATOR_PROFILE_CALIBRATION_DONOR` | Optional genuine donor for a new fixture; attachment still requires the existing exact typed binding and production guards. Failure falls back to ordinary preparation. |
| `TONIATOR_PROFILE_PROJECT_OUTPUT` | Save the requested derived current project through ordinary container IO. |
| `TONIATOR_PROFILE_PREPARE_ONLY=1` | Stop after guarded preparation/save; do not evaluate. |
| `TONIATOR_PROFILE_NATIVE=1` | Intrinsic 900×620 SVG or 1024×1024 PNG target. Use with a genuine calibrated fixture for authoritative native evidence. |
| `TONIATOR_PROFILE_WARM=1` | One fresh-cache evaluation followed by one retained-cache evaluation, reported separately. |
| `TONIATOR_PROFILE_OUTPUT` | Directory for untouched RGBA PNGs after evaluator timing; filenames include source/size/target/run. |

Native fine workloads need explicit resource admission before starting them.
Do not treat the small default diagnostic as a memory forecast for native
output: the completed vector 0.1 baseline peaks near 8.1 GiB. The
[2026-10-31 report](../target/validation/ton002-ton003-20261031/report.md)
retains pinned before/current executables, exact command JSON, fresh/warm logs,
Hyperfine/time results and the evidence-owned monitor/driver scripts. Runs are
serial, with owned process/cgroup identity checks, graceful soft-memory and
host-headroom/watchdog stops, finite hard backup and no cgroup swap. Reproduction
requires fresh output/report paths; existing evidence is never overwritten.
The recorded guards are benchmark-only settings, not product limits. Full
native 0.1 uses soft 10/hard 12 GiB only after at least 20 GiB host availability and
retains a 10 GiB reserve. Smaller native cases use the bounds recorded in the
performance report. A graceful stop is a noncompleted case, never an elapsed-time
success. Use the documented installed profiler commands/flags and preserve
optimized debug symbols for profiling; do not compare instrumented timing with
uninstrumented timing.

The ignored `personal_thumbnail_native_profile` is a private-GTK-only heartbeat
diagnostic. It requires the verified owned session and all four disposable XDG
roots from the GTK skill; it must never initialize GTK against the user's
desktop. Its custom gallery timing complements the actual normal-app Wizard
Cancel/reopen/Refresh/current-card checks; it does not replace them. Ordinary
matched records use `TONIATOR_PROFILE_SIMPLE_THUMBNAILS=1`. Report the synchronous
append interval, 5 ms heartbeat window/sample count and cold image completion
separately; a cached warm population must observe at least three heartbeat ticks.

Performance timings depend on hardware. Passing construction or exact-pixel
checks does not establish acceptable interactive latency or user artistic
acceptance. Preserve both immutable assets and their documented font caveat:
SVG live text requires matching font/XDG environment for portable exact-pixel
comparisons. Private Sway evidence does not replace GNOME/Mutter, screen-reader
user or human visual acceptance.

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
