---
name: toniator-orchestrator
description: Coordinate focused Toniator greenfield design, exploration, implementation, and verification through project custom subagents.
---

# Toniator Orchestrator

Own integration in the parent thread. Read relevant greenfield evidence before
choosing an agent, validate it against Git HEAD and the worktree, and select
only specialists whose output materially reduces uncertainty.

Read `docs/GREENFIELD_REWRITE_PLAN.md` and `ProgressTracker.md` before scope.
The parent updates the tracker at stage start, implementation review, user
acceptance, and commit. Routine tracker maintenance does not require the
documentation maintainer; roadmap or plan changes require explicit user
approval and synchronized plan/tracker updates.

When a stage exercises source loading, sampling, rendering, preview, or
export, include both project-wide inputs from `assets/` in its acceptance
scope. Preserve their bytes, keep derived output under `target/validation/`,
and apply the SVG live-text/font rule in `assets/README.md`.

## Rendered-output acceptance

Any stage that produces, changes, or can materially affect Toniator-rendered
output requires visual inspection before the parent may mark the work verified
or ready for user acceptance. This applies equally to headless engine work,
CLI output, and GUI-driven output.

If Toniator produces a `.png` during validation, the parent orchestrator must
inspect that PNG visually.

If Toniator produces an `.svg`, validation must also produce a rasterized PNG
rendering of that SVG and the parent orchestrator must inspect the PNG
visually. Structural SVG inspection, XML validation, geometry assertions,
successful parsing, or tests passing do not substitute for inspecting the
rendered result. Retain the original SVG alongside the inspection PNG.

When engine or rendering changes can affect both PNG and SVG output, exercise
the applicable export paths and visually inspect representative PNG evidence
for them. Use both project-wide inputs from `assets/` when the affected path
supports them, plus any focused fixture or edge case needed to exercise the
change.

Require rendered validation artifacts under `target/validation/`. The writer
must identify the exact artifacts produced and the conditions that generated
them. The parent must open and inspect the relevant PNG artifacts itself;
writer or reviewer statements that output looks correct are evidence only and
do not satisfy this requirement.

During inspection, compare the rendered result with the assigned intent and
look for visible defects including missing or duplicated geometry, incorrect
clipping, seams, discontinuities, unexpected branches, gaps, overlaps, wrong
transforms, malformed curves, border artifacts, incorrect density or
placement, incorrect sizing, unexpected off-canvas geometry, compositing or
stacking errors, color errors, and regressions in affected output.

Tests, numeric assertions, descriptors, logs, writer reports, reviewer
reports, and successful export are supporting evidence, not visual
acceptance. A stage that affects rendered output is not verified until the
required PNG evidence has actually been inspected by the parent.

Headless-only implementation is never an exemption when its output is
renderable. If engine or CLI work produces or influences Toniator PNG or SVG
output, apply this rendered-output acceptance gate even when no GTK code is
involved.

## GUI acceptance

Route every stage that changes or reviews `toniator-app`, GTK behavior, preview
presentation, keyboard/focus/input, accessibility exposure, or a UI regression
through `$gtk-wayland-debug` by default. Require a bounded affected-path run
with semantic AT-SPI state, relevant WayVNC input, before/after grim
screenshots, logs, and an evidence bundle; stop the private session at handoff.

For work that changes GUI-visible behavior, the parent must also inspect the
relevant captured screenshots before verification. Automated semantic state,
interaction success, logs, and tests do not substitute for inspecting the GUI
result when the GUI itself changed.

Do not require GUI screenshot inspection merely because headless engine or CLI
work was performed. For engine/rendering work, rendered PNG inspection is the
primary visual gate. Apply the GUI screenshot gate when GTK presentation or
GUI behavior is itself within the affected scope.

Skip `$gtk-wayland-debug` only for demonstrably headless-only work or behavior
the harness cannot run. Treat automated Sway/wlroots results as strong
evidence, never as human manual review or GNOME Shell/Mutter acceptance.
Request manual desktop inspection only for unreproducible or specifically
human-, Mutter-, portal-, or compositor-policy-dependent risk.

## Stage execution

Use one writer for one short bounded stage. The parent settles the stage
decision, grants exact paths and acceptance checks, including required rendered
outputs and GUI evidence when applicable, then reviews the writer's report and
raw evidence before handing off any later stage.

Writers must generate the required validation artifacts but must not
self-certify visual correctness on behalf of the parent. A report is evidence,
not approval. The parent performs the final acceptance check against the
assigned intent, tests, raw artifacts, and required visual evidence. Do not
start the next stage automatically.

Use `codebase_explorer` only for a specific unresolved path after cache review.
It explores greenfield first; Legacy is a read-only, explicitly named quarry.
Use `desktop_implementer` for bounded writing and routine verification. Use a
UX or test reviewer only when the actual risk warrants independent review.
Use `documentation_maintainer` only after a verified milestone materially
changes durable documentation.

The greenfield core, CLI, and GTK app remain hard boundaries: no headless crate
depends on GTK/libadwaita or a frontend; `toniator-engine` is the future shared
pipeline boundary; canonical geometry is shared by preview and exports. Do not
use Legacy as architecture authority or add broad compatibility layers.

Keep cache entries under `.codex-work/` generation-marked and checkout-aware.
Never write, build, test, or format `ToniatorLegacy/`. Do not edit protected
normative specification files unless the parent explicitly assigns a
specification revision. Do not commit, push, publish, deploy, or delete.
