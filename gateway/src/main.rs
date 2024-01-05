use clap::Parser;
use elgato_streamdeck::info::Kind;
use gateway::{Cli, Result};
use tracing::{debug, info};
use traits::device::{Receiver, RemoteConfig};
use traits::anyhow;

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

        let (device_sender, mut device_receiver) =
            gateway_devices::device_from_socket(stream).await?;

        // Read the first message from the satellite to get the config
        let config_msg = device_receiver.receive().await?;
        let config_msg = match config_msg {
            traits::device::Command::Config(c) => RemoteConfig {
                pid: c.pid.try_into()?,
                device_id: c.device_id,
            },
            _ => anyhow::bail!("Expected config msg to be first")
        };
        debug!("Received config: {:?}", config_msg);

        info!(
            "Connecting to companion app: {}:{}",
            args.companion_host.as_str(),
            args.companion_port
        );
        let (companion_reader, companion_writer) =
            tokio::net::TcpStream::connect((args.companion_host.as_str(), args.companion_port))
                .await?
                .into_split();

        let kind = Kind::from_pid(config_msg.pid)
            .ok_or_else(|| anyhow::anyhow!("Unknown pid {}", config_msg.pid))?;

        let companion_receiver = companion::receiver::Receiver::new(companion_reader, kind);
        let companion_sender = companion::sender::Sender::new(companion_writer, config_msg).await?;

        // Spawn off a task to handle the connection
        tokio::spawn(async move {
            let res = pumps::message_pump(
                device_sender,
                device_receiver,
                companion_sender,
                companion_receiver,
            )
            .await;
            info!("Connection closed: {:?}", res);
        });
    }
}
