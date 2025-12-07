import time


MODE_VOLUME = 0
MODE_SCREEN = 1


class EncoderHandler:
    """Handle rotary encoder with dual-mode support.

    Press encoder to toggle between:
    - Volume mode: rotation controls system volume
    - Screen mode: rotation cycles through screens
    """

    def __init__(self, macropad, on_volume_change=None, on_screen_change=None, on_mode_change=None):
        self.macropad = macropad
        self.on_volume_change = on_volume_change
        self.on_screen_change = on_screen_change
        self.on_mode_change = on_mode_change

        self.mode = MODE_VOLUME
        self.last_position = macropad.encoder
        self.last_update = time.monotonic()

    def update(self):
        """Poll encoder. Call each loop iteration."""
        # Handle encoder press - toggle mode
        self.macropad.encoder_switch_debounced.update()
        if self.macropad.encoder_switch_debounced.pressed:
            self.mode = MODE_SCREEN if self.mode == MODE_VOLUME else MODE_VOLUME
            if self.on_mode_change:
                self.on_mode_change(self.mode)

        # Handle rotation
        position = self.macropad.encoder
        delta = position - self.last_position

        if delta != 0:
            self.last_position = position

            if self.mode == MODE_VOLUME:
                if self.on_volume_change:
                    self.on_volume_change(delta)
            else:
                if self.on_screen_change:
                    self.on_screen_change(delta)

    @property
    def is_volume_mode(self):
        return self.mode == MODE_VOLUME

    @property
    def is_screen_mode(self):
        return self.mode == MODE_SCREEN
