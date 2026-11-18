#!/usr/bin/env python3
"""Check vendored Creative Suite stylesheet parity without changing any files.

The Toniator copy is canonical. Missing sibling checkouts are explicit skips;
present mismatches fail. No sibling is required to build or run an application.
"""
from hashlib import sha256
from pathlib import Path
import argparse
import sys


def main() -> int:
    """Compare present sibling copies and return nonzero for a present mismatch."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Check only (also the default)")
    parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    canonical = root / "crates/toniator-app/resources/creative-suite.css"
    expected = canonical.read_bytes()
    print(f"Creative Suite revision 1 SHA-256 {sha256(expected).hexdigest()}")
    failed = False
    for name in ("Chromiator", "Bezziator"):
        sibling = root.parent / name
        if not sibling.is_dir():
            print(f"SKIP {name}: sibling checkout absent")
            continue
        copies = list(sibling.glob("**/creative-suite.css"))
        # Build outputs are derived; compare only the checked-in source locations.
        copies = [path for path in copies if not any(part in {"target", ".git", "dist"} for part in path.relative_to(sibling).parts)]
        if len(copies) != 1:
            print(f"FAIL {name}: expected one source stylesheet, found {len(copies)}")
            failed = True
        elif copies[0].read_bytes() != expected:
            print(f"FAIL {copies[0]}: stylesheet differs")
            failed = True
        else:
            print(f"OK {copies[0]}")
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
