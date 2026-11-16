#!/usr/bin/env bash
# Throwaway keys in a stand-in Key Vault, for the release dry run: a Tauri updater key with a
# password and, unless --updater-only, a GPG key without a passphrase. Writes the settings the
# release scripts read, and this directory onto PATH (so `az` is tools/release/test/az), to
# GITHUB_ENV and GITHUB_PATH.
# Usage: tools/release/test/throwaway-keys.sh [--updater-only]   (from the repository root)
set -euo pipefail
vault=${RUNNER_TEMP:?}/vault
mkdir -p "$vault"
root=$PWD

(cd apps/client && bunx --bun tauri signer generate --ci --password dry-run --write-keys "$vault/updater" >/dev/null)
printf 'dry-run' >"$vault/updater-password"
{
  echo "FAKE_KEY_VAULT_SECRETS=$vault"
  echo "KEY_VAULT_NAME=dry-run"
  echo "UPDATER_KEY_SECRET=updater"
  echo "UPDATER_KEY_PASSWORD_SECRET=updater-password"
  echo "UPDATER_PUBLIC_KEY=$(cat "$vault/updater.pub")"
} >>"$GITHUB_ENV"

if [[ ${1:-} != --updater-only ]]; then
  home=$(mktemp -d)
  chmod 700 "$home"
  GNUPGHOME=$home gpg --batch --passphrase '' --quick-gen-key 'Release dry run <dry-run@example.invalid>' ed25519 sign never
  fingerprint=$(GNUPGHOME=$home gpg --batch --with-colons --list-secret-keys | awk -F: '/^fpr/ { print $10; exit }')
  GNUPGHOME=$home gpg --batch --armor --export-secret-keys "$fingerprint" >"$vault/gpg"
  {
    echo "GPG_KEY_SECRET=gpg"
    echo "GPG_KEY_FINGERPRINT=$fingerprint"
  } >>"$GITHUB_ENV"
fi
echo "$root/tools/release/test" >>"$GITHUB_PATH"
