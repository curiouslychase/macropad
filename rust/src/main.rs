#![no_std]
#![no_main]

use core::cell::RefCell;
use critical_section::Mutex;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text,
};
use embedded_hal::digital::v2::{InputPin, OutputPin};
use embedded_hal::PwmPin;
use frunk::{HCons, HNil};
use panic_halt as _;
use rp2040_hal::{
    clocks::{init_clocks_and_plls, Clock},
    entry,
    gpio::{FunctionSpi, Pins},
    pac,
    pac::interrupt,
    pio::PIOExt,
    pwm::Slices,
    spi::Spi,
    timer::Timer,
    usb::UsbBus,
    watchdog::Watchdog,
    Sio,
};
use sh1106::{prelude::*, Builder};
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use usb_device::{class_prelude::*, prelude::*};
use usbd_human_interface_device::device::keyboard::{NKROBootKeyboard, NKROBootKeyboardConfig};
use usbd_human_interface_device::page::Keyboard;
use usbd_human_interface_device::prelude::*;
use ws2812_pio::Ws2812;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;
const NUM_LEDS: usize = 12;
const BRIGHTNESS_LEVEL: u8 = 32;

type MyUsbHidClass = UsbHidClass<'static, UsbBus, HCons<NKROBootKeyboard<'static, UsbBus>, HNil>>;

static USB_DEVICE: Mutex<RefCell<Option<UsbDevice<'static, UsbBus>>>> =
    Mutex::new(RefCell::new(None));
static USB_HID: Mutex<RefCell<Option<MyUsbHidClass>>> = Mutex::new(RefCell::new(None));

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    MissionControl,
    VibeCode,
}

impl Mode {
    fn next(self) -> Self {
        match self {
            Mode::MissionControl => Mode::VibeCode,
            Mode::VibeCode => Mode::MissionControl,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::MissionControl => "Mission Ctrl",
            Mode::VibeCode => "Vibe Code",
        }
    }
}

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

fn poll_usb() {
    critical_section::with(|cs| {
        if let Some(usb_dev) = USB_DEVICE.borrow_ref_mut(cs).as_mut() {
            if let Some(hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
                usb_dev.poll(&mut [hid]);
            }
        }
    });
}

fn tick_usb() {
    critical_section::with(|cs| {
        if let Some(hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
            match hid.tick() {
                Ok(_) => {}
                Err(UsbHidError::WouldBlock) => {}
                Err(_) => {}
            }
        }
    });
}

fn send_keys(keys: &[Keyboard]) {
    critical_section::with(|cs| {
        if let Some(hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
            match hid.device().write_report(keys.iter().copied()) {
                Ok(_) => {}
                Err(UsbHidError::WouldBlock) => {}
                Err(UsbHidError::Duplicate) => {}
                Err(_) => {}
            }
        }
    });
    poll_usb();
}

