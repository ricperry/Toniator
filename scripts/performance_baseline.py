#!/usr/bin/env python3
"""Run bounded sequential P0 release probes without clearing OS caches or changing product inputs."""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
OUT = None
PROBE = ROOT / "target/release/examples/performance_probe"


def command(argv):
    """Capture a read-only provenance command without invoking a shell."""
    return subprocess.run(argv, cwd=ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT).stdout


def provenance():
    """Save compiler, hardware, Git, input and executable identity before measurements."""
    paths = ["Cargo.toml", "Cargo.lock", "assets/raster-sample.png", "assets/vector-sample.svg",
             "crates/toniator-engine/examples/performance_probe.rs", "scripts/performance_baseline.py"]
    hashes = {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in paths}
    for path in (OUT / "binaries").glob("*"):
        hashes[str(path.relative_to(ROOT))] = hashlib.sha256(path.read_bytes()).hexdigest()
    hashes[str(PROBE.relative_to(ROOT))] = hashlib.sha256(PROBE.read_bytes()).hexdigest()
    data = {"head": command(["git", "rev-parse", "HEAD"]).strip(), "git_status": command(["git", "status", "--short"]),
            "git_diff_stat": command(["git", "diff", "--stat"]), "compiler": command(["rustc", "-Vv"]),
            "cargo": command(["cargo", "-V"]), "cpu": command(["lscpu"]), "memory": Path("/proc/meminfo").read_text(),
            "uname": platform.uname()._asdict(), "environment": {k: os.environ.get(k) for k in ["RUSTFLAGS", "CARGO_PROFILE_RELEASE_OPT_LEVEL", "RAYON_NUM_THREADS", "CARGO_BUILD_TARGET"]},
            "profile": "Cargo default release opt-level=3; no LTO override; target-cpu not overridden",
            "cache_policy": "first process launch and subsequent launches; no OS page-cache drop or CPU governor changes",
            "hashes": hashes, "timestamp": time.time()}
    (OUT / "provenance.json").write_text(json.dumps(data, indent=2))


def run_case(mode, source, recipe, repeats=3, timeout=240):
    """Run a single isolated workload with wall/RSS supervision and a bounded timeout."""
    name = f"{mode}-{source}-{recipe.replace(':', '-') }"
    directory = OUT / name
    directory.mkdir(parents=True, exist_ok=False)
    argv = ["/usr/bin/time", "-v", "-o", str(directory / "process.time"), str(PROBE), mode, source, recipe, str(directory), str(repeats)]
    started = time.monotonic()
    with (directory / "records.jsonl").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
        try:
            process = subprocess.Popen(argv, cwd=ROOT, stdout=stdout, stderr=stderr, start_new_session=True)
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
            status = "timeout"
    record = {"case": name, "argv": argv, "status": status, "wall_s": time.monotonic() - started, "timeout_s": timeout}
    (directory / "invocation.json").write_text(json.dumps(record, indent=2))
    print(json.dumps(record), flush=True)
    return record


def summarize():
    """Flatten existing phase records to CSV for independent median/phase analysis."""
    rows = []
    for records in sorted(OUT.glob("*/records.jsonl")):
        if records.parent.name.endswith("-diagnostic"):
            continue
        for line in records.read_text().splitlines():
            entry = json.loads(line)
            if entry["kind"] == "evaluation":
                for phase in entry["records"]:
                    rows.append({"case": records.parent.name, "kind": "evaluation", "label": entry["label"], "repeat": entry["repeat"], **{k: phase.get(k) for k in ["stage", "channel", "output", "us", "cache", "execution"]}})
            elif "us" in entry or "us_after_signal" in entry:
                rows.append({"case": records.parent.name, "kind": entry["kind"], "label": entry.get("label", ""), "repeat": entry.get("repeat", ""), "us": entry.get("us", entry.get("us_after_signal"))})
    with (OUT / "timings.csv").open("w") as handle:
        writer = csv.DictWriter(handle, fieldnames=["case", "kind", "label", "repeat", "stage", "channel", "output", "us", "cache", "execution"])
        writer.writeheader()
        writer.writerows(rows)


def main():
    """Select a bounded sequential matrix; record every failed or timed-out case."""
    global OUT
    parser = argparse.ArgumentParser()
    parser.add_argument("batch", choices=["initial", "extended", "summarize"])
    parser.add_argument("--output", required=True, type=Path,
                        help="Explicit fresh evidence directory; summarize reads an existing directory.")
    args = parser.parse_args()
    OUT = args.output.resolve()
    permitted = ROOT / "target/validation/performance-20261016"
    if not (OUT.is_relative_to(permitted / "p0") or OUT.is_relative_to(permitted / "p1") or OUT.is_relative_to(permitted / "p2")):
        parser.error("output must be beneath the P0/P1/P2 performance directories")
    if args.batch != "summarize" and OUT.exists() and any(OUT.iterdir()):
        parser.error("output already contains evidence; choose a fresh directory")
    if args.batch == "extended":
        fixtures = ROOT / "target/validation/ton018-family-regression-20261013/native-projects"
        required = [fixtures / f"{recipe}-{source}-f025.toniator"
                    for recipe in ["two-guide-maze", "source-weighted-dispersion-voronoi", "curve-motif-rows", "three-guide-cells-scale"]
                    for source in ["raster", "vector"]]
        missing = [str(path.relative_to(ROOT)) for path in required if not path.is_file()]
        if missing:
            parser.error("extended requires retained current-format TON-018 validation fixtures: " + ", ".join(missing))
    OUT.mkdir(parents=True, exist_ok=True)
    if args.batch != "summarize":
        provenance()
    if args.batch == "initial":
        for recipe in ["default", "fine"]:
            for source in ["png", "svg"]:
                run_case("evaluate", source, recipe)
        run_case("calibrate", "png", "default", timeout=300)
    elif args.batch == "extended":
        for recipe in ["fixture:two-guide-maze", "fixture:source-weighted-dispersion-voronoi", "fixture:curve-motif-rows", "fixture:three-guide-cells-scale"]:
            for source in ["png", "svg"]:
                run_case("native", source, recipe, repeats=1, timeout=120)
        for source in ["png", "svg"]:
            run_case("sample", source, "default")
            run_case("animation", source, "default", timeout=240)
            run_case("stream", source, "default", timeout=240)
            run_case("cancel", source, "fine")
            run_case("evaluate", source, "tiny")
    summarize()


if __name__ == "__main__":
    main()
