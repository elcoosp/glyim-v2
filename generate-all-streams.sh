#!/usr/bin/env bash
set -euo pipefail

# Script: generate-all-streams.sh
# Purpose: Generate briefs for all streams defined in docs/agent-kit/streams.json
# Usage: Run from the root of the repository.

# Change to the directory of this script (if any) to ensure relative paths work.
# We assume the script is run from the repository root. If not, we can try to find it.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$SCRIPT_DIR"  # assumes script is at repo root

# If script is not at repo root (e.g., called from elsewhere), try to find .git
if [ ! -d "$REPO_ROOT/.git" ]; then
    REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$SCRIPT_DIR")"
fi

cd "$REPO_ROOT"

KIT_DIR="docs/agent-kit"
DATA_FILE="$KIT_DIR/streams.json"
GENERATOR="$KIT_DIR/generate-stream.sh"
OUTPUT_DIR="$KIT_DIR/briefs"

if [ ! -f "$DATA_FILE" ]; then
    echo "ERROR: streams.json not found at $DATA_FILE" >&2
    exit 1
fi

if [ ! -f "$GENERATOR" ]; then
    echo "ERROR: generate-stream.sh not found at $GENERATOR" >&2
    exit 1
fi

if ! command -v jq &>/dev/null; then
    echo "ERROR: jq is required but not installed." >&2
    exit 1
fi

# Ensure the output directory exists
mkdir -p "$OUTPUT_DIR"

# Extract all stream IDs from the JSON array
STREAM_IDS=$(jq -r '.[].id' "$DATA_FILE")

if [ -z "$STREAM_IDS" ]; then
    echo "No streams found in $DATA_FILE" >&2
    exit 0
fi

echo "Generating briefs for streams:"
echo "$STREAM_IDS"
echo ""

cd "$KIT_DIR"

for id in $STREAM_IDS; do
    echo "========================================"
    echo "Generating brief for stream $id..."
    if ./generate-stream.sh "$id"; then
        echo "Done: $id"
    else
        echo "ERROR: Failed to generate brief for $id" >&2
        # Continue with other streams
    fi
done
