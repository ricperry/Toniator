# Combined Stages 22–23 implementation

Status: **Complete at commit `0f862512a86b7bb0eb25b915a496a7b68018b333`**, 2026-09-06. The user accepts the combined
Stages 22–23 delivery and subsequent startup, toolbar and divider corrections.
Contract: [temporal workflow plan](STAGE_22_23_TEMPORAL_WORKFLOW_PLAN.md).
Baseline HEAD: `c54ad44d224cab60142c901718e329a242518259` (main and origin/main).
User-owned version, packaging, tooling, research, assets and existing deletions
are preserved. Local acceptance checkpoints and inclusion of rebuilt Flatpak and
AppImage packages are authorized. Push and release publication remain separate.

## Delivery and verification ledger

| Requirement | Implementation / evidence |
| --- | --- |
| Rational frame timing and range selection | Shared exact parsing/selection and desktop Animation settings implemented; source-range bounds, live validation, one-frame editing, persistence and Undo/Redo pass focused tests and private GTK checks |
| 20 field kinds, descriptor scope and bounds, atomic inheritance | Domain implemented; independent review fixes covered by 15 temporal and 44 focused foundational tests |
| HEX endpoints, alpha and hue on RGB and all CMYK channels | Selected-frame Advanced color picker/HEX/alpha and color/hue interpolation implemented; focused domain, persistence and private GTK checks pass; native RGB/CMYK endpoint witnesses inspected |
| Project/container and source-free Preset persistence | Container 2 embeds media with generated entry names, hashes and explicit sequence order; schema 8 retains End-only overrides and separate project timing; five temporal/media persistence and five current Preset tests pass |
| Still, animated image, ordered sequence, video providers | Shared media opening feeds desktop import, reopening, endpoint/editor previews and export; ordered-sequence import has exact rate/order/cancellation and complete preflight; source VFR/audio notices pass focused and private GTK checks |
| Frame-aware engine/cache and cancellation | Direct decoded-frame input, bounded desktop media worker, stale-result gates and accepted-cache retention pass focused checks |
| Shared export runner, PNG/SVG sequences, FFV1, AV1 | Immutable shared sequence and software-video jobs implemented; native FFV1 exact-pixel and AV1 sharing/color witnesses pass |
| Storage preflight, progress, retry, cleanup, atomic publication | Shared sequence/video jobs and desktop recovery implemented; private GTK retry, Save PNG sequence, Discard, cancellation and close checks pass |
| CLI frame/time selection and animation | Project and direct-media single-frame, numbered sequence and video paths use shared jobs; exact frame/time selectors, source defaults, ordered sequence input and portable media project creation pass focused checks |
| GTK Start frame / End frame toggle, endpoint edits and color authoring | First End selection captures keyframable settings; ordinary controls edit the selected endpoint; affected pattern dependencies reinitialize independently; ALL scalar/response batches preserve compatible differences; no scrubbing |
| Destination settings and GTK export lifecycle | Shared export/recovery and personal defaults pass private GTK checks; actual Flatpak with isolated real portals verifies persistent folder grants, restart reuse, expired-grant diagnostics and regrant export |
| Packaged media tools in AppImage/Flatpak | Both actual packages pass native PNG/SVG, ten-frame FFV1, alpha FFV1 and AV1 checks with private SDK-built tools; missing private tools fail without host fallback |
| Scoped tests, compilation, Clippy, architecture checks | Focused domain/IO/media/engine/app checks, frontend compilation, scoped strict Clippy, formatting, architecture and diff checks pass; historical exclusions remain recorded below |
| Both immutable native PNG/SVG artifacts and ten-frame video | Intrinsic endpoint PNG/SVG inspected; ten 1080×1920 FFV1 frames preserve every rendered RGBA byte; native video and AV1 witnesses inspected under target/validation/stage22-video-export |
| Independent correctness/UX review and private GTK evidence | Both final read-only reviews complete; three correctness findings fixed and independently rechecked with no further findings; real isolated portal checks pass; human GNOME/Mutter acceptance is not claimed |

The user supplies the combined end-to-end acceptance after the recorded
verification and desktop corrections. Audio and real-time playback remain outside
the agreed scope. The still-image animation visibility decision remains deferred;
acceptance retains the current five-second still-import timing and visible endpoints.

## Acceptance closeout, 2026-09-06

Completed unchanged core/media tests, native artifacts, correctness/UX reviews and
portal evidence are reused. The latest release build, focused sidebar test, app
Clippy, formatting and private GTK toolbar/divider checks pass. User confirmation
establishes the startup/window sizing correction. Print Screen recovery is not
separately verified. Release packages are rebuilt from the accepted source
checkpoint; `dist/build-info.json` and `dist/SHA256SUMS` identify the exact result.
The requested packages are tracked using Git LFS. No later stage, production
research integration, remote push, tag or release publication is performed.

The acceptance packages built from `0f862512a86b7bb0eb25b915a496a7b68018b333`
supersede every earlier review hash below:

- AppImage (117,611,000 bytes): `fec771d0f354a8e128b9bcf5d02e54a3956fe801ff631484cd9f776a9962034a`.
- Flatpak (62,435,784 bytes): `e69ba9a0bb967d7478722f164385727cc5450cebb6518d0335783e68d6b485a5`.

Both package CLIs report 0.3.0, checksums pass, and the SDK build provenance has
an empty application-source diff. Actual Flatpak startup and welcome X→Untitled
checks pass in `ui-run-20260906-115119-281125`; the actual AppImage passes in
`ui-run-20260906-115250-282774`, including the copied long-warning profile and
Message details scroll range 0..250. Native screenshots are inspected at 200%
scale. The Flatpak retains its isolated installation profile, so its absent
Message details lookup is not counted as a warning reproduction. A first
cross-package restart reused the previous app's D-Bus owner and is not counted
as AppImage launch evidence; a fresh private session supplies the successful
check. Earlier full media/portal checks are reused for unchanged consumers.
All owned private sessions are stopped. Git LFS pointers retain the exact
package hashes and sizes; the following artifact checkpoint includes them.

## Work log

The entries below retain historical milestone state and superseded review
artifacts. Current completion and acceptance-package hashes are recorded above.

- Final contract audit (2026-09-06): all 20 animatable scalar field types
  materialize through active mark, connected-curve and region descriptors under
  all six easing modes at five frame positions (600 value comparisons). The
  independent expected easing weights also prove exact endpoint values and
  unchanged Start authority. A separate regression proves black, gray and white
  remain exact under positive, negative and multi-turn hue with independent alpha.
  Both focused tests and strict domain/app/CLI Clippy pass.
- Curve response bias now advertises ordinary Start reset only for its supported
  ChannelOutput target. The channel alias does not advertise unsupported reset.
  Advanced reset buttons associate their visible field and button labels for
  distinct accessible names. Native reset evidence is recorded in the final
  UI run `ui-run-20260906-021929-228214`: setting 0.5 enables reset,
  semantic activation restores exactly -0.02 and disables reset. Screenshots
  03/04 and JSON agree, and application logs are clean. Keyboard focus events
  were observed in the previous run, but subsequent state polling was unreliable
  and Space activated the dialog's Cancel. This does not establish keyboard
  activation of reset; the final check uses its native semantic click action.
  Earlier probes remain diagnostic history. CLI render help
  now mentions animation and video output.
