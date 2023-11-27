#![no_main]
#![no_std]

use adafruit_macropad as bsp;
use bsp::{
    entry,
    hal::{clocks::init_clocks_and_plls, pio::PIOExt, Clock, Sio, Timer, Watchdog},
};

use smart_leds::{brightness, SmartLedsWrite, RGB8};

use panic_halt as _;
use ws2812_pio::Ws2812;

#[entry]
fn main() -> ! {
    let mut pac = bsp::pac::Peripherals::take().unwrap();
    let core = bsp::pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
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

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let timer = Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);

    let mut ws = Ws2812::new(
        pins.neopixel.into_function(),
        &mut pio,
        sm0,
        clocks.peripheral_clock.freq(),
        timer.count_down(),
    );

    let mut n: u8 = 128;

    loop {
        let led_states = get_led_states(n);
        ws.write(brightness(led_states.iter().copied(), 128))
            .unwrap();
        n = n.wrapping_add(1);
        delay.delay_ms(20);
    }
}

fn get_led_states(n: u8) -> [smart_leds::RGB<u8>; 12] {
    let mut led_states: [smart_leds::RGB<u8>; 12] = [RGB8::default(); 12];

    for i in 0..12 {
        led_states[i] = wheel((i * 256 / 12) as u8 + n);
    }

    led_states
}

/// Convert a number from `0..=255` to an RGB color triplet.
///
/// The colours are a transition from red, to green, to blue and back to red.
fn wheel(mut wheel_pos: u8) -> RGB8 {
    wheel_pos = 255 - wheel_pos;
    if wheel_pos < 85 {
        // No green in this sector - red and blue only
        (255 - (wheel_pos * 3), 0, wheel_pos * 3).into()
    } else if wheel_pos < 170 {
        // No red in this sector - green and blue only
        wheel_pos -= 85;
        (0, wheel_pos * 3, 255 - (wheel_pos * 3)).into()
    } else {
        // No blue in this sector - red and green only
        wheel_pos -= 170;
        (wheel_pos * 3, 255 - (wheel_pos * 3), 0).into()
    }
}
