//! # streamdeck
//!
//! A crate that implements the traits device::Sender and device::Receiver for the Elgato StreamDeck.
//!
//! This adapts the underlying elgato_streamdeck crate to the traits defined in rust_satellite

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

use elgato_streamdeck::info::Kind;
use elgato_streamdeck::AsyncStreamDeck;
use tracing::{debug, info, trace};
use traits::Result;
use traits::anyhow;
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

/// StreamDeck implements the device::Sender and device::Receiver traits for the Elgato StreamDeck.
///
/// A single StreamDeck implements both the sender and receiver traits and can be cloned to
/// create multiple instances of the same device.  The device is shared properly in the underlying
/// elgato_streamdeck::AsyncStreamDeck implementation.
#[derive(Clone)]
pub struct StreamDeck {
    keystate: KeyState,
    device: AsyncStreamDeck,
    first: bool,
}
impl StreamDeck {
    /// Get the kind of device this is.
    pub fn kind(&self) -> elgato_streamdeck::info::Kind {
        self.device.kind()
    }
    /// Create a new StreamDeck from the provided AsyncStreamDeck.
    pub fn new(device: AsyncStreamDeck) -> Self {
        let kind = device.kind();
        // Our key layout is the hardware keys, followed by virtual LCD keys, followed by encoders.
        let keycount = kind.key_count()
            + if kind.lcd_strip_size().is_some() {
                kind.column_count()
            } else {
                0
            }
            + kind.encoder_count();
        let keystate = KeyState {
            states: vec![false; keycount as usize],
        };
        Self {
            keystate,
            device,
            first: true,
        }
    }

    /// Opens the first StreamDeck found.
    pub async fn open_first() -> Result<(StreamDeck, StreamDeck)> {
        Self::open(|_| true).await
    }

    /// Constructor to create a new StreamDeck according to the predicate
    /// provided.
    pub async fn open(mut filter: impl FnMut(&Kind) -> bool) -> Result<(StreamDeck, StreamDeck)> {
        // Create instance of HidApi
        let hid = elgato_streamdeck::new_hidapi().unwrap();

        // List devices and unsafely take first one
        let (kind, serial) = elgato_streamdeck::list_devices(&hid)
            .into_iter()
            .find(|(kind,_)| filter(kind))
            .ok_or_else(|| anyhow::anyhow!("No matching devices found"))?;

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
        debug!("set_button_image: {:?}", image);
        Ok(self.device.write_image(image.button, &image.image).await?)
    }
    async fn set_lcd_image(&mut self, _image: SetLCDImage) -> Result<()> {
        // Ok(self.device.write_lcd(image.x_offset, 0, image.image).await?)
        Ok(())
    }
}

#[async_trait]
impl traits::device::Receiver for StreamDeck {
    async fn receive(&mut self) -> Result<leaf_comm::Command> {
        // the first message must be the config.
        trace!("receive");
        if self.first {
            trace!("First read");
            self.first = false;
            return Ok(leaf_comm::Command::Config(
                leaf_comm::RemoteConfig {
                    pid: self.device.kind().product_id(),
                    device_id: self.device.serial_number().await?,
                },
            ));
        }
        loop {
            let buttons = self.device.read_input(60.0).await?;
            match buttons {
                elgato_streamdeck::StreamDeckInput::NoData => {}
                elgato_streamdeck::StreamDeckInput::ButtonStateChange(buttons) => {
                    return Ok(leaf_comm::Command::ButtonChange(
                        leaf_comm::ButtonChange {
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
                    return Ok(leaf_comm::Command::EncoderTwist(
                        leaf_comm::EncoderTwist {
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
