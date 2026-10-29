#!/usr/bin/env bash
# Checks ADR numbering and the ADR index in docs/architecture/overview.md:
#
#   tools/docs/check-adr-index.sh
#
# - every file in docs/adr/ is NNNN-kebab-case-title.md;
# - the numbers run 0001, 0002, … with no gaps or duplicates;
# - the index links every ADR once, by its number and file, and links nothing else.
set -euo pipefail

ADR_DIR="docs/adr"
INDEX="docs/architecture/overview.md"
status=0

expected=1
for path in "$ADR_DIR"/*; do
  file=$(basename "$path")
  if [[ ! "$file" =~ ^([0-9]{4})-[a-z0-9]+(-[a-z0-9]+)*\.md$ ]]; then
    echo "$ADR_DIR/$file: not named NNNN-kebab-case-title.md" >&2
    status=1
    continue
  fi
  number=$((10#${BASH_REMATCH[1]}))
  if [[ $number != "$expected" ]]; then
    echo "$ADR_DIR/$file: expected number $(printf '%04d' "$expected")" >&2
    status=1
  fi
  expected=$((number + 1))
  label=${file%%-*}
  count=$(grep -cF "[$label](../adr/$file)" "$INDEX" || true)
  if [[ $count != 1 ]]; then
    echo "$INDEX: links $file $count times, expected once as [$label](../adr/$file)" >&2
    status=1
  fi
done

while read -r link; do
  if [[ ! -f "$ADR_DIR/$link" ]]; then
    echo "$INDEX: links ../adr/$link, which does not exist" >&2
    status=1
  fi
done < <(grep -oE '\(\.\./adr/[^)]+\)' "$INDEX" | sed -E 's/^\(\.\.\/adr\/(.*)\)$/\1/' | sort -u)

if [[ $status == 0 ]]; then
  echo "$(find "$ADR_DIR" -name '*.md' | wc -l | tr -d ' ') ADRs, numbered and indexed"
fi
exit $status
