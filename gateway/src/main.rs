use std::{ops::DerefMut, sync::Arc};

use bin_comm::{DeviceCommands, RemoteCommands, SetButtonImage, SetLCDImage};
use clap::Parser;
use elgato_streamdeck::info::Kind;
use gateway::{
    satellite::satellite_read_command, stream_utils::write_struct, ButtonState, Cli, Result,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use tracing::{debug, info, trace};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    // Create an async tcp listener
    let listener = tokio::net::TcpListener::bind((args.listen_address, args.listen_port)).await?;
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
        info!("Connected to companion app: {:?}", stream.peer_addr());

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
    info!(
        "Gateway for streamdeck {:?} with image format {:?}",
        kind, image_format
    );

    // This has two independent tasks:
    //  One to read from the satellite and write to the companion app
    //  One to read from the companion app and write to the satellite
    //  Both tasks are independent and can be run in parallel
    let companion_write_stream = Arc::new(Mutex::new(companion_write_stream));
    let satellite_to_companion = {
        let companion_write_stream = companion_write_stream.clone();
        tokio::spawn(async move {
            satellite_to_companion(
                satellite_read_stream,
                companion_write_stream,
                kind,
                config.device_id.to_string(),
            )
            .await
        })
    };

    let companion_to_satellite = tokio::spawn(async move {
        companion_to_satellite(
            companion_read_stream,
            satellite_write_stream,
            kind,
            image_format,
        )
        .await
    });

    let companion_ping = tokio::spawn(async move { companion_ping(companion_write_stream).await });

    // Wait for all tasks to complete.  If there is an error, abort early.
    let res = tokio::try_join!(
        satellite_to_companion,
        companion_to_satellite,
        companion_ping
    );

    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

async fn companion_ping<W>(companion_write_stream: Arc<Mutex<W>>) -> Result<()>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let mut companion_write_stream = companion_write_stream.lock().await;
        companion_write_stream.write_all(b"PING\n").await.unwrap();
        companion_write_stream.flush().await.unwrap();
    }
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

    info!("Waiting for commands from companion...");

    while let Some(line) = lines.next_line().await? {
        trace!("Received line: {}", line);
        let command = gateway::Command::parse(&line)
            .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

        match command {
            gateway::Command::Pong => {}
            _ => debug!("Received companion command: {:?}", command),
        }

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
                            image::imageops::FilterType::Nearest,
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
    companion_write_stream: Arc<Mutex<W>>,
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

    // add our device to companion
    {
        let mut companion_write_stream = companion_write_stream.lock().await;
        companion_write_stream
            .write_all(
                format!(
                    "ADD-DEVICE {}\n",
                    gateway::DeviceMsg {
                        device_id: device_id.clone(),
                        product_name: format!("RustSatellite StreamDeck: {}", device_name(&kind)),
                        keys_total: kind.key_count(),
                        keys_per_row: kind.column_count()
                    }
                    .device_msg()
                )
                .as_bytes(),
            )
            .await?;
        companion_write_stream.flush().await?;
    }

    info!("Waiting for commands from satellite...");

    loop {
        let command = satellite_read_command(&mut satellite_read_stream).await?;
        debug!("Received command: {:?}", command);
        match command {
            RemoteCommands::Config(_) => {
                anyhow::bail!("Shouldn't get command message after startup")
            }
            RemoteCommands::ButtonChange(buttons) => {
                let mut writer = companion_write_stream.lock().await;
                keystate
                    .update_all(buttons, writer.deref_mut(), &device_id)
                    .await?;
            }
            RemoteCommands::EncoderChange(encoders) => {
                let mut writer = companion_write_stream.lock().await;
                keystate
                    .update_all(
                        // offset is the keys plus one row of LCD "buttons"
                        encoders.to_buttons(encoder_offset as usize),
                        writer.deref_mut(),
                        &device_id,
                    )
                    .await?;
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
                let mut writer = companion_write_stream.lock().await;
                for _ in 0..count {
                    writer.write_all(msg).await?;
                }
                writer.flush().await?;
            }
        }
    }
}

fn device_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Original => "original",
        Kind::OriginalV2 => "originalv2",
        Kind::Mini => "mini",
        Kind::Xl => "xl",
        Kind::XlV2 => "xlv2",
        Kind::Mk2 => "mk2",
        Kind::MiniMk2 => "minimk2",
        Kind::Pedal => "pedal",
        Kind::Plus => "plus",
    }
}
