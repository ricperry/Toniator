#!/usr/bin/env python3
"""Freeze genuine engine-measured companions without fitting or changing observations.

Input directory contains projects/{hatch-parallel,hatch-rings,ranked}.toniator-pattern
and matching {key}-binding.txt, produced by the ordinary engine fixture generator.
Usage: python3 tools/update_ranked_hatch_companions.py ARTIFACT_DIRECTORY
The complete Debug binding is a conservative reuse witness, never a parsed key.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
artifacts = Path(sys.argv[1])
records = []
provenance = []
dispersion = ""

def knots(values):
    """Keep exact Python shortest-roundtrip binary64 literal values for each measurement."""
    return "\n".join(
        f'                CalibrationKnot {{ input: {float(k["input"])!r}, coverage: {float(k["coverage"])!r} }},'
        for k in values
    )

for key, expected_id in [
    ("hatch-parallel", "parallel-hatching"),
    ("hatch-rings", "concentric-hatching"),
    ("ranked", "ranked-blue-noise-marks"),
]:
    path = artifacts / "projects" / f"{key}.toniator-pattern"
    record = json.loads(path.read_text())
    artifact = record["saved_calibration"]
    assert record["metadata"]["id"] == expected_id
    assert artifact.get("uniform") is None and artifact.get("variation") is None
    if key != "ranked":
        assert "interleaved-hatch-v3" in artifact["profile"]
        assert len(artifact["raw"]) == 9
    binding = (artifacts / f"{key}-binding.txt").read_text()
    records.append((expected_id, f'''        {json.dumps(expected_id)} => (
            {json.dumps(binding)},
            {json.dumps(artifact["profile"])},
            &[
{knots(artifact["raw"])}
            ],
            None,
            None,
        ),
'''))
    if key == "ranked":
        dispersion = knots(artifact["dispersion"]["observations"])
    provenance.append({
        "id": expected_id,
        "source_artifact": str(path),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "profile": artifact["profile"],
        "raw_observations": len(artifact["raw"]),
        "measured_discrete_populations": 9 if key != "ranked" else None,
    })
source = '''//! Genuine ranked-screen and discrete hatch measurements from the ordinary engine.
//! Provenance: resources/ranked-hatch-calibration-provenance.json; no runtime fitting.
use super::{CalibrationKnot, MeasuredData};

/// Returns only the frozen complete current binding and its actually measured numeric payload.
pub(super) fn data(id: &str) -> Option<MeasuredData<'static>> {
    Some(match id {
''' + "".join(value for _, value in sorted(records)) + '''        _ => return None,
    })
}

/// Returns the ranked layout's genuine additional occupancy observations, separate from mark size.
pub(super) fn dispersion(id: &str) -> Option<&'static [CalibrationKnot]> {
    (id == "ranked-blue-noise-marks").then_some(&[
''' + dispersion + '''
    ])
}
'''
output = root / "crates/toniator-patterns/src/ranked_hatch_calibrations_data.rs"
output.write_text(source)
subprocess.run(["rustfmt", "--edition", "2024", str(output)], check=True)
metadata = {
    "generation": "greenfield-rewrite",
    "producer": "toniator_engine::reference_calibration::calibrate_saved_pattern",
    "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
    "working_tree": "ranked/hatch candidate plus preserved prior TON010/compute/shape changes",
    "reproduce_freeze": "python3 tools/update_ranked_hatch_companions.py ARTIFACT_DIRECTORY",
    "blue_noise_generator": "tools/generate_blue_noise.py",
    "blue_noise_table_sha256": hashlib.sha256((root / "crates/toniator-patterns/src/blue_noise_64.bin").read_bytes()).hexdigest(),
    "artifacts": provenance,
}
(root / "crates/toniator-patterns/resources/ranked-hatch-calibration-provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")
