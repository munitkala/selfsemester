#!/usr/bin/env bash
# Runs inside the Docker image built from scripts/linux/Dockerfile.
#
# Inputs (mounted by build.sh):
#   /src  repository checkout, mounted read-only
#   /out  output directory for the release zips
#
# Outputs:
#   $OUT/SelfSemester-<version>-linux-<arch>.zip
#   $OUT/SelfSemester-<version>-linux-<arch>.sha256.zip
set -euo pipefail

SRC="${SRC:-/src}"
OUT="${OUT:-/out}"
WORK="${WORK:-/tmp/selfsemester-build}"
: "${CARGO_BUILD_JOBS:=2}"
export CARGO_BUILD_JOBS

[ -f "$SRC/package.json" ] || { echo "No package.json in $SRC" >&2; exit 1; }
[ -f "$SRC/scripts/linux/artifact-readme.txt" ] || {
    echo "Missing $SRC/scripts/linux/artifact-readme.txt" >&2
    exit 1
}

version="$(node -p "require('$SRC/package.json').version")"
arch="$(uname -m)"
case "$arch" in
    x86_64)
        deb_arch="amd64"
        rpm_arch="x86_64"
        zip_arch="x86_64"
        ;;
    aarch64)
        deb_arch="arm64"
        rpm_arch="aarch64"
        zip_arch="aarch64"
        ;;
    *)
        echo "Unsupported architecture: $arch" >&2
        exit 1
        ;;
esac

echo "==> SelfSemester $version ($arch)"
echo "==> Copying source to $WORK"
rm -rf "$WORK"
mkdir -p "$WORK"
(
    cd "$SRC"
    tar -cf - \
        --exclude=.git \
        --exclude=node_modules \
        --exclude=build \
        --exclude=.svelte-kit \
        --exclude=src-tauri/target \
        --exclude=dist \
        . | tar -xf - -C "$WORK"
)

cd "$WORK"
export CI=1

echo "==> Installing JavaScript dependencies"
npm ci --no-audit --no-fund

echo "==> Building the frontend"
npm run build

echo "==> Building the Rust binary (CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS)"
# `tauri build` passes tauri/custom-protocol. Without it Tauri stays in dev mode and the app opens
# http://localhost:1420 instead of the bundled interface.
(
    cd src-tauri
    cargo build --release --locked --features tauri/custom-protocol
)

# Keep a pristine copy. `tauri bundle` patches target/release/selfsemester in
# place with bundle-type metadata, and the standalone executable should not
# carry that.
stage="$WORK/stage"
mkdir -p "$stage"
cp src-tauri/target/release/selfsemester "$stage/SelfSemester"

echo "==> Building deb, rpm and AppImage bundles"
# linuxdeploy ships as an AppImage; this lets it run in containers without FUSE.
export APPIMAGE_EXTRACT_AND_RUN=1
npx tauri bundle --bundles deb,rpm,appimage --ci

cp src-tauri/target/release/bundle/deb/*.deb "$stage/"
cp src-tauri/target/release/bundle/rpm/*.rpm "$stage/"
cp src-tauri/target/release/bundle/appimage/*.AppImage "$stage/"
cp LICENSE "$stage/LICENSE"
sed \
    -e "s/@VERSION@/${version}/g" \
    -e "s/@DEB_ARCH@/${deb_arch}/g" \
    -e "s/@RPM_ARCH@/${rpm_arch}/g" \
    -e "s/@ZIP_ARCH@/${zip_arch}/g" \
    scripts/linux/artifact-readme.txt > "$stage/README.txt"
chmod 755 "$stage/SelfSemester" "$stage"/*.AppImage

base="SelfSemester-${version}-linux-${zip_arch}"
mkdir -p "$OUT"
rm -f "$OUT/$base.zip" "$OUT/$base.sha256.zip"

echo "==> Writing $OUT/$base.zip"
(cd "$stage" && zip -X -9 -q "$OUT/$base.zip" *)

echo "==> Writing checksums"
hashdir="$WORK/hashes"
mkdir -p "$hashdir"
(
    cd "$stage"
    for f in *; do
        [ -f "$f" ] || continue
        sha256sum "$f"
    done > "$hashdir/SHA256SUMS.txt"
    for f in *; do
        [ -f "$f" ] || continue
        sha256sum "$f" > "$hashdir/$f.sha256"
    done
)
(cd "$hashdir" && zip -X -9 -q "$OUT/$base.sha256.zip" *)

# The container runs as root; hand the artifacts back to the invoking user.
if [ -n "${HOST_UID:-}" ] && [ -n "${HOST_GID:-}" ]; then
    chown -R "$HOST_UID:$HOST_GID" "$OUT"
fi

echo "==> Done"
ls -lh "$OUT/$base.zip" "$OUT/$base.sha256.zip"
