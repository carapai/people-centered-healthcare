#!/usr/bin/env bash
# Checks a Tauri updater signature against the public key the desktop pins, so an artifact signed
# by any other key is never published: a desktop would refuse it, and a lost or wrong updater key
# strands every installed desktop (ADR 0041). Tauri's keys and signatures are minisign's, each
# base64-wrapped: Ed25519 over the file's BLAKE2b-512 (`ED`), or over the file itself (`Ed`), and
# a second signature over the signature and its trusted comment.
# Usage: tools/release/verify-updater-signature.sh <file> <file.sig> <public key, as Tauri prints it>
set -euo pipefail

file=${1:?usage: verify-updater-signature.sh <file> <file.sig> <public key>}
sig=${2:?}
public=${3:?}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
b64() { openssl base64 -d -A; }
line() { sed -n "${1}p" | tr -d '\r'; }

printf '%s' "$public" | b64 | line 2 | b64 >"$work/pk"
b64 <"$sig" >"$work/sig.txt"
line 2 <"$work/sig.txt" | b64 >"$work/sig"
trusted=$(line 3 <"$work/sig.txt")
line 4 <"$work/sig.txt" | b64 >"$work/global"

[[ $(head -c 2 "$work/pk") == Ed && $(wc -c <"$work/pk") -eq 42 ]] || {
  echo "::error::the updater public key is not a minisign Ed25519 key" >&2
  exit 1
}
[[ $(wc -c <"$work/sig") -eq 74 ]] || { echo "::error::$sig is not a minisign signature" >&2; exit 1; }
cmp -s <(tail -c +3 "$work/pk" | head -c 8) <(tail -c +3 "$work/sig" | head -c 8) || {
  echo "::error::$sig was made by another key than the one desktops pin" >&2
  exit 1
}

# The public key as DER: the Ed25519 SubjectPublicKeyInfo prefix, then the 32 bytes.
{ printf '\x30\x2a\x30\x05\x06\x03\x2b\x65\x70\x03\x21\x00'; tail -c 32 "$work/pk"; } >"$work/pk.der"
verify() { openssl pkeyutl -verify -pubin -keyform DER -inkey "$work/pk.der" -rawin -in "$1" -sigfile "$2" >/dev/null; }

case $(head -c 2 "$work/sig") in
  ED) openssl dgst -blake2b512 -binary "$file" >"$work/message" ;;
  Ed) cp "$file" "$work/message" ;;
  *) echo "::error::$sig uses an unknown algorithm" >&2; exit 1 ;;
esac
tail -c 64 "$work/sig" >"$work/sig.raw"
verify "$work/message" "$work/sig.raw" || { echo "::error::$sig does not verify over $file" >&2; exit 1; }
{ cat "$work/sig.raw"; printf '%s' "${trusted#trusted comment: }"; } >"$work/global.message"
verify "$work/global.message" "$work/global" || {
  echo "::error::$sig's trusted comment does not verify" >&2
  exit 1
}
echo "$file: the updater signature verifies"
