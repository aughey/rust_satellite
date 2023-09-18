use bin_comm::{DeviceCommands, RemoteCommands, SetButtonImage, SetLCDImage};
use clap::Parser;
use elgato_streamdeck::info::Kind;
use gateway::{
    satellite::satellite_read_command, stream_utils::write_struct, ButtonState, Cli, Result,
};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tracing::{debug, info, trace};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    // Create an async tcp listener
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", args.listen_port)).await?;
    info!("Listening on port {}", args.listen_port);

    loop {
        // Wait for a connection
        let (stream, _) = listener.accept().await?;
        info!(
            "Satellite Connection established from: {:?}",
            stream.peer_addr()
        );

        // Connect to the host
        info!("Connecting to companion app: {}:{}", args.host, args.port);
        let companion_stream =
            tokio::net::TcpStream::connect((args.host.as_str(), args.port)).await?;
        info!("Connected to {:?}", stream.peer_addr());

        // Spawn off a task to handle the connection
        tokio::spawn(async move {
            // split the stream into a reader and writer
            let (reader, writer) = tokio::io::split(stream);
            let (companion_reader, companion_writer) = tokio::io::split(companion_stream);

            // convert the async reader into a

            if let Err(e) =
                handle_connection(reader, writer, companion_reader, companion_writer).await
            {
                debug!("Error handling connection: {:?}", e);
            }
        });
    }
}

