#!/usr/bin/env bash
# The desktop's signed apt repository (ADR 0041: deb installs update through apt): a flat
# repository holding the release's deb, its Release file signed by the release GPG key
# (InRelease and Release.gpg), and that key's public half, then checked with gpgv as apt would.
# A machine adds it with:
#   deb [signed-by=/usr/share/keyrings/health-platform.gpg] <url> ./
# Usage: tools/release/apt-repo.sh <deb> <out-dir>   (needs GPG_KEY_FINGERPRINT in GNUPGHOME)
set -euo pipefail
deb=${1:?usage: apt-repo.sh <deb> <out-dir>}
out=${2:?}
: "${GPG_KEY_FINGERPRINT:?}"

mkdir -p "$out"
cp "$deb" "$out/"
(
  cd "$out"
  apt-ftparchive packages . >Packages
  gzip --keep --force -9 Packages
  apt-ftparchive -o APT::FTPArchive::Release::Origin=health-platform release . >Release
  gpg --batch --yes --local-user "$GPG_KEY_FINGERPRINT" --clearsign --output InRelease Release
  gpg --batch --yes --local-user "$GPG_KEY_FINGERPRINT" --armor --detach-sign --output Release.gpg Release
  gpg --batch --export "$GPG_KEY_FINGERPRINT" >health-platform.gpg
  gpgv --keyring ./health-platform.gpg InRelease
  gpgv --keyring ./health-platform.gpg Release.gpg Release
)
grep --quiet "Filename: ./$(basename "$deb")" "$out/Packages"
echo "$out: a signed apt repository with $(basename "$deb")"
