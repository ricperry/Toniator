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
- Do not add broad legacy compatibility, hidden adapters, or preset-specific
  behavior. Current schemas and explicit porting decisions are authoritative.
- Read applicable `.codex-work/` evidence before broad exploration, then
  validate it against the current checkout. Evidence is not durable product
  documentation.
- Read `docs/GREENFIELD_REWRITE_PLAN.md` and `ProgressTracker.md` before
  choosing scope. Treat the plan as the approved stage contract and the
  tracker as the current ledger, both subordinate to the protected normative
  specifications.
- Update `ProgressTracker.md` at every stage transition. The parent owns
  accepted/complete transitions and checkpoint hashes; evidence cannot
  substitute for user acceptance or a commit. Plan or roadmap changes require
  explicit user approval.
- Treat `assets/raster-sample.png` and `assets/vector-sample.svg` as immutable
  project-wide test inputs. Relevant source/sampling/render/export work must
  exercise both; write derived artifacts under `target/validation/` and follow
  the semantic text/font caveat in `assets/README.md`.
- Do not commit, push, publish, deploy, delete implementations, or overwrite
  unrelated work without explicit authorization.