fn release_keys() {
    send_keys(&[]);
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

    static mut USB_BUS: Option<UsbBusAllocator<UsbBus>> = None;
    unsafe {
        USB_BUS = Some(UsbBusAllocator::new(UsbBus::new(
            pac.USBCTRL_REGS,
            pac.USBCTRL_DPRAM,
            clocks.usb_clock,
            true,
            &mut pac.RESETS,
        )));
    }

    let usb_bus = unsafe { USB_BUS.as_ref().unwrap() };

    let usb_hid = UsbHidClassBuilder::new()
        .add_device(NKROBootKeyboardConfig::default())
        .build(usb_bus);

    let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x239A, 0x8107))
        .strings(&[StringDescriptors::default()
            .manufacturer("Adafruit")
            .product("MacroPad RP2040")
            .serial_number("12345678")])
        .unwrap()
        .build();

    critical_section::with(|cs| {
        USB_HID.borrow_ref_mut(cs).replace(usb_hid);
        USB_DEVICE.borrow_ref_mut(cs).replace(usb_dev);
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ);
    }

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    let sclk = pins.gpio26.into_function::<FunctionSpi>();
    let mosi = pins.gpio27.into_function::<FunctionSpi>();
    let miso = pins.gpio28.into_function::<FunctionSpi>();
    let oled_cs = pins.gpio22.into_push_pull_output_in_state(rp2040_hal::gpio::PinState::High);
    let oled_dc = pins.gpio24.into_push_pull_output();
    let mut oled_reset =
        pins.gpio23.into_push_pull_output_in_state(rp2040_hal::gpio::PinState::High);

    let spi = Spi::<_, _, _, 8>::new(pac.SPI1, (mosi, miso, sclk));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        fugit::RateExtU32::MHz(10),
        embedded_hal::spi::MODE_0,
    );

    oled_reset.set_low().ok();
    delay.delay_ms(10_u32);
    oled_reset.set_high().ok();
    delay.delay_ms(10_u32);

    let mut display: GraphicsMode<_> = Builder::new().connect_spi(spi, oled_dc, oled_cs).into();
    display.init().ok();
    display.flush().ok();

    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);
    let mut ws = Ws2812::new(
        pins.gpio19.into_function(),
        &mut pio,
        sm0,
        clocks.peripheral_clock.freq(),
        timer.count_down(),
    );

    let _speaker_shutdown =
        pins.gpio14.into_push_pull_output_in_state(rp2040_hal::gpio::PinState::High);

    let pwm_slices = Slices::new(pac.PWM, &mut pac.RESETS);
    let mut pwm = pwm_slices.pwm0;
    pwm.set_ph_correct();
    pwm.enable();
    pwm.channel_a.output_to(pins.gpio16);

    let sys_freq = clocks.system_clock.freq().to_Hz();

    // Startup beep
    let effective_freq = sys_freq / 64;
    let top = (effective_freq / 440) as u16;
    pwm.set_div_int(64);
    pwm.set_top(top);
    pwm.channel_a.set_duty(top / 8);
    delay.delay_ms(80_u32);
    pwm.channel_a.set_duty(0);

    let encoder_a = pins.gpio18.into_pull_up_input();
    let encoder_b = pins.gpio17.into_pull_up_input();
    let _encoder_btn = pins.gpio0.into_pull_up_input();
    let mut last_a = encoder_a.is_low().unwrap_or(false);

    let key1 = pins.gpio1.into_pull_up_input();
    let key2 = pins.gpio2.into_pull_up_input();
    let key3 = pins.gpio3.into_pull_up_input();
    let key4 = pins.gpio4.into_pull_up_input();
    let key5 = pins.gpio5.into_pull_up_input();
    let key6 = pins.gpio6.into_pull_up_input();
    let key7 = pins.gpio7.into_pull_up_input();
    let key8 = pins.gpio8.into_pull_up_input();
    let key9 = pins.gpio9.into_pull_up_input();
    let key10 = pins.gpio10.into_pull_up_input();
    let key11 = pins.gpio11.into_pull_up_input();
    let key12 = pins.gpio12.into_pull_up_input();

    let mut led_data = [RGB8::default(); NUM_LEDS];
    let mut offset: u8 = 0;
    let mut prev_keys: [bool; 12] = [false; 12];
    let mut display_needs_update = true;
    let mut tick_counter: u32 = 0;
    let mut current_mode = Mode::MissionControl;

    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    loop {
        tick_counter = tick_counter.wrapping_add(1);
        if tick_counter % 10 == 0 {
            tick_usb();
        }

        // Encoder rotation for mode switching
        let a = encoder_a.is_low().unwrap_or(false);
        let b = encoder_b.is_low().unwrap_or(false);
        if a != last_a && a {
            current_mode = current_mode.next();
            display_needs_update = true;
        }
        last_a = a;

        // Update display
        if display_needs_update {
            display.clear();
            Text::new(current_mode.name(), Point::new(20, 12), text_style)
                .draw(&mut display)
                .ok();

            match current_mode {
                Mode::MissionControl => {
                    Text::new("      ZOOM  ", Point::new(5, 24), text_style)
                        .draw(&mut display)
                        .ok();
                    Text::new("      TODAY RCAST", Point::new(5, 54), text_style)
                        .draw(&mut display)
                        .ok();
                }
                Mode::VibeCode => {
                    Text::new("REC   ENTER CYCLE", Point::new(5, 24), text_style)
                        .draw(&mut display)
                        .ok();
                    Text::new("ESC", Point::new(5, 34), text_style)
                        .draw(&mut display)
                        .ok();
                }
            }

            display.flush().ok();
            display_needs_update = false;
        }

        let keys = [
            key1.is_low().unwrap_or(false),
            key2.is_low().unwrap_or(false),
            key3.is_low().unwrap_or(false),
            key4.is_low().unwrap_or(false),
            key5.is_low().unwrap_or(false),
            key6.is_low().unwrap_or(false),
            key7.is_low().unwrap_or(false),
            key8.is_low().unwrap_or(false),
            key9.is_low().unwrap_or(false),
            key10.is_low().unwrap_or(false),
            key11.is_low().unwrap_or(false),
            key12.is_low().unwrap_or(false),
        ];

        match current_mode {
            Mode::MissionControl => {
                for (i, (&pressed, &prev)) in keys.iter().zip(prev_keys.iter()).enumerate() {
                    if pressed && !prev {
                        match i {
                            2 => {
                                // Key 3: Zoom Mute (Cmd+Shift+A)
                                send_keys(&[Keyboard::LeftGUI, Keyboard::LeftShift, Keyboard::A]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(10_u32);
                            }
                            10 => {
                                // Key 11: Today - sends "!td"
                                send_keys(&[Keyboard::LeftShift, Keyboard::Keyboard1]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(50_u32);

                                send_keys(&[Keyboard::F]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(50_u32);

                                send_keys(&[Keyboard::G]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(20_u32);
                            }
                            11 => {
                                // Key 12: Raycast (Ctrl+Space)
                                send_keys(&[Keyboard::LeftControl, Keyboard::Space]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(10_u32);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Mode::VibeCode => {
                for (i, (&pressed, &prev)) in keys.iter().zip(prev_keys.iter()).enumerate() {
                    match i {
                        0 => {
                            // Key 1: REC (Cmd+Shift+R) - hold until release
                            if pressed && !prev {
                                send_keys(&[Keyboard::LeftGUI, Keyboard::LeftShift, Keyboard::R]);
                            } else if !pressed && prev {
                                release_keys();
                            }
                        }
                        1 => {
                            // Key 2: ENTER
                            if pressed && !prev {
                                send_keys(&[Keyboard::ReturnEnter]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(10_u32);
                            }
                        }
                        2 => {
                            // Key 3: CYCLE (Shift+Tab)
                            if pressed && !prev {
                                send_keys(&[Keyboard::LeftShift, Keyboard::Tab]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(10_u32);
                            }
                        }
                        3 => {
                            // Key 4: ESC (send twice)
                            if pressed && !prev {
                                send_keys(&[Keyboard::Escape]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(50_u32);
                                send_keys(&[Keyboard::Escape]);
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(10_u32);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        prev_keys = keys;

        poll_usb();

        for i in 0..NUM_LEDS {
            led_data[i] = wheel(offset.wrapping_add((i as u8) * 21));
        }
        ws.write(brightness(led_data.iter().copied(), BRIGHTNESS_LEVEL))
            .unwrap();
        offset = offset.wrapping_add(2);
        delay.delay_ms(10_u32);
    }
}

#[allow(non_snake_case)]
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    critical_section::with(|cs| {
        if let Some(usb_dev) = USB_DEVICE.borrow_ref_mut(cs).as_mut() {
            if let Some(hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
                usb_dev.poll(&mut [hid]);
            }
        }
    });
}
