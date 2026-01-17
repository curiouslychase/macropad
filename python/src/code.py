from adafruit_macropad import MacroPad

from effects import RainbowEffect
from input import KeyHandler

BRIGHTNESS = 0.125

# Initialize hardware
macropad = MacroPad()
macropad.pixels.brightness = BRIGHTNESS

# Initialize components
effect = RainbowEffect(macropad.pixels)
keys = KeyHandler(macropad)

# Main loop
while True:
    keys.update()
    effect.update()

    if keys.just_pressed():
        macropad.play_tone(440, 0.05)
