#!/usr/bin/env bash
# Every alert code a node emits has a row in alerts.tsv and a rule in loki/rules, and every row
# names a code that is emitted (#139). Run from the repository root.
set -euo pipefail
here=ops/observability
emitted=$(
  {
    grep -rhoE 'alert = "[a-z0-9-]+"' crates apps --include='*.rs' | sed -E 's/.*"(.*)"/\1/'
    grep -rhoE 'Alert::(new|about)\("[a-z0-9-]+"' crates --include='*.rs' | sed -E 's/.*"(.*)"/\1/'
  } | sort -u
)
catalogued=$(grep -v '^#' "$here/alerts.tsv" | cut -f1 | sort -u)
ruled=$(grep -hoE 'alert="[a-z0-9-]+"' "$here"/loki/rules/*/*.yaml | sed -E 's/.*"(.*)"/\1/' | sort -u)
status=0
for code in $emitted; do
  grep -qx "$code" <<<"$catalogued" || { echo "emitted but not catalogued: $code"; status=1; }
done
for code in $catalogued; do
  grep -qx "$code" <<<"$emitted" || { echo "catalogued but never emitted: $code"; status=1; }
  grep -qx "$code" <<<"$ruled" || { echo "catalogued but no rule: $code"; status=1; }
done
[ "$status" = 0 ] && echo "$(wc -l <<<"$catalogued" | tr -d ' ') alert codes, each emitted, catalogued and ruled"
exit "$status"
