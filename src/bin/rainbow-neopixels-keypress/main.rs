#![no_main]
#![no_std]

use adafruit_macropad as bsp;
use bsp::{
    entry,
    hal::{clocks::init_clocks_and_plls, pio::PIOExt, Clock, Sio, Timer, Watchdog},
};

use embedded_hal::digital::v2::InputPin;
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

    let button = pins.button.into_pull_up_input();

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
        let mut led_states = get_led_states(n);

        if key1.is_low().unwrap() {
            led_states[0] = (255, 255, 255).into();
        }

        if key2.is_low().unwrap() {
            led_states[1] = (255, 255, 255).into();
        }

        if key3.is_low().unwrap() {
            led_states[2] = (255, 255, 255).into();
        }

        if key4.is_low().unwrap() {
            led_states[3] = (255, 255, 255).into();
        }

        if key5.is_low().unwrap() {
            led_states[4] = (255, 255, 255).into();
        }

        if key6.is_low().unwrap() {
            led_states[5] = (255, 255, 255).into();
        }

        if key7.is_low().unwrap() {
            led_states[6] = (255, 255, 255).into();
        }

        if key8.is_low().unwrap() {
            led_states[7] = (255, 255, 255).into();
        }

        if key9.is_low().unwrap() {
            led_states[8] = (255, 255, 255).into();
        }

        if key10.is_low().unwrap() {
            led_states[9] = (255, 255, 255).into();
        }

        if key11.is_low().unwrap() {
            led_states[10] = (255, 255, 255).into();
        }

        if key12.is_low().unwrap() {
            led_states[11] = (255, 255, 255).into();
        }

        let mut brightness_val = 32;
        if button.is_low().unwrap() {
            brightness_val = 255;
        }

        ws.write(brightness(led_states.iter().copied(), brightness_val))
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
