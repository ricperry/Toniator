---
name: toniator-astra-orchestrator
description: Preferred GPT-6 Astra parent orchestration for bounded Toniator engineering, refactoring, verification, and development-tool maintenance. Routes specialists by task risk and evidence while preserving domain authority and visual acceptance.
---

# Toniator-Astra-Orchestrator

Own the requested outcome, settled engineering brief, integration, and final
verification. Work directly when delegation costs more than it saves.
Use specialists for concrete independent investigations or meaningful review.
Optimize successful completed work per cost, quota, latency, and correction
burden; neither minimum token price nor maximum model capability is the goal.

Repository paths below are relative to `/home/ricperry1/projects/Toniator`.
This skill is preferred over `toniator-orchestrator`, which remains available.
Do not load both orchestrators routinely.
The skill expresses routing intent; it does not switch the parent model.
Run the parent on the user-selected GPT-6 Astra configuration.

## Authority and scope

Apply system/runtime requirements first. Within project instructions, use:

1. Current explicit user instructions and their retained authorization.
2. Protected product/architecture specifications, with Addendum precedence.
3. Current accepted stage or goal contract.
4. Repository `AGENTS.md`.
5. This orchestrator.
6. Specialist skills.
7. Historical workflow notes and cached evidence.

The five files under `Project Specification/` are protected normative inputs.
Do not revise them without explicit specification-revision authorization.
An implementation brief, new mockup, or tooling task is not that authorization.
Repair stale lower-priority tooling instructions when they impede current work.
Do not reinterpret a tooling exception as permission to change product semantics.

Before writing or delegating, establish the outcome, affected subsystem,
current gate, relevant authority, intended behavior, and invariants to preserve.
Inspect Git status and relevant existing edits before selecting exact paths.
Preserve unrelated dirty/untracked files and user-owned artifacts.
Read the relevant sections of `docs/GREENFIELD_REWRITE_PLAN.md` and
`ProgressTracker.md`; distinguish approved intent from implemented status.
Verify checkpoint claims against Git and checkout-matching evidence.
If the plan's status summary is stale, use the accepted checkpoint and tracker
to establish history without silently changing the approved plan.

Honor planning-only requests, narrow allowlists, and parent-defined stop gates.
Do not start a later product stage from an earlier handoff.
Answer or diagnose without implementing unless the user authorized changes.
Continue all safe independent work when one decision remains unresolved.
Ask only about material product semantics, artistic intent, destructive
choices, cost, or UX alternatives that available authority cannot resolve.

An unrelated defect belongs in a concise deferred finding. Fix it only if it
blocks correct completion and the fix is within the user's authorized scope.
Do not expand ordinary tasks into broad audits, modernization, warning cleanup,
dependency upgrades, speculative features, or general documentation rewrites.

Never build, test, format, or write `ToniatorLegacy/`.
Read it only for an explicitly named quarry task, never as architecture authority.
Do not commit, push, tag, publish, deploy, delete implementations, or discard
unrelated work without the required explicit authorization.
Tooling maintenance authority does not grant those operations.

## Hydrate only the affected context

Search relevant `.codex-work/` evidence before broad exploration.
Validate its generation, relevant files, dirty-state assumptions, and provenance.
A different HEAD calls for checking the relevant diff, not automatic repetition
of every earlier check. Reuse unchanged evidence with its limits stated.
Source, specifications, and tests outrank stale summaries or tool output.
No cached entry substitutes for user acceptance or a real checkpoint.

Use the smallest sufficient navigation path:

- `rg --files` to find modules, tests, resources, and manifests.
- `rg -n` for definitions, references, command variants, and consumers.
- Narrow reads around matches, including callers and relevant tests.
- `git diff`, `git log -- PATH`, and `git show` for change provenance.
- `cargo metadata --no-deps --format-version 1` for workspace structure.
- `cargo tree -p CRATE` for resolved dependency direction when needed.
- rust-analyzer definitions/references/call hierarchy when exposed by the host.

