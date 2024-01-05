//! # Companion traits
//! 
//! The companion traits represent two sides of a connection to the
//! companion app.  One is asynchronous commands received from the
//! companion app and the other are actions called in response to
//! button presses, encoder twists, and other events.

use crate::Result;
use async_trait::async_trait;
use leaf_comm::{DeviceActions, RemoteConfig, ButtonChange, EncoderTwist};

/// Receiver trait receives data from the companion app and
/// converts it into commands for the device.
#[async_trait]
pub trait Receiver {
    /// asynchronously receive a device command from the companion app
    async fn receive(&mut self) -> Result<DeviceActions>;
}

/// Sender trait is used to notify the companion app of events read from
/// the device.
#[async_trait]
pub trait Sender {
    /// Configuration has changed.  This should be sent prior to any other
    /// commands and should only be called once.
    async fn config(&mut self, config: RemoteConfig) -> Result<()>;
    /// A button has changed state.  The ButtonChange object has a list of buttons
    /// that have changed.
    async fn button_change(&mut self, change: ButtonChange) -> Result<()>;
    /// An encoder has been twisted.  The EncoderTwist object has a list of encoders
    /// that have changed.
    async fn encoder_twist(&mut self, twist: EncoderTwist) -> Result<()>;
}
