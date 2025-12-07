# Rust Implementation

Uses [embassy-rs](https://embassy.dev/) async framework for RP2040.

## Prerequisites

### Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Add RP2040 target

```bash
rustup target add thumbv6m-none-eabi
```

## Build

```bash
cargo build --release
```

Binary output: `target/thumbv6m-none-eabi/release/macropad`

## Flash

### Quick Deploy (UF2)

```bash
./bootstrap.sh
```

Builds, converts to UF2, and copies to MacroPad if in bootloader mode.

### With probe-rs (SWD debugger)

```bash
cargo run --release
```

### Manual UF2

1. Install elf2uf2-rs: `cargo install elf2uf2-rs`
2. Build UF2: `elf2uf2-rs target/thumbv6m-none-eabi/release/macropad macropad.uf2`
3. Hold encoder + reset to enter bootloader
4. Drag `macropad.uf2` to `RPI-RP2`

## Structure

```
src/
└── main.rs    # All-in-one: rainbow effect, key handling, audio
```

## Dependencies

- `embassy-rp` - RP2040 HAL with async support
- `embassy-executor` - Async executor
- `ws2812-pio` - WS2812 NeoPixel driver via PIO
- `smart-leds` - LED traits

## Pin Mapping

| Function | GPIO |
|----------|------|
| NeoPixels | 19 |
| Speaker Enable | 14 |
| Speaker PWM | 16 |
| Keys 1-12 | 1-12 |
