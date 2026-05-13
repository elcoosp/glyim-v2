#!/usr/bin/env bash
set -euo pipefail

STREAM_ID="${1:?Usage: generate-stream.sh SXX}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DATA_FILE="$SCRIPT_DIR/streams.json"
TEMPLATE="$SCRIPT_DIR/stream-template.md"
OUTPUT_DIR="$SCRIPT_DIR/briefs"
mkdir -p "$OUTPUT_DIR"

if ! command -v jq &>/dev/null; then
    echo "jq is required. Install it first."
    exit 1
fi

STREAM_DATA=$(jq --arg id "$STREAM_ID" '.[] | select(.id == $id)' "$DATA_FILE")

if [ -z "$STREAM_DATA" ]; then
    echo "Stream $STREAM_ID not found in $DATA_FILE"
    exit 1
fi

NAME=$(echo "$STREAM_DATA" | jq -r '.name')
CRATE=$(echo "$STREAM_DATA" | jq -r '.crate')
SCOPE=$(echo "$STREAM_DATA" | jq -r '.scope_summary')
OWNED=$(echo "$STREAM_DATA" | jq -r '.owned_crates[]' | sed 's/^/- /')
LOCKED=$(echo "$STREAM_DATA" | jq -r '.locked_interfaces[]' | sed 's/^/- /')
MOCKING=$(echo "$STREAM_DATA" | jq -r '.mocking')
UPSTREAM=$(echo "$STREAM_DATA" | jq -r '.upstream[]' | sed 's/^/- /')
DOWNSTREAM=$(echo "$STREAM_DATA" | jq -r '.downstream[]' | sed 's/^/- /')

TEST_SECTION=$(echo "$STREAM_DATA" | jq -r '.tests | to_entries[] | "- \(.key): \(.value)"')

sed \
    -e "s|{ID}|$STREAM_ID|g" \
    -e "s|{NAME}|$NAME|g" \
    -e "s|{SCOPE_SUMMARY}|$SCOPE|g" \
    -e "s|{OWNED_CRATES_AND_MODULES}|$OWNED|g" \
    -e "s|{LOCKED_INTERFACE_LIST}|$LOCKED|g" \
    -e "s|{TEST_CASES}|$TEST_SECTION|g" \
    -e "s|{MOCKING_INSTRUCTIONS}|$MOCKING|g" \
    -e "s|{CRATE_NAME}|$CRATE|g" \
    -e "s|{UPSTREAM_DEPENDENCIES}|$UPSTREAM|g" \
    -e "s|{WHO_USES_YOUR_OUTPUT}|$DOWNSTREAM|g" \
    "$TEMPLATE" > "$OUTPUT_DIR/${STREAM_ID}.md"

echo "Generated: $OUTPUT_DIR/${STREAM_ID}.md"
