use std::ops::DerefMut;
use std::sync::Arc;

use crate::stream_utils::receive_length_prefix;
use crate::{Result, StreamDeckDevice};
use async_trait::async_trait;
use bin_comm::stream_utils::write_struct;
use bin_comm::{RemoteCommands, DeviceCommands, SetButtonImage};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;

pub async fn satellite_read_command(
    stream: &mut (impl AsyncRead + Unpin),
) -> Result<RemoteCommands> {
    let buf = receive_length_prefix(stream, Default::default()).await?;
    // bincode decode it
    Ok(bincode::deserialize(&buf)?)
}

pub struct SatelliteDevice<W> {
    writer: Arc<Mutex<W>>,
}
impl<W> SatelliteDevice<W> where W: AsyncWrite + Unpin + Send {
    pub fn new(writer: W) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}
impl<W> Clone for SatelliteDevice<W> {
    fn clone(&self) -> Self {
        Self {
            writer: self.writer.clone(),
        }
    }
}

#[async_trait]
impl<W> StreamDeckDevice for SatelliteDevice<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn set_brightness(&mut self, brightness: u8) -> Result<()> {
        let mut writer = self.writer.lock().await;
        send_satellite_command(
            writer.deref_mut(),
            &DeviceCommands::SetBrightness(bin_comm::SetBrightness {
                brightness: brightness,
            }),
        )
        .await?;
        Ok(())
    }
    async fn set_button_image(&mut self, button: u8, image: Vec<u8>) -> Result<()> {
        let image = DeviceCommands::SetButtonImage(SetButtonImage { button, image });
        let mut writer = self.writer.lock().await;
        send_satellite_command(writer.deref_mut(), &image).await?;
        Ok(())
    }
    async fn set_lcd_image(
        &mut self,
        x_offset: u16,
        x_size: u16,
        y_size: u16,
        image: Vec<u8>,
    ) -> Result<()> {
        let image = DeviceCommands::SetLCDImage(bin_comm::SetLCDImage {
            x_offset,
            x_size,
            y_size,
            image,
        });
        let mut writer = self.writer.lock().await;
        send_satellite_command(writer.deref_mut(), &image).await?;
        Ok(())
    }
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