use tokio::io::{AsyncRead, AsyncWrite};
use traits::{
    async_trait,
    device::{DeviceCommands, SetBrightness, SetButtonImage, SetLCDImage},
    Result,
};

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
    async fn button_change(&mut self, change: traits::device::ButtonChange) -> Result<()> {
        send_companion_command(&mut self.writer, traits::device::Command::ButtonChange(change)).await
    }
    async fn encoder_twist(&mut self, twist: traits::device::EncoderTwist) -> Result<()> {
        send_companion_command(&mut self.writer, traits::device::Command::EncoderTwist(twist)).await
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
impl<W> traits::device::Controller for GatewayDeviceController<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn set_brightness(&mut self, brightness: SetBrightness) -> Result<()> {
        send_device_command(&mut self.writer, DeviceCommands::SetBrightness(brightness)).await
    }
    async fn set_button_image(&mut self, image: SetButtonImage) -> Result<()> {
        send_device_command(&mut self.writer, DeviceCommands::SetButtonImage(image)).await
    }
    async fn set_lcd_image(&mut self, image: SetLCDImage) -> Result<()> {
        send_device_command(&mut self.writer, DeviceCommands::SetLCDImage(image)).await
    }
}

async fn send_device_command<W>(
    satellite_write_stream: &mut W,
    command: DeviceCommands,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    Ok(bin_comm::stream_utils::write_struct(satellite_write_stream, &command).await?)
}

async fn send_companion_command<W>(
    stream: &mut W,
    command: traits::device::Command)
    -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    Ok(bin_comm::stream_utils::write_struct(stream, &command).await?)
}