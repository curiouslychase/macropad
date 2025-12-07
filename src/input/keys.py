class KeyHandler:
    """Handle key press and release events."""

    def __init__(self, macropad):
        self.macropad = macropad
        self.pressed_keys = set()
        self.last_event = None

    def update(self):
        """Poll for key events. Call each loop iteration."""
        self.last_event = None

        event = self.macropad.keys.events.get()
        if event:
            self.last_event = event
            if event.pressed:
                self.pressed_keys.add(event.key_number)
            else:
                self.pressed_keys.discard(event.key_number)

    @property
    def any_pressed(self):
        """True if any key is currently held."""
        return len(self.pressed_keys) > 0

    def is_pressed(self, key_number):
        """Check if specific key is held."""
        return key_number in self.pressed_keys

    def just_pressed(self, key_number=None):
        """Check if key was just pressed this frame."""
        if self.last_event is None or not self.last_event.pressed:
            return False
        if key_number is None:
            return True
        return self.last_event.key_number == key_number

    def just_released(self, key_number=None):
        """Check if key was just released this frame."""
        if self.last_event is None or self.last_event.pressed:
            return False
        if key_number is None:
            return True
        return self.last_event.key_number == key_number
