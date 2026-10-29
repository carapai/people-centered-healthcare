#!/usr/bin/env bash
# Renders every ```mermaid block in the tracked Markdown, so a diagram that does not parse
# fails documentation.yml:
#
#   tools/docs/render-mermaid.sh   # needs mmdc (@mermaid-js/mermaid-cli) on PATH
set -euo pipefail

OUT=$(mktemp -d)
trap 'rm -rf "$OUT"' EXIT

# Ubuntu 24.04 runners forbid the unprivileged user namespaces Chromium's sandbox needs; the
# diagrams are our own docs.
printf '{ "args": ["--no-sandbox"] }\n' > "$OUT/puppeteer.json"

count=0
status=0
while IFS= read -r file; do
  # Write each block to its own .mmd file, named after the file and the block's start line.
  awk -v out="$OUT" -v name="${file//\//_}" '
    /^```mermaid[[:space:]]*$/ { inside = 1; path = out "/" name "_" NR ".mmd"; next }
    inside && /^```[[:space:]]*$/ { inside = 0; close(path); next }
    inside { print > path }
  ' "$file"
done < <(git ls-files '*.md' | grep -v '^third_party/')

for diagram in "$OUT"/*.mmd; do
  [[ -e "$diagram" ]] || continue
  count=$((count + 1))
  if ! error=$(mmdc --quiet --puppeteerConfigFile "$OUT/puppeteer.json" --input "$diagram" --output "${diagram%.mmd}.svg" 2>&1); then
    source_name=$(basename "${diagram%.mmd}")
    echo "Mermaid block does not render: ${source_name%_*} (line ${source_name##*_})" >&2
    echo "$error" | grep -v '^\s*at ' | head -5 >&2
    status=1
  fi
done
echo "$count Mermaid diagrams checked"
exit $status
