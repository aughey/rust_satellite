#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::string::String;
use serde::{Serialize, Deserialize};

/// The configuration of our device.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteConfig {
    /// the hardware product id of the device (usb vid/pid)
    pub pid: u16,
    /// the unique device id of the device stored in the device
    pub device_id: String
}

/// The configuration of our device.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BorrowedRemoteConfig<'a> {
    /// the hardware product id of the device (usb vid/pid)
    pub pid: u16,
    /// the unique device id of the device stored in the device
    pub device_id: &'a str
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
