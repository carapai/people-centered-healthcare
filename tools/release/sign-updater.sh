#!/usr/bin/env bash
# Signs the desktop's updater artifacts with the Tauri updater key (ADR 0041), after Authenticode
# and GPG have finished with them, since either changes the bytes the updater checks. The key and
# its password are read from Key Vault into this process alone: never into a file, never into
# Vite's environment (desktop-client.md § Signing and updates). Each signature is then checked
# against the public key desktops pin.
# Usage: tools/release/sign-updater.sh <file>...   (from the repository root)
# Needs KEY_VAULT_NAME, UPDATER_KEY_SECRET, UPDATER_KEY_PASSWORD_SECRET, UPDATER_PUBLIC_KEY and
# a signed-in `az`; the Tauri CLI comes from apps/client.
set -euo pipefail
(($#)) || { echo "usage: sign-updater.sh <file>..." >&2; exit 2; }
: "${KEY_VAULT_NAME:?}" "${UPDATER_KEY_SECRET:?}" "${UPDATER_KEY_PASSWORD_SECRET:?}" "${UPDATER_PUBLIC_KEY:?}"

secret() {
  az keyvault secret show --vault-name "$KEY_VAULT_NAME" --name "$1" --query value --output tsv
}
key=$(secret "$UPDATER_KEY_SECRET")
password=$(secret "$UPDATER_KEY_PASSWORD_SECRET")
if [[ -n ${GITHUB_ACTIONS:-} ]]; then
  echo "::add-mask::$key"
  echo "::add-mask::$password"
fi

root=$PWD
for file in "$@"; do
  path=$(cd "$(dirname "$file")" && pwd)/$(basename "$file")
  (cd "$root/apps/client" &&
    TAURI_SIGNING_PRIVATE_KEY=$key TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$password \
      bunx --bun tauri signer sign "$path" >/dev/null)
  "$root/tools/release/verify-updater-signature.sh" "$path" "$path.sig" "$UPDATER_PUBLIC_KEY"
done
