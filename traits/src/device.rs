use crate::Result;
use async_trait::async_trait;

// make Command, SetBrightness, SetButtonImage, and SetLCDImage available
// for other crates to use.
pub use leaf_comm::{Command, RemoteConfig,DeviceActions,SetBrightness, SetButtonImage, SetLCDImage};

extern crate alloc;

/// Listens async for actions from the device.
#[async_trait]
pub trait Receiver {
    /// Asynchronously receive a new action from the device.
    async fn receive(&mut self) -> Result<Command>;
}

/// Sends commands to the device to change the physical state of the device.
#[async_trait]
pub trait Sender {
    /// Set the brightness to a given value
    async fn set_brightness(&mut self, brightness: SetBrightness) -> Result<()>;
    /// Set the image of a button.
    async fn set_button_image(&mut self, image: SetButtonImage) -> Result<()>;
    /// Set the image of the LCD screen.
    async fn set_lcd_image(&mut self, image: SetLCDImage) -> Result<()>;
}