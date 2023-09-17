use std::{ops::DerefMut, sync::Arc};

use clap::Parser;
use rust_satellite::{ButtonState, Cli, Result};

use elgato_streamdeck::{asynchronous, images::ImageRect, list_devices, new_hidapi};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use tracing::{debug, info, trace};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    // Create instance of HidApi
    let hid = new_hidapi().unwrap();

    // List devices and unsafely take first one
    let (kind, serial) = list_devices(&hid).remove(0);

    // Connect to the device
    let device = asynchronous::AsyncStreamDeck::connect(&hid, kind, &serial)?;

    // Print out some info from the device
    info!(
        "Connected to '{}' with version '{}'",
        device.serial_number().await?,
        device.firmware_version().await?
    );

    device.reset().await?;

    // Set device brightness
    device.set_brightness(35).await?;

    // open up an async tcp connection to the host and port
    // and send a message
    let stream = tokio::net::TcpStream::connect((args.host.as_str(), args.port)).await?;
    info!("Connected to {}:{}", args.host, args.port);

    // turn stream into a lines stream
    let stream = tokio::io::BufStream::new(stream);
    let (reader, writer) = tokio::io::split(stream);

    let writer = Arc::new(Mutex::new(writer));

    // ping task
    {
        let writer = writer.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                let mut writer = writer.lock().await;
                writer.write_all(b"PING\n").await.unwrap();
                writer.flush().await.unwrap();
            }
        });
    }

    let device_id = device.serial_number().await?;

    {
        // button reader task
        let writer = writer.clone();
        let device = device.clone();
        let device_id = device_id.clone();
        tokio::spawn(async move {
            let mut keystate = ButtonState::new(16);

            loop {
                let buttons = device
                    .read_input(60.0)
                    .await
                    .expect("Error reading input from device");
                match buttons {
                    elgato_streamdeck::StreamDeckInput::NoData => {}
                    elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
                        debug!("Button {:?} pressed", buttons);
                        let mut writer = writer.lock().await;
                        keystate
                            .update_all(
                                buttons.into_iter().take(8).enumerate(),
                                writer.deref_mut(),
                                &device_id,
                            )
                            .await
                            .expect("success");
                    }
                    elgato_streamdeck::StreamDeckInput::EncoderStateChange(encoder) => {
                        debug!("Encoder {:?} changed", encoder);
                        let mut writer = writer.lock().await;
                        let states = encoder
                            .into_iter()
                            .take(4)
                            .enumerate()
                            .map(|(index, state)| (index + 8 + 4, state))
                            .collect::<Vec<_>>();
                        keystate
                            .update_all(states, writer.deref_mut(), &device_id)
                            .await
                            .expect("success");
                    }

                    elgato_streamdeck::StreamDeckInput::EncoderTwist(encoder) => {
                        debug!("Encoder {:?} twisted", encoder);

                        for (index, direction) in encoder.into_iter().enumerate() {
                            let button_id = index + 8 + 4;
                            if direction == 0 {
                                continue;
                            }
                            let count = direction.abs();
                            let direction = if direction < 0 { 0 } else { 1 };
                            let msg = format!("KEY-ROTATE DEVICEID={device_id} KEY={button_id} DIRECTION={direction}\n");
                            debug!("Sending: {}", msg);
                            let msg = msg.as_bytes();
                            let mut writer = writer.lock().await;
                            for _ in 0..count {
                                writer.write_all(msg).await.expect("write failed");
                            }
                            writer.flush().await.expect("flush");
                        }
                    }
                    elgato_streamdeck::StreamDeckInput::TouchScreenPress(x, y) => {
                        debug!("Touchscreen pressed at {},{}", x, y);
                    }
                    elgato_streamdeck::StreamDeckInput::TouchScreenLongPress(x, y) => {
                        debug!("Touchscreen long pressed at {},{}", x, y);
                    }
                    elgato_streamdeck::StreamDeckInput::TouchScreenSwipe(from, to) => {
                        debug!(
                            "Touchscreen swiped from {},{} to {},{}",
                            from.0, from.1, to.0, to.1
                        );
                    }
                }
            }
        });
    }

    // tell it there is a device
    {
        let mut writer = writer.lock().await;
        let kind = device.kind();
        writer
            .write_all(
                format!(
                    "ADD-DEVICE {}\n",
                    rust_satellite::DeviceMsg {
                        device_id: device_id.clone(),
                        product_name: "Satellite StreamDeck: plus".to_string(),
                        keys_total: kind.key_count(),
                        keys_per_row: kind.column_count()
                    }
                    .device_msg()
                )
                .as_bytes(),
            )
            .await?;
    }

    let mut lines = BufReader::new(reader).lines();
    let lcd_width = 120;
    let lcd_height = 120;

    while let Some(line) = lines.next_line().await? {
        let command = rust_satellite::Command::parse(&line)
            .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

        match command {
            rust_satellite::Command::KeyPress(data) => {
                debug!("Received key press: {data}");
            }
            rust_satellite::Command::KeyRotate(data) => {
                debug!("Received key rotate: {data}");
            }
            rust_satellite::Command::Pong => {
                trace!("Received PONG");
            }
            rust_satellite::Command::Begin(versions) => {
                debug!("Beginning communication: {:?}", versions);
            }
            rust_satellite::Command::AddDevice(device) => {
                debug!("Adding device: {:?}", device);
            }
            rust_satellite::Command::KeyState(keystate) => {
                debug!("Received key state: {:?}", keystate);
                debug!("  bitmap size: {}", keystate.bitmap()?.len());
                let kind = device.kind();

                let in_button_range = (keystate.key < kind.key_count()).then(|| keystate.key);

                let in_lcd_button = if in_button_range.is_some() {
                    None
                } else {
                    kind.lcd_strip_size()
                        .map(|_| kind.key_count() - keystate.key)
                        .filter(|index| index < &kind.column_count())
                };

                match (in_button_range, in_lcd_button) {
                    (Some(key), _) => {
                        debug!("Writing image to button");
                        device
                            .set_button_image(
                                key,
                                image::DynamicImage::ImageRgb8(
                                    image::ImageBuffer::from_vec(
                                        lcd_width,
                                        lcd_height,
                                        keystate.bitmap()?,
                                    )
                                    .unwrap(),
                                ),
                            )
                            .await?;
                    }
                    (None, Some(lcd_key)) => {
                        debug!("Writing image to LCD panel");
                        const PANEL_WIDTH: u32 = 800;
                        const PANEL_HEIGHT: u32 = 100;
                        let image = image::DynamicImage::ImageRgb8(
                            image::ImageBuffer::from_vec(lcd_width, lcd_height, keystate.bitmap()?)
                                .unwrap(),
                        );
                        // resize image to the height
                        let image = image.resize(
                            image.width(),
                            PANEL_HEIGHT,
                            image::imageops::FilterType::Gaussian,
                        );
                        let button_x_offset =
                            (lcd_key as u32 - 8) * ((PANEL_WIDTH - lcd_width) / 3);
                        let rect = Arc::new(ImageRect::from_image(image).unwrap());
                        device
                            .write_lcd(button_x_offset as u16, 0 as u16, rect)
                            .await?;
                    }
                    _ => {
                        debug!("Key out of range {:?}", keystate);
                    }
                }
            }
            rust_satellite::Command::Brightness(brightness) => {
                debug!("Received brightness: {:?}", brightness);
                device.set_brightness(brightness.brightness).await?;
            }
            rust_satellite::Command::Unknown(command) => {
                debug!("Unknown command: {} with data {}", command, line.len());
            }
        }
    }

    Ok(())
}
