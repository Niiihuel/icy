#!/usr/bin/env bash
set -euo pipefail

# Downloads the public Accordee demo HTML and stylesheets into design/reference/accordee/raw/
# (gitignored). Re-run whenever you want to inspect upstream reference files locally.

ROOT="$(cd -- "$(dirname -- "$0")/.." && pwd)"
RAW_DIR="$ROOT/design/reference/accordee/raw"
BASE_URL="https://demo.accordee.app"

mkdir -p "$RAW_DIR"

echo "Fetching $BASE_URL/ ..."
curl -fsSL -A "Mozilla/5.0" "$BASE_URL/" -o "$RAW_DIR/index.html"

# Extract stylesheet hrefs from <link ... rel="stylesheet" ...>
CSS_PATHS=$(grep -oE '<link[^>]+rel="stylesheet"[^>]*>' "$RAW_DIR/index.html" \
    | grep -oE 'href="[^"]+\.css[^"]*"' \
    | sed -E 's/^href="([^"]+)"$/\1/' || true)

if [[ -z "$CSS_PATHS" ]]; then
    # Also match attribute order where href appears before rel="stylesheet"
    CSS_PATHS=$(grep -oE '<link[^>]+href="[^"]+\.css[^"]*"[^>]*>' "$RAW_DIR/index.html" \
        | grep -F 'stylesheet' \
        | grep -oE 'href="[^"]+\.css[^"]*"' \
        | sed -E 's/^href="([^"]+)"$/\1/' || true)
fi

if [[ -z "$CSS_PATHS" ]]; then
    echo "No stylesheet links found in $RAW_DIR/index.html" >&2
    exit 1
fi

printf '%s\n' "$CSS_PATHS" > "$RAW_DIR/stylesheets.txt"

while IFS= read -r rel_path; do
    [[ -z "$rel_path" ]] && continue
    case "$rel_path" in
        http://*|https://*) url="$rel_path" ;;
        /*) url="$BASE_URL$rel_path" ;;
        *) url="$BASE_URL/$rel_path" ;;
    esac
    fname="$(basename "${rel_path%%\?*}")"
    echo "Fetching $url -> $RAW_DIR/$fname"
    curl -fsSL -A "Mozilla/5.0" "$url" -o "$RAW_DIR/$fname"
done <<< "$CSS_PATHS"

echo "Done. Saved HTML and CSS under $RAW_DIR"
