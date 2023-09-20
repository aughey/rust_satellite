use std::num::NonZeroUsize;

use crate::Command;
use elgato_streamdeck::info::Kind;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::{debug, trace};
use traits::{
    anyhow, async_trait,
    device::{DeviceActions, SetBrightness, SetButtonImage, SetLCDImage},
    Result,
};

trait CommandProcessor {
    fn process(
        &mut self,
        kind: Kind,
        command: Command,
    ) -> Result<Option<traits::device::DeviceActions>>;
}

#[derive(Default)]
struct DefaultCommandProcessor {}
impl CommandProcessor for DefaultCommandProcessor {
    fn process(
        &mut self,
        kind: Kind,
        command: Command,
    ) -> Result<Option<traits::device::DeviceActions>> {
        let ret = match command {
            Command::KeyPress(data) => {
                debug!("Received key press: {data}");
                None
            }
            Command::KeyRotate(data) => {
                debug!("Received key rotate: {data}");
                None
            }
            Command::Pong => {
                //trace!("Received PONG");
                None
            }
            Command::Begin(versions) => {
                debug!("Beginning communication: {:?}", versions);
                None
            }
            Command::AddDevice(device) => {
                debug!("Adding device: {:?}", device);
                None
            }
            Command::KeyState(keystate) => {
                debug!("Received key state: {:?}", keystate);
                debug!("  bitmap size: {}", keystate.bitmap()?.len());

                let (lcd_width, lcd_height) = kind.lcd_strip_size().unwrap_or((0, 0));
                let (lcd_width, lcd_height) = (lcd_width as u32, lcd_height as u32);

                let in_button_range = (keystate.key < kind.key_count()).then_some(keystate.key);

                let in_lcd_button = if in_button_range.is_some() {
                    None
                } else {
                    kind.lcd_strip_size()
                        .map(|_| kind.key_count() - keystate.key)
                        .filter(|index| index < &kind.column_count())
                };

                match (in_button_range, in_lcd_button) {
                    (Some(key), _) => {
                        trace!("Writing image to button");

                        let size = kind.key_image_format().size.0;
                        let bitmap = keystate.bitmap()?;
                        if bitmap.len() != size * size * 3 {
                            anyhow::bail!(
                                "Expected bitmap to be len {}, but was {}",
                                size * size * 3,
                                bitmap.len()
                            );
                        }
                        let image = image::DynamicImage::ImageRgb8(
                            image::ImageBuffer::from_vec(
                                size.try_into()?,
                                size.try_into()?,
                                keystate.bitmap()?,
                            )
                            .ok_or_else(|| anyhow::anyhow!("Couldn't extract image buffer"))?,
                        );

                        let image = elgato_streamdeck::images::convert_image(kind, image)?;

                        let ret =
                            DeviceActions::SetButtonImage(SetButtonImage { button: key, image });

                        Some(ret)
                    }
                    (None, Some(lcd_key)) => {
                        debug!("Writing image to LCD panel");
                        let size = kind.key_image_format().size.0.try_into()?;
                        let image = image::DynamicImage::ImageRgb8(
                            image::ImageBuffer::from_vec(size, size, keystate.bitmap()?).unwrap(),
                        );
                        // resize image to the height
                        let image = image.resize(
                            image.width(),
                            lcd_height,
                            image::imageops::FilterType::Gaussian,
                        );
                        let button_x_offset =
                            (lcd_key as u32 - 8) * ((lcd_width - image.width()) / 3);

                        Some(DeviceActions::SetLCDImage(SetLCDImage {
                            x_offset: button_x_offset.try_into()?,
                            x_size: lcd_height.try_into()?,
                            y_size: lcd_height.try_into()?,
                            image: image.into_bytes(),
                        }))
                    }
                    _ => {
                        debug!("Key out of range {:?}", keystate);
                        None
                    }
                }
            }
            Command::Brightness(brightness) => {
                debug!("Received brightness: {:?}", brightness);
                Some(DeviceActions::SetBrightness(SetBrightness {
                    brightness: brightness.brightness,
                }))
            }
            Command::Unknown(command) => {
                debug!("Unknown command: {}", command);
                None
            }
        };

        Ok(ret)
    }
}

pub struct Receiver<R> {
    reader: BufReader<R>,
    kind: Kind,
    processor: DefaultCommandProcessor,
    cache: lru::LruCache<String, traits::device::DeviceActions>,
}
impl<R> Receiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    pub fn new(reader: R, kind: Kind) -> Self {
        Self {
            reader: tokio::io::BufReader::new(reader),
            kind,
            processor: Default::default(),
            cache: lru::LruCache::new(NonZeroUsize::new(100).unwrap()),
        }
    }
}
#[async_trait]
impl<R> traits::companion::Receiver for Receiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    async fn receive(&mut self) -> Result<traits::device::DeviceActions> {
        // read a line from the stream
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).await?;

            if let Some(command) = self.cache.get(&line) {
                return Ok(command.clone());
            }

            let command = Command::parse(&line)?;

            let processor = &mut self.processor;
            if let Some(commands) = processor.process(self.kind, command)? {
                self.cache.put(line, commands.clone());
                return Ok(commands);
            }
        }
    }
}
