#![no_std]

use anyhow::Result;
use elgato_streamdeck_local::HidDevice;

extern crate alloc;
use alloc::vec::Vec;
use leaf_comm::{Command, DeviceActions, RemoteConfig};

fn rust_try_read_network() -> Result<Option<u8>> {
    let mut buf = [0u8; 1];
    let success = unsafe { arduino_try_read_network(buf.as_mut_ptr()) };
    if success {
        Ok::<_, anyhow::Error>(Some(buf[0]))
    } else {
        Ok(None)
    }
}

fn rust_write_network(buf: &[u8]) -> Result<()> {
    let success = unsafe { arduino_write_network(buf.as_ptr(), buf.len() as u32) };
    if success {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Could not write to network"))
    }
}

#[no_mangle]
pub extern "C" fn run_rust() {
    let usb = ArduinoUSB {};
    _ = run_teensy(rust_try_read_network, rust_write_network, usb);
}

#[no_mangle]
pub extern "C" fn run_led_test() {
    loop {
        unsafe {
            arduino_led(true);
        }
        unsafe {
            arduino_sleep_seconds(1);
        }
        unsafe {
            arduino_led(false);
        }
        unsafe {
            arduino_sleep_seconds(1);
        }
    }
}

struct ArduinoUSB {}
impl HidDevice for ArduinoUSB {
    fn read_timeout(
        &self,
        buf: &mut [u8],
        _timeout: i32,
    ) -> core::prelude::v1::Result<(), elgato_streamdeck_local::HidError> {
        // Call the arduino C version of this
        let success = unsafe { arduino_usb_read_timeout(buf.as_mut_ptr(), buf.len() as u32) };
        if success {
            Ok(())
        } else {
            Err(elgato_streamdeck_local::HidError {})
        }
    }

    fn read(
        &self,
        buf: &mut [u8],
    ) -> core::prelude::v1::Result<(), elgato_streamdeck_local::HidError> {
        let success = unsafe { arduino_usb_read(buf.as_mut_ptr(), buf.len() as u32) };
        if success {
            Ok(())
        } else {
            Err(elgato_streamdeck_local::HidError {})
        }
    }

    fn write(
        &self,
        payload: &[u8],
    ) -> core::prelude::v1::Result<usize, elgato_streamdeck_local::HidError> {
        let success = unsafe { arduino_usb_write(payload.as_ptr(), payload.len() as u32) };
        if success {
            Ok(payload.len())
        } else {
            Err(elgato_streamdeck_local::HidError {})
        }
    }

    fn get_feature_report(
        &self,
        buf: &mut [u8],
    ) -> core::prelude::v1::Result<(), elgato_streamdeck_local::HidError> {
        let success = unsafe { arduino_usb_get_feature_report(buf.as_mut_ptr(), buf.len() as u32) };
        if success {
            Ok(())
        } else {
            Err(elgato_streamdeck_local::HidError {})
        }
    }

    fn send_feature_report(
        &self,
        payload: &[u8],
    ) -> core::prelude::v1::Result<(), elgato_streamdeck_local::HidError> {
        let success = unsafe { arduino_usb_send_feature_report(payload.as_ptr(), payload.len() as u32) };
        if success {
            Ok(())
        } else {
            Err(elgato_streamdeck_local::HidError {})
        }
    }
}

#[cfg(feature = "arduino_allocator")]
#[global_allocator]
static GLOBAL: ArduinoAllocator = ArduinoAllocator;

#[cfg(feature = "arduino_allocator")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    unsafe {
        arduino_led(true);
    }
    loop {}
}

struct ArduinoAllocator;
// Implement the GlobalAlloc trait for ArduinoAllocator
unsafe impl core::alloc::GlobalAlloc for ArduinoAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        arduino_malloc(layout.size() as u32)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: core::alloc::Layout) {
        arduino_free(ptr)
    }
}

// These are methods that we can call defined in C
extern "C" {
    fn arduino_try_read_network(byte_ptr: *mut u8) -> bool;
    fn arduino_write_network(byte_ptr: *const u8, len: u32) -> bool;

    fn arduino_usb_read_timeout(buf: *mut u8, len: u32) -> bool;
    fn arduino_usb_read(buf: *mut u8, len: u32) -> bool;
    fn arduino_usb_write(buf: *const u8, len: u32) -> bool;
    fn arduino_usb_get_feature_report(buf: *mut u8, len: u32) -> bool;
    fn arduino_usb_send_feature_report(payload: *const u8, len: u32) -> bool;

    fn arduino_malloc(size: u32) -> *mut u8;
    fn arduino_free(ptr: *mut u8);

    fn arduino_led(on: bool);
    fn arduino_sleep_seconds(seconds: u32);
}

