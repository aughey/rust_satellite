use std::{sync::Arc, thread, time::Duration};

use clap::Parser;
use rust_satellite::{Cli, Result};

use elgato_streamdeck::{asynchronous, list_devices, new_hidapi, StreamDeck};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use tracing::{info, trace};

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
    println!(
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
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let mut writer = writer.lock().await;
                writer.write_all(b"PING\n").await.unwrap();
                writer.flush().await.unwrap();
            }
        });
    }

    const DEVICE_ID: &str = "JohnAughey";

    {
        // button reader task
        let writer = writer.clone();
        let device = device.clone();
        tokio::spawn(async move {
            let mut keystate  = [false; 16];
            loop {
                let buttons = device.read_input(60.0).await.expect("Error reading input from device");
                match buttons {
                    elgato_streamdeck::StreamDeckInput::NoData => {}
                    elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
                        println!("Button {:?} pressed", buttons);
                        let mut writer = writer.lock().await;
                        for (index, button) in buttons.into_iter().enumerate().take(8) {
                            if keystate[index] == button {
                                continue;
                            }
                            keystate[index] = button;
                            let pressed = if button { 1 } else { 0 };
                            let msg = format!("KEY-PRESS DEVICEID={DEVICE_ID} KEY={index} PRESSED={pressed}\n");
                            info!("Sending: {}", msg);
                            writer.write_all(msg.as_bytes()).await.expect("write failed");
                        }
                        writer.flush().await.expect("flush");
                    }
                    elgato_streamdeck::StreamDeckInput::EncoderStateChange(encoder) => {
                        println!("Encoder {:?} changed", encoder);
                    }

                    elgato_streamdeck::StreamDeckInput::EncoderTwist(encoder) => {
                        println!("Encoder {:?} twisted", encoder);
                    }
                    elgato_streamdeck::StreamDeckInput::TouchScreenPress(x, y) => {
                        println!("Touchscreen pressed at {},{}", x, y);
                    }
                    elgato_streamdeck::StreamDeckInput::TouchScreenLongPress(x, y) => {
                        println!("Touchscreen long pressed at {},{}", x, y);
                    }
                    elgato_streamdeck::StreamDeckInput::TouchScreenSwipe(from, to) => {
                        println!(
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
        writer
            .write_all(
                format!(
                    "ADD-DEVICE {}\n",
                    rust_satellite::DeviceMsg {
                        device_id: DEVICE_ID.to_string(),
                        product_name: "Satellite StreamDeck: plus".to_string(),
                        keys_total: 16,
                        keys_per_row: 4,
                    }
                    .device_msg()
                )
                .as_bytes(),
            )
            .await?;
    }

    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let command = rust_satellite::Command::parse(&line)
            .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

        match command {
            rust_satellite::Command::KeyPress(data) => {
                info!("Received key press: {data}");
            }
            rust_satellite::Command::Pong => {
                trace!("Received PONG");
            }
            rust_satellite::Command::Begin(versions) => {
                info!("Beginning communication: {:?}", versions);
            }
            rust_satellite::Command::AddDevice(device) => {
                info!("Adding device: {:?}", device);
            }
            rust_satellite::Command::KeyState(keystate) => {
                info!("Received key state: {:?}", keystate);
                info!("  bitmap size: {}", keystate.bitmap()?.len());
                match keystate.key {
                    0..=7 => {
                        info!("Writing image to button");
                        device
                            .set_button_image(
                                keystate.key as u8,
                                image::DynamicImage::ImageRgb8(
                                    image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?)
                                        .unwrap(),
                                ),
                            )
                            .await?;
                    }
                    id => {
                        // ignore for now...
                        info!("Ignoring button out of range: {}", id);
                    }
                }
            }
            rust_satellite::Command::Brightness(brightness) => {
                info!("Received brightness: {:?}", brightness);
                device.set_brightness(brightness.brightness).await?;
            }
            rust_satellite::Command::Unknown(command) => {
                info!("Unknown command: {} with data {}", command, line.len());
            }
        }
    }

    Ok(())
}
