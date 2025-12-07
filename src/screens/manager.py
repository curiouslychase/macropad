class ScreenManager:
    """Manage multiple key layout screens."""

    def __init__(self, screens=None):
        self.screens = screens or []
        self.current_index = 0

    def add_screen(self, screen):
        """Add a screen config dict with 'name' and 'keys'."""
        self.screens.append(screen)

    @property
    def current(self):
        """Get current screen config."""
        if not self.screens:
            return None
        return self.screens[self.current_index]

    @property
    def name(self):
        """Get current screen name."""
        if self.current:
            return self.current.get("name", f"Screen {self.current_index}")
        return "No Screens"

    def next(self):
        """Move to next screen."""
        if self.screens:
            self.current_index = (self.current_index + 1) % len(self.screens)

    def prev(self):
        """Move to previous screen."""
        if self.screens:
            self.current_index = (self.current_index - 1) % len(self.screens)

    def change(self, delta):
        """Change screen by delta amount."""
        if delta > 0:
            for _ in range(delta):
                self.next()
        elif delta < 0:
            for _ in range(-delta):
                self.prev()

    def get_key_action(self, key_number):
        """Get action for key on current screen."""
        if self.current and "keys" in self.current:
            keys = self.current["keys"]
            if key_number < len(keys):
                return keys[key_number]
        return None
