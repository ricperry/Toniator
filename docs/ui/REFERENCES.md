# Main-window UI references

Established during Astra onboarding on 2026-09-04. These are design references,
not a record of implemented features or authorization to begin a product stage.

Product vocabulary follows the user's
[Pattern/Presets clarification](../../PatternAndPresetTerminologyRefactor.md):
Patterns are structural recipes applicable to All or a named channel; Presets
are reusable document-level configurations that may contain different Patterns
and settings per channel. The gallery/wizard and current personal library hold
Patterns. A `.toniator` file is the actual project, not either reusable resource.
Use Pattern Recipe, Pattern Gallery, Current Pattern, Change..., and Edit channel.
Gate 21B-4 personal management remains personal Patterns; this clarification
does not expand it to document-level Presets.
The user added Gate 21B-5 on 2026-09-04 for document-level Presets after
Gate 21B-4. As a supplement to the mockup, place **Load preset...** and
**Save preset**, in that order, under the main-window **New** split/dropdown
button, preserving its primary New action. These entries serve reusable
document/per-channel configurations, including different Patterns per channel.
They are separate from the Pattern Gallery and do not establish a permanent
Preset manager panel. The preferred storage direction is the `.toniator`
document structure without source data or source-specific references, pending
an explicit source-free format contract. Detailed save/load semantics remain Gate 21B-5 planning
work; the entries are planned, not implemented.
Preserve existing internal `preset_format_version`, preset-v4, `presets/`,
registry/CLI/Rust names and versions. They are internal naming artifacts where
they store structural Patterns, not evidence of a document-Preset implementation.
The clarification file also describes a bounded normative-document correction;
this onboarding incorporates its vocabulary without claiming that separate
protected-specification/documentation pass has been performed.

- Specification: [Toniator Main UI Specification v1.2](../../assets/Stage21D_Mockup/toniator_main_ui_spec_v1_2.pdf).
  User identified this updated repository reference during onboarding;
  SHA-256 `7c0696a8c8085a73b38683c366d9e0ba1e7512671daffdf0b1cfa22cab902141`.
- Main mockup: [MainWindowMockup.png](../../assets/Stage21D_Mockup/MainWindowMockup.png).
  Existing user-owned repository image; SHA-256
  `45a53ba92c4ca89f5d56e48f357821b2c02ab056d06c558a576ac9b812691e3b`.
  Editable original: `assets/Stage21D_Mockup/MainWindowMockup.kra`.
  Visual inspection shows the matching no-sidebar layout, right-hand Color &
  Channel / Pattern Recipe / Appearance / Variation / Options / Advanced groups,
  Family / Sites / Connections summary, and Preview / Source control.
  The v1.2 PDF embeds this visually matching reference on page 1 and names its
  attachment baseline `MainWindowMockup(1).png`; the stable repository filename
  is `MainWindowMockup.png`. No exact embedded-image byte comparison is claimed.
  Do not substitute an older image or a backup `~` file.

- Startup mockup: [SplashMockup.png](../../assets/Stage21D_Mockup/SplashMockup.png).
  On 2026-09-04 the user authorized its startup screen and persistent Recent
  Files. The left card has one **Start New Project** button, with a textual
  hint explaining that it also opens existing projects. Close returns here
  after resolving unsaved work; Exit and window X quit after that decision.
  Cards and controls inherit the system light/dark theme; the mockup supplies
  the banner artwork. This follow-up does not begin document Presets.

For presentation decisions, later explicit user instruction outranks v1.2,
which outranks its corresponding mockup and older UI artifacts. v1.2 supersedes
the onboarding document's v1.1 reference. Use pure GTK4; do not reintroduce
libadwaita. Persistent New and Help belong in the header; the dismissible status
area belongs between Fit and View. Channel selection uses the actual model's
channels, readable channel-colored highlights, and explicit text labels.

The protected Addendum and accepted domain contracts remain semantic authority.
The PDF is not a domain/schema revision. Apply its presentation through current
contracts:

- v1.2 section 6 explicitly defers mixed recipe presentation to existing
  authority. Addendum sections 15/17 govern document-base edits, retained channel
  deltas, and Pattern-replacement reset semantics (older internal/specification
  text calls these "presets"). Do not falsely imply identical recipes.
- Use actual implemented channel models; the PDF adds no new models and edit
  selection never implicitly isolates preview.
- Pattern scale and Stretch (X/Y) must project defined domain units. They do
  not authorize independent density, shape-size, or transform authorities.
- Menu contents, Change workflow, Export UI, and Advanced details are explicitly
  outside the exhaustive scope of this main-window specification.

No protected specification, stage status, or product implementation changed as
part of establishing these references.
