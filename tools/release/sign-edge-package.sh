#!/usr/bin/env bash
# Signs an edge update package with the release-signing key, which never leaves Key Vault
# (facility-edge.md § Updates; ADR 0045), then checks the signature with the edge's own verifier
# against the public key the installer pins, so a package that would not stage is never published.
# Key Vault signs the package's SHA-256 with ES256 and returns r || s; the edge takes it as hex.
# Usage: tools/release/sign-edge-package.sh <facility-edge binary> <package> <signature out>
# Needs KEY_VAULT_NAME, EDGE_RELEASE_KEY, EDGE_RELEASE_PUBLIC_KEY and a signed-in `az`.
set -euo pipefail

binary=${1:?usage: sign-edge-package.sh <binary> <package> <signature out>}
package=${2:?}
out=${3:?}
: "${KEY_VAULT_NAME:?}" "${EDGE_RELEASE_KEY:?}" "${EDGE_RELEASE_PUBLIC_KEY:?}"

digest=$(openssl dgst -sha256 -binary "$package" | openssl base64 -A)
signature=$(az keyvault key sign --vault-name "$KEY_VAULT_NAME" --name "$EDGE_RELEASE_KEY" \
  --algorithm ES256 --digest "$digest" --query signature --output tsv)
# Base64 or base64url, padded or not.
signature=$(printf '%s' "$signature" | tr -- '-_' '+/' | tr -d '\r\n')
while ((${#signature} % 4)); do signature+='='; done
printf '%s' "$signature" | openssl base64 -d -A | od -An -v -tx1 | tr -d ' \n' >"$out"
[[ $(wc -c <"$out") -eq 128 ]] || {
  echo "::error::Key Vault returned a signature of $(($(wc -c <"$out") / 2)) bytes, not 64" >&2
  exit 1
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
printf '%s\n' "$EDGE_RELEASE_PUBLIC_KEY" >"$work/release.pub"
HEALTH_DATA_DIR=$work/store HEALTH_UPDATE_KEY_FILE=$work/release.pub \
  "$binary" update verify "$package" "$out"
