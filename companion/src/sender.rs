use std::sync::Arc;

use leaf_comm::{RemoteConfig, ButtonChange, EncoderTwist};
use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::Mutex,
};
use tracing::debug;
use traits::anyhow;
use traits::async_trait;
use traits::Result;

pub struct Sender<W> {
    device_id: String,
    writer: Arc<Mutex<W>>,
    ping: tokio::task::JoinHandle<Result<()>>,
}
impl<W> Sender<W>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub async fn new(mut writer: W, config: RemoteConfig) -> Result<Self> {
        // Get our kind from the config
        let kind = elgato_streamdeck::info::Kind::from_pid(config.pid)
            .ok_or_else(|| anyhow::anyhow!("Unknown pid {}", config.pid))?;

        let image_format = kind.key_image_format();
        debug!(
            "Creating Companion sender for streamdeck {:?} with image format {:?}",
            kind, image_format
        );

        writer
            .write_all(
                format!(
                    "ADD-DEVICE {}\n",
                    crate::DeviceMsg {
                        device_id: config.device_id.clone(),
                        product_name: format!("RustSatellite StreamDeck: {}", kind.to_string()),
                        keys_total: kind.key_count(),
                        keys_per_row: kind.column_count(),
                        resolution: kind.key_image_format().size.0.try_into()?,
                    }
                    .device_msg()
                )
                .as_bytes(),
            )
            .await?;

        let writer = Arc::new(Mutex::new(writer));
        let ping = tokio::spawn(companion_ping(writer.clone()));

        Ok(Self {
            ping,
            device_id: config.device_id.clone(),
            writer,
        })
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        // Abort the ping task
        self.ping.abort();
    }
}

async fn companion_ping<W>(companion_write_stream: Arc<Mutex<W>>) -> Result<()>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    debug!("Starting ping task");
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let mut companion_write_stream = companion_write_stream.lock().await;
        companion_write_stream.write_all(b"PING\n").await?;
        companion_write_stream.flush().await?;
    }
}

#[async_trait]
impl<W> traits::companion::Sender for Sender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn config(&mut self, _config: RemoteConfig) -> Result<()> {
        Ok(())
    }
    async fn button_change(&mut self, buttons: ButtonChange) -> Result<()> {
        let mut writer = self.writer.lock().await;
        for (index, pressed) in buttons.buttons {
            let pressed = if pressed { 1 } else { 0 };

            let msg = format!(
                "KEY-PRESS DEVICEID={} KEY={index} PRESSED={pressed}\n",
                self.device_id
            );
            debug!("Sending: {}", msg);
            writer.write_all(msg.as_bytes()).await?;
        }
        writer.flush().await?;
        Ok(())
    }
    async fn encoder_twist(&mut self, encoders: EncoderTwist) -> Result<()> {
        let mut writer = self.writer.lock().await;
        for (index, value) in encoders.encoders {
            let count = value.abs();
            let direction = if value < 0 { 0 } else { 1 };
            let button_id = index;
            let msg = format!(
                "KEY-ROTATE DEVICEID={} KEY={button_id} DIRECTION={direction}\n",
                self.device_id
            );
            debug!("Sending: {}", msg);
            let msg = msg.as_bytes();
            for _ in 0..count {
                writer.write_all(msg).await?;
            }
        }
        writer.flush().await?;
        Ok(())
    }
}