- Final local artifacts (source changes after the full media run concern only
  reset capability/accessibility, progress presentation and CLI help):
  - `Toniator-0.3.0-x86_64.AppImage`: `733c2b7e94b99001cfa33b9d1be3822c665dbc7d61514f4385a7f83c8f2278b4`
  - `Toniator-0.3.0-x86_64.flatpak`: `53ff5b11565f234cca15230736f3e3e19a5600c3bfbf32909e476e2c4dc2729d`
  Final launch/artifact checks pass. Renderer/encoder evidence from
  `target/validation/stage22-packaged-media/1788672826576021058/` is reused;
  these hashes supersede earlier work-log hashes. Real portal checks above close
  folder-grant verification. Private sessions are stopped. No commit, push,
  release, stage acceptance or later-stage implementation is performed.

- Real portal verification (2026-09-06): the actual installed Flatpak runs against
  owned document, permission-store, GTK and desktop portal services on the private
  Sway bus. Permission/configuration data is isolated below
  `target/validation/stage22-private-portals/1788673148637485614/`. The chooser is
  inspected inside the private compositor. The wrapper asserts the host export
  directory is inaccessible directly; only the portal's app-scoped FUSE grant
  permits writes. A test-only symlink compensates for Flatpak's standard document
  mount path versus the private runtime directory; no host export-folder grant
  or production permission change is added.
- The portal records `persistent: 1, directory: 1`. Saving the returned folder
  default, fully exiting the app and relaunching preserves the exact path and
  permits another export. Deleting only that owned test document grant produces
  the choose-again diagnostic with no new output; selecting the folder through
  the real portal again returns a new grant and exports successfully. All three
  FFV1 witnesses decode to identical 108×192 RGBA. These are bounded folder-workflow
  witnesses; the separate full-resolution media evidence remains authoritative.
  Runs `ui-run-20260906-014623-202075` and `014732-203310`, the earlier chooser
  screenshots in `013935-195110`, portal logs and `results.json` retain evidence.
  The actual Flatpak exits through its GTK unsaved guard. All four owned portal
  services are stopped. This is real portal evidence under Sway with a GTK backend,
  not human GNOME/Mutter acceptance.
- The expired-grant screenshot exposed stale completion text from the preceding
  successful job. `temporal_export::submit` now resets failed input validation to
  zero progress and “Not started”, preserving recovery ownership and previous
  output. Native run `ui-run-20260906-015332-209289` records the preceding value 1
  in semantic actions, the final value 0 in `accessibility-current.txt`, and the
  inspected `01-validation-not-started.png`. Logs are clean; the session is stopped.
  Two exploratory assertion failures were harness assumptions (progress-bar text
  is painted rather than a label, and a repeated diagnostic needs an Export
  animation ancestor); direct numeric readback, screenshot and retained bundle
  verify the product behavior. The helper is corrected. App build, scoped strict
  Clippy, formatting, architecture and diff checks pass.
- Packages are refreshed for that presentation-only fix. AppImage SHA256:
  `def1af94b1251d6305df82e19b88fdda0f764040fdf27fb398d32ee6c0079616`;
  Flatpak: `6a42a49b0cfc7e0c9c0d46dfd433d687182b477fbf9b1e6c0ec4e784aa0a0597`.
  Both CLI launch checks pass. The unchanged renderer/encoder evidence from
  `1788672826576021058` is reused. Final direct per-field interpolation coverage
  is recorded below; no stage acceptance or checkpoint is implied.

- Final review follow-up (2026-09-06): a new End alpha override under hue
  rotation inherits the group's easing; an existing alpha override preserves its
  own easing. The regression proves QuadraticIn midpoint alpha, unchanged Start,
  Undo and preserved Hold. Shared recipe replacement resets document pattern
  dependencies only when it targets the document-base definition; other shared
  definitions retain linked-channel scope. Its regression proves independent
  translation and Undo survive. Five color-authoring cases, the 19 existing temporal
  cases plus the corrected new shared-base case, and four channel-batch cases pass.
  Scoped strict domain/app Clippy and frontend builds pass. These are current
  stage tests, not a historical package-wide test sweep.
- Initial preview submission now waits for two equal positive canvas allocations.
  Native run `ui-run-20260906-012708-186327` shows a sharp first preview; editing
  Rotation and then Undo restores exactly the same canvas pixels (zero differing
  RGB components). Screenshots are inspected and the evidence bundle is retained;
  the private session is stopped. Existing control identities, keyboard paths and
  generation/visibility/stale-work guards remain intact. Independent correctness
  re-review verifies all three fixes; the selected-frame UX review has no blocking
  findings.
- Refreshed actual packages pass the full bounded media check again under
  `target/validation/stage22-packaged-media/1788672826576021058/`. Native PNG/SVG,
  ten-frame 1080×1920 FFV1 RGBA, fractional-alpha FFV1 and explicit-matte AV1 pass
  for both formats. Output PNG pixels and SVG bytes match the previously inspected
  package witnesses. Current AppImage SHA256 is
  `fba961f0dc4b29183699e17a407815518575b7b5de0e08766f834cf2e3d2b8aa`;
  Flatpak is `62990fb36a98b2219052d189f896cb9f496a278cb47a0fe402576b39d83fea73`.
  Both are local unreleased 0.3.0 builds. Formatting, architecture, diff checks and
  all three immutable-input hashes pass; HEAD/upstream remain at the baseline,
  ahead/behind is 0/0, and the index is empty.
- Export-folder audit: `gtk::FileDialog::select_folder` retains its returned local
  path in personal defaults. The upstream [FileChooser contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html)
  states that its document grants persist across sessions. Toniator rejects an
  unavailable saved directory with a choose-again message before starting a job;
  Browse folders remains available. This establishes the intended implementation,
  not actual GNOME portal behavior. Remaining desktop acceptance: in the Flatpak,
  choose a folder, save that default, export, restart and export to it again;
  then verify recovery by choosing again if the saved grant becomes unavailable.
  No private-harness chooser, CLI filesystem grant, or upstream documentation is
  claimed as a substitute for that test. Both stages remain unaccepted; the goal
  remains active pending this acceptance boundary. No commit or push occurred.

- Source notice and packaged-tool milestone (2026-09-06): `source_notice.rs`
  retains only already-probed VFR/audio facts and session dismissal. The existing
  banner derives the displayed rate from current document timing and explains
  frame repetition/skipping and silent export. Reopening reconstructs facts from
  embedded media. Dismissing an unrelated error does not consume an unseen notice.
  The banner wraps; Fit, comparison buttons and Dismiss retain normal heights.
  A focused notice test, four current media-worker tests, app/CLI builds, scoped
  strict Clippy, formatting, architecture and diff checks pass.
