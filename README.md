# Adafruit MacroPad

CircuitPython code for my Adafruit MacroPad RP2040.

## Getting Started

### 1. Install CircuitPython

First time setup - get your MacroPad into bootloader mode:

1. **Hold the encoder button** (press down on the rotary knob)
2. While holding, **press the reset button** (small button on side)
3. Release both - MacroPad mounts as `RPI-RP2`
4. Download CircuitPython UF2 from [circuitpython.org/board/adafruit_macropad_rp2040](https://circuitpython.org/board/adafruit_macropad_rp2040/)
5. Drag the `.uf2` file onto `RPI-RP2`
6. MacroPad reboots and mounts as `CIRCUITPY`

### 2. Install Libraries

Download the [CircuitPython library bundle](https://circuitpython.org/libraries) and copy these to `/Volumes/CIRCUITPY/lib/`:

- `neopixel.mpy`

### 3. Deploy Code

```bash
./bootstrap.sh
```

This copies `src/code.py` and all modules to your MacroPad. The device auto-reloads when files change.

## Troubleshooting

**CIRCUITPY not showing up?**
- Try a different USB-C cable (some are charge-only)
- Try a different USB port
- Re-enter bootloader mode and reinstall CircuitPython

**Safe Mode (yellow LED blink):**
- Press reset twice quickly to bypass user code
- Fix any syntax errors in code.py

**Back to Bootloader:**
- Hold encoder + press reset
