use elgato_streamdeck::info::Kind;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tracing::debug;
use traits::{anyhow, async_trait, Result, device::{DeviceCommands, SetButtonImage, SetLCDImage, SetBrightness}};
use crate::Command;

pub struct Receiver<R> {
    reader: BufReader<R>,
    kind: Kind,
}
impl<R> Receiver<R>
where
    R: AsyncRead + Unpin + Send,
{
    pub fn new(reader: R, kind: Kind) -> Self {
        Self {
            reader: tokio::io::BufReader::new(reader),
            kind
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
                    debug!("Received key state: {:?}", keystate);
                    debug!("  bitmap size: {}", keystate.bitmap()?.len());

                    let kind = &self.kind;
                    let image_format = kind.key_image_format();

                    let (lcd_width, lcd_height) = self.kind.lcd_strip_size().unwrap_or((0, 0));
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
                            debug!("Writing image to button");
                            let image = image::DynamicImage::ImageRgb8(
                                image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?)
                                    .ok_or_else(|| {
                                        anyhow::anyhow!("Couldn't extract image buffer")
                                    })?,
                            );
                            let image = image.resize_exact(
                                image_format.size.0 as u32,
                                image_format.size.1 as u32,
                                image::imageops::FilterType::Gaussian,
                            );

                            return Ok(DeviceCommands::SetButtonImage(SetButtonImage{
                                button: key,
                                image: image.into_bytes()
                            }));
                        }
                        (None, Some(lcd_key)) => {
                            debug!("Writing image to LCD panel");
                            let image = image::DynamicImage::ImageRgb8(
                                image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?).unwrap(),
                            );
                            // resize image to the height
                            let image = image.resize(
                                image.width(),
                                lcd_height,
                                image::imageops::FilterType::Gaussian,
                            );
                            let button_x_offset =
                                (lcd_key as u32 - 8) * ((lcd_width - image.width()) / 3);

                            return Ok(DeviceCommands::SetLCDImage(SetLCDImage {
                                x_offset: button_x_offset.try_into()?,
                                x_size: lcd_height.try_into()?,
                                y_size: lcd_height.try_into()?,
                                image: image.into_bytes()
                            }));
                        }
                        _ => {
                            debug!("Key out of range {:?}", keystate);
                        }
                    }
                }
                Command::Brightness(brightness) => {
                    debug!("Received brightness: {:?}", brightness);
                    return Ok(
                        DeviceCommands::SetBrightness(SetBrightness {
                            brightness: brightness.brightness
                        })
                    );
                }
                Command::Unknown(command) => {
                    debug!("Unknown command: {} with data {}", command, line.len());
                }
            }
        }
    }
}

