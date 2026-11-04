#!/usr/bin/env bash
# Builds the platform core package (#87; ADR 0049; ADR 0053) from its FSH sources:
#
#   tools/fhir/build-core-package.sh        # needs bun, java 17+ and jq
#
# 1. SUSHI compiles packages/fhir-core-package/input/fsh into differentials.
# 2. The HL7 validator, a tooling oracle only (fhir-r5-model.md § Validation), generates each
#    StructureDefinition's snapshot: packages ship snapshots, and nodes never generate them.
# 3. The package goes to packages/fhir-core-package/package/: the conformance resources, its
#    package.json and the manifest Library pinning every canonical it depends on. The examples
#    go to packages/fhir-core-package/examples/, outside the package.
#
# The output is committed; re-running the script on unchanged sources changes nothing.
set -euo pipefail

SUSHI_VERSION="3.21.0"
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
SRC="$ROOT/packages/fhir-core-package"
OUT="$SRC/package"
EXAMPLES="$SRC/examples"
JAR=$(cd "$ROOT" && tools/fhir/fetch-java-validator.sh | tail -1)
JAR="$ROOT/$JAR"

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cp -R "$SRC/sushi-config.yaml" "$SRC/input" "$WORK/"
(cd "$WORK" && bunx "fsh-sushi@$SUSHI_VERSION" build . >"$WORK/sushi.log" 2>&1) || {
  cat "$WORK/sushi.log" >&2
  exit 1
}
if ! grep -q " 0 Errors" "$WORK/sushi.log"; then
  cat "$WORK/sushi.log" >&2
  exit 1
fi
GENERATED="$WORK/fsh-generated/resources"

CANONICAL=$(sed -n 's/^canonical: //p' "$SRC/sushi-config.yaml")
VERSION=$(sed -n 's/^version: //p' "$SRC/sushi-config.yaml")
NAME="health.platform.core"

rm -rf "$OUT" "$EXAMPLES"
mkdir -p "$OUT" "$EXAMPLES"

# The validator refuses a file whose name contains "template" ("Unsupported format"), read or
# written, so it reads and writes the definitions under neutral names.
mkdir -p "$WORK/definitions"
n=0
for file in "$GENERATED"/StructureDefinition-*.json "$GENERATED"/CodeSystem-*.json \
  "$GENERATED"/ValueSet-*.json; do
  n=$((n + 1))
  cp "$file" "$WORK/definitions/definition-$n.json"
  echo "definition-$n.json $(basename "$file")" >>"$WORK/names"
done

for file in "$GENERATED"/*.json; do
  base=$(basename "$file")
  case "$base" in
    StructureDefinition-*)
      neutral=$(awk -v base="$base" '$2 == base { print $1 }' "$WORK/names")
      java -jar "$JAR" snapshot -version 5.0.0 -tx n/a -ig "$WORK/definitions" \
        -output "$WORK/snapshot-$neutral" "$WORK/definitions/$neutral" >"$WORK/snapshot.log" 2>&1 || {
        cat "$WORK/snapshot.log" >&2
        exit 1
      }
      jq . "$WORK/snapshot-$neutral" >"$OUT/$base"
      ;;
    CodeSystem-* | ValueSet-*)
      jq . "$file" >"$OUT/$base"
      ;;
    *)
      jq . "$file" >"$EXAMPLES/$base"
      ;;
  esac
done

jq -n --arg name "$NAME" --arg version "$VERSION" --arg canonical "$CANONICAL" '{
  name: $name,
  version: $version,
  canonical: $canonical,
  title: "Health platform core",
  description: "The platform core package (ADR 0049): the engine-native Published Definition profiles, their extensions, and the platform CodeSystem.",
  fhirVersions: ["5.0.0"],
  dependencies: { "hl7.fhir.r5.core": "5.0.0" },
  kind: "national-core",
  platform: { minSoftware: "0.1.0", formatVersion: 1 }
}' >"$OUT/package.json"

# Every canonical the package depends on, pinned: the base definitions its profiles and
# extensions derive from, and its own artefacts, which it is composed of.
depends=$(jq -r 'select(.resourceType == "StructureDefinition") | .baseDefinition' "$OUT"/*.json |
  LC_ALL=C sort -u | sed 's/$/|5.0.0/')
composed=$(jq -r 'select(.url != null) | "\(.url)|\(.version)"' "$OUT"/StructureDefinition-*.json \
  "$OUT"/CodeSystem-*.json "$OUT"/ValueSet-*.json | LC_ALL=C sort -u)
jq -n --arg url "$CANONICAL/Library/$NAME-manifest" --arg version "$VERSION" \
  --arg depends "$depends" --arg composed "$composed" '{
  resourceType: "Library",
  id: "manifest",
  url: $url,
  version: $version,
  name: "HealthPlatformCoreManifest",
  status: "active",
  type: { coding: [{ system: "http://terminology.hl7.org/CodeSystem/library-type", code: "asset-collection" }] },
  relatedArtifact: (
    [$depends | split("\n")[] | select(. != "") | { type: "depends-on", resource: . }] +
    [$composed | split("\n")[] | select(. != "") | { type: "composed-of", resource: . }]
  )
}' >"$OUT/Library-manifest.json"

echo "$(find "$OUT" -name '*.json' | wc -l | tr -d ' ') files in $OUT, $(find "$EXAMPLES" -name '*.json' | wc -l | tr -d ' ') examples in $EXAMPLES"
