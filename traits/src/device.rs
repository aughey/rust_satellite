use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

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

/// The configuration of our device.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteConfig {
    /// the hardware product id of the device (usb vid/pid)
    pub pid: u16,
    /// the unique device id of the device stored in the device
    pub device_id: String,
}

/// A button has changed state.
#[derive(Serialize, Deserialize, Debug)]
pub struct ButtonChange {
    /// List of button indicies and their current state
    pub buttons: Vec<(u8, bool)>,
}

/// An encoder has been twisted.
#[derive(Serialize, Deserialize, Debug)]
pub struct EncoderTwist {
    /// List of encoder indicies and their current state
    pub encoders: Vec<(u8, i8)>,
}

/// All commands that can be received from the device
#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    /// Configuration
    Config(RemoteConfig),
    /// Button changing state
    ButtonChange(ButtonChange),
    /// Encoder changing state
    EncoderTwist(EncoderTwist),
}
impl Command {
    /// Convenience method to convert a command into a config.
    pub fn as_config(self) -> Result<RemoteConfig> {
        match self {
            Command::Config(c) => Ok(c),
            _ => anyhow::bail!("Not a config"),
        }
    }
}

/// Action to set an LCD image
#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct SetLCDImage {
    /// The x offset of the image on the LCD
    pub x_offset: u16,
    /// A width of the image.
    pub x_size: u16,
    /// A height of the image.
    pub y_size: u16,
    /// image is an image pre-formatted for the device
    pub image: Vec<u8>,
}

/// Action to set the brightness of the LCD screen
#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct SetBrightness {
    /// Brightness value
    pub brightness: u8,
}

/// Action to set a button image
#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct SetButtonImage {
    /// The index of the button to set
    pub button: u8,
    /// image is an image pre-formatted for the device
    pub image: Vec<u8>,
}

/// All device actions that can be sent to the device.
#[derive(Serialize, Clone, Deserialize, Debug)]
pub enum DeviceActions {
    /// Set the image of a button.
    SetButtonImage(SetButtonImage),
    /// Set the image of the LCD screen.
    SetLCDImage(SetLCDImage),
    /// Set the brightness of the LCD screen
    SetBrightness(SetBrightness),
}
