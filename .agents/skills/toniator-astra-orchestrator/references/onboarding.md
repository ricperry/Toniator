# Astra onboarding decisions — 2026-09-04

Read when revising model/tool routing or checking setup provenance, not on every
task. This record is development-tooling evidence, not product acceptance.

## Checkout and scope

Repository: `/home/ricperry1/projects/Toniator`.
HEAD: `8cd4a2f5dca4521f3acc974e3ba8a8954efc97d1`, `main`, tracking `origin/main`.
The starting worktree already contained edits to all seven agent definitions,
four skills, GTK helpers, AGENTS/README/.gitignore, deleted planning/assets,
and untracked artwork, references, accessibility helpers, and product artifacts.
Those changes were preserved. This pass changes development guidance, model
routing, and stable UI references only; no Rust/product edits or commits.

The tracker and Git record Gate 21B-3 accepted at implementation `68ef02e` and
documentation `8cd4a2f`. Gate 21B-4 remains planned. The roadmap's Stage 21B
status paragraph still calls Gate 3 planned; this stale status was recorded,
not used to reopen work or change the approved roadmap.

## Model evidence and support

All sources below were opened on 2026-09-04; public prices and availability can
change. Historical benchmark costs are not current account quota estimates.

- [OpenAI custom-agent configuration](https://learn.chatgpt.com/docs/agent-configuration/subagents)
  documents role-local `model` / `model_reasoning_effort`, inheritance, and
  custom-file precedence. Narrow agents suit Luna; demanding agents may need Sol.
- [Luna model documentation](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
  advertises Max and image input. Its displayed standard API price is
  $0.20 input / $1.20 output per million tokens; pricing conditions apply.
- [OpenAI model catalog](https://developers.openai.com/api/docs/models/all)
  identifies Astra, Sol, Terra, and Luna; the active host confirms their exact IDs.
- [OpenAI GPT-5.6 evaluation](https://openai.com/index/gpt-5-6/)
  reports OSWorld 2.0 at Sol 62.6, Terra 50.2, Luna 45.6. This supports stronger
  escalation for difficult GUI recovery, not a claim of equivalent GUI skill.
  The release page records later price reductions; its launch prices are stale.
- [Artificial Analysis family evaluation](https://artificialanalysis.ai/articles/gpt-5-6-has-landed/)
  compares Max configurations in Codex: Coding Agent Index Sol 80, Terra 77,
  Luna 75, including repository work, terminal tasks, and code Q&A. It reports
  Luna/Sol as better intelligence/cost choices than Terra across tested efforts.
  These July measurements support an initial Luna default; they do not prove
  equal Toniator performance or current dollars per task.
- [Artificial Analysis Astra evaluation](https://artificialanalysis.ai/articles/benchmarking-gpt-6-astra)
  provides a September update for the frontier parent, complementing
  [official Astra guidance](https://developers.openai.com/api/docs/guides/latest-model?model=gpt-6-astra).
  Parent selection is explicitly user-directed; specialist use remains selective.

Do not compare unrelated benchmark scores as a single ranking. No public
evaluation establishes Toniator-specific review correctness or GTK accessibility
success. Observe real tasks rather than creating a model benchmark project.

Local verification:

- `codex --version`: `codex-cli 0.153.2`.
- Inspected every `.codex/agents/*.toml` definition and current collaboration
  role metadata. Names, model IDs, and reasoning efforts are separate settings.
- `models_cache.json` reports client 0.153.2, refreshed 2026-09-04, and
  `gpt-5.6-luna` supports low/medium/high/xhigh/max. Astra/Sol/Terra additionally
  advertise ultra. This is local host metadata, not proof of every account limit.
- Generated app-server JSON schema under
  `target/validation/astra-onboarding/codex-schema/`; `ReasoningEffort` is a
  nonempty string. Therefore schema acceptance alone cannot validate effort
  support; model metadata and the active spawn surface provide that evidence.
- The active runtime already advertised Luna Max for the custom roles even
  though disk files held older values. Disk edits align future loading; they
  do not prove a live role was hot-reloaded. A generic Luna Max navigation
  agent was explicitly requested for useful onboarding work.

## Complete agent scrub

Old values are the working-tree definitions at onboarding start, not HEAD.
All seven files predate Astra and retain useful earlier workflow responsibilities.
Their exact initial model-choice rationale is undocumented, so it is not
invented here. No role had an Astra-era assignment recorded on disk.

| Agent | Old | New | Task characteristics and decision |
| --- | --- | --- | --- |
| desktop_implementer | Terra High | Luna Max | Settled Rust/GTK implementation; close coding results favor starting cheaper. Escalate difficult geometry/refactors. |
| codebase_explorer | Luna Medium | Luna Max | Focused ownership/caller tracing; Max has comparable-harness Q&A evidence. Lower effort remains eligible after task evidence. |
| test_reviewer | Luna Medium | Luna Max | Defect and contract review; bounded first pass with parent triage. Consequential architecture/numerical review can go directly to Sol. |
| creative_tester | Terra Medium | Luna Max | Bounded screenshot/artifact comparison; parent independently inspects pixels. Escalate difficult perception/recovery. |
| ux_reviewer | Terra High | Luna Max | Concrete affected-workflow review; no default Terra niche demonstrated. |
| ux_strategist | Luna High | Luna Max | Bounded options against known authority; unresolved cross-domain decisions stay with Astra or escalate to Sol. |
| documentation_maintainer | Luna High | Luna Max | Milestone synthesis from verified evidence; initial common Max baseline, not mandatory effort forever. |

The default is a hypothesis, not seven role-specific measured wins. Public
lower-effort comparisons do not establish equal reliable Toniator outcomes;
keep routing simple initially and lower effort where real evidence supports it.
Ordinary extraction/mechanical work may be done directly or at lower effort.
Do not send a difficult task to Luna merely because a named role defaults there.
Configured roles override spawn settings: use an available generic role with
explicit model/effort and the specialist brief when escalating a fixed role.

## Preserved and simplified infrastructure

Retained: protected specs/assets, current schemas, one semantic authority,
one writer, stage gates, narrow scope, GTK-only frontend boundaries, parent
visual inspection, truthful AT-SPI controls, private Sway isolation, and Git
authorization. Existing accessibility improvements were preserved.

Simplified: mandatory per-agent CACHE_UPDATE headings and immediate writes,
routine delegation, repeated unchanged checks, and model-specific Terra prose.
The evidence and milestone skills remain useful conditional tools.
The previous orchestrator remains intact apart from a preference/precedence
notice. No semantic-map command was executed. AGENTS and README retire active
requirements; historical reports/plans/cache and the separate project remain.

## Repository-navigation decision

Within `crates/toniator-app/src/main.rs`, the read-only navigation audit traced
`schedule_main_preview_submission` (`main.rs:15835`) →
`submit_if_viewport_ready` (`main.rs:15839`) → `submit_current_source`
(`main.rs:18455`) → history-snapshot `EvaluationRequest` and scheduler submission
(`main.rs:18491`) → `EvaluationScheduler::submit`
(`crates/toniator-engine/src/lib.rs:2836`).
Narrow reads verified prior-ticket cancellation and queued evaluation; `rg`
located callers and Git tied the seam to `68ef02e`. Cargo confirmed the engine's
headless domain/IO/patterns/render/sampling dependencies. No LSP interaction
was claimed merely from installed rust-analyzer 1.94.1.

The candidate [sunerpy/codegraph-rust](https://github.com/sunerpy/codegraph-rust)
has active [releases](https://github.com/sunerpy/codegraph-rust/releases)
(v0.50.3 on 2026-09-02), MIT licensing, SQLite/FTS5 indexing, tree-sitter
extraction, and caller/impact queries. Its
[architecture](https://github.com/sunerpy/codegraph-rust/blob/main/docs/architecture.md)
uses custom name/import resolution rather than rust-analyzer/HIR.
Its [MCP setup](https://github.com/sunerpy/codegraph-rust/blob/main/docs/mcp.md)
can create configuration, instructions, a project index, and watcher state.
Documentation examples show version/tool-count drift. These are candidate
capabilities, not verified Toniator query results: it was not installed or run.
The similarly named Jakedismo project has a heavier database/embedding setup.

Decision: keep `rg` + Git + Cargo + available LSP. The real path was resolved
without a demonstrated gap; another persistent index has not earned its setup
and freshness cost. Reconsider on a concrete repeated navigation limitation,
with an isolated pinned trial and measured useful results. No new navigation
skill, daemon, dependency, or repository index was created.

## Computer-use functional verification

Used the existing debug executable without rebuilding product code; this is a
harness smoke check, not fresh-build or product-stage acceptance.
`preflight` passed. A new private session launched `assets/raster-sample.png`.
Used `ui wait`, scoped `controls`, and `inspect` for Channel settings, then
`toggle`, `wait --state unpressed`, and `inspect`. Native pressed state changed
true → false and the inspector disappeared in the captured screenshot.
A second toggle followed by `wait --state pressed` restored it. Immediate
action readback can precede GTK propagation; the existing bounded wait resolves
that without guessed delays or a tooling rewrite.

Parent inspected `astra-before.png` and `astra-hidden.png` in
`.codex-work/evidence/ui-run-20260904-170944-51707/`.
Evidence bundle collected; session-stop succeeded. The app log contains one
GtkStack minimum-width warning (60 requested, 74 needed); no crash was observed.
This warning was not diagnosed or fixed during tooling onboarding.
No semantic helper or Sway script changes were needed. This is automated
Sway/wlroots evidence, not manual GNOME/Mutter acceptance or a rendering audit.

## UI references

`docs/ui/REFERENCES.md` records stable v1.2 PDF/mockup paths and exact hashes.
The user identified `assets/Stage21D_Mockup/toniator_main_ui_spec_v1_2.pdf` as the
updated spec. Its four pages and the existing mockup were visually inspected;
page 1 embeds the visually matching mockup. v1.2 resolves v1.1's header/status
placement differences and explicitly preserves actual channel/mixed-value
authority. The earlier exact v1.1 copy was moved to
`target/validation/astra-onboarding/toniator_main_ui_spec_v1_1.pdf`; it is not
active design authority. No UI semantics or protected specification changed.

The user also supplied `PatternAndPresetTerminologyRefactor.md` as terminology
clarification. The skill and reference note distinguish structural Patterns,
document-level Presets, and actual `.toniator` projects; Gate 21B-4 remains
personal Pattern management. Internal preset-v4 and related names/versions are
preserved. This onboarding does not claim to have executed the separate
normative-document correction described in that file.

## Verification summary

Skill frontmatter validation passed for the new orchestrator and all three
revised existing skills. Checked all seven complete role-file structures,
unique names, descriptions, sandbox settings, and model/effort fields against
advertised Luna Max support. This is a bounded structural/configuration check,
not an exhaustive TOML parser or proof of future model quality.
`git diff --check` passed. Protected specifications, Rust/Cargo files, tracker,
and roadmap have no onboarding diff. No product test suite was run because no
product code or dependency changed. Real navigation and private GTK checks are
recorded above. HEAD remains `8cd4a2f`, with zero ahead/behind against the local
`origin/main` ref; no fetch, commit, or push was performed.

Independent Luna Max forward review covered clipping, v1.2 UI implementation,
Sol escalation, and small documentation work. It found no remaining active-path
or excess-process defect, but requested a concrete escalation mechanism for
fixed Luna roles. The parent resolved that with the available collaboration
schema: generic `default`, explicit Sol/effort, no full-history fork, and a
complete specialist brief. This same generic override mechanism was already
used for useful Luna navigation/review work. No Sol execution/model-attestation
claim is made. The review exceeded its initial timebox; it was interrupted and
returned existing findings without further investigation. One such run does
not establish a broad model-quality conclusion.

## Invalidation

Revisit relevant findings after role/runtime/model changes, affected helper or
app changes, changed stage authorization, reference replacement, or substantial
new navigation/routing evidence. Do not rerun this entire onboarding on each task.
