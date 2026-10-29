#!/usr/bin/env bash
# Downloads the official FHIR R5 example corpus (examples-json.zip, 2,824 files), checks its
# SHA-256 and unpacks it, for the health-fhir-models round-trip test (ADR 0050):
#
#   tools/fhir/fetch-r5-examples.sh [dir]   # default: target/fhir/r5-examples
#   HEALTH_FHIR_R5_EXAMPLES=target/fhir/r5-examples cargo test -p health-fhir-models
#
# R5 5.0.0 is final, so the archive never changes; a checksum mismatch means a corrupted or
# substituted download, and the script stops.
set -euo pipefail

URL="https://hl7.org/fhir/R5/examples-json.zip"
SHA256="a6937601b88ec9d187afb93c7c4d7a4902c8591439c6baaa1d50981c29328921"
DIR="${1:-target/fhir/r5-examples}"

mkdir -p "$DIR"
ZIP="$DIR/examples-json.zip"
if [[ ! -f "$ZIP" ]]; then
  curl --fail --silent --show-error --location --output "$ZIP.part" "$URL"
  mv "$ZIP.part" "$ZIP"
fi
if command -v sha256sum >/dev/null; then
  actual=$(sha256sum "$ZIP" | cut -d' ' -f1)
else
  actual=$(shasum -a 256 "$ZIP" | cut -d' ' -f1)
fi
if [[ "$actual" != "$SHA256" ]]; then
  echo "checksum mismatch for $ZIP: expected $SHA256, got $actual" >&2
  rm -f "$ZIP"
  exit 1
fi
rm -rf "$DIR/json"
mkdir -p "$DIR/json"
unzip -q "$ZIP" -d "$DIR/json"
echo "$(find "$DIR/json" -name '*.json' | wc -l | tr -d ' ') examples in $DIR/json"
