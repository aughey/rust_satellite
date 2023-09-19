use elgato_streamdeck::AsyncStreamDeck;
use tracing::{debug, info};
use traits::Result;
use traits::{
    async_trait,
    device::{SetBrightness, SetButtonImage, SetLCDImage},
};

#[derive(Clone)]
struct KeyState {
    states: Vec<bool>,
}
impl KeyState {
    fn update_state<'a>(
        &'a mut self,
        offset: usize,
        changes: impl IntoIterator<Item = (usize, bool)> + 'a,
    ) -> impl Iterator<Item = (u8, bool)> + 'a {
        changes.into_iter().filter_map(move |(index, state)| {
            let index = index + offset;
            if *self.states.get(index)? == state {
                None
            } else {
                self.states[index] = state;
                Some((index as u8, state))
            }
        })
    }
}

#[derive(Clone)]
pub struct StreamDeck {
    keystate: KeyState,
    device: AsyncStreamDeck,
    first: bool,
}
impl StreamDeck {
    pub fn new(device: AsyncStreamDeck) -> Self {
        let kind = device.kind();
        let keycount = kind.key_count()
            + if kind.lcd_strip_size().is_some() {
                kind.column_count()
            } else {
                0
            };
        let keystate = KeyState {
            states: vec![false; keycount as usize],
        };
        Self {
            keystate,
            device,
            first: true,
        }
    }

    pub async fn open() -> Result<(impl traits::device::Sender, impl traits::device::Receiver)> {
        // Create instance of HidApi
        let hid = elgato_streamdeck::new_hidapi().unwrap();

        // List devices and unsafely take first one
        let (kind, serial) = elgato_streamdeck::list_devices(&hid).remove(0);
        let image_format = kind.key_image_format();
        info!("Found kind {:?} with image format {:?}", kind, image_format);

        // Connect to the device
        let device =
            elgato_streamdeck::asynchronous::AsyncStreamDeck::connect(&hid, kind, &serial)?;

        // Print out some info from the device
        info!(
            "Connected to '{}' with version '{}'",
            device.serial_number().await?,
            device.firmware_version().await?
        );

        device.reset().await?;

        // Set device brightness
        device.set_brightness(35).await?;

        let device_sender = Self::new(device.clone());
        let device_receiver = device_sender.clone();
        Ok((device_sender, device_receiver))
    }
}

#[async_trait]
impl traits::device::Sender for StreamDeck {
    async fn set_brightness(&mut self, brightness: SetBrightness) -> Result<()> {
        Ok(self.device.set_brightness(brightness.brightness).await?)
    }
    async fn set_button_image(&mut self, image: SetButtonImage) -> Result<()> {
        Ok(self.device.write_image(image.button, &image.image).await?)
    }
    async fn set_lcd_image(&mut self, _image: SetLCDImage) -> Result<()> {
        // Ok(self.device.write_lcd(image.x_offset, 0, image.image).await?)
        Ok(())
    }
}

#[async_trait]
impl traits::device::Receiver for StreamDeck {
    async fn receive(&mut self) -> Result<traits::device::Command> {
        // the first message must be the config.
        if self.first {
            self.first = false;
            return Ok(traits::device::Command::Config(
                traits::device::RemoteConfig {
                    pid: self.device.kind().product_id(),
                    device_id: "ZZZZ".to_string(),
                },
            ));
        }
        loop {
            let buttons = self.device.read_input(60.0).await?;
            debug!("Got usb command {:?}", buttons);
            match buttons {
                elgato_streamdeck::StreamDeckInput::NoData => {}
                elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
                    return Ok(traits::device::Command::ButtonChange(
                        traits::device::ButtonChange {
                            buttons: self
                                .keystate
                                .update_state(0, buttons.into_iter().enumerate())
                                .collect(),
                        },
                    ))
                }
                elgato_streamdeck::StreamDeckInput::EncoderTwist(twist) => {
                    let twists = twist
                        .into_iter()
                        .take(self.device.kind().key_count() as usize)
                        .enumerate()
                        .filter(|(_i, v)| *v != 0)
                        .map(|(i, v)| (i as u8, v));
                    return Ok(traits::device::Command::EncoderTwist(
                        traits::device::EncoderTwist {
                            encoders: twists.collect(),
                        },
                    ));
                }
                elgato_streamdeck::StreamDeckInput::EncoderStateChange(_) => {}
                elgato_streamdeck::StreamDeckInput::TouchScreenPress(_, _) => {}
                elgato_streamdeck::StreamDeckInput::TouchScreenLongPress(_, _) => {}
                elgato_streamdeck::StreamDeckInput::TouchScreenSwipe(_, _) => {}
            }
        }
    }
}