async fn handle_connection<R, W>(
    mut satellite_read_stream: R,
    satellite_write_stream: W,
    companion_read_stream: R,
    companion_write_stream: W,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // Handshaking.  Wait for a message from the satellite telling us what it is
    let config = satellite_read_command(&mut satellite_read_stream).await?;
    debug!("Received config: {:?}", config);
    let config = match config {
        RemoteCommands::Config(config) => config,
        #[allow(unreachable_patterns)]
        _ => {
            anyhow::bail!("Expected config, got {:?}", config);
        }
    };

    // Get our kind from the config
    let kind =
        Kind::from_pid(config.pid).ok_or_else(|| anyhow::anyhow!("Unknown pid {}", config.pid))?;

    let image_format = kind.key_image_format();
    info!("Found kind {:?} with image format {:?}", kind, image_format);

    // This has two independent tasks:
    //  One to read from the satellite and write to the companion app
    //  One to read from the companion app and write to the satellite
    //  Both tasks are independent and can be run in parallel
    let satellite_to_companion = tokio::spawn(async move {
        satellite_to_companion(
            satellite_read_stream,
            companion_write_stream,
            kind,
            config.device_id.to_string(),
        )
    });

    let companion_to_satellite = tokio::spawn(async move {
        companion_to_satellite(
            companion_read_stream,
            satellite_write_stream,
            kind,
            image_format,
        )
    });

    // Wait for all tasks to complete.  If there is an error, abort early.
    let res = tokio::try_join!(satellite_to_companion, companion_to_satellite);

    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }

    // // Print out some info from the device
    // info!(
    //     "Connected to '{}' with version '{}'",
    //     device.serial_number().await?,
    //     device.firmware_version().await?
    // );

    // device.reset().await?;

    // // Set device brightness
    // device.set_brightness(35).await?;

    // // open up an async tcp connection to the host and port
    // // and send a message
    // let stream = tokio::net::TcpStream::connect((args.host.as_str(), args.port)).await?;
    // info!("Connected to {}:{}", args.host, args.port);

    // // turn stream into a lines stream
    // let stream = tokio::io::BufStream::new(stream);
    // let (reader, writer) = tokio::io::split(stream);

    // let writer = Arc::new(Mutex::new(writer));

    // // ping task
    // {
    //     let writer = writer.clone();
    //     tokio::spawn(async move {
    //         loop {
    //             tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    //             let mut writer = writer.lock().await;
    //             writer.write_all(b"PING\n").await.unwrap();
    //             writer.flush().await.unwrap();
    //         }
    //     });
    // }

    // let device_id = device.serial_number().await?;

    // {
    //     // button reader task
    //     let writer = writer.clone();
    //     let device = device.clone();
    //     let device_id = device_id.clone();
    //     tokio::spawn(async move {
    //         let mut keystate = ButtonState::new(16);

    //         loop {
    //             let buttons = device
    //                 .read_input(60.0)
    //                 .await
    //                 .expect("Error reading input from device");
    //             match buttons {
    //                 elgato_streamdeck::StreamDeckInput::NoData => {}
    //                 elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
    //                     debug!("Button {:?} pressed", buttons);
    //                     let mut writer = writer.lock().await;
    //                     keystate
    //                         .update_all(
    //                             buttons.into_iter().take(8).enumerate(),
    //                             writer.deref_mut(),
    //                             &device_id,
    //                         )
    //                         .await
    //                         .expect("success");
    //                 }
    //                 elgato_streamdeck::StreamDeckInput::EncoderStateChange(encoder) => {
    //                     debug!("Encoder {:?} changed", encoder);
    //                     let mut writer = writer.lock().await;
    //                     let states = encoder
    //                         .into_iter()
    //                         .take(4)
    //                         .enumerate()
    //                         .map(|(index, state)| (index + 8 + 4, state))
    //                         .collect::<Vec<_>>();
    //                     keystate
    //                         .update_all(states, writer.deref_mut(), &device_id)
    //                         .await
    //                         .expect("success");
    //                 }

    //                 elgato_streamdeck::StreamDeckInput::EncoderTwist(encoder) => {
    //                     debug!("Encoder {:?} twisted", encoder);

    //                     for (index, direction) in encoder.into_iter().enumerate() {
    //                         let button_id = index + 8 + 4;
    //                         if direction == 0 {
    //                             continue;
    //                         }
    //                         let count = direction.abs();
    //                         let direction = if direction < 0 { 0 } else { 1 };
    //                         let msg = format!("KEY-ROTATE DEVICEID={device_id} KEY={button_id} DIRECTION={direction}\n");
    //                         debug!("Sending: {}", msg);
    //                         let msg = msg.as_bytes();
    //                         let mut writer = writer.lock().await;
    //                         for _ in 0..count {
    //                             writer.write_all(msg).await.expect("write failed");
    //                         }
    //                         writer.flush().await.expect("flush");
    //                     }
    //                 }
    //                 elgato_streamdeck::StreamDeckInput::TouchScreenPress(x, y) => {
    //                     debug!("Touchscreen pressed at {},{}", x, y);
    //                 }
    //                 elgato_streamdeck::StreamDeckInput::TouchScreenLongPress(x, y) => {
    //                     debug!("Touchscreen long pressed at {},{}", x, y);
    //                 }
    //                 elgato_streamdeck::StreamDeckInput::TouchScreenSwipe(from, to) => {
    //                     debug!(
    //                         "Touchscreen swiped from {},{} to {},{}",
    //                         from.0, from.1, to.0, to.1
    //                     );
    //                 }
    //             }
    //         }
    //     });
    // }

    // // tell it there is a device
    // {
    //     let mut writer = writer.lock().await;
    //     let kind = device.kind();
    //     writer
    //         .write_all(
    //             format!(
    //                 "ADD-DEVICE {}\n",
    //                 gateway::DeviceMsg {
    //                     device_id: device_id.clone(),
    //                     product_name: format!("RustSatellite StreamDeck: {}", device_name(&kind)),
    //                     keys_total: kind.key_count(),
    //                     keys_per_row: kind.column_count()
    //                 }
    //                 .device_msg()
    //             )
    //             .as_bytes(),
    //         )
    //         .await?;
    // }

    // let mut lines = BufReader::new(reader).lines();
    // let (lcd_width, lcd_height) = kind.lcd_strip_size().unwrap_or((0, 0));
    // let (lcd_width, lcd_height) = (lcd_width as u32, lcd_height as u32);

    // while let Some(line) = lines.next_line().await? {
    //     trace!("Received line: {}", line);
    //     let command = gateway::Command::parse(&line)
    //         .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

    //     match command {
    //         gateway::Command::KeyPress(data) => {
    //             debug!("Received key press: {data}");
    //         }
    //         gateway::Command::KeyRotate(data) => {
    //             debug!("Received key rotate: {data}");
    //         }
    //         gateway::Command::Pong => {
    //             //trace!("Received PONG");
    //         }
    //         gateway::Command::Begin(versions) => {
    //             debug!("Beginning communication: {:?}", versions);
    //         }
    //         gateway::Command::AddDevice(device) => {
    //             debug!("Adding device: {:?}", device);
    //         }
    //         gateway::Command::KeyState(keystate) => {
    //             debug!("Received key state: {:?}", keystate);
    //             debug!("  bitmap size: {}", keystate.bitmap()?.len());
    //             let kind = device.kind();

    //             let in_button_range = (keystate.key < kind.key_count()).then(|| keystate.key);

    //             let in_lcd_button = if in_button_range.is_some() {
    //                 None
    //             } else {
    //                 kind.lcd_strip_size()
    //                     .map(|_| kind.key_count() - keystate.key)
    //                     .filter(|index| index < &kind.column_count())
    //             };

    //             match (in_button_range, in_lcd_button) {
    //                 (Some(key), _) => {
    //                     debug!("Writing image to button");
    //                     let image = image::DynamicImage::ImageRgb8(
    //                         image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?)
    //                             .ok_or_else(|| anyhow::anyhow!("Couldn't extract image buffer"))?,
    //                     );
    //                     let image = image.resize_exact(
    //                         image_format.size.0 as u32,
    //                         image_format.size.1 as u32,
    //                         image::imageops::FilterType::Gaussian,
    //                     );

    //                     device.set_button_image(key, image).await?;
    //                 }
    //                 (None, Some(lcd_key)) => {
    //                     debug!("Writing image to LCD panel");
    //                     let image = image::DynamicImage::ImageRgb8(
    //                         image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?).unwrap(),
    //                     );
    //                     // resize image to the height
    //                     let image = image.resize(
    //                         image.width(),
    //                         lcd_height,
    //                         image::imageops::FilterType::Gaussian,
    //                     );
    //                     let button_x_offset =
    //                         (lcd_key as u32 - 8) * ((lcd_width - image.width()) / 3);
    //                     let rect = Arc::new(ImageRect::from_image(image).unwrap());
    //                     device
    //                         .write_lcd(button_x_offset as u16, 0 as u16, rect)
    //                         .await?;
    //                 }
    //                 _ => {
    //                     debug!("Key out of range {:?}", keystate);
    //                 }
    //             }
    //         }
    //         gateway::Command::Brightness(brightness) => {
    //             debug!("Received brightness: {:?}", brightness);
    //             device.set_brightness(brightness.brightness).await?;
    //         }
    //         gateway::Command::Unknown(command) => {
    //             debug!("Unknown command: {} with data {}", command, line.len());
    //         }
    //     }
    // }
}

