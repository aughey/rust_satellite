use crate::Command;
use elgato_streamdeck::info::Kind;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::{debug, trace};
use traits::{
    anyhow, async_trait,
    device::{DeviceCommands, SetBrightness, SetButtonImage, SetLCDImage},
    Result,
};

trait CommandProcessor {
    fn process(
        &mut self,
        kind: Kind,
        command: Command,
    ) -> Result<Option<traits::device::DeviceCommands>>;
}

struct DefaultCommandProcessor {
    cache: lru::LruCache<String, traits::device::DeviceCommands>,
}
impl Default for DefaultCommandProcessor {
    fn default() -> Self {
        Self {
            cache: lru::LruCache::new(std::num::NonZeroUsize::new(200).unwrap()),
        }
    }
}
impl CommandProcessor for DefaultCommandProcessor {
    fn process(
        &mut self,
        kind: Kind,
        command: Command,
    ) -> Result<Option<traits::device::DeviceCommands>> {
        match command {
            Command::KeyPress(data) => {
                debug!("Received key press: {data}");
            }
            Command::KeyRotate(data) => {
                debug!("Received key rotate: {data}");
            }
            Command::Pong => {
                //trace!("Received PONG");
            }
            Command::Begin(versions) => {
                debug!("Beginning communication: {:?}", versions);
            }
            Command::AddDevice(device) => {
                debug!("Adding device: {:?}", device);
            }
            Command::KeyState(keystate) => {
                if let Some(cache) = self.cache.get(keystate.bitmap_base64.as_ref()) {
                    return Ok(Some(cache.clone()));
                }

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

                        let ret = DeviceCommands::SetButtonImage(SetButtonImage {
                            button: key,
                            image,
                        });

                        self.cache.put(keystate.bitmap_base64.as_ref().to_string(), ret.clone());

                        return Ok(Some(ret));
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

                        return Ok(Some(DeviceCommands::SetLCDImage(SetLCDImage {
                            x_offset: button_x_offset.try_into()?,
                            x_size: lcd_height.try_into()?,
                            y_size: lcd_height.try_into()?,
                            image: image.into_bytes(),
                        })));
                    }
                    _ => {
                        debug!("Key out of range {:?}", keystate);
                    }
                }
            }
            Command::Brightness(brightness) => {
                debug!("Received brightness: {:?}", brightness);
                return Ok(Some(DeviceCommands::SetBrightness(SetBrightness {
                    brightness: brightness.brightness,
                })));
            }
            Command::Unknown(command) => {
                debug!("Unknown command: {}", command);
            }
        }
        Ok(None)
    }
}

pub struct Receiver<R> {
    reader: BufReader<R>,
    kind: Kind,
    processor: DefaultCommandProcessor,
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
        }
    }
}
#[async_trait]
impl<R> traits::companion::Receiver for Receiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    async fn receive(&mut self) -> Result<traits::device::DeviceCommands> {
        // read a line from the stream
        loop {
            let mut line = String::new();
            self.reader.read_line(&mut line).await?;
            let command = Command::parse(&line)?;

            let processor = &mut self.processor;
            if let Some(commands) = processor.process(self.kind, command)? {
                return Ok(commands);
            }
        }
    }
}
