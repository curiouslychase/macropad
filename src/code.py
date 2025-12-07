from adafruit_macropad import MacroPad

from effects import RainbowEffect
from input import KeyHandler

BRIGHTNESS = 0.125

# Piano note frequencies (C4 to B4 chromatic scale)
NOTES = [
    262,  # C4  - key 0
    277,  # C#4 - key 1
    294,  # D4  - key 2
    311,  # D#4 - key 3
    330,  # E4  - key 4
    349,  # F4  - key 5
    370,  # F#4 - key 6
    392,  # G4  - key 7
    415,  # G#4 - key 8
    440,  # A4  - key 9
    466,  # A#4 - key 10
    494,  # B4  - key 11
]

# Initialize hardware (rotation disables default startup tone)
macropad = MacroPad(rotation=0)
macropad.play_file = lambda *args: None  # Disable click sounds
macropad.pixels.brightness = BRIGHTNESS

# Initialize components
effect = RainbowEffect(macropad.pixels)
keys = KeyHandler(macropad)

# Main loop
while True:
    keys.update()
    effect.update()

    # Play note for pressed key
    if keys.just_pressed():
        key_num = keys.last_event.key_number
        macropad.play_tone(NOTES[key_num], 0.2)
