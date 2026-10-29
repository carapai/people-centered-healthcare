#!/usr/bin/env bash
# Regenerates health-fhir-validation's element table from an unpacked R5 core package and
# fails if the committed table differs (ADR 0051):
#
#   tools/fhir/check-element-table.sh [package dir]   # default: ~/.fhir/packages/hl7.fhir.r5.core#5.0.0/package
#
# Pass --write to update the committed table instead.
set -euo pipefail

WRITE=0
if [[ "${1:-}" == "--write" ]]; then
  WRITE=1
  shift
fi
PACKAGE="${1:-$HOME/.fhir/packages/hl7.fhir.r5.core#5.0.0/package}"
TABLE="crates/contracts/health-fhir-validation/data/r5-elements.json"
GENERATED=$(mktemp)
trap 'rm -f "$GENERATED"' EXIT

cargo run --quiet -p fhir-element-table -- "$PACKAGE" > "$GENERATED"
if [[ $WRITE == 1 ]]; then
  cp "$GENERATED" "$TABLE"
  echo "wrote $TABLE"
elif ! diff -q "$GENERATED" "$TABLE" >/dev/null; then
  echo "$TABLE is out of date; run tools/fhir/check-element-table.sh --write" >&2
  exit 1
else
  echo "$TABLE is up to date"
fi
