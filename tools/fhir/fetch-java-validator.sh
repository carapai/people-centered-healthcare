#!/usr/bin/env bash
# Downloads the HL7 FHIR validator, a CI oracle only and never shipped (fhir-r5-model.md
# § Validation), and checks its SHA-256:
#
#   tools/fhir/fetch-java-validator.sh [dir]   # default: target/fhir/java-validator
#
# A checksum mismatch means a corrupted or substituted download, and the script stops.
# Upgrading means changing VERSION and SHA256 together, then re-running the differential.
set -euo pipefail

VERSION="7.0.2"
SHA256="e3e50f06b806437821b7aef8189a1b790415cb96c5036e0812ed62981452b377"
URL="https://github.com/hapifhir/org.hl7.fhir.core/releases/download/$VERSION/validator_cli.jar"
DIR="${1:-target/fhir/java-validator}"

mkdir -p "$DIR"
JAR="$DIR/validator_cli-$VERSION.jar"
if [[ ! -f "$JAR" ]]; then
  curl --fail --silent --show-error --location --output "$JAR.part" "$URL"
  mv "$JAR.part" "$JAR"
fi
if command -v sha256sum >/dev/null; then
  actual=$(sha256sum "$JAR" | cut -d' ' -f1)
else
  actual=$(shasum -a 256 "$JAR" | cut -d' ' -f1)
fi
if [[ "$actual" != "$SHA256" ]]; then
  echo "checksum mismatch for $JAR: expected $SHA256, got $actual" >&2
  rm -f "$JAR"
  exit 1
fi
echo "$JAR"
