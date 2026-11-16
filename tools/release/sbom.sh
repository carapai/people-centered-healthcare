#!/usr/bin/env bash
# CycloneDX SBOMs for a release (ADR 0045): one per shipped Rust binary, from cargo-cyclonedx,
# and one for the web app's Bun dependencies, from cdxgen. Each is checked to list the
# dependencies it must, so an empty SBOM fails the release instead of being attached.
# Usage: tools/release/sbom.sh <out-dir>   (needs cargo-cyclonedx; run from the repository root)
set -euo pipefail

out=${1:?usage: tools/release/sbom.sh <out-dir>}
mkdir -p "$out"

cargo cyclonedx --format json --spec-version 1.5 --describe binaries
for crate in central-server facility-edge; do
  dir=$(cargo metadata --format-version 1 --no-deps --locked |
    jq -r --arg c "$crate" '.packages[] | select(.name == $c) | .manifest_path' | xargs dirname)
  sbom=$(find "$dir" -maxdepth 1 -name "${crate}_bin.cdx.json" -o -maxdepth 1 -name "$crate.cdx.json" | head -n 1)
  [[ -n $sbom ]] || { echo "::error::no SBOM for $crate" >&2; exit 1; }
  mv "$sbom" "$out/$crate.cdx.json"
done
# The other crates' SBOMs are not shipped.
find apps crates tools -name '*.cdx.json' -not -path '*/node_modules/*' -delete

# The Bun workspace's one lock file, at the root.
bunx @cyclonedx/cdxgen@12.9.0 --type js --spec-version 1.6 --no-recurse \
  --output "$out/web.cdx.json" .

# Each lists what it must: SurrealDB in both binaries, React in the web app.
for check in 'central-server:surrealdb' 'facility-edge:surrealdb' 'web:react'; do
  file="$out/${check%%:*}.cdx.json"
  jq -e --arg n "${check#*:}" '[.components[]? | select(.name == $n)] | length > 0' "$file" >/dev/null || {
    echo "::error::$file does not list ${check#*:}" >&2
    exit 1
  }
done
ls -l "$out"
