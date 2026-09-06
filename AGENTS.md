# Toniator Codex guidance

- `Project Specification/Addendum.md` is normative and supersedes conflicts in
  the other specification documents. Keep the five files under `Project
  Specification/` as protected normative inputs unless the parent explicitly
  authorizes a specification revision.
- Treat `ToniatorLegacy/` as a read-only legacy reference. Do not build, test,
  format, or write there. Consult it only for an explicitly named quarry task.
- Work in short, bounded stages with one writer. Stop at each parent-defined
  approval gate; do not begin a later stage from an earlier-stage handoff.
- Preserve hard boundaries: headless core crates never depend on a frontend or
  GTK/libadwaita; `toniator-cli` is headless; GTK/libadwaita belongs only in
  `toniator-app`; canonical geometry remains the shared preview/PNG/SVG
  boundary; authoritative commands report the correct invalidation level.
- Toniator is pre-release. Do not implement, preserve, or extend backward
  compatibility for superseded in-development schemas, containers, behaviors,
  fixtures, or adapters unless the current stage contract or protected
  normative specification explicitly requires it. Current schemas and explicit
  porting decisions are authoritative; reject obsolete formats instead of
  migrating them by default.
- Keep verification stage-scoped. Add focused tests for each stage's new or
  changed behavior and run only those tests plus directly relevant current
  foundational checks. Do not use workspace- or package-wide test commands that
  sweep obsolete historical export or compatibility tests, and do not regenerate
  earlier-stage validation directories. Run a historical test only for an
  explicitly authorized quarry task.
- Read applicable `.codex-work/` evidence before broad exploration, then
  validate it against the current checkout. Evidence is not durable product
  documentation.
- Treat `.agents/skills/gtk-wayland-debug` as a first-class development tool
  for the remainder of Toniator development. Use it by default for work that
  changes or reviews `toniator-app`, GTK behavior, preview presentation,
  keyboard/focus/input, accessibility exposure, or a UI regression. Prefer
  AT-SPI for semantic state, WayVNC for input, grim for pixels, and process
  logs/backtraces for diagnostics; collect an evidence bundle and stop the
  private session at handoff. Skip it only for work that is demonstrably
  headless-only or when the affected behavior cannot run in the harness.
- Keep its evidence boundary explicit: automated Sway/wlroots results are
  strong semantic, visual, input, and diagnostic evidence, but not human manual
  review or GNOME Shell/Mutter acceptance. Ask for manual desktop inspection
  only when the private harness cannot reproduce the behavior or the remaining
  risk is specifically human-, Mutter-, portal-, or compositor-policy-dependent.
- For every future `toniator-app` diff, account for every new or changed
  interactive GTK4 control even when its existing accessibility code remains
  sufficient. Check stable product name/hierarchy; real GTK role, state,
  value/selection, and actions; visible-label relation where applicable;
  truthful enabled/applicability state; keyboard path; semantic action/readback
  test; and relevant screenshot/log evidence. Derive metadata from visible
  product vocabulary and descriptor authority; do not create a second
  accessibility schema. Use `ui wait`, scoped `ui controls`, and `ui inspect`
  before full-tree diagnostics. AT-SPI is the normal route for ordinary widgets;
  coordinates remain for genuinely spatial canvas interaction. Any visible UI
  change still requires screenshot inspection in the private harness.
- Read `docs/GREENFIELD_REWRITE_PLAN.md` and `ProgressTracker.md` before
  choosing scope. Treat the plan as the approved stage contract and the
  tracker as the current ledger, both subordinate to the protected normative
  specifications.
- On every future Rust edit, ensure each touched non-trivial named function,
  method, and test has literal `///` documentation that states its present-tense
  responsibility and relevant authority boundaries, invariants, bounds, side
  effects, and `# Errors`, `# Panics`, or `# Safety` conditions. Do not use
  computed `#[doc = ...]` attributes for this purpose. Apply this rule on touch;
  do not initiate a repository-wide documentation pass without explicit
  authorization.
- Prefer `.agents/skills/toniator-astra-orchestrator/SKILL.md` for parent
  orchestration. The existing orchestrator remains available; do not load both
  routinely. Select specialists only when their work materially helps, and
  preserve one writer, including the parent. Agent model/effort defaults live in
  `.codex/agents/`; verify live runtime support before changing assignments.
- Semantic-map is retired for Toniator unless the user explicitly reactivates
  it. Do not run it, refresh its cache, or require usage-evaluation entries.
  Historical tool requirements are superseded. Use focused `rg`, Git, Cargo,
  and available rust-analyzer/LSP navigation; verify real callers/consumers.
  Keep the separate semantic-map project untouched.
- Project-local development skills and agent instructions may be refined for
  concrete tooling deficiencies within the current goal. This does not permit
  product/specification changes, later-stage work, destructive actions, or
  unrelated cleanup. Current UI references are indexed in `docs/ui/REFERENCES.md`;
  presentation guidance never creates a second domain authority.
- Update `ProgressTracker.md` at every stage transition. The parent owns
  accepted/complete transitions and checkpoint hashes; evidence cannot
  substitute for user acceptance or a commit. Plan or roadmap changes require
  explicit user approval.
- Treat `assets/raster-sample.png` and `assets/vector-sample.svg` as immutable
  project-wide test inputs. Relevant source/sampling/render/export work must
  exercise both; write derived artifacts under `target/validation/` and follow
  the semantic text/font caveat in `assets/README.md`.
- Visual-review convention (non-normative): primary review artifacts are the
  exact app/CLI files. Preserve native RGBA unchanged; do not default to a
  flatten, checkerboard, or background composite, and do not confuse a
  viewer's background with file content. Inspect RGB and alpha separately,
  distinguishing visible color, coverage, and hidden RGB. Alpha statistics and
  native viewers are allowed. A clearly labeled composited derivative is
  permitted only on explicit user request and never substitutes for raw output.
- Do not commit, push, publish, deploy, delete implementations, or overwrite
  unrelated work without explicit authorization.
