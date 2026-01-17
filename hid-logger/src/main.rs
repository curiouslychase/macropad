use hidapi::HidApi;
use std::time::Duration;

fn main() {
    let api = HidApi::new().expect("Failed to create HID API");

    // List all HID devices
    println!("Looking for MacroPad (VID: 0x239A, PID: 0x8107)...\n");

    let mut found = false;
    for device in api.device_list() {
        if device.vendor_id() == 0x239A && device.product_id() == 0x8107 {
            println!("Found MacroPad!");
            println!("  Manufacturer: {:?}", device.manufacturer_string());
            println!("  Product: {:?}", device.product_string());
            println!("  Path: {:?}", device.path());
            println!("  Interface: {}", device.interface_number());
            println!("  Usage Page: 0x{:04X}", device.usage_page());
            println!("  Usage: 0x{:04X}", device.usage());
            println!();
            found = true;
        }
    }

    if !found {
        println!("MacroPad not found! Make sure it's connected.");
        println!("\nAll HID devices:");
        for device in api.device_list() {
            println!("  VID: 0x{:04X}, PID: 0x{:04X} - {:?}",
                device.vendor_id(),
                device.product_id(),
                device.product_string());
        }
        return;
    }

    // Try to open the keyboard interface (usage page 0x01, usage 0x06)
    let device = api.device_list()
        .find(|d| d.vendor_id() == 0x239A && d.product_id() == 0x8107 && d.usage_page() == 0x01 && d.usage() == 0x06)
        .and_then(|d| d.open_device(&api).ok());

    let device = match device {
        Some(d) => d,
        None => {
            println!("Could not open MacroPad keyboard interface.");
            println!("Try running with sudo or check permissions.");
            return;
        }
    };

    println!("Opened MacroPad keyboard interface!");
    println!("Press keys on the macropad to see raw HID reports...");
    println!("Press Ctrl+C to exit.\n");

    let mut buf = [0u8; 64];

    // Set non-blocking mode
    device.set_blocking_mode(false).ok();

    loop {
        match device.read_timeout(&mut buf, 100) {
            Ok(len) if len > 0 => {
                print!("Report ({} bytes): ", len);
                for i in 0..len {
                    print!("{:02X} ", buf[i]);
                }

                // Parse keyboard report
                if len >= 8 {
                    let modifier = buf[0];
                    let keycode = buf[2]; // Standard keyboard report

                    print!(" | Mod: 0x{:02X}", modifier);
                    if modifier & 0x01 != 0 { print!(" LCTRL"); }
                    if modifier & 0x02 != 0 { print!(" LSHIFT"); }
                    if modifier & 0x04 != 0 { print!(" LALT"); }
                    if modifier & 0x08 != 0 { print!(" LGUI"); }

                    if keycode != 0 {
                        print!(" | Key: 0x{:02X}", keycode);
                    }
                }
                println!();
            }
            Ok(_) => {} // No data
            Err(e) => {
                eprintln!("Error reading: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
