#!/usr/bin/env bash
# Build SelfSemester for Linux with the pinned Ubuntu 22.04 toolchain.
#
# Usage:
#   scripts/linux/build.sh
#
# Artifacts are written to dist/ (override with DIST_DIR=...).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
image="${IMAGE:-selfsemester-linux-builder:latest}"
out="${DIST_DIR:-$repo/dist}"

command -v docker >/dev/null 2>&1 || {
    echo "docker is required: https://docs.docker.com/engine/install/" >&2
    exit 1
}

echo "==> Building $image"
docker build \
    --build-arg "NODE_VERSION=${NODE_VERSION:-22.23.5}" \
    --build-arg "RUST_VERSION=${RUST_VERSION:-1.100.0}" \
    -t "$image" \
    -f "$here/Dockerfile" \
    "$here"

mkdir -p "$out"

echo "==> Building SelfSemester"
docker run --rm \
    -e "HOST_UID=$(id -u)" \
    -e "HOST_GID=$(id -g)" \
    -e "CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2}" \
    -v "$repo:/src:ro" \
    -v "$out:/out" \
    "$image"

echo "==> Artifacts in $out"
ls -lh "$out"
