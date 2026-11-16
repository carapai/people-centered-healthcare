#!/usr/bin/env bash
# Signed or failed (ADR 0045): every job of release.yml runs this before it builds anything, and
# it fails when any signing setting is missing, so a release that cannot be signed in full
# publishes nothing. The settings are variables of the `release` environment; none is a secret,
# because the keys stay in Azure and are reached through GitHub OIDC (deployment.md § Releases).
# Usage: tools/release/check-signing.sh          check the environment, or, when RELEASE_VARS is
#                                                set, that JSON object (release.yml passes
#                                                `toJSON(vars)`)
#        tools/release/check-signing.sh --list   print the names, one per line
set -euo pipefail

required=(
  # GitHub OIDC federation to Azure, trusted for the `release` environment only.
  AZURE_TENANT_ID
  AZURE_CLIENT_ID
  AZURE_SUBSCRIPTION_ID
  # Authenticode for the Windows installers and binaries: Azure Artifact Signing.
  ARTIFACT_SIGNING_ENDPOINT
  ARTIFACT_SIGNING_ACCOUNT
  ARTIFACT_SIGNING_PROFILE
  # The Key Vault holding the keys below.
  KEY_VAULT_NAME
  # The Tauri updater key: Key Vault secrets holding the private key and its password, and the
  # public key the desktop pins.
  UPDATER_KEY_SECRET
  UPDATER_KEY_PASSWORD_SECRET
  UPDATER_PUBLIC_KEY
  # GPG for the AppImage and the apt repository: a Key Vault secret holding the armoured private
  # key, and its fingerprint.
  GPG_KEY_SECRET
  GPG_KEY_FINGERPRINT
  # The edge release-signing key: an EC P-256 Key Vault key that never leaves the vault, and its
  # public half as the installer pins it (hex SEC1).
  EDGE_RELEASE_KEY
  EDGE_RELEASE_PUBLIC_KEY
)

if [[ ${1:-} == --list ]]; then
  printf '%s\n' "${required[@]}"
  exit 0
fi

value() {
  if [[ -n ${RELEASE_VARS:-} ]]; then
    jq -r --arg name "$1" '.[$name] // empty' <<<"$RELEASE_VARS"
  else
    printf '%s' "${!1:-}"
  fi
}

missing=()
for name in "${required[@]}"; do
  [[ -n $(value "$name") ]] || missing+=("$name")
done

if ((${#missing[@]})); then
  echo "::error title=Signed or failed::the release environment lacks ${missing[*]}; nothing is built or published" >&2
  exit 1
fi
echo "every signing setting is present"
