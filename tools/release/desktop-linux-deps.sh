#!/usr/bin/env bash
# What building and bundling the desktop needs on Ubuntu: WebKitGTK 4.1, and for the AppImage,
# file and the tools linuxdeploy calls. apt-utils gives apt-ftparchive for the apt repository.
# Usage: tools/release/desktop-linux-deps.sh
set -euo pipefail
sudo apt-get update
sudo apt-get install --yes --no-install-recommends \
  libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev libxdo-dev \
  file patchelf desktop-file-utils apt-utils gnupg
