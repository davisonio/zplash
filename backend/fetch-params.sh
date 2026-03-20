#!/bin/bash
# Download Sapling proving parameters (~65MB total)
# These are needed for constructing shielded transactions.

PARAMS_DIR="${HOME}/.zcash-params"
mkdir -p "$PARAMS_DIR"

SPEND_URL="https://download.z.cash/downloads/sapling-spend.params"
OUTPUT_URL="https://download.z.cash/downloads/sapling-output.params"

if [ -f "$PARAMS_DIR/sapling-spend.params" ] && [ -f "$PARAMS_DIR/sapling-output.params" ]; then
    echo "Sapling parameters already present in $PARAMS_DIR"
    exit 0
fi

echo "Downloading Sapling spend parameters (~48MB)..."
curl -L -# -o "$PARAMS_DIR/sapling-spend.params" "$SPEND_URL"

echo "Downloading Sapling output parameters (~3.5MB)..."
curl -L -# -o "$PARAMS_DIR/sapling-output.params" "$OUTPUT_URL"

echo "Done. Parameters saved to $PARAMS_DIR"
