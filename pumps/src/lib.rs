use traits::Result;

pub async fn message_pump(
    device_controller: impl traits::device::Controller,
    device_receiver: impl traits::device::Receiver,
    companion_sender: impl traits::companion::Sender,
    companion_receiver: impl traits::companion::Receiver,
) -> Result<()> {
    let device_to_companion = handle_device_to_companion(device_receiver, companion_sender);
    let companion_to_device = handle_companion_to_device(companion_receiver, device_controller);

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
        match action {
            traits::device::Command::Config(_) => {}
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
    mut device_controller: impl traits::device::Controller,
) -> Result<()> {
    loop {
        let action = companion_receiver.receive().await?;
        match action {
            traits::device::DeviceCommands::SetButtonImage(image) => {
                device_controller.set_button_image(image).await?
            }
            traits::device::DeviceCommands::SetLCDImage(image) => {
                device_controller.set_lcd_image(image).await?
            }
            traits::device::DeviceCommands::SetBrightness(brightness) => {
                device_controller.set_brightness(brightness).await?
            }
        }
    }
}
