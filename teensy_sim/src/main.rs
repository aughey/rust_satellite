use hidapi::{HidApi, HidDevice, HidError};
use anyhow::Result;

pub const ELGATO_VENDOR_ID: u16 = 0x0fd9;
pub const PID_STREAMDECK_MK2: u16 = 0x0080;
pub const SERIAL: u16 = 0x0001;

fn main() -> Result<()> {
    let hidapi = HidApi::new()?;
    let devices = hidapi.device_list().filter_map(|d| {
        if d.vendor_id() != ELGATO_VENDOR_ID {
            return None;
        }

        if let Some(serial) = d.serial_number() {
            if !serial.chars().all(|c| c.is_alphanumeric()) {
                return None;
            }

            Some((d.product_id(), serial.to_string()))
        } else {
            None
        }
    });
    for device in devices {
        println!("{:?}", device);
    }
    //let device = hidapi.open_serial(ELGATO_VENDOR_ID, PID_STREAMDECK_MK2, SERIAL)?;

    Ok(())
}
