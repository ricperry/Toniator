#!/usr/bin/env python3
"""Check fixed CreatiatorSuite stylesheet locations without changing any files.

The suite crate is canonical when present. Missing sibling checkouts are explicit
skips; present mismatches fail. Each application builds from its own crate copy.
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
    relative = Path("shared/creatiator/resources/creative-suite.css")
    local = root / relative
    suite = root.parent / "CreatiatorSuite"
    canonical = suite / "creatiator/resources/creative-suite.css" if suite.is_dir() else local
    if not canonical.is_file():
        print(f"FAIL {canonical}: canonical stylesheet missing")
        return 1
    if not suite.is_dir():
        print("SKIP CreatiatorSuite: sibling checkout absent; using local baseline")
    expected = canonical.read_bytes()
    print(f"Creative Suite revision 1 SHA-256 {sha256(expected).hexdigest()}")
    failed = False
    locations = (
        ("Toniator", root),
        ("Chromiator", root.parent / "Chromiator"),
        ("Bezziator", root.parent / "Bezziator"),
    )
    for name, sibling in locations:
        if not sibling.is_dir():
            print(f"SKIP {name}: sibling checkout absent")
            continue
        copy = sibling / relative
        if not copy.is_file():
            print(f"FAIL {copy}: source stylesheet missing")
            failed = True
        elif copy.read_bytes() != expected:
            print(f"FAIL {copy}: stylesheet differs")
            failed = True
        else:
            print(f"OK {copy}")
    return int(failed)


if __name__ == "__main__":
    sys.exit(main())
