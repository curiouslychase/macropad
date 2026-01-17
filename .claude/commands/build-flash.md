# Build and Flash Macropad Firmware

Build the Rust firmware and flash it to the RP2040 macropad.

## Steps

1. Build release firmware:
```bash
cd rust && ~/.cargo/bin/cargo build --release
```

2. Check if device is in bootloader mode and flash:
```bash
if [ -d /Volumes/RPI-RP2 ]; then
  ~/.cargo/bin/elf2uf2-rs rust/target/thumbv6m-none-eabi/release/macropad /Volumes/RPI-RP2/macropad.uf2
  echo "Flashed successfully!"
else
  echo "Device not in bootloader mode. Hold BOOT button while plugging in USB."
fi
```
