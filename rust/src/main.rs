#![no_std]
#![no_main]

use adafruit_macropad::{
    entry,
    hal::{
        clocks::{init_clocks_and_plls, Clock},
        gpio::PinState,
        pac,
        pio::PIOExt,
        timer::Timer,
        watchdog::Watchdog,
        Sio,
    },
    Pins, XOSC_CRYSTAL_FREQ,
};
use embedded_hal::digital::v2::{InputPin, OutputPin};
use panic_halt as _;
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use ws2812_pio::Ws2812;

const NUM_LEDS: usize = 12;
const BRIGHTNESS_LEVEL: u8 = 32; // ~12.5% brightness

/// Convert 0-255 position to RGB color (rainbow wheel)
fn wheel(pos: u8) -> RGB8 {
    let pos = 255 - pos;
    if pos < 85 {
        RGB8::new(255 - pos * 3, 0, pos * 3)
    } else if pos < 170 {
        let pos = pos - 85;
        RGB8::new(0, pos * 3, 255 - pos * 3)
    } else {
        let pos = pos - 170;
        RGB8::new(pos * 3, 255 - pos * 3, 0)
    }
}

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    let clocks = init_clocks_and_plls(
        XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());
    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Setup NeoPixels
    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);
    let mut ws = Ws2812::new(
        pins.neopixel.into_function(),
        &mut pio,
        sm0,
        clocks.peripheral_clock.freq(),
        timer.count_down(),
    );

    // Setup speaker shutdown pin (active low - set high to enable speaker)
    let mut speaker_shutdown = pins
        .speaker_shutdown
        .into_push_pull_output_in_state(PinState::Low);

    // Setup keys (directly access the underlying pins)
    let key1 = pins.key1.into_pull_up_input();
    let key2 = pins.key2.into_pull_up_input();
    let key3 = pins.key3.into_pull_up_input();
    let key4 = pins.key4.into_pull_up_input();
    let key5 = pins.key5.into_pull_up_input();
    let key6 = pins.key6.into_pull_up_input();
    let key7 = pins.key7.into_pull_up_input();
    let key8 = pins.key8.into_pull_up_input();
    let key9 = pins.key9.into_pull_up_input();
    let key10 = pins.key10.into_pull_up_input();
    let key11 = pins.key11.into_pull_up_input();
    let key12 = pins.key12.into_pull_up_input();

    let mut led_data = [RGB8::default(); NUM_LEDS];
    let mut offset: u8 = 0;
    let mut prev_any_pressed = false;

    loop {
        // Check for any key press (keys are active low)
        let any_pressed = key1.is_low().unwrap_or(false)
            || key2.is_low().unwrap_or(false)
            || key3.is_low().unwrap_or(false)
            || key4.is_low().unwrap_or(false)
            || key5.is_low().unwrap_or(false)
            || key6.is_low().unwrap_or(false)
            || key7.is_low().unwrap_or(false)
            || key8.is_low().unwrap_or(false)
            || key9.is_low().unwrap_or(false)
            || key10.is_low().unwrap_or(false)
            || key11.is_low().unwrap_or(false)
            || key12.is_low().unwrap_or(false);

        // Play tone on new key press (simple beep via speaker shutdown toggle)
        if any_pressed && !prev_any_pressed {
            // Simple beep by toggling speaker shutdown rapidly
            for _ in 0..50 {
                speaker_shutdown.set_high().unwrap();
                delay.delay_us(1136); // ~440Hz half period
                speaker_shutdown.set_low().unwrap();
                delay.delay_us(1136);
            }
        }
        prev_any_pressed = any_pressed;

        // Calculate rainbow colors for each LED
        // Each pixel offset by 21 (256/12) for even distribution
        for i in 0..NUM_LEDS {
            led_data[i] = wheel(offset.wrapping_add((i as u8) * 21));
        }

        // Write to LEDs with brightness
        ws.write(brightness(led_data.iter().copied(), BRIGHTNESS_LEVEL))
            .unwrap();

        offset = offset.wrapping_add(2);

        // ~50ms delay for animation frame rate
        delay.delay_ms(50);
    }
}
