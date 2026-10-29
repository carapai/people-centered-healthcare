#!/usr/bin/env bash
# Runs the HL7 Java validator differential (#166): writes the cases, validates them with the
# Java validator, and compares its verdicts with ours (health-fhir-validation's
# tests/java_differential.rs):
#
#   tools/fhir/java-differential.sh <R5 examples dir>   # needs java 17+
#
# Writes into target/fhir/differential/. Disagreements must be recorded, with reasons, in
# crates/contracts/health-fhir-validation/tests/fixtures/java-differential/.
set -euo pipefail

EXAMPLES=$(cd "$1" && pwd)
ROOT=$(pwd)
OUT="$ROOT/target/fhir/differential"
JAR=$(tools/fhir/fetch-java-validator.sh | tail -1)
CUSTOM="$ROOT/crates/contracts/health-fhir-validation/tests/fixtures/custom-profiles"

rm -rf "$OUT"
mkdir -p "$OUT/results"
cargo run --quiet --locked --release -p health-fhir-validation --example differential_cases -- "$EXAMPLES" "$OUT/cases"

# One run per directory, so a validator failure loses only that directory. Absolute paths:
# the test reads each validated file by the path in the validator's output.
for dir in tier3 tier1 corpus; do
  echo "validating $dir"
  java -Xmx6g -jar "$JAR" -version 5.0.0 -tx n/a -ig "$CUSTOM" \
    "$OUT/cases/$dir" -output "$OUT/results/$dir.json" > "$OUT/$dir.log" 2>&1 || true
  if [[ ! -s "$OUT/results/$dir.json" ]]; then
    echo "the Java validator wrote no results for $dir:" >&2
    tail -40 "$OUT/$dir.log" >&2
    exit 1
  fi
done

HEALTH_JAVA_VALIDATOR_RESULTS="$OUT/results" \
  cargo test --locked --release -p health-fhir-validation --test java_differential -- --nocapture
