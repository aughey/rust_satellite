use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, ToSocketAddrs},
};
use tracing::trace;
use traits::{
    async_trait,
    device::{DeviceCommands, SetBrightness, SetButtonImage, SetLCDImage},
    Result,
};

pub async fn connect(
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

pub async fn connect_device_to_socket(
    socket: TcpStream,
) -> Result<(impl traits::device::Sender, impl traits::device::Receiver)> {
    let (companion_reader, companion_writer) = socket.into_split();

    let sender = GatewayDeviceController::new(companion_writer);
    let receiver = GatewayDeviceReceiver::new(companion_reader);
    Ok((sender, receiver))
}

pub struct GatewayCompanionReceiver<R> {
    reader: R,
}
impl<R> GatewayCompanionReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}
#[async_trait]
impl<R> traits::companion::Receiver for GatewayCompanionReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    async fn receive(&mut self) -> Result<DeviceCommands> {
        let command: DeviceCommands = bin_comm::stream_utils::read_struct(&mut self.reader).await?;
        trace!("GatewayCompanionReceiver::Receiver: {:?}", command);
        Ok(command)
    }
}

pub struct GatewayDeviceReceiver<R> {
    reader: R,
}
impl<R> GatewayDeviceReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    pub fn new(reader: R) -> Self {
        Self { reader }
    }
}
#[async_trait]
impl<R> traits::device::Receiver for GatewayDeviceReceiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    async fn receive(&mut self) -> Result<traits::device::Command> {
        let command: traits::device::Command =
            bin_comm::stream_utils::read_struct(&mut self.reader).await?;
        trace!("GatewayDeviceReceiver::Receiver: {:?}", command);
        Ok(command)
    }
}

pub struct GatewayCompanionSender<W> {
    writer: W,
}
impl<W> GatewayCompanionSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl<W> traits::companion::Sender for GatewayCompanionSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn config(&mut self, config: traits::device::RemoteConfig) -> Result<()> {
        GatewayCompanionSender::send_companion_command(
            &mut self.writer,
            traits::device::Command::Config(config),
        )
        .await
    }
    async fn button_change(&mut self, change: traits::device::ButtonChange) -> Result<()> {
        GatewayCompanionSender::send_companion_command(
            &mut self.writer,
            traits::device::Command::ButtonChange(change),
        )
        .await
    }
    async fn encoder_twist(&mut self, twist: traits::device::EncoderTwist) -> Result<()> {
        GatewayCompanionSender::send_companion_command(
            &mut self.writer,
            traits::device::Command::EncoderTwist(twist),
        )
        .await
    }
}
impl<W> GatewayCompanionSender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn send_companion_command(
        stream: &mut W,
        command: traits::device::Command,
    ) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        trace!("GatewayDeviceController::send_companion_command: {:?}", command);
        Ok(bin_comm::stream_utils::write_struct(stream, &command).await?)
    }
}

pub struct GatewayDeviceController<W> {
    writer: W,
}
impl<W> GatewayDeviceController<W>
where
    W: AsyncWrite + Unpin + Send,
{
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

#[async_trait]
impl<W> traits::device::Sender for GatewayDeviceController<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn set_brightness(&mut self, brightness: SetBrightness) -> Result<()> {
        GatewayDeviceController::send_device_command(&mut self.writer, DeviceCommands::SetBrightness(brightness)).await
    }
    async fn set_button_image(&mut self, image: SetButtonImage) -> Result<()> {
        GatewayDeviceController::send_device_command(&mut self.writer, DeviceCommands::SetButtonImage(image)).await
    }
    async fn set_lcd_image(&mut self, image: SetLCDImage) -> Result<()> {
        GatewayDeviceController::send_device_command(&mut self.writer, DeviceCommands::SetLCDImage(image)).await
    }
}
impl<W> GatewayDeviceController<W>
where
    W: AsyncWrite + Unpin + Send,
{

    async fn send_device_command(
        satellite_write_stream: &mut W,
        command: DeviceCommands,
    ) -> Result<()>
    where
        W: AsyncWrite + Unpin,
    {
        trace!("GatewayDeviceController::send_device_command: {:?}", command);
        Ok(bin_comm::stream_utils::write_struct(satellite_write_stream, &command).await?)
    }
}


