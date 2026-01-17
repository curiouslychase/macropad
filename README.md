# Adafruit MacroPad RP2040

Firmware for the Adafruit MacroPad RP2040 in multiple languages.

## Implementations

| Language | Directory | Framework |
|----------|-----------|-----------|
| [Python](python/) | `python/` | CircuitPython |
| [Rust](rust/) | `rust/` | embassy-rs |

## Features

- Rainbow LED effect across all 12 NeoPixels
- Key press detection with audio feedback (440Hz tone)

## Hardware

- **MCU**: RP2040 (Dual Cortex M0+ @ 130MHz)
- **Keys**: 12 Cherry MX-compatible switches
- **LEDs**: 12 NeoPixels (WS2812)
- **Audio**: Speaker with Class D amp
- **Display**: 128x64 OLED (SH1106)
- **Encoder**: Rotary with push button

See [AGENTS.md](AGENTS.md) for detailed hardware reference.
