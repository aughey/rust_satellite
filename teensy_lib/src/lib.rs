use anyhow::Result;
use companion::Command;
use elgato_streamdeck_local::HidDevice;

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
    println!("Serial number: {}", serial_number);

    // Get our kind from the config
    let pid = 0x0080;
    let kind = elgato_streamdeck_local::info::Kind::from_pid(pid)
        .ok_or_else(|| anyhow::anyhow!("Unknown pid {}", pid))?;

    write_network(
        format!(
            "ADD-DEVICE {}\n",
            companion::DeviceMsg {
                device_id: serial_number,
                product_name: format!("TeensySatellite StreamDeck: {}", kind.to_string()),
                keys_total: kind.key_count(),
                keys_per_row: kind.column_count(),
                resolution: kind
                    .key_image_format()
                    .size
                    .0
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("Cannot convert resolution"))?,
            }
            .device_msg()
        )
        .as_bytes(),
    )?;

    // do something with device
    device
        .reset()
        .map_err(|_| anyhow::anyhow!("Could not reset device"))?;
    device
        .set_brightness(10)
        .map_err(|_| anyhow::anyhow!("Could not set brightness"))?;

    // loop forever
    let mut line_accumulator = LineAccumulator::default();
    let mut ping_time = std::time::Instant::now();
    loop {
        // Handle ping
        if ping_time.elapsed().as_secs() > 1 {
            write_network(b"PING\n")?;
            ping_time = std::time::Instant::now();
        }
        // Try reading from socket
        let value = try_read_network()?;
        match value {
            None => {}
            Some(value) => {
                if let Some(line) = line_accumulator.add_char(value) {
                    let command = Command::parse(line)?;
                    match command {
                        Command::Pong => {}
                        Command::KeyPress(_) => {}
                        Command::KeyRotate(_) => {}
                        Command::Begin(info) => {
                            println!("Begin: {:?}", info);
                            // api version must be 1.5.1
                            if info.api_version.as_str() != "1.5.1" {
                                anyhow::bail!(
                                    "Unsupported api version: {}",
                                    info.api_version.as_str()
                                );
                            }
                        }
                        Command::AddDevice(info) => {
                            // Must be success
                            if info.success {
                                println!("Added device: {:?}", info);
                            } else {
                                println!("Failed to add device: {:?}", info);
                                anyhow::bail!("Failed to add device");
                            }
                        }
                        Command::KeyState(ks) => {
                            let size = kind.key_image_format().size.0;
                            let bitmap = ks.bitmap()?;
                            if bitmap.len() != size * size * 3 {
                                anyhow::bail!(
                                    "Expected bitmap to be len {}, but was {}",
                                    size * size * 3,
                                    bitmap.len()
                                );
                            }
                            let image = image::DynamicImage::ImageRgb8(
                                image::ImageBuffer::from_vec(
                                    size.try_into()
                                        .map_err(|_| anyhow::anyhow!("Could not convert size"))?,
                                    size.try_into()
                                        .map_err(|_| anyhow::anyhow!("Could not convert size"))?,
                                    ks.bitmap()?,
                                )
                                .ok_or_else(|| anyhow::anyhow!("Couldn't extract image buffer"))?,
                            );
                            let image = elgato_streamdeck_local::images::convert_image(kind, image)
                                .map_err(|_| anyhow::anyhow!("Could not convert image"))?;
                            device
                                .write_image(ks.key, &image)
                                .map_err(|_| anyhow::anyhow!("Could not write image"))?;
                        }
                        Command::Brightness(b) => {
                            println!("Got brightness: {}", b.brightness);
                            device.set_brightness(b.brightness).map_err(|_| anyhow::anyhow!("Could not set brightness"))?;
                        }
                        Command::Unknown(_) => todo!(),
                    }
                    line_accumulator.clear();
                }
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[derive(Default)]
struct LineAccumulator {
    buf: Vec<u8>,
}
impl LineAccumulator {
    fn clear(&mut self) {
        self.buf.clear();
    }
    fn add_char(&mut self, c: u8) -> Option<&str> {
        if c == b'\n' {
            let s = std::str::from_utf8(&self.buf).unwrap();
            Some(s)
        } else {
            self.buf.push(c);
            None
        }
    }
}
