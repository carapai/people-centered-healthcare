#!/usr/bin/env bash
# One platform version (ADR 0045): the tag `v<version>` must name the version that central, the
# edge, the desktop and the web app all carry, and the tagged commit must be on main, where the
# merge queue ran CI over it, including the export → import round trip over every migration
# (health-backup's round_trip test).
# Usage: tools/release/check-version.sh <tag>   (run from the repository root, with main fetched)
set -euo pipefail

tag=${1:?usage: tools/release/check-version.sh <tag>}
[[ $tag =~ ^v([0-9]+\.[0-9]+\.[0-9]+)$ ]] || {
  echo "::error::$tag is not a release tag (v<major>.<minor>.<patch>)" >&2
  exit 1
}
version=${BASH_REMATCH[1]}

workspace=$(cargo metadata --format-version 1 --no-deps --locked |
  jq -r '.packages[] | select(.name == "central-server") | .version')
desktop=$(sed -n 's/^version = "\(.*\)"$/\1/p' apps/client/src-tauri/Cargo.toml | head -n 1)
tauri=$(jq -r .version apps/client/src-tauri/tauri.conf.json)
web=$(jq -r '.version // empty' apps/client/package.json)

status=0
for pair in "workspace:$workspace" "desktop crate:$desktop" "tauri.conf.json:$tauri" "web app:${web:-$version}"; do
  if [[ ${pair#*:} != "$version" ]]; then
    echo "::error::the $tag release, but the ${pair%%:*} is at ${pair#*:}" >&2
    status=1
  fi
done

if ! git merge-base --is-ancestor HEAD origin/main; then
  echo "::error::$tag is not on main, so the merge queue never ran CI over it" >&2
  status=1
fi
exit $status