pub fn run_teensy(
    mut try_read_network: impl FnMut() -> Result<Option<u8>>,
    mut write_network: impl FnMut(&[u8]) -> Result<()>,
    usb: impl HidDevice,
) -> Result<()> {
    // Connect to the device
    let device =
        elgato_streamdeck_local::StreamDeck::new(usb, elgato_streamdeck_local::info::Kind::Mk2);

    // Connect to companion
    // Read from the companion stream and write to console
    let serial_number = device
        .serial_number()
        .map_err(|_| anyhow::anyhow!("Could not get serial number"))?;
    //println!("Serial number: {}", serial_number);

    // Get our kind from the config
    let pid = 0x0080;

    // Send config to companion
    let config = RemoteConfig {
        pid,
        device_id: serial_number,
    };
    // Write this to the network
    frame_write(&Command::Config(config), &mut write_network)?;

    // write_network(
    //     format!(
    //         "ADD-DEVICE {}\n",
    //         companion::DeviceMsg {
    //             device_id: serial_number,
    //             product_name: format!("TeensySatellite StreamDeck: {}", kind.to_string()),
    //             keys_total: kind.key_count(),
    //             keys_per_row: kind.column_count(),
    //             resolution: kind
    //                 .key_image_format()
    //                 .size
    //                 .0
    //                 .try_into()
    //                 .map_err(|_| anyhow::anyhow!("Cannot convert resolution"))?,
    //         }
    //         .device_msg()
    //     )
    //     .as_bytes(),
    // )?;

    // do something with device
    device
        .reset()
        .map_err(|_| anyhow::anyhow!("Could not reset device"))?;
    device
        .set_brightness(10)
        .map_err(|_| anyhow::anyhow!("Could not set brightness"))?;

    // loop forever
    let mut frame_accumulator = FrameAccumulator::default();
    loop {
        // Try reading from socket
        let value = try_read_network()?;
        match value {
            None => {}
            Some(value) => {
                if let Some(frame) = frame_accumulator.add_char(value) {
                    //println!("Got frame size: {}", frame.len());
                    let action: DeviceActions = postcard::from_bytes(frame)
                        .map_err(|_| anyhow::anyhow!("Cannot generate from bytes"))?;
                    match action {
                        DeviceActions::SetButtonImage(b) => {
                            //println!("Set button image: {:?}", b.button);
                            device
                                .write_image(b.button, &b.image)
                                .map_err(|_| anyhow::anyhow!("Could not write image"))?;
                        }
                        DeviceActions::SetLCDImage(_l) => {
                            //println!("Set LCD image: {:?}", l);
                        }
                        DeviceActions::SetBrightness(b) => {
                            //println!("Set brightness: {:?}", b);
                            device
                                .set_brightness(b.brightness)
                                .map_err(|_| anyhow::anyhow!("Could not set brightness"))?;
                        }
                    }
                    frame_accumulator.clear();
                }
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[derive(Default)]
struct FrameAccumulator {
    buf: Vec<u8>,
    size: Option<usize>,
}
impl FrameAccumulator {
    fn clear(&mut self) {
        self.buf.clear();
        self.size = None;
    }
    fn add_char(&mut self, c: u8) -> Option<&[u8]> {
        self.buf.push(c);
        match self.size {
            Some(size) => {
                if self.buf.len() == size {
                    Some(self.buf.as_slice())
                } else {
                    None
                }
            }
            None => {
                if self.buf.len() == 4 {
                    let size =
                        u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]);
                    if size != 0 {
                        self.size = Some(size as usize);
                        self.buf.clear();
                        None
                    } else {
                        Some(&[])
                    }
                } else {
                    None
                }
            }
        }
    }
}

fn frame_write<D>(data: &D, mut write_network: impl FnMut(&[u8]) -> Result<()>) -> Result<()>
where
    D: serde::Serialize,
{
    let data =
        postcard::to_vec::<_, 128>(data).map_err(|_| anyhow::anyhow!("Cannot serialize data"))?;
    let size: u32 = data
        .len()
        .try_into()
        .map_err(|_| anyhow::anyhow!("data len too big"))?;
    let size = size.to_be_bytes();
    write_network(&size)?;
    write_network(&data)?;
    Ok(())
}
