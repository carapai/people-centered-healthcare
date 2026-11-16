#!/usr/bin/env bash
# Builds the desktop's Linux packages (ADR 0041): the AppImage, GPG-signed in its embedded
# signature section, and the deb, published with a signed apt repository. The webview's bundle
# must already be built (apps/client: bun run build:tauri), and the release GPG key imported.
# Usage: tools/release/desktop-linux.sh <version> <out-dir>   (needs GPG_KEY_FINGERPRINT)
set -euo pipefail
version=${1:?usage: desktop-linux.sh <version> <out-dir>}
out=${2:?}
: "${GPG_KEY_FINGERPRINT:?}"
root=$PWD
mkdir -p "$out"
out=$(cd "$out" && pwd)

# appimagetool signs with the key; FUSE is not needed to build in CI.
(cd apps/client &&
  SIGN=1 SIGN_KEY=$GPG_KEY_FINGERPRINT APPIMAGETOOL_SIGN_PASSPHRASE='' APPIMAGETOOL_FORCE_SIGN=1 \
    APPIMAGE_EXTRACT_AND_RUN=1 bunx --bun tauri build --bundles appimage,deb --ci)

bundle=$root/apps/client/src-tauri/target/release/bundle
appimage="$out/health-platform-$version-x86_64.AppImage"
deb="$out/health-platform-$version-amd64.deb"
mv "$bundle"/appimage/*.AppImage "$appimage"
mv "$bundle"/deb/*.deb "$deb"

# The embedded signature is there, and is by the release key.
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
objcopy --dump-section .sha256_sig="$work/sig" "$appimage" "$work/copy"
tr -d '\0' <"$work/sig" >"$work/sig.asc"
grep --quiet 'BEGIN PGP SIGNATURE' "$work/sig.asc" || {
  echo "::error::the AppImage carries no GPG signature" >&2
  exit 1
}
gpg --batch --list-packets "$work/sig.asc" | grep --quiet --ignore-case "keyid ${GPG_KEY_FINGERPRINT: -16}" || {
  echo "::error::the AppImage is signed by another key than $GPG_KEY_FINGERPRINT" >&2
  exit 1
}

dpkg-deb --info "$deb" | grep --quiet "Version: $version"
"$root/tools/release/apt-repo.sh" "$deb" "$work/apt"
tar -czf "$out/apt-repo-$version.tar.gz" -C "$work/apt" .
ls -l "$out"