Text matches are candidate references, not proof of resolved call relationships.
Trace actual dispatch, trait implementations, feature gates, generated code,
and tests when those details determine impact.
An empty search is not proof that a behavior or dependency is absent.
Do not claim an LSP interaction merely because rust-analyzer is installed.

Semantic-map is retired for Toniator. Do not invoke it, refresh its cache,
require its reports, or maintain an inefficiency log for not using it.
Only explicit user reactivation changes that decision.
Leave the separate semantic-map project untouched.
Historical plans, reports, and cache entries remain historical evidence.
Their semantic-map requirements do not apply to current workflow.

Do not install a graph index merely to replace the retired tool.
The initial navigation choice is `rg` + Git + Cargo, with available LSP support.
Reconsider codegraph-rust or another maintained tool only for a demonstrated
navigation gap; compare setup/index cost, correctness, useful queries,
maintenance, licensing, and measured time/context saved on the real task.
Validate the actual tool on one representative path before adopting it.
Do not create a new navigation skill when ordinary tools suffice.

## Settle ownership before substantive changes

For a refactor, identify authoritative state, module/crate boundaries, callers,
data flow, invalidation/cache reuse, persistence, undo/redo, and preview/export
consumers. Define the destination and intended semantic differences.
Preserve one authority for each concept.
Do not stack adapters or duplicate fields around obsolete implementation.
Differentiate accepted contracts, provisional implementation, verified defects,
technical debt, and unimplemented capabilities.
Accepted semantics deserve stability; provisional inefficient code can change
when the task and evidence justify it.
Do not redesign accepted architecture based solely on stylistic preference.

Preserve Toniator's domain and pipeline boundaries:

- Document/domain commands own authored state and validation.
- The domain resolves effective base/channel values before evaluation.
- GTK and CLI project/edit the same authority; neither invents effective values.
- Immutable snapshots may feed evaluators; revision/ticket checks guard results.
- Patterns/geometry provide canonical construction for preview, PNG, and SVG.
- Renderers/exporters consume that construction without Pattern-name/ID branches.
- Canvas clipping belongs to final consumers, not topology construction.
- Invalidation reports the narrowest correct layer; preserve unaffected reuse.
- Cancellation, deterministic ordering, and atomic publication remain intact.
- Headless crates and CLI never depend on GTK or a frontend.
- GTK4 belongs in `toniator-app`; do not reintroduce libadwaita.

For ALL editing, ordinary base edits preserve compatible channel deltas;
Pattern replacement has the explicit reset semantics in the Addendum
(currently described there with older "preset" terminology).
Keep named-channel edits, source mapping, appearance, and history boundaries
consistent with the current domain contract.
Widgets, renderer state, and cache metadata must not become parallel authorities.
Do not regenerate geometry for an unrelated presentation property.

Toniator is pre-release: current schemas and explicit porting decisions govern.
Reject obsolete formats instead of adding migration/compatibility by default.
Do not remove implementations just because compatibility is unnecessary;
obtain the deletion authorization required by repository governance.
On Rust edits, document every touched non-trivial named function/method/test
with literal `///`: present responsibility, authority, invariants, bounds,
side effects, and applicable Errors/Panics/Safety conditions.
Do not turn this on-touch rule into a repository-wide documentation pass.

## Library-first and performance decisions

Before implementing or substantially extending a nontrivial algorithm,
explicitly consider mature open-source implementations.
Prioritize this for geometry, spatial/graph operations, curves, polygons,
clipping, sampling, image processing, and delicate numerical code.
Existing custom code earns no preference merely by already existing.
Evaluate directly involved slow, fragile, complex, or duplicated machinery.
Do not conduct a repository-wide library hunt.

For a meaningful candidate:

1. Define the exact Toniator semantics and relevant boundary requirements.
2. Research current primary documentation and inspect the candidate API/source.
3. Check license compatibility, maintenance, documentation, and dependency weight.
4. Compare determinism, numerical behavior, geometry/output semantics, and bounds.
5. Estimate integration/removal cost and measure representative performance
   when performance is part of the justification.
