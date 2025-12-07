#!/bin/bash

DEVICE_PATH="/Volumes/RPI-RP2"
BINARY="target/thumbv6m-none-eabi/release/macropad"
TARGET="thumbv6m-none-eabi"

# Check for cargo
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found"
    echo ""
    echo "Install Rust with:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check for rustup
if ! command -v rustup &> /dev/null; then
    echo "Error: rustup not found"
    echo ""
    echo "Install Rust with:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Ensure default toolchain is set
if ! rustup show active-toolchain &> /dev/null; then
    echo "Setting default toolchain to stable..."
    rustup default stable
fi

# Always ensure target is installed
echo "Ensuring $TARGET target is installed..."
rustup target add "$TARGET"

# Check for elf2uf2-rs
if ! command -v elf2uf2-rs &> /dev/null; then
    echo "Installing elf2uf2-rs..."
    cargo install elf2uf2-rs
fi

# Clean and build release
echo "Cleaning previous build..."
cargo clean

echo "Building release..."
cargo build --release

if [ $? -ne 0 ]; then
    echo "Build failed!"
    exit 1
fi

# Convert to UF2
echo "Converting to UF2..."
elf2uf2-rs "$BINARY" macropad.uf2

if [ ! -d "$DEVICE_PATH" ]; then
    echo ""
    echo "UF2 created: macropad.uf2"
    echo ""
    echo "To flash manually:"
    echo "  1. Hold encoder button + press reset"
    echo "  2. Drag macropad.uf2 onto RPI-RP2"
    exit 0
fi

echo "Flashing to MacroPad..."
cp macropad.uf2 "$DEVICE_PATH/"
echo ""
echo "Deploy complete! MacroPad will reboot."
