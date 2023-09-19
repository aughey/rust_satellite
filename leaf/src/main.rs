

use anyhow::Result;
use clap::Parser;

use gateway_devices::{GatewayCompanionSender, GatewayCompanionReceiver};
use leaf::Cli;

use tracing::{info};

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

// async fn gateway_to_device(
//     mut gateway_reader: impl AsyncRead + Unpin + Send,
//     device: AsyncStreamDeck,
// ) -> Result<()> {
//     info!("Waiting for commands from gateway...");
//     loop {
//         let command =
//             bin_comm::stream_utils::read_struct::<bin_comm::DeviceCommands>(&mut gateway_reader)
//                 .await?;
//         match command {
//             bin_comm::DeviceCommands::SetButtonImage(image) => {
//                 device.write_image(image.button, &image.image).await?;
//             }
//             bin_comm::DeviceCommands::SetLCDImage(image) => {
//                 let lcd_image = Arc::new(ImageRect::from_device_image(
//                     image.x_size,
//                     image.y_size,
//                     image.image,
//                 ));

//                 device.write_lcd(image.x_offset, 0, lcd_image).await?;
//             }
//             bin_comm::DeviceCommands::SetBrightness(brightness) => {
//                 device.set_brightness(brightness.brightness).await?
//             }
//         }
//     }
// }

// async fn device_to_gateway(
//     mut gateway_writer: impl AsyncWrite + Unpin + Send,
//     device: AsyncStreamDeck,
// ) -> Result<()> {
//     write_gateway(
//         &mut gateway_writer,
//         DeviceSendCommands::Config(RemoteConfig {
//             device_id: device.serial_number().await?,
//             pid: device.kind().product_id(),
//         }),
//     )
//     .await?;

//     info!("Waiting for commands from USB device...");

//     loop {
//         let buttons = device
//             .read_input(60.0)
//             .await
//             .expect("Error reading input from device");
//         debug!("Got usb command {:?}", buttons);
//         match buttons {
//             elgato_streamdeck::StreamDeckInput::NoData => {}
//             elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
//                 write_gateway(
//                     &mut gateway_writer,
//                     DeviceSendCommands::ButtonChange(ButtonChange { buttons }),
//                 )
//                 .await?
//             }
//             elgato_streamdeck::StreamDeckInput::EncoderStateChange(encoders) => {
//                 write_gateway(
//                     &mut gateway_writer,
//                     DeviceSendCommands::EncoderChange(EncoderChange { encoders }),
//                 )
//                 .await?
//             }
//             elgato_streamdeck::StreamDeckInput::EncoderTwist(twist) => {
//                 for (index, value) in twist.into_iter().enumerate().filter(|(_i, v)| *v != 0) {
//                     write_gateway(
//                         &mut gateway_writer,
//                         DeviceSendCommands::EncoderTwist(EncoderTwist {
//                             index: index.try_into()?,
//                             value,
//                         }),
//                     )
//                     .await?;
//                 }
//             }
//             elgato_streamdeck::StreamDeckInput::TouchScreenPress(_, _) => todo!(),
//             elgato_streamdeck::StreamDeckInput::TouchScreenLongPress(_, _) => todo!(),
//             elgato_streamdeck::StreamDeckInput::TouchScreenSwipe(_, _) => todo!(),
//         }
//     }
// }

// async fn write_gateway(
//     gateway_writer: &mut (impl AsyncWrite + Unpin + Send),
//     command: bin_comm::DeviceSendCommands,
// ) -> Result<()> {
//     bin_comm::stream_utils::write_struct(gateway_writer, &command).await?;
//     Ok(())
// }
