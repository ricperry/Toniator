# SUPERSEDED — TON-010 Stage 1 regression review

This review covered the pre-policy compatibility-preservation implementation.
The current v7 document definition rejects obsolete definitions instead.

- Review status: not accepted at first review.
- Major finding: `Document.compatibility_pattern` was not carried by inactive
  treatment snapshots, so opaque values could be lost during output/treatment
  transitions.
- Gate finding: the registry wrappers had no explicit legacy adapter functions
  or output-parity coverage, while the Stage 1 gate required legacy-adapter
  evidence.
- Minor finding: record validation was persistence-boundary-only by design;
  retain or clarify that delayed-failure contract if it remains intentional.
- No TON-013 overlap was found in the reviewed model/persistence/pattern
  symbols; dirty UI/resource/docs changes remain unrelated and preserved.
- Required follow-up: preserve compatibility values across transitions, add
  explicit Shapes/Curves adapter entry points and focused parity tests, then
  rerun Stage 1 verification.
