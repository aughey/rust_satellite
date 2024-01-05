//! # Gateway Devices
//! These are structs that implement the traits defined in the `traits` crate
//! that forward commands to a device over a TCP connection.  The gateway
//! is intended to ship properly formatted bits down to a leaf device in a 
//! binary format specifically formatted for that device.  This eliminates
//! the need for the leaf device to do ascii parsing, image scaling, and image
//! conversion.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, ToSocketAddrs},
};
use tracing::trace;
use traits::{
    async_trait,
    device::{DeviceActions, SetBrightness, SetButtonImage, SetLCDImage},
    Result,
};

/// Create a connection to the gateway and return objects implementing
/// the companion sender and receiver traits.
pub async fn connect_to_gateway(
    addr: impl ToSocketAddrs,
) -> Result<(
    impl traits::companion::Sender,
    impl traits::companion::Receiver,
)> {
    let (companion_reader, companion_writer) =
        tokio::net::TcpStream::connect(addr).await?.into_split();

    let companion_receiver = GatewayCompanionReceiver::new(companion_reader);
    let companion_sender = GatewayCompanionSender::new(companion_writer);
    Ok((companion_sender, companion_receiver))
}

/// Create a set of devices objects from an already connected socket.
pub async fn device_from_socket(
    socket: TcpStream,
) -> Result<(impl traits::device::Sender, impl traits::device::Receiver)> {
    let (companion_reader, companion_writer) = socket.into_split();

    let sender = GatewayDeviceSender::new(companion_writer);
    let receiver = GatewayDeviceReceiver::new(companion_reader);
    Ok((sender, receiver))
}

/// GatewayCompanionReceiver implements the companion receiver trait.  The
/// The operations are received from the provided reader, deserialized,
/// and provided to the caller in the receive method.
pub struct GatewayCompanionReceiver<R> {
    reader: R,
}
impl<R> GatewayCompanionReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    /// Create a new GatewayCompanionReceiver from the provided reader.
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl<R> traits::companion::Receiver for GatewayCompanionReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    /// Receive a command from the reader and return it to the caller.
    async fn receive(&mut self) -> Result<DeviceActions> {
        let command: DeviceActions = bin_comm::stream_utils::read_struct(&mut self.reader).await?;
        trace!("GatewayCompanionReceiver::Receiver: {:?}", command);
        Ok(command)
    }
}

/// GatewayDeviceReceiver implements the device receiver trait.  The
/// operations are received from the provided reader, deserialized,
/// and provided to the caller in the receive method.
pub struct GatewayDeviceReceiver<R> {
    reader: R,
}
impl<R> GatewayDeviceReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    /// Create a new GatewayDeviceReceiver from the provided reader.
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}

#[async_trait]
impl<R> traits::device::Receiver for GatewayDeviceReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    /// read the command from the provided reader and return it to the caller.
    async fn receive(&mut self) -> Result<leaf_comm::Command> {
        let command: leaf_comm::Command =
            bin_comm::stream_utils::read_struct(&mut self.reader).await?;
        trace!("GatewayDeviceReceiver::Receiver: {:?}", command);
        Ok(command)
    }
}

/// GatewayCompanionSender implements the companion sender trait.  Methods
/// called on the companion sender are serialized and sent to the provided
/// writer.
pub struct GatewayCompanionSender<W> {
    writer: W,
}
impl<W> GatewayCompanionSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    /// Create a new GatewayCompanionSender from the provided writer.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl<W> traits::companion::Sender for GatewayCompanionSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn config(&mut self, config: leaf_comm::RemoteConfig) -> Result<()> {
        GatewayCompanionSender::send_companion_command(
            &mut self.writer,
            leaf_comm::Command::Config(config),
        )
        .await
    }
    async fn button_change(&mut self, change: leaf_comm::ButtonChange) -> Result<()> {
        GatewayCompanionSender::send_companion_command(
            &mut self.writer,
            leaf_comm::Command::ButtonChange(change),
        )
        .await
    }
    async fn encoder_twist(&mut self, twist: leaf_comm::EncoderTwist) -> Result<()> {
        GatewayCompanionSender::send_companion_command(
            &mut self.writer,
            leaf_comm::Command::EncoderTwist(twist),
        )
        .await
    }
}

impl<W> GatewayCompanionSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn send_companion_command(stream: &mut W, command: leaf_comm::Command) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        trace!(
            "GatewayDeviceSender::send_companion_command: {:?}",
            command
        );
        Ok(bin_comm::stream_utils::write_struct(stream, &command).await?)
    }
}

/// GatewayDeviceSender implements the device sender trait.  Methods
/// called on the device sender are serialized and sent to the provided
/// writer.
pub struct GatewayDeviceSender<W> {
    writer: W,
}
impl<W> GatewayDeviceSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    /// Create a new GatewayDeviceSender from the provided writer.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl<W> traits::device::Sender for GatewayDeviceSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn set_brightness(&mut self, brightness: SetBrightness) -> Result<()> {
        GatewayDeviceSender::send_device_command(
            &mut self.writer,
            DeviceActions::SetBrightness(brightness),
        )
        .await
    }
    async fn set_button_image(&mut self, image: SetButtonImage) -> Result<()> {
        GatewayDeviceSender::send_device_command(
            &mut self.writer,
            DeviceActions::SetButtonImage(image),
        )
        .await
    }
    async fn set_lcd_image(&mut self, image: SetLCDImage) -> Result<()> {
        GatewayDeviceSender::send_device_command(
            &mut self.writer,
            DeviceActions::SetLCDImage(image),
        )
        .await
    }
}

impl<W> GatewayDeviceSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn send_device_command(
        satellite_write_stream: &mut W,
        command: DeviceActions,
    ) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        trace!(
            "GatewayDeviceSender::send_device_command: {:?}",
            command
        );
        Ok(bin_comm::stream_utils::write_struct(satellite_write_stream, &command).await?)
    }
}
