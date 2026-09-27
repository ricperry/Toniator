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
