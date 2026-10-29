#!/usr/bin/env bash
# Downloads the FHIR R5 core package (hl7.fhir.r5.core#5.0.0), checks its SHA-256 and
# unpacks it, for tools/fhir/check-element-table.sh (ADR 0051):
#
#   tools/fhir/fetch-r5-core.sh [dir]   # default: target/fhir/r5-core
#   tools/fhir/check-element-table.sh target/fhir/r5-core/package
#
# R5 5.0.0 is final, so the package never changes; a checksum mismatch means a corrupted or
# substituted download, and the script stops.
set -euo pipefail

URL="https://packages.fhir.org/hl7.fhir.r5.core/5.0.0"
SHA256="74b27cd1bfce9e80eaceac431edf230b0945a443564fbf5512f82e5fa50a80d4"
DIR="${1:-target/fhir/r5-core}"

mkdir -p "$DIR"
TGZ="$DIR/hl7.fhir.r5.core-5.0.0.tgz"
if [[ ! -f "$TGZ" ]]; then
  curl --fail --silent --show-error --location --output "$TGZ.part" "$URL"
  mv "$TGZ.part" "$TGZ"
fi
if command -v sha256sum >/dev/null; then
  actual=$(sha256sum "$TGZ" | cut -d' ' -f1)
else
  actual=$(shasum -a 256 "$TGZ" | cut -d' ' -f1)
fi
if [[ "$actual" != "$SHA256" ]]; then
  echo "checksum mismatch for $TGZ: expected $SHA256, got $actual" >&2
  rm -f "$TGZ"
  exit 1
fi
rm -rf "$DIR/package"
tar -xzf "$TGZ" -C "$DIR"
echo "$(find "$DIR/package" -name 'StructureDefinition-*.json' | wc -l | tr -d ' ') StructureDefinitions in $DIR/package"
