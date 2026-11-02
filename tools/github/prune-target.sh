#!/usr/bin/env bash
# Leaves only the dependencies' build output in target/, so the CI cache saved after it holds
# what changes with Cargo.lock and nothing that changes with every commit: the workspace's own
# crates, test binaries and incremental state are removed (deployment.md § Workflows).
# Usage: tools/github/prune-target.sh [target-dir]
set -euo pipefail

target=${1:-${CARGO_TARGET_DIR:-target}}
[[ -d $target ]] || exit 0
before=$(du -sk "$target" | cut -f1)

# Package names (the .fingerprint and build directories) and target names (artefact files).
names=()
while IFS= read -r name; do names+=("$name"); done < <(
  cargo metadata --format-version 1 --no-deps --locked |
    jq -r '.packages[] | .name, .targets[].name' | sort -u
)

shopt -s nullglob
for profile in "$target"/{debug,release} "$target"/*/{debug,release}; do
  [[ -d $profile ]] || continue
  rm -rf "$profile/incremental" "$profile/examples"
  for name in "${names[@]}"; do
    stem=${name//-/_}
    rm -rf "$profile/.fingerprint/$name"-* "$profile/build/$name"-* \
      "$profile/deps/$stem"-* "$profile/deps/lib$stem"-* \
      "$profile/$name" "$profile/$name".{d,exe,pdb} "$profile/$stem".{d,exe,pdb} "$profile/lib$stem".*
  done
done

after=$(du -sk "$target" | cut -f1)
echo "prune-target: $((before / 1024)) MB -> $((after / 1024)) MB"
