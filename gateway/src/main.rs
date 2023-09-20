use clap::Parser;
use elgato_streamdeck::info::Kind;
use gateway::{Cli, Result};
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

        let (device_sender,mut device_receiver) =
            gateway_devices::connect_device_to_socket(stream).await?;

        // Read the first message from the satellite to get the config
        let config_msg = device_receiver.receive().await?.as_config()?;
        debug!("Received config: {:?}",config_msg );
       

        info!("Connecting to companion app: {}:{}", args.host.as_str(), args.port);
        let (companion_reader, companion_writer) =
            tokio::net::TcpStream::connect((args.host.as_str(), args.port))
                .await?
                .into_split();
        
        let kind = Kind::from_pid(config_msg.pid).ok_or_else(|| anyhow::anyhow!("Unknown pid {}", config_msg.pid))?;

        let companion_receiver = companion::receiver::Receiver::new(companion_reader, kind);
        let companion_sender = companion::sender::Sender::new(
            companion_writer,
            config_msg
        )
        .await?;

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

// async fn handle_connection<R, W>(
//     satellite_read_stream: R,
//     satellite_write_stream: W,
//     companion_read_stream: R,
//     companion_write_stream: W,
// ) -> Result<()>
// where
//     R: AsyncRead + Unpin + Send + 'static,
//     W: AsyncWrite + Unpin + Send + 'static,
// {
//     // Handshaking.  Wait for a message from the satellite telling us what it is
//     let mut device_receiver = GatewayDeviceReceiver::new(satellite_read_stream);
//     let device_sender = GatewayDeviceController::new(satellite_write_stream);

//     // Read the first message from the satellite to get the config
//     let config = device_receiver.receive().await?;
//     debug!("Received config: {:?}", config);
//     let config = match config {
//         traits::device::Command::Config(config) => config,
//         #[allow(unreachable_patterns)]
//         _ => {
//             anyhow::bail!("Expected config, got {:?}", config);
//         }
//     };

//     // Get our kind from the config
//     let kind =
//         Kind::from_pid(config.pid).ok_or_else(|| anyhow::anyhow!("Unknown pid {}", config.pid))?;

//     let image_format = kind.key_image_format();
//     info!(
//         "Gateway for streamdeck {:?} with image format {:?}",
//         kind, image_format
//     );

//     let companion_receiver = companion::receiver::Receiver::new(companion_read_stream, kind);
//     let companion_sender = companion::sender::Sender::new(
//         companion_write_stream,
//         config.device_id.to_string(),
//         "zzzz",
//         kind.key_count().into(),
//         kind.column_count().into(),
//         kind.key_image_format().size.0.try_into()?,
//     )
//     .await?;

//     pumps::message_pump(
//         device_sender,
//         device_receiver,
//         companion_sender,
//         companion_receiver,
//     )
//     .await
// }
