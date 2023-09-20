use std::future::Future;

use tracing::trace;
use traits::Result;

pub async fn run_satellite<DS, DR, CS, CR, CD, CC, CDF, CCF>(
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

async fn handle_companion_to_device(
    mut companion_receiver: impl traits::companion::Receiver,
    mut device_sender: impl traits::device::Sender,
) -> Result<()> {
    loop {
        let action = companion_receiver.receive().await?;
        trace!("handle_device_to_companion: {:?}", action);
        match action {
            traits::device::DeviceCommands::SetButtonImage(image) => {
                device_sender.set_button_image(image).await?
            }
            traits::device::DeviceCommands::SetLCDImage(image) => {
                device_sender.set_lcd_image(image).await?
            }
            traits::device::DeviceCommands::SetBrightness(brightness) => {
                device_sender.set_brightness(brightness).await?
            }
        }
    }
}