- Private GTK run `ui-run-20260906-010507-164808` checks visible combined VFR/audio
  text, exact `30000/1001` rate, Apply/Undo/Redo and keyboard Space on the named
  Dismiss message button; a subsequent End render leaves the notice dismissed.
  Fresh process `010900-174780` reopens a portable project and restores its correct
  source notice/rate. The parent inspects wrapping and ordinary control sizes at
  1100×900. No new interactive control is introduced; existing names, native roles,
  enabled states, action/readback and keyboard path are retained. Final logs are
  clean, evidence bundles are captured and the private session is stopped.
- `packaging/media.py` builds pinned FFmpeg 8.1.2 with static SVT-AV1, dav1d and
  VP8/VP9 libvpx decoders inside the offline GNOME 50 SDK. A read-only review
  found two provenance gaps; fresh source roots per build and archive rehashing
  immediately before installation now prevent reuse or shipment of changed input.
  Both packages include original corresponding sources, recipe, notices, logs,
  hashes and SDK commit. Dynamic dependency audits exclude media/GPL extension
  libraries. SDK-built Toniator resolves `libexec/toniator-media` relative to its
  executable, with no host PATH fallback; development builds retain PATH tools.
- Actual final AppImage and isolated-installation Flatpak checks pass under
  `target/validation/stage22-packaged-media/1788671372198098377/`. Both render the
  immutable PNG at 1024×1024 and SVG at 900×620 identically. All ten 1080×1920
  video PNGs exactly match decoded FFV1 RGBA; both two-frame VP9-alpha inputs also
  match their FFV1 output, including fractional alpha and hidden color. AV1 WebM
  succeeds with explicit white backing and 10-bit 4:2:0; it is inspected separately
  and is not claimed lossless. Parent inspection covers native PNG, an alpha-preserving
  SVG derivative, first/middle/last video frames, alpha and AV1 outputs. A relocated
  packaged CLI without private tools fails cleanly without creating an output.
  The installed test Flatpak uses only the existing IPC/Wayland/fallback-X11/DRI
  permissions; CLI witnesses add scoped test file grants, not production permissions.
  This does not establish portal chooser/grant persistence or GNOME/Mutter acceptance.
  Final combined correctness/UX review remains in progress; neither stage is accepted.

- Media consumer/provider follow-up (2026-09-06): Pattern Editor previews now use
  an owned cancellable media worker and a single completion bridge. The private
  draft keeps authored history while preview evaluation materializes the selected
  workspace endpoint. Nested Wizard editors use the real selected artwork frame;
  the Wizard's own neutral-source preview remains separate. Four focused worker,
  stale-result/export and bridge-lifetime tests pass. An accidental broader
  `cargo test -p toniator-app pattern_editor -- --format=terse` had eight passes
  and two historical failures: `narrow_pattern_editor_policy_keeps_controls_reachable`
  expects obsolete “Mirror odd rows” wording, and
  `private_pattern_editor_history_never_mutates_the_main_workspace` opens obsolete
  container version 1. Neither historical contract was restored or claimed passing.
- Private GTK run `ui-run-20260906-002738-115613` reproduced the former compressed-video
  still-decoder error. Final run `004536-128214` verifies actual video Start/End
  previews, Undo/Redo, Cancel/discard and Apply to Pattern, then Wizard discard.
  The parent inspected the screenshots and confirmed distinct endpoint preview
  pixels. Existing controls retain native roles, names, state and semantic actions;
  no new editor controls were added. Final logs contain no panic or GTK critical.
  The bundle is captured and the private Sway session is stopped.
- Valid VP8/VP9 alpha now selects matching software decoders in both ffprobe and
  FFmpeg, rejecting unavailable alpha decoding instead of silently becoming opaque.
  Validated stream color tags explicitly supply input conversion when a decoder
  drops per-frame metadata. Packed RGB alpha is converted to planar channels before
  sample-aspect scaling, fixing an observed alpha 128-to-129 rounding error.
  Two focused `stage22_alpha_sar` tests cover both frames of VP8/VP9/FFV1 YUV alpha,
  missing decoder failure and exact square-pixel RGBA including hidden color.
  They pass alongside supplied-video/rotation and immutable-source checks.
  Current CLI alpha witnesses under `stage22-provider-witnesses/vp9-tagging-review/`
  (`fixed-cli-webm.png`, `fixed-cli-mkv.png`) are inspected with alpha intact.
  App/CLI build, scoped strict Clippy, formatting and architecture checks pass
  for this milestone. Further package/source-notice changes are still in progress.
- Remaining sequence checks now pass in private GTK run `000840-102885`: native
  keyboard list traversal, frame-rate label relation, Cancel during validation of
  200 large SVG inputs (2.61 seconds), and unsaved Cancel/Discard/Save paths.
  Cancel retains the existing workspace; Save preserves exact temporal/sequence
  content before opening the import sheet. The evidence bundle is captured.
  These no-portals Sway checks do not establish GNOME/Mutter or portal acceptance.
- Packaging preparation builds pinned FFmpeg 8.1.2, SVT-AV1 3.1.2, dav1d 1.5.1
  and libvpx 1.15.2 offline in GNOME SDK 50. Source hashes and the FFmpeg release
  signature are verified; dependencies are audited to exclude dynamic media/GPL
  extension libraries. The local recipe, corresponding original sources and logs
  will accompany both packages. Package assembly/runtime checks remain unfinished;
  no release, install into the user's app set, commit or push is authorized.

- Ordered image-sequence import milestone (2026-09-06): New > Import image sequence…
  uses the existing unsaved-document guard and a private, cancellable sheet. Native
  multi-file selection appends to a virtualized, named list; Remove, Move earlier
  and Move later define the reviewed source order. Exact integer/decimal/fraction
  frame rates use the shared parser. A worker installs the new unsaved project only
  after successful import; failure retains the list and existing workspace.
- Shared import now checks every unique sequence image before publication, with
  one decoded-frame cache and cancellation between frames. A focused regression
  exposed the prior lazy-validation gap: a later image with mismatched dimensions
  was accepted until rendering. Both GTK and CLI now reject it during import.
- The focused sequence app test covers both immutable stills, repeated source order,
  exact timing, save/reopen, cancellation, invalid inputs and native PNG/SVG export.
  The expanded lifecycle test and two current CLI tests pass. App build, scoped
  app binary/engine library strict Clippy, formatting, architecture and diff checks
  pass. An accidental all-target Clippy invocation reached deleted historical
  Reddit fixtures and existing test-only lints; it did not pass and was not rerun.
  No obsolete fixtures were restored. Immutable PNG/SVG/video hashes are unchanged.
  Native PNG 1024×1024 and transparent SVG 900×620 under
  `target/validation/stage22-sequence-import/run-92312-1788666702015634801/`
  were inspected directly (SVG through an alpha-preserving Rsvg derivative).
