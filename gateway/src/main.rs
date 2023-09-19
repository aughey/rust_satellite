use std::{ops::DerefMut, sync::Arc};

use bin_comm::RemoteCommands;
use clap::Parser;
use elgato_streamdeck::info::Kind;
use gateway::{
    satellite::{self, satellite_read_command},
    Cli, Result
};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    sync::Mutex,
};
use tracing::{debug, info};

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

    let satellite_device = satellite::SatelliteDevice::new(satellite_write_stream);

    let companion_to_satellite = tokio::spawn(async move {
        companion_to_device::companion_to_device(companion_read_stream, satellite_device, kind, image_format).await
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

pub struct ButtonState {
    buttons: Vec<bool>,
}
impl ButtonState {
    pub fn new(size: usize) -> Self {
        let mut v = Vec::with_capacity(size);
        v.resize_with(size, || false);
        Self { buttons: v }
    }
    pub async fn update_all(
        &mut self,
        states: impl IntoIterator<Item = (usize, bool)>,
        writer: &mut (impl AsyncWrite + Unpin),
        device_id: &str,
    ) -> Result<()> {
        for (index, state) in states {
            self.update_button(index, state, writer, device_id).await?;
        }
        writer.flush().await?;
        Ok(())
    }
    pub async fn update_button(
        &mut self,
        index: usize,
        pressed: bool,
        writer: &mut (impl AsyncWrite + Unpin),
        device_id: &str,
    ) -> Result<()> {
        if self.buttons[index] == pressed {
            Ok(())
        } else {
            self.buttons[index] = pressed;

            let pressed = if pressed { 1 } else { 0 };

            let msg = format!("KEY-PRESS DEVICEID={device_id} KEY={index} PRESSED={pressed}\n");
            debug!("Sending: {}", msg);
            writer.write_all(msg.as_bytes()).await?;
            Ok(())
        }
    }
}


#[derive(Debug, PartialEq, Eq)]
pub struct DeviceMsg {
    pub device_id: String,
    pub product_name: String,
    pub keys_total: u8,
    pub keys_per_row: u8,
}
impl DeviceMsg {
    pub fn device_msg(&self) -> String {
        format!("DEVICEID={} PRODUCT_NAME=\"{}\" KEYS_TOTAL={}, KEYS_PER_ROW={} BITMAPS=120 COLORS=0 TEXT=0",
            self.device_id, self.product_name, self.keys_total, self.keys_per_row)
    }
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
                    DeviceMsg {
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

    info!("Processing commands from satellite.");

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
