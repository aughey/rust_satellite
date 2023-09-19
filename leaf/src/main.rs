use anyhow::Result;
use clap::Parser;
use gateway_devices::{GatewayCompanionReceiver, GatewayCompanionSender};
use leaf::Cli;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    // Create instance of HidApi
    let hid = elgato_streamdeck::new_hidapi().unwrap();

    // List devices and unsafely take first one
    let (kind, serial) = elgato_streamdeck::list_devices(&hid).remove(0);
    let image_format = kind.key_image_format();
    info!("Found kind {:?} with image format {:?}", kind, image_format);

    // Connect to the device
    let device = elgato_streamdeck::asynchronous::AsyncStreamDeck::connect(&hid, kind, &serial)?;

    // Print out some info from the device
    info!(
        "Connected to '{}' with version '{}'",
        device.serial_number().await?,
        device.firmware_version().await?
    );

    device.reset().await?;

    // Set device brightness
    device.set_brightness(35).await?;

    let device_sender = streamdeck::StreamDeck::new(device.clone());
    let device_receiver = device_sender.clone();

    // open up an async tcp connection to the host and port
    // and send a message
    let gateway_stream = tokio::net::TcpStream::connect((args.host.as_str(), args.port)).await?;
    info!("Connected to {}:{}", args.host, args.port);

    let (gateway_reader, gateway_writer) = gateway_stream.into_split();
    let leaf_sender = GatewayCompanionSender::new(gateway_writer);
    let leaf_receiver = GatewayCompanionReceiver::new(gateway_reader);


    pumps::message_pump(device_sender, device_receiver, leaf_sender, leaf_receiver).await?;

    Ok(())
}
