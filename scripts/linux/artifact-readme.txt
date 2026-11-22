SelfSemester @VERSION@ - Linux @ZIP_ARCH@
=========================================

SelfSemester turns any subject into a full, deadline-based, graded course.
Built from source by this project.

Contents
--------
  SelfSemester
      plain 64-bit ELF executable (PIE, stripped)
  SelfSemester_@VERSION@_@DEB_ARCH@.deb
      Debian/Ubuntu/Mint package
  SelfSemester-@VERSION@-1.@RPM_ARCH@.rpm
      Fedora/RHEL/openSUSE package
  SelfSemester_@VERSION@_@DEB_ARCH@.AppImage
      portable single-file bundle
  README.txt
      this file
  LICENSE
      MIT licence

Pick one of the four, depending on your distribution.

1. Debian / Ubuntu / Linux Mint
-------------------------------
  sudo apt install ./SelfSemester_@VERSION@_@DEB_ARCH@.deb
  selfsemester

  This installs /usr/bin/selfsemester, a desktop-menu entry and icons, and
  pulls in WebKitGTK/GTK automatically. Uninstall with:
    sudo apt remove self-semester

2. Fedora / RHEL / Rocky / Alma / openSUSE
------------------------------------------
  sudo dnf install ./SelfSemester-@VERSION@-1.@RPM_ARCH@.rpm
  selfsemester

  On openSUSE use:
    sudo zypper install ./SelfSemester-@VERSION@-1.@RPM_ARCH@.rpm

3. Any distribution - AppImage
------------------------------
  chmod +x SelfSemester_@VERSION@_@DEB_ARCH@.AppImage
  ./SelfSemester_@VERSION@_@DEB_ARCH@.AppImage

  The AppImage bundles GTK 3 and WebKitGTK, so they do not need to be
  installed. It does need FUSE 2 to mount itself; if your distribution only
  ships FUSE 3 (Ubuntu 22.04+, Fedora 36+), either install libfuse2:
    sudo apt install libfuse2        # Debian/Ubuntu/Mint (libfuse2t64 on Ubuntu 24.04+)
    sudo dnf install fuse-libs       # Fedora
  or run it without FUSE:
    ./SelfSemester_@VERSION@_@DEB_ARCH@.AppImage --appimage-extract-and-run

4. Plain ELF executable - no installer
--------------------------------------
  chmod +x SelfSemester
  ./SelfSemester

  The smallest option and the easiest to drop anywhere, but it uses the
  WebKitGTK and GTK already installed on the system:

    Debian / Ubuntu / Linux Mint:
      sudo apt install libwebkit2gtk-4.1-0 libgtk-3-0 libsoup-3.0-0 \
                       libjavascriptcoregtk-4.1-0

    Fedora:
      sudo dnf install webkit2gtk4.1 gtk3 libsoup3

    Arch / Manjaro:
      sudo pacman -S webkit2gtk-4.1 gtk3 libsoup3

    openSUSE:
      sudo zypper install libwebkit2gtk-4_1-0 libgtk-3-0 libsoup-3_0-0

Compatibility
-------------
  Architecture : @ZIP_ARCH@, 64-bit
  C library    : glibc 2.35 or newer for the AppImage and packages
                 (all built on Ubuntu 22.04). The plain ELF only uses glibc
                 symbols up to 2.34, so it also runs on 2.34 systems.

                 In practice: Ubuntu 22.04+ / Linux Mint 21+, Debian 12+,
                 Fedora 36+, RHEL/Rocky/Alma 9+, Arch, openSUSE 15.5+.

  Window system: X11 or Wayland (via GTK 3)

  These builds are for @ZIP_ARCH@. They will not run on other architectures.
  The AppImage is the most portable of the four; the plain ELF is the
  smallest and starts fastest.

Data locations
--------------
  ~/.local/share/SelfSemester   database, courses, sealed exams, logs
  ~/Documents/SelfSemester      completed-course archives and backups

Set SELFSEMESTER_HOME=/some/folder to keep everything under one directory
(useful for a portable or demo copy). Set SELFSEMESTER_NO_CLAUDE=1 to stop
the app from starting Claude jobs.

Notes
-----
  - Courses written by Claude require Claude Code (https://claude.com/claude-code)
    installed and signed in. The app starts and the course builder works
    without it.
  - Everything is stored locally. The app contacts Anthropic only through your
    own Claude Code, and Wikimedia Commons for lesson sounds (can be disabled).
  - The .deb and .rpm add a desktop-menu entry. The AppImage and plain ELF do
    not. To remove the app, uninstall the package or delete the file; delete
    the two data folders above to remove your data.
  - Computer voices use espeak-ng, if it is installed. Install it from your
    distribution to have a voice when no free recording exists.
  - If the window stays blank on a computer with NVIDIA graphics, start the
    app with WEBKIT_DISABLE_DMABUF_RENDERER=1 (or, with an older WebKitGTK,
    WEBKIT_DISABLE_COMPOSITING_MODE=1).
  - SHA-256 checksums for every file in this archive are provided in the
    companion file SelfSemester-@VERSION@-linux-@ZIP_ARCH@.sha256.zip.

Verify
------
  unzip SelfSemester-@VERSION@-linux-@ZIP_ARCH@.sha256.zip
  sha256sum -c SHA256SUMS.txt

Licence
-------
MIT. See LICENSE. SelfSemester is an independent project, not affiliated with
Anthropic. Claude and Claude Code are trademarks of Anthropic.
