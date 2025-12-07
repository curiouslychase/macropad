# Adafruit MacroPad RP2040 Development

## Hardware Overview

- **MCU**: RP2040 (Dual Cortex M0+ @ 130MHz, 264KB RAM)
- **Flash**: 8MB QSPI
- **Keys**: 12 Cherry MX-compatible switches (GPIO-wired, not matrix)
- **LEDs**: 12 NeoPixels (one per key)
- **Display**: 128x64 SH1106 OLED (SPI)
- **Encoder**: Rotary with 20 detents + push button
- **Audio**: 8mm speaker with Class D amp
- **Connectivity**: USB-C, STEMMA QT (I2C)

## CircuitPython Development

### Setup

1. Download UF2 from [circuitpython.org/board/adafruit_macropad_rp2040](https://circuitpython.org/board/adafruit_macropad_rp2040/)
2. Hold BOOT (encoder button) + press reset until RPI-RP2 appears
3. Drag UF2 to RPI-RP2 drive
4. Device remounts as CIRCUITPY

### Required Libraries

Copy to `/lib` from CircuitPython bundle:
- `adafruit_macropad.mpy`
- `adafruit_debouncer.mpy`
- `adafruit_simple_text_display.mpy`
- `neopixel.mpy`
- `adafruit_display_text/`
- `adafruit_hid/`
- `adafruit_midi/`
- `adafruit_ticks.mpy`

### Basic Template

```python
from adafruit_macropad import MacroPad

macropad = MacroPad()

while True:
    key_event = macropad.keys.events.get()
    if key_event:
        if key_event.pressed:
            macropad.pixels[key_event.key_number] = (255, 0, 0)
            macropad.play_tone(440, 0.1)
        else:
            macropad.pixels[key_event.key_number] = (0, 0, 0)

    macropad.encoder_switch_debounced.update()
    if macropad.encoder_switch_debounced.pressed:
        print("Encoder pressed")

    position = macropad.encoder
```

## GPIO Pin Reference

| Component | CircuitPython | Arduino |
|-----------|---------------|---------|
| Keys 1-12 | `board.KEY1`-`board.KEY12` | Pins 1-12 |
| Encoder A | `board.ENCODER_A` | `PIN_ROTA` |
| Encoder B | `board.ENCODER_B` | `PIN_ROTB` |
| Encoder Switch | `board.ENCODER_SWITCH` | `PIN_SWITCH` |
| NeoPixels | `board.NEOPIXEL` | `PIN_NEOPIXEL` |
| Speaker | `board.SPEAKER` | - |
| Speaker Enable | `board.SPEAKER_ENABLE` | - |
| Status LED | `board.LED` | `PIN_LED` |
| I2C | `board.SCL`, `board.SDA` | - |

## Key Features

### HID (Keyboard/Mouse)
```python
from adafruit_hid.keyboard import Keyboard
from adafruit_hid.keycode import Keycode

macropad.keyboard.send(Keycode.CONTROL, Keycode.C)
macropad.keyboard_layout.write("Hello")
```

### Display
```python
macropad.display_text[0].text = "Line 1"
macropad.display_text.show()
```

### MIDI
```python
macropad.midi.send(macropad.NoteOn(60, 127))
macropad.midi.send(macropad.NoteOff(60, 0))
```

### NeoPixels
```python
macropad.pixels.brightness = 0.5
macropad.pixels[0] = (255, 0, 0)  # Red
macropad.pixels.fill((0, 0, 255))  # All blue
```

## Recovery

**Safe Mode**: Press reset twice quickly (yellow LED blink) to bypass user code.

**Bootloader**: Hold encoder + reset to access UF2 bootloader.

## Resources

- [Official Guide](https://learn.adafruit.com/adafruit-macropad-rp2040)
- [CircuitPython Downloads](https://circuitpython.org/board/adafruit_macropad_rp2040/)
- [Library Bundle](https://circuitpython.org/libraries)
