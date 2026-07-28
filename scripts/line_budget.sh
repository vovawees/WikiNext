set -euo pipefail

max_lines=800
status=0

while IFS= read -r -d '' file; do
  lines=$(wc -l < "$file" | tr -d '[:space:]')
  if [ "$lines" -gt "$max_lines" ]; then
    echo "$file: $lines lines"
    status=1
  fi
done < <(find crates -type f -name '*.rs' -print0)

exit $status