6. Prefer the dependency when it materially reduces defects, complexity,
   maintenance burden, or runtime while satisfying the required contract.

Prefer compatible permissive licenses such as MIT, Apache-2.0, or BSD.
Do not replace trivial clear code with a dependency or introduce one for
abstraction alone. Record decisive tradeoffs briefly.
An attractive package description is not evidence of suitability.
Do not claim a speedup without measurements or similarly concrete evidence.
Distinguish intended geometry changes, numerical differences, and regressions.

For hot paths, check algorithmic complexity, recomputation, invalidation,
allocations, conversions, indexes, cancellation granularity, and shared caches.
Parallelize evaluation only where existing ordering and publication contracts
permit it. Prefer algorithmic improvements to speculative micro-optimization.
Leave cold code alone without evidence.

Use abstraction for real domain boundaries, substitution, or useful reuse.
Avoid one-use frameworks, generic registries duplicating schema authority,
unnecessary service/factory layers, and speculative plugin infrastructure.
Treat Toniator as a local creative desktop app. Handle malformed input,
files, Rust errors, bounds, failed exports, and user data correctly when touched.
Do not invent enterprise/network security infrastructure for hypothetical use.
Retain existing concrete persistence and isolation safeguards.

## Delegate selectively, with one writer

The parent owns the final interpretation, settled brief, and integration.
Use at most one writing agent at a time, including the parent.
Parallel read-only tasks must answer independent questions.
Do not ask multiple agents to rediscover the same code.
Explicitly identify allowed paths, outcome, authority, acceptance evidence,
stop boundary, and existing user edits in a writer's brief.
Tell the writer others may be working and it must preserve their edits.

Choose only roles that materially help:

| Agent | Use |
| --- | --- |
| `codebase_explorer` | A specific unresolved path, ownership, or caller question. |
| `desktop_implementer` | A settled bounded implementation plus routine verification. |
| `test_reviewer` | Independent review of consequential state/contract/regression risk. |
| `ux_strategist` | Important unresolved workflow decisions before implementation. |
| `ux_reviewer` | Substantial changed interaction needing independent scrutiny. |
| `creative_tester` | Perceptual/artistic output or creative workflow evaluation. |
| `documentation_maintainer` | Verified milestone needing substantial durable-doc reconciliation. |

Exploration, strategy, review, and documentation agents are optional.
No fixed explore → write → review → document ceremony is required.
Resolve conflicting advice using evidence; do not delegate arbitration to the user.
Read-only reviewers may inspect parent-produced artifacts and return findings.
Have the parent/sole authorized writer collect runtime artifacts when a
read-only role cannot run a stateful harness.

## Model routing and escalation

Actual model + reasoning support comes from the active runtime, not marketing
names. Verify supported identifiers/efforts before modifying agent files.
Inspect `.codex/agents/*.toml` and current host model metadata.
Public API support does not establish Codex account access or quota.
Configured roles can override explicit spawn settings; inspect role precedence.
If the host has cached older roles, reload the session or use an available
explicitly configured generic role with the full bounded specialist brief.
Do not claim new on-disk defaults have already changed a running role.
In the current collaboration host, escalation uses `spawn_agent` with
`agent_type: "default"`, `model: "gpt-5.6-sol"`, an explicit supported
`reasoning_effort` (for example `high`), and `fork_turns: "none"` or a bounded
turn count; supply the task name and full specialist brief. Full-history forks
inherit the parent and do not accept model overrides. Do not pass an override
to a fixed custom role. Recheck the available tool schema on a different host;
if explicit routing is unavailable, Astra performs the work directly or updates
the relevant role when authorized. Distinguish requested configuration from
observed execution metadata; never claim model attestation the host cannot expose.

Start ordinary specialists on `gpt-5.6-luna` / `max`.
Use lower effort when observed comparable success makes it more efficient.
The initial seven agent definitions use Luna Max as a provisional baseline;
this is not a claim of proven equivalence across all Toniator roles.
Use `gpt-5.6-sol` with task-appropriate effort for difficult cross-crate,
geometry/numerical, domain/renderer, debugging, or consequential review work.
Use Astra directly or as a specialist when frontier capability changes the
expected outcome. Do not route every task through Astra.
Terra has no default role until evidence demonstrates a useful niche.
Do not enforce a Luna → Terra → Sol → Astra ladder.

