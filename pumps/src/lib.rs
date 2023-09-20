//! # pumps
//! 
//! This crate provides message pump functions to asynchronously move data between
//! senders and receivers.
//! 
//! Once the connections to the underlying device and the companion app are established,
//! the primary function of all applications is to asynchronously wait for messages from
//! one side and pass them to the other side.  This generically implements that functionality
//! to be used across all applications.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

use std::future::Future;

use tracing::trace;
use traits::Result;

/// Create devices and connect them together with a message pump.
/// In the common case, this can create an entire application in
/// a single call with provided factory functions.
/// 
/// create_device is a factory function that creates the device sender and receiver.
/// 
/// create_companion is a factory function that creates the companion sender and receiver.
pub async fn create_and_run<DS, DR, CS, CR, CD, CC, CDF, CCF>(
    create_device: CD,
    create_companion: CC,
) -> traits::Result<()>
where
    CD: Fn() -> CDF,
    CDF: Future<Output = Result<(DS, DR)>>,
    CC: Fn((&mut DS, &mut DR)) -> CCF,
    CCF: Future<Output = Result<(CS, CR)>>,
    DS: traits::device::Sender + Send + 'static,
    DR: traits::device::Receiver + Send + 'static,
    CS: traits::companion::Sender + Send + 'static,
    CR: traits::companion::Receiver + Send + 'static,
{
    let mut devices = create_device().await?;
    let companions = create_companion((&mut devices.0, &mut devices.1)).await?;

    message_pump(devices.0, devices.1, companions.0, companions.1).await
}

/// message_pump takes all four sender and receiver traits and asynchronously
/// moves data between them.  This is the core of all applications.
/// 
/// Internally, this will create two independent asynchronous operations that
/// move data between the device to the companion, and the companion to the device.
/// 
/// This function will return when either of the two operations returns an error or
/// if they both succeed (using tokio::tryjoin!).
pub async fn message_pump(
    device_sender: impl traits::device::Sender,
    device_receiver: impl traits::device::Receiver,
    companion_sender: impl traits::companion::Sender,
    companion_receiver: impl traits::companion::Receiver,
) -> Result<()> {
    let device_to_companion = handle_device_to_companion(device_receiver, companion_sender);
    let companion_to_device = handle_companion_to_device(companion_receiver, device_sender);

    // Wait for all tasks to complete.  If there is an error, abort early.
    let res = tokio::try_join!(device_to_companion, companion_to_device);

    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

/// handle_device_to_companion takes a device receiver and a companion sender
/// and asynchronously moves data between them.  A complete match statement
/// is provided to handle all possible device commands and any new commands
/// added to the device trait will be a compile time error until the match
/// statement is updated.
async fn handle_device_to_companion(
    mut device_receiver: impl traits::device::Receiver,
    mut companion_sender: impl traits::companion::Sender,
) -> Result<()> {
    loop {
        let action = device_receiver.receive().await?;
        trace!("handle_device_to_companion: {:?}", action);
        match action {
            traits::device::Command::Config(c) => companion_sender.config(c).await?,
            traits::device::Command::ButtonChange(change) => {
                companion_sender.button_change(change).await?
            }
            traits::device::Command::EncoderTwist(twist) => {
                companion_sender.encoder_twist(twist).await?
            }
        }
    }
}

/// handle_companion_to_device takes a companion receiver and a device sender
/// and asynchronously moves data between them.  A complete match statement
/// is provided to handle all possible companion commands and any new commands
/// added to the companion trait will be a compile time error until the match
/// statement is updated.
async fn handle_companion_to_device(
    mut companion_receiver: impl traits::companion::Receiver,
    mut device_sender: impl traits::device::Sender,
) -> Result<()> {
    loop {
        let action = companion_receiver.receive().await?;
        trace!("handle_device_to_companion: {:?}", action);
        match action {
            traits::device::DeviceActions::SetButtonImage(image) => {
                device_sender.set_button_image(image).await?
            }
            traits::device::DeviceActions::SetLCDImage(image) => {
                device_sender.set_lcd_image(image).await?
            }
            traits::device::DeviceActions::SetBrightness(brightness) => {
                device_sender.set_brightness(brightness).await?
            }
        }
    }
}
