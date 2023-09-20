use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Listens async for commands from the device.
#[async_trait]
pub trait Receiver {
    async fn receive(&mut self) -> Result<Command>;
}

#[async_trait]
pub trait Sender {
    async fn set_brightness(&mut self, brightness: SetBrightness) -> Result<()>;
    async fn set_button_image(&mut self, image: SetButtonImage) -> Result<()>;
    async fn set_lcd_image(&mut self, image: SetLCDImage) -> Result<()>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteConfig {
    pub pid: u16,
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ButtonChange {
    pub buttons: Vec<(u8, bool)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncoderTwist {
    pub encoders: Vec<(u8, i8)>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Config(RemoteConfig),
    ButtonChange(ButtonChange),
    EncoderTwist(EncoderTwist),
}
impl Command {
    pub fn as_config(self) -> Result<RemoteConfig> {
        match self {
            Command::Config(c) => Ok(c),
            _ => anyhow::bail!("Not a config"),
        }
    }
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct SetLCDImage {
    pub x_offset: u16,
    pub x_size: u16,
    pub y_size: u16,
    /// image is an image formatted for the device
    pub image: Vec<u8>,
}

#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct SetBrightness {
    pub brightness: u8,
}

#[derive(Serialize, Clone,Deserialize, Debug)]
pub struct SetButtonImage {
    pub button: u8,
    /// image is an image formatted for the device
    pub image: Vec<u8>,
}

#[derive(Serialize,Clone, Deserialize, Debug)]
pub enum DeviceCommands {
    SetButtonImage(SetButtonImage),
    SetLCDImage(SetLCDImage),
    SetBrightness(SetBrightness),
}