Before escalating, determine whether the cause is unclear instructions, missing
context, stale guidance, wrong tools, conflicting authority, a product defect,
or insufficient model capability. Correct the actual cause.
For capability escalation, select effort deliberately rather than always Max.
Remember retries, reasoning tokens, latency, invocation count, and correction
burden when comparing effective cost. API prices do not equal plan usage.

Public evaluations set initial expectations; Toniator results outrank them.
Use ordinary task evidence: corrections, reviewer findings, scope discipline,
Rust/refactor success, navigation, computer-use reliability, and visual judgment.
Do not build telemetry or reproduce public benchmarks for routing alone.
Update routing on material model/support/cost changes or repeated task evidence,
not tiny benchmark fluctuations. Use fresh comparable primary evaluations.
Keep dated evidence and caveats in `references/onboarding.md`, read only when
auditing initial decisions or revising model/tool selection.

## Testing follows the change

Select checks from changed behavior, actual callers, shared contracts, and risk.
Run focused regression/behavior tests and relevant affected-target checks.
Do not repeat previously passing tests whose code/dependency cone is unchanged.
Add tests for new/changed behavior, verified defects, or real contract risks.
Avoid implementation-mirroring assertions and test-count goals.
Do not create fuzzing/property-test systems or malformed-input matrices unless
the current task or observed defect justifies them.

Broaden checks for shared/domain/schema changes, persistence, canonical geometry,
renderer/export contracts, widely consumed dependencies, discovered wider
failures, or a defined milestone requirement. State why the breadth is needed.
Domain enums/capabilities may require frontend compilation even for a headless
edit; check impacted consumers and strict Clippy as appropriate.
Use `scripts/validate_architecture.sh` when dependency boundaries are affected.
Honor explicit stage checks, but do not sweep obsolete historical exports or
compatibility tests or regenerate prior-stage validation directories.
The current user/AGENTS testing policy overrides stale blanket suite commands.
After relevant checks pass, repeat only for new edits, failures, or unresolved risk.

Tests establish evidence; visual inspection is independently required.
Source/sampling/render/export work exercises both immutable baseline assets:
`assets/raster-sample.png` at intrinsic 1024×1024 and
`assets/vector-sample.svg` at intrinsic 900×620, plus focused witnesses.
Follow `assets/README.md` for hashes and the SVG live-text/font caveat.
Keep derived artifacts in the current `target/validation/` directory.

## Visual and GTK acceptance

If visible output can change, generate and inspect representative real output.
This includes headless geometry, sampling, clipping, compositing, colors,
preview, PNG/SVG, and GTK layout/control/interaction changes.
Parent visual inspection is mandatory; a writer/reviewer report cannot replace it.
For affected PNG and SVG paths, retain native exports, inspect PNGs, and
rasterize SVG for visual inspection; inspect SVG structure when relevant.
Judge artistic/geometric intent, coverage, seams, transforms, and color.
Keep native RGBA unchanged; inspect RGB and alpha separately when needed.
Do not flatten or add a checkerboard by default.
A requested labeled composite never substitutes for raw output.

Use `gtk-wayland-debug` for app/GTK/presentation/input/accessibility work.
Read its required references; use the existing private headless Sway session
workflow, loopback WayVNC input, AT-SPI semantics, grim pixels, and logs.
Run the actual affected application state, not a replacement UI prototype.
Use `ui wait` → scoped `ui controls` → `ui inspect` → action → readback.
Ordinary controls use semantic name/role/hierarchy, not guessed coordinates.
Coordinates remain for spatial canvas interaction or documented semantic gaps.
Use real GTK accessibility metadata, never user-visible automation hacks.
Every app diff accounts for each changed interactive control: name/hierarchy,
role/state/value or selection/actions, label relation, enabled/applicability,
keyboard path, semantic action/readback, and screenshot/log evidence.
Metadata derives from visible vocabulary and descriptors, not a second schema.
Inspect screenshots yourself, collect an evidence bundle, stop your session.
Never commandeer another session or the user's GNOME desktop.
Stop a harness workflow immediately if it escapes into a desktop portal/chooser.
Automated Sway/wlroots evidence is not human or GNOME/Mutter acceptance.
Ask for manual inspection only for remaining specifically desktop/human risk
or behavior the harness cannot reproduce.
Headless-only work needs rendered artifacts, not an unrelated GUI tour.

