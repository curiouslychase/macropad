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
use heapless::String;
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
use usbd_serial::SerialPort;
use ws2812_pio::Ws2812;

#[link_section = ".boot2"]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;
const NUM_LEDS: usize = 12;
const BRIGHTNESS_LEVEL: u8 = 32;

// =============================================================================
// Layers & Status
// =============================================================================

#[derive(Clone, Copy, PartialEq)]
enum Layer {
    Vibe,
    Media,
    Snippet,
}

impl Layer {
    fn next(self) -> Self {
        match self {
            Layer::Vibe => Layer::Media,
            Layer::Media => Layer::Vibe,
            Layer::Snippet => Layer::Snippet, // encoder doesn't change snippet
        }
    }

    fn name(self) -> &'static str {
        match self {
            Layer::Vibe => "VIBE",
            Layer::Media => "MEDIA",
            Layer::Snippet => "SNIPPET",
        }
    }

    fn default_labels(self) -> [&'static str; 12] {
        match self {
            Layer::Vibe => ["REC", "STOP", "CYCLE", "ESC", "ENTER", "TAB", "UNDO", "REDO", "SAVE", "COPY", "PASTE", "SNIP"],
            Layer::Media => ["PREV", "PLAY", "NEXT", "MUTE", "VOL-", "VOL+", "RWD", "STOP", "FWD", "MIC", "CAM", "SNIP"],
            Layer::Snippet => ["!td", "!sh", "RKT", "SNP04", "SNP05", "SNP06", "SNP07", "SNP08", "SNP09", "SNP10", "SNP11", "EXIT"],
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Idle,
    Run,
    Wait,
    Err,
}

impl Status {
    fn icon(self) -> char {
        match self {
            Status::Idle => 'o',
            Status::Run => '*',
            Status::Wait => '~',
            Status::Err => 'X',
        }
    }
}

// =============================================================================
// State
// =============================================================================

struct State {
    layer: Layer,
    prev_layer: Layer,
    message: String<20>,
    status: Status,
    custom_colors: Option<[RGB8; 12]>,
    display_dirty: bool,
}

impl State {
    fn new() -> Self {
        Self {
            layer: Layer::Vibe,
            prev_layer: Layer::Vibe,
            message: String::new(),
            status: Status::Idle,
            custom_colors: None,
            display_dirty: true,
        }
    }

    fn toggle_snippet(&mut self) {
        if self.layer == Layer::Snippet {
            self.layer = self.prev_layer;
        } else {
            self.prev_layer = self.layer;
            self.layer = Layer::Snippet;
        }
        self.display_dirty = true;
    }

    fn set_layer(&mut self, layer: Layer) {
        if layer != Layer::Snippet {
            self.layer = layer;
            self.prev_layer = layer;
            self.display_dirty = true;
        }
    }

    fn reset(&mut self) {
        self.custom_colors = None;
        self.message.clear();
        self.status = Status::Idle;
        self.display_dirty = true;
    }
}

// =============================================================================
// USB Globals
// =============================================================================

type MyUsbHidClass = UsbHidClass<'static, UsbBus, HCons<NKROBootKeyboard<'static, UsbBus>, HNil>>;

static USB_DEVICE: Mutex<RefCell<Option<UsbDevice<'static, UsbBus>>>> =
    Mutex::new(RefCell::new(None));
static USB_HID: Mutex<RefCell<Option<MyUsbHidClass>>> = Mutex::new(RefCell::new(None));
static USB_SERIAL: Mutex<RefCell<Option<SerialPort<'static, UsbBus>>>> =
    Mutex::new(RefCell::new(None));

fn poll_usb() {
    critical_section::with(|cs| {
        if let Some(usb_dev) = USB_DEVICE.borrow_ref_mut(cs).as_mut() {
            let mut hid = USB_HID.borrow_ref_mut(cs);
            let mut serial = USB_SERIAL.borrow_ref_mut(cs);
            if let (Some(hid), Some(serial)) = (hid.as_mut(), serial.as_mut()) {
                usb_dev.poll(&mut [hid, serial]);
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

fn read_serial(buf: &mut [u8]) -> usize {
    critical_section::with(|cs| {
        if let Some(serial) = USB_SERIAL.borrow_ref_mut(cs).as_mut() {
            match serial.read(buf) {
                Ok(count) => count,
                Err(_) => 0,
            }
        } else {
            0
        }
    })
}

// =============================================================================
// Serial Protocol Parser
// =============================================================================

fn parse_hex_color(s: &[u8]) -> Option<RGB8> {
    if s.len() != 6 {
        return None;
    }
    let r = hex_byte(&s[0..2])?;
    let g = hex_byte(&s[2..4])?;
    let b = hex_byte(&s[4..6])?;
    Some(RGB8::new(r, g, b))
}

fn hex_byte(s: &[u8]) -> Option<u8> {
    if s.len() != 2 {
        return None;
    }
    let high = hex_digit(s[0])?;
    let low = hex_digit(s[1])?;
    Some((high << 4) | low)
}

fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

fn parse_key_num(s: &[u8]) -> Option<usize> {
    if s.is_empty() || s.len() > 2 {
        return None;
    }
    let mut n: usize = 0;
    for &c in s {
        if c < b'0' || c > b'9' {
            return None;
        }
        n = n * 10 + (c - b'0') as usize;
    }
    if n >= 1 && n <= 12 {
        Some(n - 1)
    } else {
        None
    }
}

fn process_command(cmd: &[u8], state: &mut State) {
    if cmd.len() < 4 {
        return;
    }

    // MSG:<text>
    if cmd.starts_with(b"MSG:") {
        state.message.clear();
        let text = &cmd[4..];
        for &c in text.iter().take(20) {
            if c >= 0x20 && c < 0x7F {
                let _ = state.message.push(c as char);
            }
        }
        state.display_dirty = true;
        return;
    }

    // STS:<state>
    if cmd.starts_with(b"STS:") {
        let status_str = &cmd[4..];
        state.status = match status_str {
            b"IDLE" => Status::Idle,
            b"RUN" => Status::Run,
            b"WAIT" => Status::Wait,
            b"ERR" => Status::Err,
            _ => state.status,
        };
        state.display_dirty = true;
        return;
    }

    // RGB:<key>:<hex>
    if cmd.starts_with(b"RGB:") {
        let rest = &cmd[4..];
        if let Some(colon_pos) = rest.iter().position(|&c| c == b':') {
            if let Some(key_idx) = parse_key_num(&rest[..colon_pos]) {
                if let Some(color) = parse_hex_color(&rest[colon_pos + 1..]) {
                    if state.custom_colors.is_none() {
                        state.custom_colors = Some([RGB8::default(); 12]);
                    }
                    if let Some(ref mut colors) = state.custom_colors {
                        colors[key_idx] = color;
                    }
                }
            }
        }
        return;
    }

    // CLR:
    if cmd.starts_with(b"CLR:") {
        state.message.clear();
        state.display_dirty = true;
        return;
    }

    // RST:
    if cmd.starts_with(b"RST:") {
        state.reset();
        return;
    }
}

// =============================================================================
// LED Effects
// =============================================================================

fn lerp_color(a: RGB8, b: RGB8, t: u8) -> RGB8 {
    let t16 = t as u16;
    let inv_t = 255 - t16;
    RGB8::new(
        ((a.r as u16 * inv_t + b.r as u16 * t16) / 255) as u8,
        ((a.g as u16 * inv_t + b.g as u16 * t16) / 255) as u8,
        ((a.b as u16 * inv_t + b.b as u16 * t16) / 255) as u8,
    )
}

fn vibe_gradient(pos: u8) -> RGB8 {
    // purple -> cyan -> blue
    let purple = RGB8::new(128, 0, 255);
    let cyan = RGB8::new(0, 255, 255);
    let blue = RGB8::new(0, 0, 255);

    if pos < 128 {
        lerp_color(purple, cyan, pos * 2)
    } else {
        lerp_color(cyan, blue, (pos - 128) * 2)
    }
}

fn media_gradient(pos: u8) -> RGB8 {
    // orange -> pink -> red
    let orange = RGB8::new(255, 128, 0);
    let pink = RGB8::new(255, 0, 128);
    let red = RGB8::new(255, 0, 0);

    if pos < 128 {
        lerp_color(orange, pink, pos * 2)
    } else {
        lerp_color(pink, red, (pos - 128) * 2)
    }
}

fn pulse_green(tick: u32) -> RGB8 {
    // Sinusoidal pulse using lookup approximation
    let phase = ((tick / 2) % 256) as u8;
    let intensity = if phase < 128 {
        phase * 2
    } else {
        (255 - phase) * 2
    };
    RGB8::new(0, intensity, 0)
}

fn compute_leds(state: &State, tick: u32) -> [RGB8; NUM_LEDS] {
    let mut leds = [RGB8::default(); NUM_LEDS];
    let offset = (tick / 2) as u8;

    match state.layer {
        Layer::Vibe => {
            for i in 0..11 {
                let pos = offset.wrapping_add((i as u8) * 23);
                leds[i] = vibe_gradient(pos);
            }
            leds[11] = pulse_green(tick);
        }
        Layer::Media => {
            for i in 0..11 {
                let pos = offset.wrapping_add((i as u8) * 23);
                leds[i] = media_gradient(pos);
            }
            leds[11] = pulse_green(tick);
        }
        Layer::Snippet => {
            for i in 0..11 {
                leds[i] = RGB8::new(255, 255, 255); // solid white
            }
            leds[11] = RGB8::new(0, 255, 0); // solid green
        }
    }

    // Apply custom colors if set
    if let Some(ref custom) = state.custom_colors {
        for i in 0..12 {
            if custom[i].r != 0 || custom[i].g != 0 || custom[i].b != 0 {
                leds[i] = custom[i];
            }
        }
    }

    leds
}

// =============================================================================
// Key Actions
// =============================================================================

fn send_string(s: &str, delay: &mut cortex_m::delay::Delay) {
    for c in s.chars() {
        let key = char_to_key(c);
        if let Some((k, shift)) = key {
            if shift {
                send_keys(&[Keyboard::LeftShift, k]);
            } else {
                send_keys(&[k]);
            }
            delay.delay_ms(30_u32);
            release_keys();
            delay.delay_ms(20_u32);
        }
    }
}

// Map desired char directly to (Keyboard key, shift) for Colemak layout
fn char_to_key(c: char) -> Option<(Keyboard, bool)> {
    // Colemak: maps desired output char to QWERTY physical key
    // o -> Semicolon (QWERTY ; position = Colemak o)
    // y -> O (QWERTY o position = Colemak y)
    // ; -> P (QWERTY p position = Colemak ;)
    // : -> P+Shift
    match c {
        'a' => Some((Keyboard::A, false)),
        'b' => Some((Keyboard::B, false)),
        'c' => Some((Keyboard::C, false)),
        'd' => Some((Keyboard::G, false)),
        'e' => Some((Keyboard::K, false)),
        'f' => Some((Keyboard::E, false)),
        'g' => Some((Keyboard::T, false)),
        'h' => Some((Keyboard::H, false)),
        'i' => Some((Keyboard::L, false)),
        'j' => Some((Keyboard::Y, false)),
        'k' => Some((Keyboard::N, false)),
        'l' => Some((Keyboard::U, false)),
        'm' => Some((Keyboard::M, false)),
        'n' => Some((Keyboard::J, false)),
        'o' => Some((Keyboard::Semicolon, false)),
        'p' => Some((Keyboard::R, false)),
        'q' => Some((Keyboard::Q, false)),
        'r' => Some((Keyboard::S, false)),
        's' => Some((Keyboard::D, false)),
        't' => Some((Keyboard::F, false)),
        'u' => Some((Keyboard::I, false)),
        'v' => Some((Keyboard::V, false)),
        'w' => Some((Keyboard::W, false)),
        'x' => Some((Keyboard::X, false)),
        'y' => Some((Keyboard::O, false)),
        'z' => Some((Keyboard::Z, false)),
        'A' => Some((Keyboard::A, true)),
        'B' => Some((Keyboard::B, true)),
        'C' => Some((Keyboard::C, true)),
        'D' => Some((Keyboard::G, true)),
        'E' => Some((Keyboard::K, true)),
        'F' => Some((Keyboard::E, true)),
        'G' => Some((Keyboard::T, true)),
        'H' => Some((Keyboard::H, true)),
        'I' => Some((Keyboard::L, true)),
        'J' => Some((Keyboard::Y, true)),
        'K' => Some((Keyboard::N, true)),
        'L' => Some((Keyboard::U, true)),
        'M' => Some((Keyboard::M, true)),
        'N' => Some((Keyboard::J, true)),
        'O' => Some((Keyboard::Semicolon, true)),
        'P' => Some((Keyboard::R, true)),
        'Q' => Some((Keyboard::Q, true)),
        'R' => Some((Keyboard::S, true)),
        'S' => Some((Keyboard::D, true)),
        'T' => Some((Keyboard::F, true)),
        'U' => Some((Keyboard::I, true)),
        'V' => Some((Keyboard::V, true)),
        'W' => Some((Keyboard::W, true)),
        'X' => Some((Keyboard::X, true)),
        'Y' => Some((Keyboard::O, true)),
        'Z' => Some((Keyboard::Z, true)),
        '0' => Some((Keyboard::Keyboard0, false)),
        '1' => Some((Keyboard::Keyboard1, false)),
        '2' => Some((Keyboard::Keyboard2, false)),
        '3' => Some((Keyboard::Keyboard3, false)),
        '4' => Some((Keyboard::Keyboard4, false)),
        '5' => Some((Keyboard::Keyboard5, false)),
        '6' => Some((Keyboard::Keyboard6, false)),
        '7' => Some((Keyboard::Keyboard7, false)),
        '8' => Some((Keyboard::Keyboard8, false)),
        '9' => Some((Keyboard::Keyboard9, false)),
        ' ' => Some((Keyboard::Space, false)),
        '\n' => Some((Keyboard::ReturnEnter, false)),
        '\t' => Some((Keyboard::Tab, false)),
        '!' => Some((Keyboard::Keyboard1, true)),
        ':' => Some((Keyboard::P, true)),
        ';' => Some((Keyboard::P, false)),
        _ => None,
    }
}

fn handle_vibe_key(key: usize, delay: &mut cortex_m::delay::Delay) {
    match key {
        0 => {
            // REC: Cmd+Shift+R
            send_keys(&[Keyboard::LeftGUI, Keyboard::LeftShift, Keyboard::R]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        1 => {
            // STOP: Escape
            send_keys(&[Keyboard::Escape]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        2 => {
            // CYCLE: Shift+Tab
            send_keys(&[Keyboard::LeftShift, Keyboard::Tab]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        3 => {
            // ESC: Escape x2
            send_keys(&[Keyboard::Escape]);
            delay.delay_ms(50_u32);
            release_keys();
            delay.delay_ms(50_u32);
            send_keys(&[Keyboard::Escape]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        4 => {
            // ENTER
            send_keys(&[Keyboard::ReturnEnter]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        5 => {
            // TAB
            send_keys(&[Keyboard::Tab]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        6 => {
            // UNDO: Cmd+Z
            send_keys(&[Keyboard::LeftGUI, Keyboard::Z]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        7 => {
            // REDO: Cmd+Shift+Z
            send_keys(&[Keyboard::LeftGUI, Keyboard::LeftShift, Keyboard::Z]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        8 => {
            // SAVE: Cmd+S
            send_keys(&[Keyboard::LeftGUI, Keyboard::S]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        9 => {
            // COPY: Cmd+C
            send_keys(&[Keyboard::LeftGUI, Keyboard::C]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        10 => {
            // PASTE: Cmd+V
            send_keys(&[Keyboard::LeftGUI, Keyboard::V]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        _ => {}
    }
}

fn handle_media_key(key: usize, delay: &mut cortex_m::delay::Delay) {
    // Media keys use consumer control codes, but we'll use keyboard shortcuts
    match key {
        0 => {
            // PREV: F7 (common media key)
            send_keys(&[Keyboard::F7]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        1 => {
            // PLAY: F8
            send_keys(&[Keyboard::F8]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        2 => {
            // NEXT: F9
            send_keys(&[Keyboard::F9]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        3 => {
            // MUTE: F10
            send_keys(&[Keyboard::F10]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        4 => {
            // VOL-: F11
            send_keys(&[Keyboard::F11]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        5 => {
            // VOL+: F12
            send_keys(&[Keyboard::F12]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        6 => {
            // RWD: Cmd+Left
            send_keys(&[Keyboard::LeftGUI, Keyboard::LeftArrow]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        7 => {
            // STOP: Escape
            send_keys(&[Keyboard::Escape]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        8 => {
            // FWD: Cmd+Right
            send_keys(&[Keyboard::LeftGUI, Keyboard::RightArrow]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        9 => {
            // MIC: Cmd+Shift+M (common mute mic)
            send_keys(&[Keyboard::LeftGUI, Keyboard::LeftShift, Keyboard::M]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        10 => {
            // CAM: Cmd+Shift+V (common toggle cam)
            send_keys(&[Keyboard::LeftGUI, Keyboard::LeftShift, Keyboard::V]);
            delay.delay_ms(50_u32);
            release_keys();
        }
        _ => {}
    }
}

fn handle_snippet_key(key: usize, delay: &mut cortex_m::delay::Delay) {
    // Type snippet text
    let snippets = [
        "!td", "!sh", ":rocket:", "snip04", "snip05",
        "snip06", "snip07", "snip08", "snip09", "snip10", "snip11",
    ];
    if key < 11 {
        send_string(snippets[key], delay);
    }
}

// =============================================================================
// Main Entry
// =============================================================================

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

    // USB Setup
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

    let usb_serial = SerialPort::new(usb_bus);

    let usb_dev = UsbDeviceBuilder::new(usb_bus, UsbVidPid(0x239A, 0x8107))
        .strings(&[StringDescriptors::default()
            .manufacturer("VibePad")
            .product("Vibe Pad")
            .serial_number("VIBE001")])
        .unwrap()
        .composite_with_iads()
        .build();

    critical_section::with(|cs| {
        USB_HID.borrow_ref_mut(cs).replace(usb_hid);
        USB_SERIAL.borrow_ref_mut(cs).replace(usb_serial);
        USB_DEVICE.borrow_ref_mut(cs).replace(usb_dev);
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::USBCTRL_IRQ);
    }

    // GPIO Setup
    let pins = Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // OLED SPI
    let sclk = pins.gpio26.into_function::<FunctionSpi>();
    let mosi = pins.gpio27.into_function::<FunctionSpi>();
    let miso = pins.gpio28.into_function::<FunctionSpi>();
    let oled_cs = pins.gpio22.into_push_pull_output_in_state(rp2040_hal::gpio::PinState::High);
    let oled_dc = pins.gpio24.into_push_pull_output();
    let mut oled_reset = pins.gpio23.into_push_pull_output_in_state(rp2040_hal::gpio::PinState::High);

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

    // NeoPixels
    let (mut pio, sm0, _, _, _) = pac.PIO0.split(&mut pac.RESETS);
    let mut ws = Ws2812::new(
        pins.gpio19.into_function(),
        &mut pio,
        sm0,
        clocks.peripheral_clock.freq(),
        timer.count_down(),
    );

    // Speaker (startup beep)
    let _speaker_shutdown = pins.gpio14.into_push_pull_output_in_state(rp2040_hal::gpio::PinState::High);
    let pwm_slices = Slices::new(pac.PWM, &mut pac.RESETS);
    let mut pwm = pwm_slices.pwm0;
    pwm.set_ph_correct();
    pwm.enable();
    pwm.channel_a.output_to(pins.gpio16);

    let sys_freq = clocks.system_clock.freq().to_Hz();
    let effective_freq = sys_freq / 64;
    let top = (effective_freq / 440) as u16;
    pwm.set_div_int(64);
    pwm.set_top(top);
    pwm.channel_a.set_duty(top / 8);
    delay.delay_ms(80_u32);
    pwm.channel_a.set_duty(0);

    // Encoder
    let encoder_a = pins.gpio18.into_pull_up_input();
    let encoder_b = pins.gpio17.into_pull_up_input();
    let _encoder_btn = pins.gpio0.into_pull_up_input();
    let mut last_a = encoder_a.is_low().unwrap_or(false);

    // Keys
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

    // State
    let mut state = State::new();
    let mut prev_keys: [bool; 12] = [false; 12];
    let mut tick_counter: u32 = 0;
    let mut serial_buf: [u8; 64] = [0; 64];
    let mut serial_pos: usize = 0;

    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    loop {
        tick_counter = tick_counter.wrapping_add(1);

        // USB tick
        if tick_counter % 10 == 0 {
            tick_usb();
        }

        // Read serial data
        let mut temp_buf = [0u8; 32];
        let count = read_serial(&mut temp_buf);
        for i in 0..count {
            let c = temp_buf[i];
            if c == b'\n' || c == b'\r' {
                if serial_pos > 0 {
                    process_command(&serial_buf[..serial_pos], &mut state);
                    serial_pos = 0;
                }
            } else if serial_pos < serial_buf.len() {
                serial_buf[serial_pos] = c;
                serial_pos += 1;
            }
        }

        // Encoder rotation
        let a = encoder_a.is_low().unwrap_or(false);
        let b = encoder_b.is_low().unwrap_or(false);
        if a != last_a && a {
            if state.layer != Layer::Snippet {
                let new_layer = state.layer.next();
                state.set_layer(new_layer);
            }
        }
        last_a = a;
        let _ = b; // silence unused warning

        // Update display
        if state.display_dirty {
            display.clear();

            // Get labels for current layer
            let labels = state.layer.default_labels();

            // Row 1: keys 1-3 (y=10)
            Text::new(labels[0], Point::new(5, 10), text_style).draw(&mut display).ok();
            Text::new(labels[1], Point::new(47, 10), text_style).draw(&mut display).ok();
            Text::new(labels[2], Point::new(89, 10), text_style).draw(&mut display).ok();

            // Row 2: keys 4-6 (y=22)
            Text::new(labels[3], Point::new(5, 22), text_style).draw(&mut display).ok();
            Text::new(labels[4], Point::new(47, 22), text_style).draw(&mut display).ok();
            Text::new(labels[5], Point::new(89, 22), text_style).draw(&mut display).ok();

            // Row 3: keys 7-9 (y=34)
            Text::new(labels[6], Point::new(5, 34), text_style).draw(&mut display).ok();
            Text::new(labels[7], Point::new(47, 34), text_style).draw(&mut display).ok();
            Text::new(labels[8], Point::new(89, 34), text_style).draw(&mut display).ok();

            // Row 4: keys 10-12 (y=46)
            Text::new(labels[9], Point::new(5, 46), text_style).draw(&mut display).ok();
            Text::new(labels[10], Point::new(47, 46), text_style).draw(&mut display).ok();
            Text::new(labels[11], Point::new(89, 46), text_style).draw(&mut display).ok();

            // Bottom: Layer name + status OR Claude message (y=60)
            if !state.message.is_empty() {
                Text::new(state.message.as_str(), Point::new(5, 60), text_style)
                    .draw(&mut display)
                    .ok();
            } else {
                let layer_name = state.layer.name();
                Text::new(layer_name, Point::new(5, 60), text_style)
                    .draw(&mut display)
                    .ok();
                let mut icon_buf = [0u8; 4];
                let icon_str = state.status.icon().encode_utf8(&mut icon_buf);
                Text::new(icon_str, Point::new(120, 60), text_style)
                    .draw(&mut display)
                    .ok();
            }

            display.flush().ok();
            state.display_dirty = false;
        }

        // Read keys
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

        // Process key presses
        for (i, (&pressed, &prev)) in keys.iter().zip(prev_keys.iter()).enumerate() {
            if pressed && !prev {
                // Key 12 is always snippet toggle
                if i == 11 {
                    state.toggle_snippet();
                } else {
                    match state.layer {
                        Layer::Vibe => handle_vibe_key(i, &mut delay),
                        Layer::Media => handle_media_key(i, &mut delay),
                        Layer::Snippet => handle_snippet_key(i, &mut delay),
                    }
                }
            }
        }
        prev_keys = keys;

        poll_usb();

        // Update LEDs
        let leds = compute_leds(&state, tick_counter);
        ws.write(brightness(leds.iter().copied(), BRIGHTNESS_LEVEL))
            .unwrap();

        delay.delay_ms(10_u32);
    }
}

#[allow(non_snake_case)]
#[interrupt]
unsafe fn USBCTRL_IRQ() {
    critical_section::with(|cs| {
        if let Some(usb_dev) = USB_DEVICE.borrow_ref_mut(cs).as_mut() {
            let mut hid = USB_HID.borrow_ref_mut(cs);
            let mut serial = USB_SERIAL.borrow_ref_mut(cs);
            if let (Some(hid), Some(serial)) = (hid.as_mut(), serial.as_mut()) {
                usb_dev.poll(&mut [hid, serial]);
            }
        }
    });
}