- Private GTK run `ui-run-20260906-000032-97538` verifies native list names/selection,
  both reorder directions, removal, invalid-rate sensitivity, dimension-error recovery,
  native chooser cancellation, Cancel/X retaining the old workspace, repeated paths,
  successful import, Start/End rendering and native Save. The saved container has
  order `source-1, source-2, source-1`, rate `30000/1001`, two embedded SVGs and
  initialized End values. Fresh-process run `000542-101283` checks reopening.
  Screenshots are inspected; these no-portals Sway checks do not establish
  GNOME/Mutter or packaged portal acceptance. Live cancellation during a long import,
  list keyboard traversal and native unsaved-guard paths need additional UI evidence.
  Final runs contain no Rust panic or GTK critical; evidence bundles are captured
  and the private Sway session is stopped. No commit or push occurred.
  Neither stage is accepted; packaging, remaining provider/consumer witnesses and
  final combined integration review remain required.

- Project timing milestone (2026-09-05): the main menu's Animation settings sheet
  edits exact frame rate and the half-open source interval. It uses the shared
  media selector, preserves source speed and all authored End values, and publishes
  one history command on Apply. Cancel/X discard the projection. Equivalent numeric
  spellings preserve exact timing authority and do not add history. CLI and GTK now
  share decimal/fraction parsing through `parse_media_time`/`parse_media_rate`.
  Metadata probing runs on a cancellable owned worker; Apply stays disabled until
  metadata and current inputs are valid. Live validation shows output-frame count
  and VFR/audio notices without rendering or retaining stale error messages.
- The main menu reuses the existing visible-label accessibility helper, supplying
  real menu item names instead of unnamed native actions. Each timing entry retains
  its real label relationship and keyboard path. No playback or scrubber is added.
  One-frame timing selects Start for editing when endpoint controls disappear;
  stored End artwork remains intact for a later longer interval. This normalization
  runs on ordinary UI synchronization, including timing Apply and Undo/Redo.
- The focused `project_timing_sheet_preserves_artwork_and_exact_history` app test
  covers both immutable stills and the test video, exact/equivalent text, invalid
  source bounds, one-frame selection, unchanged End values/source bytes, one Apply
  Undo/Redo and exact project persistence. The two current CLI `stage22_frames`
  integration tests pass after shared parser extraction. App/CLI builds, strict
  app/CLI/engine Clippy, formatting, architecture and diff checks pass. Native
  PNG 1024×1024, SVG 900×620 and video PNG 1080×1920 in
  `target/validation/stage22-project-timing/run-69811-1788665110182492027/`
  are directly inspected with original alpha; immutable source hashes are unchanged.
- Private GTK final workflow run `ui-run-20260905-233616-85239` verifies named menu
  actions, rate/range entries, truthful invalid-input sensitivity, corrected-input
  recovery, Cancel, Apply, exact saved Undo/Redo, X close and equivalent-value no-op.
  Run `233411-83174` verifies Cancel and X during a live delayed metadata probe;
  both return in under two seconds and reap the probe. Run `233941-87593` verifies
  shortening an active End view to one frame: endpoint controls disappear and normal
  rotation edits change Start while saved End values remain exact. Screenshots are
  directly inspected. These are private Sway checks, not GNOME/Mutter acceptance.
  Final run `234202-89479` verifies native field label relations and that status
  accessibility exposes the current visible result/error text. Final timing runs
  contain no Rust panic or GTK critical. Evidence bundles are captured and the
  private session is stopped; no implementation subprocess remains.
  Ordered-sequence import UI, remaining source notices/provider witnesses, packaged
  tools, Pattern Editor media consumers and final combined acceptance remain pending.

- All-channel editing milestone (2026-09-05): domain-owned batches enumerate compatible
  channel and channel-output scalar targets, validate the complete change, and preserve
  existing differences. The Advanced All group displays their average and mixed range;
  editing the average shifts compatible values equally. Individual channel settings,
  including the selected-frame color picker, remain in named disclosures below.
  Layout controls retain their existing main-inspector home; RGB component inputs are
  not added to the artist-facing All group. End edits preserve each target's easing;
  alpha edits preserve RGB/CMYK paint colors and hue modes. Shared output definitions
  remain unchanged. Ordinary Start edits preserve initialized End values.
- Four focused `stage22_channel_batches` tests cover coupled response bounds,
  incompatible output filtering, CMYK alpha/hue/easing preservation, rejection,
  no-op behavior and Undo/Redo. The focused private-draft app test covers one Apply,
  exact persistence and native exports from both immutable stills and the video.
  The 19 temporal and four color-authoring domain regressions also pass. App build,
  strict app/domain Clippy, formatting, architecture, diff and immutable hashes pass.
  Native RGB PNG 1024×1024, SVG 900×620 and video PNG 1080×1920 under
  `target/validation/stage22-channel-batches/run-59034-1788664236750200564/`
  are directly inspected; the SVG inspection derivative preserves transparency.
- Private GTK run `ui-run-20260905-231350-61675` verifies mixed contrast values,
  End-only contrast/alpha edits, Cancel, exact saved one-Apply Undo/Redo, subsequent
  Start edits preserving End, and invalid batches leaving the document unchanged.
  Native entries, label/description relations, disclosure state, named-channel paint
  access, keyboard traversal and screenshots are checked. Explicit GTK label and
  description relations and invalid state accompany the new fields. The summary
  helper omits relations on this host; `native-batch-relations.json` independently
  reads the real AT-SPI targets. The existing GTK root-focus critical recurs, and
  the helper cannot verify focus on the nested HEX control; native scrolling and
  semantic readback still expose the selected-frame controls. No Rust panic occurs.
  The evidence bundle is captured and the private Sway session is stopped.
  Automated Sway evidence does not establish GNOME/Mutter acceptance. This milestone
  does not complete the remaining import/timing UI, packaged tools, provider witnesses
  or combined-stage acceptance requirements.

- Desktop export milestone (2026-09-05): `temporal_export.rs` connects the main Export
  action to immutable shared PNG-sequence, FFV1/Matroska and optional AV1/WebM jobs.
  The sheet shows inclusive frame bounds, source frame rate, dimensions, background,
  antialiasing, audio availability, destination and job name. Export current frame
  retains the existing selected-endpoint PNG/SVG route. Trimming preserves source
  speed and stretches the authored transition over the chosen interval; it does not
  change the open document or create history. Project timing/import configuration
  remains a separate unfinished requirement.
- Personal destination and temporary-folder defaults use bounded atomic IO in
  `export_defaults.rs`, outside documents and Presets. Missing defaults prompt in
  the sheet; unavailable folders require reselection. Native folder selection and
  cancellation, saved defaults across fresh processes and clearing defaults pass.
  The private chooser uses GTK's no-portals test fallback; this does not establish
  Flatpak grant persistence or GNOME portal acceptance.
