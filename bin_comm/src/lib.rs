use serde::{Serialize, Deserialize};

pub mod stream_utils;

#[derive(Serialize, Deserialize, Debug)]
pub struct RemoteConfig {
    pub pid: u16,
    pub device_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ButtonChange {
    pub buttons: Vec<bool>
}
impl IntoIterator for ButtonChange {
    type IntoIter = std::iter::Enumerate<std::vec::IntoIter<bool>>;
    type Item = (usize, bool);

    fn into_iter(self) -> Self::IntoIter {
        self.buttons.into_iter().enumerate()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncoderChange {
    pub encoders: Vec<bool>
}
impl EncoderChange {
    /// Converts an encoder "button" to a button with an offset.
    pub fn to_buttons(&self, offset: usize) -> impl Iterator<Item=(usize, bool)> + '_ {
        self.encoders.iter().enumerate().map(move |(i, b)| (i + offset, *b))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EncoderTwist {
    pub index: u8,
    pub value: i8
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetButtonImage {
    pub button: u8,
    pub image: Vec<u8>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetBrightness {
    pub brightness: u8
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RemoteCommands {
    Config(RemoteConfig),
    ButtonChange(ButtonChange),
    EncoderChange(EncoderChange),
    EncoderTwist(EncoderTwist)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetLCDImage {
    pub x_offset: u16,
    pub x_size: u16,
    pub y_size: u16,
    pub image: Vec<u8>
}

#[derive(Serialize, Deserialize, Debug)]
pub enum DeviceCommands {
    SetButtonImage(SetButtonImage),
    SetLCDImage(SetLCDImage),
    SetBrightness(SetBrightness)
}