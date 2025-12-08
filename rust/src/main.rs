#![no_std]
#![no_main]

use adafruit_macropad::{
    entry,
    hal::{
        clocks::{init_clocks_and_plls, Clock},
        gpio::PinState,
        pac,
        pac::interrupt,
        pio::PIOExt,
        pwm::Slices,
        spi::Spi,
        timer::Timer,
        usb::UsbBus,
        watchdog::Watchdog,
        Sio,
    },
    Pins, XOSC_CRYSTAL_FREQ,
};
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
use panic_halt as _;
use sh1106::{prelude::*, Builder};
use smart_leds::{brightness, SmartLedsWrite, RGB8};
use usb_device::{class_prelude::*, prelude::*};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor}; // KeyboardReport used for desc()
use usbd_hid::hid_class::HIDClass;
use ws2812_pio::Ws2812;

const NUM_LEDS: usize = 12;
const BRIGHTNESS_LEVEL: u8 = 32;

// Piano note frequencies (C4 to C6 chromatic scale) in Hz - extended for arpeggios
const NOTES: [u32; 25] = [
    262, 277, 294, 311, 330, 349, 370, 392, 415, 440, 466, 494,  // C4-B4
    523, 554, 587, 622, 659, 698, 740, 784, 831, 880, 932, 988,  // C5-B5
    1047, // C6
];

const TONE_DURATION_MS: u32 = 200;
const ARPEGGIO_NOTE_MS: u32 = 100;

// Arpeggio patterns (intervals from root note in semitones)
// Major triad: root, major 3rd, perfect 5th
const ARPEGGIO_MAJOR: [i8; 4] = [0, 4, 7, 12];
// Minor triad: root, minor 3rd, perfect 5th
const ARPEGGIO_MINOR: [i8; 4] = [0, 3, 7, 12];

// Mario theme startup melody (frequency in Hz, duration in ms)
const MARIO_MELODY: [(u32, u32); 13] = [
    (660, 100), (660, 100), (0, 100), (660, 100), (0, 100),
    (520, 100), (660, 100), (0, 100), (784, 150), (0, 150),
    (392, 150), (0, 150), (0, 0),
];

// Descending melody
const MELODY_2: [(u32, u32); 9] = [
    (740, 150), (659, 150), (587, 150), (554, 150),
    (494, 150), (440, 150), (415, 150), (440, 200), (0, 0),
];

// USB keyboard modifiers
const MOD_LCTRL: u8 = 0x01;
const MOD_LSHIFT: u8 = 0x02;
const MOD_LALT: u8 = 0x04;
const MOD_LGUI: u8 = 0x08;  // Cmd on Mac

// USB keyboard keycodes (HID Usage Table) - COLEMAK layout
// HID sends physical positions, OS interprets based on layout
// From test: QWERTY 'f' -> Colemak 't', QWERTY 'g' -> Colemak 'd'
const KEY_A: u8 = 0x04;  // 'a' same position
const KEY_D: u8 = 0x0A;  // 'd' is at QWERTY 'g' position (0x0A)
const KEY_T: u8 = 0x09;  // 't' is at QWERTY 'f' position (0x09)
const KEY_1: u8 = 0x1E;  // '1' (Shift+'1' = '!')
const KEY_SPACE: u8 = 0x2C;

// Global USB state
static USB_DEVICE: Mutex<RefCell<Option<UsbDevice<UsbBus>>>> = Mutex::new(RefCell::new(None));
static USB_HID: Mutex<RefCell<Option<HIDClass<UsbBus>>>> = Mutex::new(RefCell::new(None));

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Music,
    MissionControl,
}

impl Mode {
    fn next(self) -> Self {
        match self {
            Mode::Music => Mode::MissionControl,
            Mode::MissionControl => Mode::Music,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Music => "Music Mode",
            Mode::MissionControl => "Mission Ctrl",
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
            if let Some(usb_hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
                usb_dev.poll(&mut [usb_hid]);
            }
        }
    });
}

