#!/usr/bin/env bash
# Imports the release GPG key (AppImage and apt repository; ADR 0041) from Key Vault into a
# throwaway keyring for this job, and checks it is the key named by GPG_KEY_FINGERPRINT. The key
# is stored without a passphrase: Key Vault's access policy protects it.
# Usage: tools/release/import-gpg-key.sh   (exports GNUPGHOME to later steps on GitHub Actions)
# Needs KEY_VAULT_NAME, GPG_KEY_SECRET, GPG_KEY_FINGERPRINT and a signed-in `az`.
set -euo pipefail
: "${KEY_VAULT_NAME:?}" "${GPG_KEY_SECRET:?}" "${GPG_KEY_FINGERPRINT:?}"

export GNUPGHOME=${GNUPGHOME:-$(mktemp -d)}
chmod 700 "$GNUPGHOME"
az keyvault secret show --vault-name "$KEY_VAULT_NAME" --name "$GPG_KEY_SECRET" \
  --query value --output tsv | gpg --batch --quiet --import
gpg --batch --list-secret-keys "$GPG_KEY_FINGERPRINT" >/dev/null || {
  echo "::error::the key in $GPG_KEY_SECRET is not $GPG_KEY_FINGERPRINT" >&2
  exit 1
}
[[ -z ${GITHUB_ENV:-} ]] || echo "GNUPGHOME=$GNUPGHOME" >>"$GITHUB_ENV"
echo "imported $GPG_KEY_FINGERPRINT"
