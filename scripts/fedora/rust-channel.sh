#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"

awk -F '"' '
    /^\[toolchain\][[:space:]]*$/ { in_toolchain = 1; next }
    /^\[/ { in_toolchain = 0 }
    in_toolchain && /^[[:space:]]*channel[[:space:]]*=/ {
        count++
        if (NF < 3 || $2 !~ /^[0-9]+\.[0-9]+\.[0-9]+$/) {
            invalid = 1
        }
        channel = $2
    }
    END {
        if (count != 1 || invalid) {
            print "rust-toolchain.toml must contain one pinned semantic toolchain channel" > "/dev/stderr"
            exit 1
        }
        print channel
    }
' "${REPO_ROOT}/rust-toolchain.toml"
