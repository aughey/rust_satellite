use std::sync::Arc;

use clap::Parser;
use elgato_streamdeck::info::Kind;
use gateway::{Cli, Result};
use gateway_devices::{GatewayDeviceReceiver, GatewayDeviceController};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    sync::Mutex,
};
use tracing::{debug, info};
use traits::device::Receiver;

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
    satellite_read_stream: R,
    satellite_write_stream: W,
    companion_read_stream: R,
    companion_write_stream: W,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // Handshaking.  Wait for a message from the satellite telling us what it is
    let mut device_receiver = GatewayDeviceReceiver::new(satellite_read_stream);
    let device_sender = GatewayDeviceController::new(satellite_write_stream);

    // Read the first message from the satellite to get the config
    let config = device_receiver.receive().await?;
    debug!("Received config: {:?}", config);
    let config = match config {
        traits::device::Command::Config(config) => config,
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

    let companion_receiver = companion::receiver::Receiver::new(companion_read_stream, kind);
    let companion_sender =
        companion::sender::Sender::new(companion_write_stream, config.device_id.to_string());

    pumps::message_pump(device_sender, device_receiver, companion_sender, companion_receiver).await
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