async fn companion_to_satellite<R, W>(
    companion_read_stream: R,
    mut satellite_write_stream: W,
    kind: Kind,
    image_format: elgato_streamdeck::info::ImageFormat,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(companion_read_stream).lines();
    while let Some(line) = lines.next_line().await? {
        trace!("Received line: {}", line);
        let command = gateway::Command::parse(&line)
            .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

        debug!("Received command: {:?}", command);

        let (lcd_width, lcd_height) = kind.lcd_strip_size().unwrap_or((0, 0));
        let (lcd_width, lcd_height) = (lcd_width as u32, lcd_height as u32);

        match command {
            gateway::Command::Pong => {}
            gateway::Command::KeyPress(_) => {}
            gateway::Command::KeyRotate(_) => {}
            gateway::Command::Begin(_) => {}
            gateway::Command::AddDevice(_) => {}
            gateway::Command::KeyState(keystate) => {
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
                        let image = image::DynamicImage::ImageRgb8(
                            image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?)
                                .ok_or_else(|| anyhow::anyhow!("Couldn't extract image buffer"))?,
                        );
                        let image = image.resize_exact(
                            image_format.size.0 as u32,
                            image_format.size.1 as u32,
                            image::imageops::FilterType::Gaussian,
                        );
                        // Convert the image into the EXACT format needed for the remote device
                        let image = elgato_streamdeck::images::convert_image(kind, image)?;
                        // Send this to the satellite
                        let image = DeviceCommands::SetButtonImage(SetButtonImage { key, image });
                        write_struct(&mut satellite_write_stream, &image).await?;
                    }
                    (None, Some(lcd_key)) => {
                        debug!("Writing image to LCD panel");
                        let image = image::DynamicImage::ImageRgb8(
                            image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?).unwrap(),
                        );
                        // resize image to the height
                        let image = image.resize(
                            lcd_height,
                            lcd_height,
                            image::imageops::FilterType::Gaussian,
                        );
                        let button_x_offset =
                            (lcd_key as u32 - 8) * ((lcd_width - image.width()) / 3);

                        // Convert the image into the EXACT format needed for the remote device
                        let image = elgato_streamdeck::images::convert_image(kind, image)?;
                        let image = DeviceCommands::SetLCDImage(SetLCDImage {
                            x_offset: button_x_offset as u16,
                            x_size: lcd_height as u16,
                            y_size: lcd_height as u16,
                            image,
                        });
                        // Send this to the satellite
                        send_satellite_command(&mut satellite_write_stream, &image).await?;
                    }
                    _ => {
                        debug!("Key out of range {:?}", keystate);
                    }
                }
            }
            gateway::Command::Brightness(brightness) => {
                send_satellite_command(
                    &mut satellite_write_stream,
                    &DeviceCommands::SetBrightness(bin_comm::SetBrightness {
                        brightness: brightness.brightness,
                    }),
                )
                .await?
            }
            gateway::Command::Unknown(_) => todo!(),
        }
    }

    Ok(())
}

