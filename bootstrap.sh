#!/bin/bash

DEVICE_PATH="/Volumes/CIRCUITPY"

if [ ! -d "$DEVICE_PATH" ]; then
    echo "Error: CIRCUITPY not found at $DEVICE_PATH"
    echo ""
    echo "To mount your MacroPad:"
    echo "  1. Hold encoder button + press reset"
    echo "  2. Drag CircuitPython UF2 onto RPI-RP2"
    echo "  3. Wait for CIRCUITPY to appear"
    exit 1
fi

echo "Deploying to MacroPad..."

# Copy main code.py
cp src/code.py "$DEVICE_PATH/code.py"
echo "✓ Copied code.py"

# Copy module directories
for module in effects input; do
    if [ -d "src/$module" ]; then
        rm -rf "$DEVICE_PATH/$module"
        cp -r "src/$module" "$DEVICE_PATH/$module"
        echo "✓ Copied $module/"
    fi
done

# Create lib directory if needed
mkdir -p "$DEVICE_PATH/lib"

# Copy CircuitPython libraries if they exist locally
if [ -d "lib" ]; then
    cp -r lib/* "$DEVICE_PATH/lib/"
    echo "✓ Copied libraries"
fi

echo ""
echo "Deploy complete! MacroPad will auto-reload."
