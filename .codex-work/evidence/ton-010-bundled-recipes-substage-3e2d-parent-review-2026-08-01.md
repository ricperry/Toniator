# TON-010 bundled recipes — Substage 3E2D parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed Curves emission, complete
  generic execution, limits, tests, and retained production isolation.

## Accepted findings

- `curves.emit-paths` consumes only narrow modulated geometry, requires the
  execution artboard and semantic output channel, and exclusively owns
  enabled/color/opacity/layer construction.
- It returns one canonical `Paths` output with the existing `CurveGeometry`,
  `CurveInkLayer`, and `InkLayer` representation. CMYK and RGB identities map
  through the existing semantic channel adapter.
- Disabled output retains one disabled semantic layer with no outlines,
  matching established Shapes behavior. Crosshatch color and assignment remain
  outside the recipe operation.
- Final emission revalidates the 10,000-outline/4,000,000-command ceilings with
  checked arithmetic and cancellation; it does not regenerate geometry.
- All six Curves native operation bodies now execute through the generic
  registry and return canonical Paths for one semantic channel. No public
  multi-channel orchestrator, consumer routing, or live dispatch is claimed.
- Production retained Curves rendering continues to invoke zero native nodes.

## Verification

- Parent `cargo test --locked curves_native --lib`: 18 passed.
- Parent static production-dispatch scan and `git diff --check` passed. Writer
  full suite passed: 232 library and 48 binary/UI tests, locked release check,
  strict all-target Clippy, format, and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This closes native-body Substage 3E2. Invalidate when Curves operation/runtime
contracts, retained shared geometry helpers, semantic layer representation,
resource policy, orchestration/live dispatch, HEAD, or dirty state change.
