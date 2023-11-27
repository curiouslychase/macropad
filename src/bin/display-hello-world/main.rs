#![no_main]
#![no_std]

use adafruit_macropad as bsp;
use bsp::hal::{clocks::init_clocks_and_plls, Clock, Sio, Watchdog};

use fugit::RateExtU32;

use embedded_graphics::prelude::*;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    text::{Baseline, Text},
};

use panic_halt as _;

use sh1106::prelude::*;

#[bsp::entry]
fn main() -> ! {
    let mut pac = bsp::pac::Peripherals::take().unwrap();
    let core = bsp::pac::CorePeripherals::take().unwrap();

    let sio = Sio::new(pac.SIO);

    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    let external_xtal_freq_hz = 12_000_000u32;

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

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

    let oled_cs = pins.oled_cs.into_push_pull_output();
    let oled_dc = pins.oled_dc.into_push_pull_output();
    let mut oled_reset = pins.oled_reset.into_push_pull_output();

    let spi = bsp::hal::Spi::<_, _, _, 8>::new(
        pac.SPI1,
        (
            pins.mosi.into_function(),
            pins.miso.into_function(),
            pins.sclk.into_function(),
        ),
    );

    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        400.kHz(),
        embedded_hal::spi::MODE_0,
    );

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let mut display: GraphicsMode<_> = sh1106::Builder::new()
        .connect_spi(spi, oled_dc, oled_cs)
        .into();

    display.reset(&mut oled_reset, &mut delay).unwrap();
    display.init().unwrap();

    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    // Empty the display:
    display.clear();

    Text::with_baseline("Hello world!", Point::zero(), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    Text::with_baseline("Hello Rust!", Point::new(0, 16), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();

    loop {}
}
