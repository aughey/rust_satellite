use async_trait::async_trait;
use crate::Result;

/// Receiver trait receives data from the companion app and
/// converts it into commands for the device.
#[async_trait]
pub trait Receiver {
    async fn receive(&mut self) -> Result<crate::device::DeviceCommands>;
}

#[async_trait]
pub trait Sender {
    async fn button_change(&mut self, change: crate::device::ButtonChange) -> Result<()>;
    async fn encoder_twist(&mut self, twist: crate::device::EncoderTwist) -> Result<()>;
}