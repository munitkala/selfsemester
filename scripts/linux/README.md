# Linux builds

Builds the Linux release: a plain ELF executable, a `.deb`, an `.rpm` and an
AppImage, then packs them into a zip with a separate checksum zip.

The build runs in a pinned **Ubuntu 22.04** container. That is deliberate:
Ubuntu 22.04 has glibc 2.35, so the executable runs on Ubuntu 22.04+, Debian
12+, Linux Mint 21+, Fedora 36+, RHEL/Rocky/Alma 9+, Arch, openSUSE 15.5+ and
other current distributions. Building on a newer release would raise that
minimum.

## Requirements

- Docker (with the daemon running)
- Network access (apt, npm, crates.io, and the AppImage tooling)
- About 10 GB of free disk space
- 8 GB RAM recommended; 2 GB works with `CARGO_BUILD_JOBS=2` (the default)
- 20-40 minutes, mostly the first Rust compile

## Usage

From the repository root:

```bash
scripts/linux/build.sh
```

The first run builds the container image (installs Node 22, Rust, GTK/WebKitGTK
and the bundling tools); later runs reuse it. Source is mounted read-only and
the build happens in `/tmp` inside the container.

Artifacts land in `dist/`:

```
dist/
  SelfSemester-<version>-linux-<arch>.zip
  SelfSemester-<version>-linux-<arch>.sha256.zip
```

The main zip contains the plain `SelfSemester` ELF, the `.deb`, the `.rpm`, the
`.AppImage`, `README.txt` and `LICENSE`. The checksum zip contains
`SHA256SUMS.txt` plus one `.sha256` file per artifact.

## Options

| Variable / build arg | Default | Meaning |
| --- | --- | --- |
| `CARGO_BUILD_JOBS` | `2` | Parallel Rust jobs. Lower it on memory-constrained machines; raise it with more RAM. |
| `DIST_DIR` | `<repo>/dist` | Where the two zips are written. |
| `IMAGE` | `selfsemester-linux-builder:latest` | Docker image tag. |
| `NODE_VERSION` | `22.23.5` | Node 22 build to download. |
| `RUST_VERSION` | `1.100.0` | Rust toolchain to install. |

The last two are Docker build args:

```bash
NODE_VERSION=22.23.5 RUST_VERSION=1.100.0 scripts/linux/build.sh
```

## What it does

1. Builds the SvelteKit frontend (`npm ci && npm run build`).
2. Compiles the Tauri backend in release mode with `tauri/custom-protocol`, the feature the
   Tauri CLI adds so the app uses the bundled interface rather than the dev server.
3. Generates the `.deb`, `.rpm` and AppImage with `tauri bundle`.
4. Copies the standalone binary **before** bundling, because the bundler
   patches it in place with bundle-type metadata.
5. Packs everything and writes SHA-256 checksums.

## Notes

- The AppImage bundles GTK 3 and WebKitGTK. It needs FUSE 2 at runtime, or it
  can be run with `--appimage-extract-and-run`. The other formats use the
  system WebKitGTK 4.1.
- The `.deb` and `.rpm` add a desktop-menu entry and icons; the AppImage and
  plain ELF do not.
- `tauri bundle` downloads `linuxdeploy` and its plugins from GitHub on first
  use. They are cached in the container's `~/.cache/tauri`.
- Only x86-64 and aarch64 are wired up. Building for aarch64 on an x86-64 host
  needs qemu/binfmt (`docker run --privileged tonistiigi/binfmt --install arm64`)
  and is much slower under emulation.
- The source tree is not modified. `dist/` is ignored by git.