- Worker ownership spans metadata, rendering, encoding and recovery. Progress uses
  the shared jobs' roughly 10 Hz frame-work updates and separate encoding phase.
  Cancel export and the window X cancel and reap active work before closing.
  Encoding failures retain PNGs for Retry encoding, Save PNG sequence or Discard.
  Runtime checks cover all three recovery actions, exclusive destinations, cleanup,
  invalid-destination Discard, and preventing editing during work/recovery. Direct
  sensitivity on the sheet's native fields corrects GTK's inherited-child accessibility
  readback; no change to the existing AT-SPI helper remains from this investigation.
- Three focused desktop export tests and one personal-default IO test pass. They
  exercise exact fractional timing, endpoint interpolation under trim, separate phase
  percentages, native 1024×1024 and 900×620 PNGs from both immutable still inputs,
  the test video's software encode, cancellation, no overwrite and unchanged workspace
  snapshots. Native still outputs in `target/validation/stage22-desktop-export/`
  `run-38760-1788661659952238187/` are directly inspected with original alpha.
- Private GTK runs `ui-run-20260905-222831-40382`, `223635-45145` and final
  `224012-47197` verify actual PNG/FFV1/AV1 exports, fractional source trim metadata,
  recovery, cancellation/X close, restored defaults, native folder selection, the
  existing current-frame chooser, keyboard menu/dropdown paths and truthful states.
  AV1 rejects Transparent and exports three 64×64 yuv420p10le frames after explicit
  White selection. FFV1 is BGRA at the source 6 fps. These small GTK video artifacts
  supplement the already verified native 1080×1920 video evidence; they do not replace it.
- Final run `ui-run-20260905-224700-52411` checks an audio-bearing derived source:
  the sheet explicitly says the export is silent, and clearing personal defaults writes
  null paths. Final sheet/recovery/rendering/chooser/audio screenshots are directly
  inspected; app logs contain no Rust panic. Focused build, strict app/IO Clippy,
  formatting, architecture, diff and immutable-source checks pass. All private sessions
  are stopped, and no commit or push occurred. Neither combined stage is accepted.

- Selected-frame color correction (2026-09-05): the user's latest direction supersedes
  paired Start/End paint widgets. Advanced now contains one native color picker, HEX
  entry and alpha field per channel, using the selected frame and the existing private
  Apply/Cancel history boundary. The main Color & Channel group has no extra paint block.
  Color interpolation offers direct colors or unwrapped hue, with its interpolation
  choice applied to paint and alpha together. Static settings remain shared and available
  from either endpoint. No keyframe arming controls or scrubber are introduced.
- Domain initialization stores every active End scalar, including equal values, without
  duplicating Start or adding a second document. Repeated selection is idempotent.
  Pattern replacement clears only its affected pattern-relative End fields; other
  channels, translation, mapping, paint and opacity remain intact. The next End selection
  captures missing values from Start. Ordinary Start response edits rebase End deltas;
  selected-copy output IDs and reordered outputs retain their individual End values.
- Current focused evidence: 19 temporal domain tests, four color-authoring tests and
  six temporal persistence tests pass. App paint transaction and inspector temporal
  tests pass. Coverage includes equal-value initialization, repeated selection, exact
  Undo/Redo, Red-only replacement, shared/selected response edits, copied painter order,
  picker conversion, color/alpha authority and project round trips. This is a bounded
  correction within the still-active combined goal, not stage acceptance.
- Private GTK run `ui-run-20260905-215028-19364` exercises Start editing, first End
  initialization, independent subsequent edits, precision under focus traversal, Cancel,
  one Apply Undo/Redo, palette selection, hue 450 degrees and interpolation. Saved JSON
  agrees with native controls. Screenshots `01-start-color.png`, `03-end-interpolation.png`,
  `04-picker.png` and `06-hue.png` are directly inspected. Existing `gtk_root_get_focus`
  criticals recur during keyboard traversal; no Rust panic or failed edit occurs.
  Native RGB PNG 1024×1024 and CMYK SVG 900×620 color witnesses from
  `target/validation/stage22-color-authoring/run-273606-1788656725276025639/` were
  already directly inspected. Automated Sway evidence is not GNOME/Mutter acceptance.
- Fresh-process color verification exposed a one-ULP JSON parser drift:
  `0.21586050011389926` reopened as `0.2158605001138993`. The initialized-End
  persistence regression reproduces the failure before the fix. IO now enables
  serde_json's existing `float_roundtrip` feature, preserving authored f64 values
  exactly without a schema change or new dependency. This fixes save/reopen precision;
  the failure was not an End edit changing Start.
- Final fresh-process run `ui-run-20260905-220255-33706` reopens the original
  high-precision witness, verifies the End HEX/alpha values, selects a native palette
  color, edits hue/alpha, applies one interpolation choice to both, and checks exact
  saved Start equality plus one-Apply Undo. It passes with float-roundtrip parsing;
  `06-hue.png` is directly inspected. App build, strict app/domain/IO scoped Clippy,
  formatting, architecture, diff checks and immutable input hashes pass. Evidence
  bundles are captured and the private Sway session is stopped. The existing GTK
  focus critical remains; combined-stage acceptance and the listed remaining work
  are still pending. No commit or push occurred.

- Initial audit: no live implementation process and no product Rust changes.
  Source is single-frame. Prior planning and codec-only evidence do not prove
  any implementation requirement. Domain temporal authority starts first;
  media/persistence integration is inspected read-only alongside it.
- Domain writer completed exact timing, end-only overrides, atomic endpoint
  commands, descriptor temporal capability, and static frame snapshots retaining
  the original revision token. Seven temporal tests and focused history (2),
  document Preset (5), effective-pattern (18), capability (9), descriptor (7),
  and Stage 20S/21A (5) checks passed. Domain check, strict temporal-target Clippy,
  formatting and diff checks passed. Parent caught an invalid RationalTime
  derived default; the writer corrected it to 0/1 and added a regression assertion.
- Latest user direction: ordinary document settings are the Start state; extra
  animation storage contains only End overrides and easing. Start/End toggle
  belongs below canvas; Preview/Source placement is unchanged. Scrubbing is
  excluded until hardware acceleration, not merely deferred within this effort.
- Parent implemented IO-owned schema-8 timing and End-override DTOs. No Start
  snapshot is added; empty override lists are omitted. Project and source-free
  Preset round trips preserve scalar, color and hue End intent. Both Preset
  input formats retain destination source, canvas and timing. Three new
  persistence tests, five current document-Preset tests and the serialized-field
  inventory check pass. Strict scoped IO Clippy and app/CLI all-target compilation
  pass. Container remains version 1 until the pending multi-entry media manifest
  implementation; this is not a completed moving-media persistence claim.
- Independent domain review found stale-command, Start inheritance, structural
  response ordering, equivalent-override ordering, overflow, applicability and
  invalidation gaps. These are corrected and covered by 15 temporal tests plus
  44 focused history/configuration/descriptor/effective-pattern checks. Export
  must still materialize every requested frame during preflight to reject
  intermediate coupled-response crossings before publication.
- Independent IO review confirmed End-only DTO and Preset timing boundaries,
  but found project wrapper fields could be ignored. Project decoding now rejects
  unknown root, source, document and nested fields; eight focused persistence
  and current document-Preset tests pass after this correction.
