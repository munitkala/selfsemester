# Reproducible Linux builds for SelfSemester.
#
# Ubuntu 22.04 is deliberate. It ships glibc 2.35, so the resulting executable
# runs on Ubuntu 22.04+, Debian 12+, Linux Mint 21+, Fedora 36+, RHEL/Rocky/
# Alma 9+, and other current distributions. Building on a newer release would
# raise that minimum.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=never

# Rust/Tauri build dependencies, the runtime libraries linuxdeploy needs to
# resolve, and everything the deb/rpm/AppImage bundlers call.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl wget file xz-utils pkg-config \
        libssl-dev \
        libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
        librsvg2-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev \
        libglib2.0-dev \
        libwebkit2gtk-4.1-0 libgtk-3-0 libsoup-3.0-0 \
        libjavascriptcoregtk-4.1-0 librsvg2-2 libayatana-appindicator3-1 \
        libglib2.0-bin libgtk-3-bin libgdk-pixbuf2.0-bin \
        shared-mime-info hicolor-icon-theme desktop-file-utils \
        libgirepository1.0-dev gobject-introspection \
        gir1.2-glib-2.0 gir1.2-gtk-3.0 gir1.2-gdkpixbuf-2.0 \
        gir1.2-pango-1.0 gir1.2-atk-1.0 \
        zip unzip patchelf binutils dpkg-dev rpm libfuse2 zsync \
    && rm -rf /var/lib/apt/lists/*

# Node.js 22. Ubuntu 22.04's Node 12 is too old for SvelteKit and Vite 8.
ARG NODE_VERSION=22.23.5
RUN set -eux; \
    arch="$(dpkg --print-architecture)"; \
    case "$arch" in \
        amd64) node_arch="x64" ;; \
        arm64) node_arch="arm64" ;; \
        *) echo "Unsupported architecture: $arch" >&2; exit 1 ;; \
    esac; \
    tarball="node-v${NODE_VERSION}-linux-${node_arch}.tar.xz"; \
    base="https://nodejs.org/dist/v${NODE_VERSION}"; \
    curl -fsSL "$base/$tarball" -o "/tmp/$tarball"; \
    curl -fsSL "$base/SHASUMS256.txt" -o /tmp/SHASUMS256.txt; \
    (cd /tmp && grep " $tarball\$" SHASUMS256.txt | sha256sum -c -); \
    mkdir -p /opt/node; \
    tar -xJf "/tmp/$tarball" -C /opt/node --strip-components=1; \
    ln -sf /opt/node/bin/node /usr/local/bin/node; \
    ln -sf /opt/node/bin/npm /usr/local/bin/npm; \
    ln -sf /opt/node/bin/npx /usr/local/bin/npx; \
    rm -f "/tmp/$tarball" /tmp/SHASUMS256.txt; \
    node --version; npm --version

# Rust. Edition 2024 needs 1.85 or newer; this is the release used for 0.7.1.
ARG RUST_VERSION=1.100.0
RUN curl -fsSL https://sh.rustup.rs -o /tmp/rustup.sh \
    && sh /tmp/rustup.sh -y --profile minimal --default-toolchain "$RUST_VERSION" \
    && rm -f /tmp/rustup.sh \
    && /root/.cargo/bin/rustc --version && /root/.cargo/bin/cargo --version
ENV PATH="/root/.cargo/bin:${PATH}"

COPY build-inside.sh /usr/local/bin/build-selfsemester
RUN chmod +x /usr/local/bin/build-selfsemester

ENTRYPOINT ["/usr/local/bin/build-selfsemester"]
