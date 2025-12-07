# CircuitPython MacroPad Development

## Project Structure

```
src/
├── code.py              # Main entry point - orchestrates components
├── effects/             # Visual effects for NeoPixels
│   ├── __init__.py      # Exports effect classes
│   └── rainbow.py       # Rainbow animation effect
└── input/               # Input handling
    ├── __init__.py      # Exports handlers
    └── keys.py          # Key press/release handler
```

## Architecture Pattern

Use **update loop pattern** - no blocking, no `time.sleep()` in components:

```python
class MyComponent:
    def __init__(self):
        self.last_update = time.monotonic()
        self.interval = 0.05  # seconds between updates

    def update(self):
        now = time.monotonic()
        if now - self.last_update < self.interval:
            return
        self.last_update = now
        # Do work here
```

Main loop calls `update()` on all components each iteration.

## Adding New Effects

1. Create `src/effects/my_effect.py`:
```python
import time

class MyEffect:
    def __init__(self, pixels):
        self.pixels = pixels
        self.last_update = time.monotonic()

    def update(self):
        # Non-blocking animation logic
        pass
```

2. Export in `src/effects/__init__.py`:
```python
from .my_effect import MyEffect
```

3. Add to deploy script if new module directory

## Adding New Input Handlers

1. Create `src/input/my_handler.py`
2. Export in `src/input/__init__.py`
3. Instantiate in `code.py` and call `update()` in main loop

## Key APIs

### MacroPad
- `macropad.pixels[0-11]` - NeoPixel colors (RGB tuple)
- `macropad.pixels.brightness` - 0.0 to 1.0
- `macropad.keys.events.get()` - Key events
- `macropad.encoder` - Rotary position (int)
- `macropad.encoder_switch` - Encoder button state
- `macropad.play_tone(freq, duration)` - Audio feedback

### Color Helpers
Use `wheel(0-255)` from effects/rainbow.py for rainbow colors.

## Deploying

```bash
./bootstrap.sh
```

Copies `code.py` and all module directories to CIRCUITPY.

## Required Libraries

Copy to `/Volumes/CIRCUITPY/lib/` from CircuitPython bundle:
- `adafruit_macropad.mpy`
- `adafruit_debouncer.mpy`
- `neopixel.mpy`
- `adafruit_hid/`
- `adafruit_ticks.mpy`