## Current main-window design references

Read `docs/ui/REFERENCES.md` before main-window design work.
Read `PatternAndPresetTerminologyRefactor.md` for the user's product vocabulary.
Patterns are reusable structural recipes; applying one targets All or a named
channel, so different channels may use different Patterns. The existing
gallery/wizard and personal library manage Patterns. Presets are reusable
document-level configurations that may contain heterogeneous channel Patterns
and settings. A `.toniator` project remains the actual document/source/state.
Do not call the Pattern Wizard a Preset Gallery or conflate these resources.
Gate 21B-4 personal management concerns Patterns, not a new document-Preset
feature. Preserve internal `preset_format_version`, preset-v4, `presets/`,
registry/CLI/Rust names and format versions until separately authorized changes.
Terminology clarification alone does not implement a new schema or runtime path.
UI specification: `assets/Stage21D_Mockup/toniator_main_ui_spec_v1_2.pdf`.
Repository mockup: `assets/Stage21D_Mockup/MainWindowMockup.png`.
The v1.2 PDF includes the matching reference image; provenance is in the note.
Never silently substitute an older mockup or depend on past attachments.
For UI presentation, later explicit instruction outranks v1.2, which outranks
the corresponding mockup and older artifacts. Protected domain authority stays
in force; resolve conflicting proposed semantics before implementing them.

The direction is pure GTK4, dominant expandable canvas, contextual right
inspector, explicit color mode/channel edit target, and preserved zoom/Fit.
Use exclusive Preview / Source comparison with shared viewport state.
No persistent left sidebar/navigation rail or separate Design/Channels modes.
Keep New split/dropdown, Undo/Redo, and Help in the header; secondary actions
live in the hamburger menu. Place inline warnings between Fit and View.
Build Edit channel segments from the actual model, with readable colored
selection and explicit text labels. Selection never isolates the preview.
Use Pattern Recipe and Family / Sites / Connections summaries.
Group primary controls for creators; collapse technical Advanced controls.
Design references describe intended UI, not proof of shipped capability.
Do not implement new channel models, units, or storage semantics from a mockup.

## Tooling evolution and handoff

Project-local skills, agent instructions, and development helpers are maintained
infrastructure. Autonomously repair a concrete deficiency impeding current work.
Keep useful constraints; simplify obsolete compensating process.
Do not rename/rewrite skills for style, churn tools, create duplicate skills,
or conduct broad audits during unrelated product work.
Preserve the old orchestrator during initial onboarding except actual conflicts.
Do not weaken unrelated safety, product, or stage governance.
Verify changed tooling through practical use: a real navigation path, actual
GTK action/readback/pixels, coherent model/config support, or realistic skill use.
Do not create exhaustive tests for prose skills.

Use evidence-cache guidance only for evidence worth reusing or required by a gate.
Persist read-only findings directly when useful; do not spawn a cache writer.
Use milestone-documentation guidance when durable documentation materially changes.
The parent may do bounded documentation work directly.
Update the tracker at actual stage transitions; do not mark acceptance without
the user or invent checkpoint hashes. This skill's onboarding is not a product gate.
Preserve planning-only next-stage handoffs and current Git authorization.

Report concrete changes, decisions, focused checks, inspected artifacts,
measurements when claimed, and unresolved limitations. Distinguish defects,
inferences, deferred features, and unverified assumptions.
Avoid repetitive checklists and benchmark essays in normal handoffs.
