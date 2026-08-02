# TON-010 recipe-contract documentation reconciliation

- Date: 2026-08-01
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD assumption: `262c7e857446ded100d4a90fd23d651e52460665` on
  `TON-010-Stage5-Framework-Restart`; no commit, push, publish, or source
  implementation change was made by this documentation pass.
- Working-tree assumption: existing implementation changes, parent evidence,
  user-owned `ISSUES.md` TON-021 content, `nextPrompt.md`, and Stage 5 manual
  assets were preserved. The appended TON-021 section was not edited.

## Documentation reviewed

- `ISSUES.md` canonical TON-010 issue and adjacent TON-011 boundary.
- `docs/TON-010_STAGE_5_ARCHITECTURE_MAP.md`.
- `docs/TON-010_STAGE_5_FRAMEWORK_RESTART.md`.
- `docs/TON-010_STAGE_4_5_BASELINE_RESTORATION.md` and
  `docs/TON-010_STAGE_4_SCHEMA_UI.md` for preserved rationale and stage
  boundaries.
- `.codex-work/cache-index.md` and the four recipe-contract writer/parent
  evidence records under `.codex-work/agents/desktop-implementer/` and
  `.codex-work/evidence/`.

## Documentation changed

- Reconciled TON-010 scope so declarative `.tnpattern` v1 recipes, safe native
  runtime, bundled/user/project resolution, guided and graph editors,
  library/import/export, embedding, and recovery are TON-010 staged work rather
  than a separate custom-pattern follow-up.
- Added the 2026-08-01 declarative contract milestone to `ISSUES.md`, recording
  that it is implemented and automated-validated but not wired to bundled
  recipes, production renderer dispatch, document/preset persistence, library
  I/O, or editor UI. Current document/preset versions remain v8/v5; future
  persistence is explicitly strict document v9 / `.tntr` v6.
- Updated the Stage 5 status and completion record: Weighted Voronoi
  implementation/correction is automated-validated, while GNOME/Wayland,
  Krita-reference, and Inkscape **Break Apart** acceptance remains pending.
- Reconciled the Stage 4.5 heading so its parent-reviewed completion no longer
  leaves Stage 5 described as blocked.
- Updated the Stage 5 architecture map and framework-restart record with the
  recipe contract boundary, algorithm authority, Preview Surface versus Export
  Background separation, and explicit non-shipped integration gaps.
- Added this reconciliation record to `.codex-work/cache-index.md` for
  checkout-aware reuse.

## Implementation evidence used

- Recipe writer suites: 171, 176, 178, and 180 library tests across 2A, 2B,
  2C1, and 2C2; each also reported 48 binary/UI tests.
- Parent focused handoffs: 3, 5, 5, and 2 contract/execution tests, with
  `git diff --check` passing at each handoff.
- Stage 5 final validation: 161 library tests, 48 binary/UI tests; focused
  Weighted Voronoi 6, site distribution 5, Voronoi geometry 4, and realized
  GTK selector/control 1; strict Clippy, locked release build, formatting,
  and diff checks passed.
- Parent evidence confirms recipe execution remains bounded test-native
  operation work; no production bundled recipe operations, renderer dispatch,
  persistence, library I/O, or UI were added.
- Registry wording was corrected to preserve accepted 2B precedence: bundled
  content is immutable, same-layer ambiguity fails, and project-embedded custom
  content wins over differing local-library content while surfacing the
  shadowed source/fingerprint diagnostic.

## Stale or contradictory documentation found

- TON-010 repeatedly described the full custom recipe/editor/library/
  import/export/embedding ecosystem as a separate follow-up and excluded it
  from closeout. Those claims were replaced with staged TON-010 scope.
- The old Stage 5 issue status still said “Planned — blocked” despite the
  parent-reviewed automated implementation and correction evidence. It now
  records automated validation with human acceptance pending.
- Existing Stage 5 architecture rationale was retained: `site_distribution.rs`
  and `voronoi_geometry.rs` remain algorithm authorities; canonical output feeds
  preview/PNG/SVG; Preview Surface and Export Background remain separate.

## Remaining documentation gaps

- Do not document bundled `.tnpattern` resources, production recipe operation
  bodies, renderer dispatch, filesystem library/import/export, project
  embedding, document v9 / `.tntr` v6 persistence, or guided/graph editor UI as
  shipped until their implementation and review gates pass.
- Record the three human Stage 5 acceptance results before TON-010 closeout.
- Reconcile this record when the next recipe integration substage changes the
  loader/runtime, persistence, renderer, or UI boundaries.

## Invalidation conditions

Invalidate this documentation entry if HEAD/working-tree assumptions change,
the four parent-reviewed recipe contract substages are amended, Stage 5
manual acceptance is completed, document/preset versions change, or recipe
resources/runtime/persistence/renderer/UI integration lands.
