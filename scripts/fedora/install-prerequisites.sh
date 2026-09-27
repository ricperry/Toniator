#!/usr/bin/env bash
set -Eeuo pipefail

if [[ ! -r /etc/os-release ]]; then
    echo "Cannot identify the operating system from /etc/os-release." >&2
    exit 1
fi
# shellcheck disable=SC1091
. /etc/os-release
if [[ "${ID:-}" != fedora || "${VERSION_ID:-}" != 44 ]]; then
    echo "This installer requires Fedora 44 (found ${ID:-unknown} ${VERSION_ID:-unknown})." >&2
    exit 1
fi
if [[ "$(uname -m)" != x86_64 ]]; then
    echo "This installer requires x86_64 (found $(uname -m))." >&2
    exit 1
fi
if [[ "${EUID}" -ne 0 ]]; then
    echo "Run this script as root, for example: sudo bash scripts/fedora/install-prerequisites.sh" >&2
    exit 1
fi

dnf -y install \
    gcc \
    gcc-c++ \
    pkgconf-pkg-config \
    gtk4-devel \
    libdav1d-devel \
    ffmpeg-free \
    fontconfig \
    dejavu-sans-fonts \
    blueprint-compiler \
    glib2-devel \
    gawk \
    tar \
    gzip \
    zstd \
    git \
    jq \
    ripgrep \
    curl \
    ca-certificates \
    rustup