- Media foundation uses existing still/image animation decoders and bounded
  FFmpeg/ffprobe processes. Metadata is inspected before frame decoding;
  compressed input is limited to 128 MiB, decoded source to 64 megapixels,
  probe JSON to 16 MiB, timing tables to one million entries, and each probe or
  decoded-frame wait to 20 seconds. Sequential video retains one decoded field
  and bounded raw-frame buffering. Cancel/drop disconnects the bounded channel,
  kills/reaps the child, and joins readers before deleting its private input.
  Source times are exact and half-open; backward requests restart sequential
  decoding. This supports endpoint requests, not scrubbing.
- Six media tests exercise both immutable native still assets, ordered repeated
  images, the supplied ten-frame 6 fps video, unequal PTS intervals, right-angle
  display rotation, one-pass APNG/GIF/WebP, direct hidden RGB, and FFV1 alpha.
  The supplied clip's first decoded RGBA hash matches the earlier independently
  probed SDR conversion. Untagged full-range RGB explicitly follows the existing
  still-image sRGB interpretation (recorded in FrameIdentity); incomplete YUV
  color metadata and HDR are rejected. Animated AVIF, SAR and non-RGB alpha
  conversion have implementation paths but still need final focused witnesses.
- Engine ResolvedSource distinguishes encoded still input from SourceFrame,
  retains decoded fields by Arc, and includes frame timing, decoder/color
  identity and decoded pixels in source cache identity. Three focused tests
  cover direct input, truthful identity, source/geometry/presentation cache
  reuse, cancellation, original revision tokens, and native endpoint artifacts.
  The two app-test adjustments only adapt encoded-source accessors; no product
  GTK control or behavior changes in this milestone, so no UI harness is run.
- Native artifacts in target/validation/stage22-frame-engine include unchanged
  intrinsic 1024×1024 PNG and 900×620 SVG dimensions at Start and End. Parent
  inspected PNGs and unflattened SVG rasterizations. A solid red SVG witness
  changes from alpha 1 to 64/255 at (600,20), confirming that retained straight
  RGB must not be mistaken for unchanged opacity. No composite derivative is
  substituted for the native files.
- Final milestone checks: six media tests, three engine temporal tests, scoped
  strict media/IO/library Clippy, app/CLI all-target compilation, architecture
  validation and diff checks pass. An attempted engine `--tests` Clippy command
  reached the obsolete source_identity test's deleted Reddit fixtures and could
  not compile that historical target; no historical tests ran and no user asset
  was restored. Keep future checks scoped to current library/temporal targets.
- Container 2 now stores a sorted unique source-entry manifest with generated
  numbered names and per-entry integrity hashes, plus an independent media-kind
  and authored sequence-order manifest. Source entries remain capped at 10,000
  and aggregate 128 MiB; repeated sequence references do not duplicate entries.
  Missing/orphaned references, wrong primary source, duplicate IDs, unsafe entry
  names, bad hashes/rates, unknown fields and container 1 are rejected. Five
  focused temporal/media persistence tests and five current Preset tests pass.
- Shared engine media opening consumes the IO manifest and shares compressed
  bytes with sampling providers. Two engine checks cover both immutable stills,
  repeated sequence references, cancellation and the supplied video's End frame.
  Sequence memory accounting and fingerprinting now process each shared encoded
  allocation once, preserving explicit repetitions without charging duplicate
  resident bytes or repeatedly hashing the payload. Its focused regression passes.
- Domain source-time mapping distinguishes absolute frame selections from an
  explicitly trimmed source interval, retains half-open boundaries, and uses
  checked reduced rational addition. Focused timing tests include large reducible
  arithmetic. Shared frame requests preserve the session revision and obtain
  domain-materialized endpoint settings before passing decoded pixels to evaluation.
- CLI project `render --frame` uses that shared request; omitted selection renders
  the project's Start frame. A current container-2 video fixture renders visible
  Start and zero-opacity End PNGs, rejects an out-of-range frame without creating
  output, and leaves the project unchanged. This is a 48×48 semantic witness;
  intrinsic PNG/SVG visual evidence remains the earlier engine artifacts. It does
  not establish sequence/video export or the pending desktop workflow.
- Integration checks pass: strict domain/IO/sampling/engine library Clippy,
  scoped temporal/sequence/CLI Clippy, app and CLI all-target compilation,
  architecture validation and diff checks. The architecture scanner initially
  matched a frontend toolkit name in a headless module comment; the comment now
  describes frontend workers generically. Baseline raster, vector and video
  hashes are unchanged; the index remains empty and no checkpoint is created.
- Shared `SequenceExportJob` captures immutable document/source/options, opens
  its provider inside the owning worker, and validates every requested frame's
  materialized settings and exact source-time bounds before creating output.
  Time-interval frame counts must equal `ceil(duration * fps)`. A coupled-response
  pair with valid endpoints but an invalid interior frame is rejected before
  directory creation. The initial output count is capped at one million frames.
- IO `SequenceWriter` creates an exclusive child directory and performs fixed-name
  frame writes relative to an open directory handle. It synchronizes frames and
  atomically publishes them without replacing existing files. The manifest carries
  exact frame rate, source interval, absolute input range, dimensions, frame count
  and completion state. Cancellation retains completed frames with `complete: false`;
  collisions and moved/replaced directories cannot redirect writes or produce a
  false success path. Available storage is checked before and during output.
  The estimate uses twice raw RGBA bytes plus overhead, rather than compressed
  input size; SVG size remains scene-dependent and actual writes still check space.
- Export requests select final PNG background/target/antialiasing before the
  evaluator's raster pass, avoiding a second full rasterization. Consumer choices
  affect only the raster cache key; canonical scene reuse and reference renderer
  equality pass a focused check. SVG output consumes the unchanged native scene
  while the evaluator's retained raster slot uses a minimal derived preview.
  Current-frame work updates are coalesced to about 10 Hz, with explicit frame and
  lifecycle boundaries; no ETA is fabricated and completion follows finalization.
- Four engine export tests, two IO publication tests and the expanded CLI frame
  integration test pass. CLI syntax is `render -i project.toniator -o
  new-directory/frame-%06d.png` (or `.svg`). SIGINT/SIGTERM use the same cancellation
  flag as decode/evaluation/publication; a real SIGTERM test confirms graceful
  cancellation. Direct-media CLI sequence creation and time/range flags are not yet
  wired and remain required by the combined contract.
- Native sequence evidence is retained at
  `target/validation/stage22-sequence-export/run-183190-1788646461799080417/`:
  1024×1024 PNG Start/End and 900×620 SVG Start/End, plus a ten-frame 48×48 video
  sequence as a source-order/endpoint semantic witness. Parent inspected native
  PNGs and unflattened SVG review rasterizations. At SVG pixel (600,20), red remains
  red while alpha changes from 1 to 64/255. The small video sequence is not a
  native-resolution encoded-video acceptance artifact.