fn send_keyboard_report(modifier: u8, keycode: u8) {
    // KeyboardReport descriptor expects 9 bytes: modifier(1), reserved(1), leds(1), keycodes(6)
    // But leds is OUTPUT only, so for INPUT we send modifier, reserved, then keycodes
    // Let's try matching the struct layout exactly
    let report: [u8; 9] = [modifier, 0, 0, keycode, 0, 0, 0, 0, 0];

    critical_section::with(|cs| {
        if let Some(hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
            let _ = hid.push_raw_input(&report);
        }
    });

    // Poll to ensure report is sent
    poll_usb();
}

fn release_keys() {
    send_keyboard_report(0, 0);
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

    // Setup USB
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

    let usb_hid = HIDClass::new(usb_bus, KeyboardReport::desc(), 10);
    let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x239A, 0x8107))
        .manufacturer("Adafruit")
        .product("MacroPad RP2040")
        .serial_number("12345678")
        .device_class(0)
        .build();

    critical_section::with(|cs| {
        USB_HID.borrow_ref_mut(cs).replace(usb_hid);
        USB_DEVICE.borrow_ref_mut(cs).replace(usb_dev);
    });

    // Enable USB interrupt
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ);
    }

    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Setup OLED display
    let sclk = pins.sclk.into_function::<rp2040_hal::gpio::FunctionSpi>();
    let mosi = pins.mosi.into_function::<rp2040_hal::gpio::FunctionSpi>();
    let miso = pins.miso.into_function::<rp2040_hal::gpio::FunctionSpi>();
    let oled_cs = pins.oled_cs.into_push_pull_output_in_state(PinState::High);
    let oled_dc = pins.oled_dc.into_push_pull_output();
    let mut oled_reset = pins.oled_reset.into_push_pull_output_in_state(PinState::High);

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

    // Setup NeoPixels
    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);
    let mut ws = Ws2812::new(
        pins.neopixel.into_function(),
        &mut pio,
        sm0,
        clocks.peripheral_clock.freq(),
        timer.count_down(),
    );

    // Setup speaker
    let _speaker_shutdown = pins
        .speaker_shutdown
        .into_push_pull_output_in_state(PinState::High);

    let pwm_slices = Slices::new(pac.PWM, &mut pac.RESETS);
    let mut pwm = pwm_slices.pwm0;
    pwm.set_ph_correct();
    pwm.enable();
    pwm.channel_a.output_to(pins.speaker);

    let sys_freq = clocks.system_clock.freq().to_Hz();

    // Play startup melody
    for &(freq, duration) in MARIO_MELODY.iter() {
        if duration == 0 { break; }
        if freq == 0 {
            delay.delay_ms(duration);
        } else {
            let effective_freq = sys_freq / 64;
            let top = (effective_freq / freq) as u16;
            pwm.set_div_int(64);
            pwm.set_top(top);
            pwm.channel_a.set_duty(top / 2);
            delay.delay_ms(duration);
            pwm.channel_a.set_duty(0);
        }
        delay.delay_ms(20);
    }

    // Setup encoder
    let encoder_a = pins.encoder_rota.into_pull_up_input();
    let encoder_b = pins.encoder_rotb.into_pull_up_input();
    let encoder_btn = pins.button.into_pull_up_input();
    let mut last_a = encoder_a.is_low().unwrap_or(false);
    let mut last_btn = encoder_btn.is_low().unwrap_or(false);

    // Setup keys
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
    let mut prev_keys: [bool; 12] = [false; 12];
    let mut current_mode = Mode::MissionControl; // Start in Mission Control
    let mut mode_changed = true;
    let mut arpeggio_mode = false; // Toggle with encoder button in Music mode

    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    loop {
        // Check encoder rotation for mode change
        let a = encoder_a.is_low().unwrap_or(false);
        let b = encoder_b.is_low().unwrap_or(false);
        if a != last_a && a {
            if b != a {
                current_mode = current_mode.next();
            } else {
                current_mode = current_mode.next();
            }
            mode_changed = true;
        }
        last_a = a;

        // Check encoder button for arpeggio toggle (only in Music mode)
        let btn = encoder_btn.is_low().unwrap_or(false);
        if btn && !last_btn && current_mode == Mode::Music {
            arpeggio_mode = !arpeggio_mode;
            mode_changed = true;
        }
        last_btn = btn;

        // Update display on mode change
        if mode_changed {
            display.clear();

            Text::new(current_mode.name(), Point::new(20, 12), text_style)
                .draw(&mut display)
                .ok();

            match current_mode {
                Mode::Music => {
                    if arpeggio_mode {
                        Text::new("[ARPEGGIO]", Point::new(20, 28), text_style)
                            .draw(&mut display)
                            .ok();
                        Text::new("Maj: C D E F G A", Point::new(5, 40), text_style)
                            .draw(&mut display)
                            .ok();
                        Text::new("Min: C#D#F#G#A#B", Point::new(5, 52), text_style)
                            .draw(&mut display)
                            .ok();
                    } else {
                        Text::new("C  C# D  D#", Point::new(10, 28), text_style)
                            .draw(&mut display)
                            .ok();
                        Text::new("E  F  F# G", Point::new(10, 40), text_style)
                            .draw(&mut display)
                            .ok();
                        Text::new("G# A  A# B", Point::new(10, 52), text_style)
                            .draw(&mut display)
                            .ok();
                    }
                }
                Mode::MissionControl => {
                    // 4 rows x 3 cols, 6 char labels
                    Text::new("Mario        ", Point::new(5, 24), text_style)
                        .draw(&mut display)
                        .ok();
                    Text::new("      Mute   ", Point::new(5, 34), text_style)
                        .draw(&mut display)
                        .ok();
                    Text::new("             ", Point::new(5, 44), text_style)
                        .draw(&mut display)
                        .ok();
                    Text::new("      Today  Raycst", Point::new(5, 54), text_style)
                        .draw(&mut display)
                        .ok();
                }
            }

            display.flush().ok();
            mode_changed = false;
        }

        // Check keys
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

        // Handle key presses based on mode
        for (i, (&pressed, &prev)) in keys.iter().zip(prev_keys.iter()).enumerate() {
            if pressed && !prev {
                match current_mode {
                    Mode::Music => {
                        if arpeggio_mode {
                            // Even keys (0,2,4,6,8,10) = major arpeggios on C,D,E,F,G,A
                            // Odd keys (1,3,5,7,9,11) = minor arpeggios on C#,D#,F#,G#,A#,B
                            let is_minor = i % 2 == 1;
                            let pattern = if is_minor { &ARPEGGIO_MINOR } else { &ARPEGGIO_MAJOR };

                            // Play arpeggio up then down
                            for &interval in pattern.iter() {
                                let note_idx = (i as i8 + interval) as usize;
                                if note_idx < NOTES.len() {
                                    let freq = NOTES[note_idx];
                                    let effective_freq = sys_freq / 64;
                                    let top = (effective_freq / freq) as u16;
                                    pwm.set_div_int(64);
                                    pwm.set_top(top);
                                    pwm.channel_a.set_duty(top / 2);
                                    delay.delay_ms(ARPEGGIO_NOTE_MS);
                                    pwm.channel_a.set_duty(0);
                                    delay.delay_ms(10);
                                }
                            }
                            // Play back down (skip last since we just played it)
                            for &interval in pattern.iter().rev().skip(1) {
                                let note_idx = (i as i8 + interval) as usize;
                                if note_idx < NOTES.len() {
                                    let freq = NOTES[note_idx];
                                    let effective_freq = sys_freq / 64;
                                    let top = (effective_freq / freq) as u16;
                                    pwm.set_div_int(64);
                                    pwm.set_top(top);
                                    pwm.channel_a.set_duty(top / 2);
                                    delay.delay_ms(ARPEGGIO_NOTE_MS);
                                    pwm.channel_a.set_duty(0);
                                    delay.delay_ms(10);
                                }
                            }
                        } else {
                            let freq = NOTES[i];
                            let effective_freq = sys_freq / 64;
                            let top = (effective_freq / freq) as u16;
                            pwm.set_div_int(64);
                            pwm.set_top(top);
                            pwm.channel_a.set_duty(top / 2);
                            delay.delay_ms(TONE_DURATION_MS);
                            pwm.channel_a.set_duty(0);
                        }
                    }
                    Mode::MissionControl => {
                        match i {
                            0 => {
                                // Key 1: Mario melody
                                for &(freq, duration) in MARIO_MELODY.iter() {
                                    if duration == 0 { break; }
                                    if freq == 0 {
                                        delay.delay_ms(duration);
                                    } else {
                                        let effective_freq = sys_freq / 64;
                                        let top = (effective_freq / freq) as u16;
                                        pwm.set_div_int(64);
                                        pwm.set_top(top);
                                        pwm.channel_a.set_duty(top / 2);
                                        delay.delay_ms(duration);
                                        pwm.channel_a.set_duty(0);
                                    }
                                    delay.delay_ms(20);
                                }
                            }
                            2 => {
                                // Key 3: Zoom Mute (Cmd+Shift+A)
                                // Poll USB first
                                poll_usb();

                                // Send keyboard shortcut
                                send_keyboard_report(MOD_LGUI | MOD_LSHIFT, KEY_A);
                                delay.delay_ms(10_u32);
                                poll_usb();
                                delay.delay_ms(10_u32);
                                poll_usb();
                                delay.delay_ms(30_u32);

                                // Release keys
                                release_keys();
                                delay.delay_ms(10_u32);
                                poll_usb();
                                delay.delay_ms(10_u32);

                                // Play confirmation beep after send
                                let effective_freq = sys_freq / 64;
                                let top = (effective_freq / 880) as u16; // High A
                                pwm.set_div_int(64);
                                pwm.set_top(top);
                                pwm.channel_a.set_duty(top / 2);
                                delay.delay_ms(50_u32);
                                pwm.channel_a.set_duty(0);
                            }
                            10 => {
                                // Key 11: Today - sends "!td" for Obsidian
                                poll_usb();
                                delay.delay_ms(50_u32);

                                // '!' = Shift + 1
                                send_keyboard_report(MOD_LSHIFT, KEY_1);
                                delay.delay_ms(20_u32);
                                poll_usb();
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(20_u32);
                                poll_usb();
                                delay.delay_ms(50_u32);

                                // 't'
                                send_keyboard_report(0, KEY_T);
                                delay.delay_ms(20_u32);
                                poll_usb();
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(20_u32);
                                poll_usb();
                                delay.delay_ms(50_u32);

                                // 'd'
                                send_keyboard_report(0, KEY_D);
                                delay.delay_ms(20_u32);
                                poll_usb();
                                delay.delay_ms(50_u32);
                                release_keys();
                                delay.delay_ms(20_u32);
                                poll_usb();

                                // Confirmation beep
                                let effective_freq = sys_freq / 64;
                                let top = (effective_freq / 660) as u16;
                                pwm.set_div_int(64);
                                pwm.set_top(top);
                                pwm.channel_a.set_duty(top / 2);
                                delay.delay_ms(50_u32);
                                pwm.channel_a.set_duty(0);
                            }
                            11 => {
                                // Key 12: Raycast (Ctrl+Space)
                                poll_usb();
                                send_keyboard_report(MOD_LCTRL, KEY_SPACE);
                                delay.delay_ms(10_u32);
                                poll_usb();
                                delay.delay_ms(30_u32);
                                release_keys();
                                delay.delay_ms(10_u32);
                                poll_usb();

                                // Confirmation beep
                                let effective_freq = sys_freq / 64;
                                let top = (effective_freq / 660) as u16;
                                pwm.set_div_int(64);
                                pwm.set_top(top);
                                pwm.channel_a.set_duty(top / 2);
                                delay.delay_ms(50_u32);
                                pwm.channel_a.set_duty(0);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        prev_keys = keys;

        // Poll USB to keep it active
        poll_usb();

        // Rainbow LED animation
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
            if let Some(usb_hid) = USB_HID.borrow_ref_mut(cs).as_mut() {
                usb_dev.poll(&mut [usb_hid]);
            }
        }
    });
}
