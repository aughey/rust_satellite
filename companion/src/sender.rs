use std::sync::Arc;

use tokio::{
    io::{AsyncWrite, AsyncWriteExt},
    sync::Mutex,
};
use tracing::debug;
use traits::Result;
use traits::{async_trait, device::EncoderTwist};

pub struct Sender<W> {
    device_id: String,
    writer: Arc<Mutex<W>>,
    ping: tokio::task::JoinHandle<Result<()>>,
}
impl<W> Sender<W>
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(writer: W, device_id: String) -> Self {
        let writer = Arc::new(Mutex::new(writer));
        let ping = tokio::spawn(companion_ping(writer.clone()));

        Self {
            ping,
            device_id,
            writer,
        }
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
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        let mut companion_write_stream = companion_write_stream.lock().await;
        companion_write_stream.write_all(b"PING\n").await.unwrap();
        companion_write_stream.flush().await.unwrap();
    }
}

#[async_trait]
impl<W> traits::companion::Sender for Sender<W>
where
    W: AsyncWrite + Unpin + Send,
{
    async fn button_change(&mut self, buttons: traits::device::ButtonChange) -> Result<()> {
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
        for (index,value) in encoders.encoders {
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