- Filesystem publication uses [rustix 1.1.4](https://docs.rs/rustix/1.1.4/rustix/)
  for safe directory-relative operations, no-replace rename and space queries,
  retaining the IO crate's unsafe-code prohibition. CLI termination uses
  [signal-hook 0.4.4](https://docs.rs/signal-hook/0.4.4/signal_hook/flag/), with
  default features disabled. Their registry license declarations include MIT
  and Apache-2.0 alternatives; no custom unsafe syscall or signal handler is added.
- `VideoExportJob` now uses the sequence runner for private PNG intermediates
  under random `/tmp/toniator-render-*` directories (or a caller-selected parent).
  A real one-frame encode/decode-count probe checks the chosen software encoder,
  pixel format, muxer, output dimensions and exact rate before expensive rendering.
  PNG and video space estimates are combined when their directories share a
  filesystem, and running writes/encoding check available space again.
- Default FFV1 v3/Matroska uses BGRA without RGB-to-YUV conversion. Optional
  SVT-AV1/WebM uses CRF 18, preset 6, 10-bit 4:2:0 and requires an explicit opaque
  black or white matte. Its pipeline explicitly converts full-range sRGB through
  BT.709 matrix/transfer/range handling; decoded color-patch centers differ by no
  more than five encoded-sRGB byte values in the focused witness. Both outputs
  are silent. No hardware acceleration or audio processing is added.
- The encoder writes an owned regular-file descriptor through `/proc/self/fd/1`,
  preserving seekable container finalization without opening a user destination.
  Validation reads the owned descriptor, decodes/counts every frame, rejects
  decoder errors, and checks exactly one video stream, dimensions, pixel format,
  frame count and rational rate before atomic no-replace publication. The process
  runner concurrently drains bounded diagnostics/progress, observes cancellation
  and storage, and reaps processes/readers. Capability encoding has a 30-second
  limit; active encoding has a 180-second progress-inactivity bound. Complete-file
  validation remains cancellable without a duration-based whole-video deadline.
- `VideoRecovery` retains complete PNGs across failed retries and offers retry
  to a valid destination, copying to a new ordinary PNG sequence, and explicit
  discard. Cancellation removes the owned temporary video and render workspace;
  unexpected workspace files stop cleanup before deletion. Successful encoding
  distinguishes an actual cleanup limitation from successful video publication.
  The CLI reports retained PNG locations on encoding failure; desktop recovery
  actions still need wiring to this API.
- Video checks pass for native FFV1, AV1 sharing, SDR color patches, failed retry,
  fractional 30000/1001 fps retry, hidden RGB beneath zero alpha, PNG recovery,
  explicit discard, destination collisions and cancellation. A real realtime
  FFmpeg process reports a frame before cancellation and is reaped promptly.
  IO checks cover temporary cleanup, publication collisions and unknown-file
  preservation. The CLI integration renders ten video frames and verifies cleanup
  under an explicitly chosen temporary parent.
- Native video evidence:
  `target/validation/stage22-video-export/run-189370-1788647829264347747/native-ffv1.mkv`
  contains ten 1080×1920 BGRA frames at 6 fps (1.667-second container duration).
  Sequential raw decoding matches the SHA-256 of every rendered RGBA frame.
  Exact first/last rendered PNGs were retained and directly inspected. AV1 evidence
  at `run-189370-1788647829264342957/sharing.webm` uses two native 1024×1024 frames;
  the first frame was decoded with explicit sRGB conversion and inspected. These
  are software/host artifact witnesses; packaged-tool verification remains pending.
- CLI `.mkv` selects FFV1 and `.webm` selects AV1; AV1 requires `--background black`
  or `--background white`. `--temporary-directory` selects an explicit video
  staging parent. The following integration milestone adds direct-media input and
  range selection. The combined goal still requires the full desktop workflow,
  packaging, remaining media-provider witnesses and final integration review.

### Shared media import and CLI timing milestone

- `engine/src/media_import.rs` supplies bounded local import and shared timing
  selection for both frontends. It detects animated PNG/WebP from decoder headers
  and AVIF sequences from container brands, retains explicit sequence order, and
  deduplicates canonical-path repetitions within the portable source bounds.
  Import results are transferable; decoder providers remain inside their worker.
- CLI `render` and `document create` accept moving media. Repeat `--sequence-frame`
  to append stills after `--input` in explicit order, with `--fps` assigning the
  sequence rate. No glob expansion or directory sorting is inferred.
- `--fps` accepts exact integers, decimals and rationals such as `30000/1001`.
  `--start-frame`/`--end-frame` are inclusive, zero-based source indices at that
  output rate. `--start-time`/`--end-time` are exact seconds with an exclusive end.
  Frame/time selectors conflict; `--frame` selects one output file within its
  authored range and conflicts with range selectors. Rate-only project changes
  preserve the selected source interval; trims stretch Start-to-End interpolation
  across the selected job without changing source playback speed. Input project
  bytes/settings remain unchanged. A one-frame range uses Start.
- Moving-media defaults use full finite duration and nominal rate; still imports
  propose five seconds at 30 fps. Variable-rate normalization and omitted source
  audio are reported. The shared selector rejects invalid/empty ranges, source
  overruns, inconsistent time/frame counts and more than one million output frames.
- SIGINT/SIGTERM cancellation now spans import/probing, frame decoding, canonical
  evaluation and export. Single-file PNG evaluation selects output backing and
  antialiasing before its one raster pass; single SVG evaluates a minimal derived
  raster while serializing unchanged native geometry.
- Animated AVIF testing found that the first exposed video stream can be a poster
  image. Animated-image probing now selects the first finite timed video track;
  ordinary video retains first-video-stream selection. The frame probe uses that
  exact stream index. Tagged SDR AVIF, APNG, GIF and WebP import, both endpoint
  frames, and exclusive-duration rejection pass. Untagged YUV/HDR policies remain
  unchanged; local libaom AVIF fixtures set explicit codec color parameters.
- Shared import/timing tests cover both immutable stills, the ten-frame video,
  ordered repeated entries, 30000/1001 timing, exact source mapping, one-frame
  ranges and invalid selectors. CLI integration additionally checks portable
  video/sequence projects, distinct red/blue/red frame order, trimmed End alpha,
  no project mutation, SVG sequence output, lossless video and actual SIGTERM.
  Six current sampling regressions pass after the stream-selection fix.
- Direct CLI native evidence is in `target/validation/stage22-media-import/`:
  `native-raster.png` is intrinsic 1024×1024 and `native-vector.svg` is 900×620.
  The PNG and unflattened ImageMagick SVG review raster were inspected; both
  retain alpha from zero through one. Immutable input hashes remain unchanged.
  No GTK control has been added by this milestone; Start/End-only UI and private
  harness verification are still required.

### Desktop endpoint preview and inline editing milestone

- 2026-09-06 layout correction places Start frame / End frame in the same
  canvas toolbar as Preview / Source, retaining separate linked pairs. The
  sidebar divider now receives its default width once; removing the 100 ms
  reset lets native dragging, F8/arrow keyboard resizing, and hide/show retain
  the selected width. This is session layout, not persisted document state.
  The release build, strict app Clippy and focused sidebar sizing test pass.
  Private Sway at 200% scale verifies the video toolbar, End/Source actions,
  Start keyboard activation with a retained WayVNC seat, divider values
  640→610 retained across hide/show, F8/Left→609, and pointer drag→644 retained
  afterward. Screenshots are inspected in `ui-run-20260906-111811-65309/`.
  GTK exposes the paned value through an unnamed native panel despite the
  declared Canvas and settings accessibility label; testing selects it by
  its inspected shallow hierarchy and native value interface. An initial
  endpoint focus attempt without a retained seat fails; the retained-seat
  keyboard test passes. These are automated checks, not GNOME acceptance.
- Visibility clarification: the current toggle condition uses a source plus
  a project range longer than one frame. Still imports default to five seconds
  at 30 fps, so they currently show the endpoint pair too, as verified with
  `raster-sample.png` in `ui-run-20260906-112029-97786/`. Animation settings
  remains available. The plan calls for contextual visibility, but no explicit
  enable-animation workflow for still sources or blanket hiding of video
  features is settled. The layout correction does not change that behavior.
- Start frame / End frame now selects the preview and inspector endpoint below
  the canvas. Selection adds no history entry; Preview / Source stays in its
  existing toolbar. No scrubbing, playback or arbitrary frame transport is added.
- One bounded worker owns media providers and a replaceable pending request.
  Decode and source comparison preparation run off the GTK thread. Results match
  workspace, revision, endpoint and request epoch before scheduler submission;
  cancelled candidates cannot publish. Waiting for media retains accepted engine
  caches; a focused scheduler check proves reuse and rejection of a cancelled
  candidate. Worker shutdown cancels decoding and joins the owned thread.
- Desktop source import and current project reopening use shared media authority.
  Single PNG/SVG export now uses the selected endpoint and the same media/frame
  request as the CLI. A focused video-backed test distinguishes visible Start
  from transparent End output. Full sequence/video export controls remain pending.
- Eligible inline scalar rows edit effective End values through domain commands.
  Animation disclosures provide easing and Reset End, enabled only for stored
  overrides. Start remains ordinary document authority; static definition controls
  remain unavailable at End. The subsequent milestone integrates Advanced Settings;
  full color authoring remains pending. Domain commands normalize named-channel deltas and preserve easing
  when changing an endpoint. A focused test covers inheritance, reset and Undo/Redo.
- Private visual testing exposed Source blanking during repeated endpoint updates.
  The viewport now uses an immutable GDK paintable that appends its texture at
  paint time, preserving logical canvas dimensions and preview-padding removal.
  Repeated keyboard switching passes with visible Source pixels at Fit and 150%.
  This uses the [GDK paintable interface](https://docs.gtk.org/gdk4/iface.Paintable.html);
  canonical geometry and export pixels are unchanged.
- GTK evidence includes native Tab/arrow/Space endpoint operation, semantic
  scalar/reset actions, native dropdown keyboard selection, and screenshot review.
  An owned project in `target/validation/stage22-desktop-preview/` saves exactly
  one document End rotation of 30 degrees with `quadratic_in` easing while Start
  remains zero. Reopening restores both; Reset End and Undo restore the expected
  values. A separate named-channel edit displays Red End 45 degrees while Green
  retains the All End value of 30 degrees.
- Verified runs are `.codex-work/evidence/ui-run-20260905-195457-238055/`
  (Fit/zoom switching), `195631-239512` (save/reset/Undo), and `195928-241862`
  (reopen/named edits). The private session is stopped. An existing GTK critical
  during Tab traversal was localized by GDB to `gtk_expander_focus` calling
  `gtk_root_get_focus`; traversal still reached the endpoints and no Rust panic
  occurred. Automated Sway evidence is not GNOME/Mutter or human acceptance.
- Three focused app temporal tests, the new scheduler cancellation/cache test,
  app build, strict app/engine Clippy, architecture and diff checks pass. Raster,
  vector and video input hashes are unchanged; main still matches origin/main at
  the baseline HEAD and the index is empty. The combined goal remains active.

### Private Advanced Settings endpoint milestone

- Advanced Settings now opens at the selected Start or End endpoint. One
  cancellable worker opens shared media and prepares a box-averaged decoded
  source proxy with a 128-pixel longest edge. Frame/time identity is retained;
  no PNG encode/decode round trip is added. Results are accepted only by the
  live modal epoch. Closing cancels and joins source preparation and stops the
  private evaluation bridge; preview failures remain local to the dialog.
- End edits, reset and easing use the existing domain commands for mapping,
  solid-paint components and applicable output responses. Static settings are
  disabled at End. Numeric entries retain full precision instead of silently
  committing four-decimal display rounding. Scalar edits retain their widgets;
  capability-changing choices rebuild their descriptor projection after the
  initiating GTK signal returns.
- All currently exposes each real channel in its own named group, with outputs
  in painter order. Separate channel values survive private editing, Cancel,
  Apply and reopening. This is not the planned bulk command that applies one
  value to all compatible channel/output targets; that work remains pending.
- Channel/output groups and animation disclosures expose stable names from
  visible labels. Native entries, switches, dropdowns and Reset End buttons
  project applicability and override state; visible field labels supply mnemonic
  relations. Private Sway checks exercise native keyboard entry and dropdown
  selection, semantic reset/actions and readback, and inspected screenshots.
- The owned video project saves Red End contrast `1.234567` with `quadratic_in`
  easing, Red paint alpha `0.35`, and effective Red/Green output maximum fill
  `0.3`/`0.6`. One Apply is one Undo/Redo step over the four edits. Cancel leaves
  them unpublished. Saved Start mapping, paint and response values are unchanged;
  the output End values persist as domain-normalized channel deltas.
- Evidence: `.codex-work/evidence/ui-run-20260905-203336-260020/`, including
  `advanced-end-actions.json`, `advanced-saved-states.json` and native screenshots
  `08-private-end-edits.png`, `10-end-easing.png`, `11-after-apply.png`.
  `13-start-private-preview.png` separately witnesses ordinary Start edits and
  the selected source frame. Fresh process run `204157-266292` restores the exact
  contrast value and reports the native `Ease in` list item as selected, agreeing
  with `15-reopened-selected-easing.png`. The private session is stopped.
  The earlier GTK `gtk_expander_focus`/root critical also occurs during this
  keyboard traversal; no Rust panic occurs. This remains automated Sway evidence,
  not human or GNOME/Mutter acceptance.
- Two focused Advanced endpoint/source and history tests, the directly relevant
  selected-copy locator and bounded-resampler checks, app build, strict app and
  sampling Clippy, formatting, architecture and diff checks pass. Both immutable
  stills and the video are exercised; their hashes are unchanged. Full HEX/hue
  authoring, ALL batches, temporal import/export/settings UI, packaged media tools,
  remaining provider witnesses and final acceptance are still required.