async fn send_satellite_command<W>(
    satellite_write_stream: &mut W,
    command: &DeviceCommands,
) -> std::io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    write_struct(satellite_write_stream, command).await
}

async fn satellite_to_companion<R, W>(
    mut satellite_read_stream: R,
    mut companion_write_stream: W,
    kind: Kind,
    device_id: String,
) -> Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut keystate = ButtonState::new(
        (kind.key_count() + kind.encoder_count() * kind.column_count() + kind.encoder_count())
            as usize,
    );
    let encoder_offset = kind.key_count() + kind.column_count();

    loop {
        let command = satellite_read_command(&mut satellite_read_stream).await?;
        debug!("Received command: {:?}", command);
        match command {
            RemoteCommands::Config(_) => {
                anyhow::bail!("Shouldn't get command message after startup")
            }
            RemoteCommands::ButtonChange(buttons) => {
                keystate
                    .update_all(buttons, &mut companion_write_stream, &device_id)
                    .await?
            }
            RemoteCommands::EncoderChange(encoders) => {
                keystate
                    .update_all(
                        // offset is the keys plus one row of LCD "buttons"
                        encoders.to_buttons(encoder_offset as usize),
                        &mut companion_write_stream,
                        &device_id,
                    )
                    .await?
            }
            RemoteCommands::EncoderTwist(encoder) => {
                let count = encoder.value.abs();
                let direction = if encoder.value < 0 { 0 } else { 1 };
                let button_id = encoder.index + encoder_offset;
                let msg = format!(
                    "KEY-ROTATE DEVICEID={device_id} KEY={button_id} DIRECTION={direction}\n",
                );
                debug!("Sending: {}", msg);
                let msg = msg.as_bytes();
                for _ in 0..count {
                    companion_write_stream
                        .write_all(msg)
                        .await
                        .expect("write failed");
                }
                companion_write_stream.flush().await.expect("flush");
            }
        }
    }
}
