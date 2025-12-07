import time


def wheel(pos):
    """Convert 0-255 to RGB color tuple transitioning r->g->b->r."""
    pos = 255 - pos
    if pos < 85:
        return (255 - pos * 3, 0, pos * 3)
    elif pos < 170:
        pos -= 85
        return (0, pos * 3, 255 - pos * 3)
    else:
        pos -= 170
        return (pos * 3, 255 - pos * 3, 0)


class RainbowEffect:
    """Rainbow fade effect across all NeoPixels."""

    def __init__(self, pixels, speed=2, interval=0.05):
        self.pixels = pixels
        self.speed = speed
        self.interval = interval
        self.offset = 0
        self.last_update = time.monotonic()

    def update(self):
        """Update animation frame if enough time has passed."""
        now = time.monotonic()
        if now - self.last_update < self.interval:
            return

        self.last_update = now

        # Each pixel offset by 21 (256/12) for even rainbow distribution
        for i in range(len(self.pixels)):
            self.pixels[i] = wheel((self.offset + i * 21) & 255)
        self.pixels.show()

        self.offset = (self.offset + self.speed) & 255
