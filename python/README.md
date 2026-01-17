# CircuitPython Implementation

## Setup

### 1. Install CircuitPython

1. Hold encoder button + press reset until `RPI-RP2` mounts
2. Download UF2 from [circuitpython.org/board/adafruit_macropad_rp2040](https://circuitpython.org/board/adafruit_macropad_rp2040/)
3. Drag `.uf2` onto `RPI-RP2`
4. Device reboots as `CIRCUITPY`

### 2. Install Libraries

Copy from [CircuitPython bundle](https://circuitpython.org/libraries) to `/Volumes/CIRCUITPY/lib/`:

- `adafruit_macropad.mpy`
- `neopixel.mpy`

### 3. Deploy

```bash
./bootstrap.sh
```

Copies `src/` contents to MacroPad. Auto-reloads on file change.

## Structure

```
src/
├── code.py          # Entry point
├── effects/
│   └── rainbow.py   # Rainbow LED effect
├── input/
│   ├── keys.py      # Key handler
│   └── encoder.py   # Rotary encoder
└── screens/
    └── manager.py   # Screen management
```

## Troubleshooting

- **No CIRCUITPY?** Try different USB cable/port, reinstall CircuitPython
- **Safe Mode (yellow LED)?** Double-tap reset, fix code.py errors
- **Bootloader?** Hold encoder + reset
