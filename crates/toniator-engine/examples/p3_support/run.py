#!/usr/bin/env python3
"""Run serialized current random-family release cases with bounded process groups."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / "target/validation/performance-20261016/p3"
RECIPES = ["even-random-circles", "clustered-dispersion-random-links", "source-weighted-dispersion-voronoi"]


def run(binary, mode, source, recipe, scale, repeats, suffix="", env=None, timeout=120):
    """Supervise one isolated process, retaining failed invocations and every raw record."""
    name = f"{mode}-{source}-{recipe.replace(':','-')}-f{scale}{suffix}"
    directory = OUT / name
    directory.mkdir(exist_ok=False)
    argv = ["/usr/bin/time", "-v", "-o", str(directory / "process.time"),
            str(OUT / "binaries" / binary), mode, source, recipe, scale, str(directory), str(repeats)]
    start = time.monotonic()
    with (directory / "records.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
        process = subprocess.Popen(argv, cwd=ROOT, env={**os.environ, **(env or {})},
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            status = "timeout"
    record = {"case": name, "argv": argv, "env": env or {}, "status": status,
              "wall_s": time.monotonic() - start, "timeout_s": timeout,
              "binary_sha256": hashlib.sha256((OUT / "binaries" / binary).read_bytes()).hexdigest()}
    (directory / "invocation.json").write_text(json.dumps(record, indent=2))
    print(json.dumps(record), flush=True)


def main():
    """Select explicit matrix partitions so building, GUI and other probes never overlap."""
    parser = argparse.ArgumentParser()
    parser.add_argument("batch", choices=["family", "engine", "controls", "diagnostic", "limits", "family-verified", "engine-verified", "edits", "allocation"])
    args = parser.parse_args()
    binary = "random-dispersion-baseline-final"
    if args.batch in ["family", "engine", "diagnostic", "family-verified", "engine-verified"]:
        mode = "engine" if args.batch in ["engine", "engine-verified"] else "family"
        if args.batch == "diagnostic":
            binary = "random-dispersion-diagnostic"
        for recipe in RECIPES:
            for source in ["png", "svg"]:
                for scale in ["1", "025"]:
                    run(binary, mode, source, recipe, scale, 3 if args.batch in ["family", "family-verified"] else 1,
                        "-diagnostic" if args.batch == "diagnostic" else "-verified" if "verified" in args.batch else "",
                        {"P3_DIAGNOSTICS": "1"} if args.batch == "diagnostic" else {"P3_SKIP_SVG":"1"} if args.batch=="engine-verified" else None)
    elif args.batch == "controls":
        for control in ["raw", "stratified", "raw-exclusion", "clustered-exclusion", "raw-lloyd", "raw-lloyd-weighted"]:
            for source in (["png", "svg"] if "weighted" in control else ["png"]):
                run(binary, "family", source, f"control:{control}", "1", 3)
                run("random-dispersion-diagnostic", "family", source, f"control:{control}", "1", 1,
                    "-diagnostic", {"P3_DIAGNOSTICS":"1"})
        for seed in ["0", "123456789"]:
            for recipe in RECIPES:
                run(binary, "family", "png", recipe, "1", 2, f"-seed{seed}", {"P3_SEED": seed})
    elif args.batch == "edits":
        for recipe in [RECIPES[0], RECIPES[2]]:
            run(binary, "engine", "png", recipe, "1", 1, "-edits", {"P3_SKIP_SVG":"1", "P3_EDITS":"1"})
    elif args.batch == "allocation":
        for recipe in RECIPES:
            for source in ["png", "svg"]:
                run("random-dispersion-allocation-final", "family", source, recipe, "025", 1, "-allocation")
    elif args.batch == "limits":
        for recipe in RECIPES:
            run(binary, "family", "png", recipe, "025", 1, "-limit", {"P3_LIMIT": "100"})
            for delay in ["0", "10", "50"]:
                run(binary, "family", "png", recipe, "025", 1, f"-cancel{delay}", {"P3_CANCEL_MS": delay})


if __name__ == "__main__":
    main()
