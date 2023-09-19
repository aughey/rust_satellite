use anyhow::Result;
use async_trait::async_trait;
use elgato_streamdeck::info::Kind;
use keyvalue::StringOrStr;
use tokio::io::{AsyncRead, BufReader, AsyncBufReadExt};
use tracing::{info, trace, debug};
pub mod keyvalue;

#[async_trait]
pub trait StreamDeckDevice {
    async fn set_brightness(&mut self, brightness: u8) -> Result<()>;
    async fn set_button_image(&mut self, button: u8, image: Vec<u8>) -> Result<()>;
    async fn set_lcd_image(
        &mut self,
        x_offset: u16,
        x_size: u16,
        y_size: u16,
        image: Vec<u8>,
    ) -> Result<()>;
}

pub async fn companion_to_device<R>(
    companion_read_stream: R,
    streamdeck_device: impl StreamDeckDevice + Send + Clone + 'static,
    kind: Kind,
    image_format: elgato_streamdeck::info::ImageFormat,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(companion_read_stream).lines();

    info!("Processing commands from companion.");

    // multiple access to write stream
    while let Some(line) = lines.next_line().await? {
        trace!("Received line: {}", line);
        let streamdeck_device = streamdeck_device.clone();
        // Run the processing of EVERY line asynchronously.  This has the advantage of using
        // as many cores as possible for image processing.
        tokio::spawn(async move {
            if let Err(e) = handle_companion_data(line, streamdeck_device, kind, image_format).await
            {
                tracing::error!("Error: {:?}", e);
            }
        });
    }

    Ok(())
}

async fn handle_companion_data(
    line: String,
    mut streamdeck_device: impl StreamDeckDevice + Send + 'static,
    kind: Kind,
    image_format: elgato_streamdeck::info::ImageFormat,
) -> Result<()>
{
    let command = Command::parse(&line)
        .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

    match command {
        Command::Pong => {}
        _ => debug!("Received companion command: {:?}", command),
    }

    match command {
        Command::Pong => {}
        Command::KeyPress(_) => {}
        Command::KeyRotate(_) => {}
        Command::Begin(_) => {}
        Command::AddDevice(_) => {}
        Command::KeyState(keystate) => {
            let in_button_range = (keystate.key < kind.key_count()).then_some(keystate.key);

            let in_lcd_button = if in_button_range.is_some() {
                None
            } else {
                kind.lcd_strip_size()
                    .map(|_| kind.key_count() - keystate.key)
                    .filter(|index| index < &kind.column_count())
            };
            // Run this in a separate task because it can be CPU intensive
            // and we want to allow many image processing tasks to happen in
            // parallel if it can.
            match (in_button_range, in_lcd_button) {
                (Some(key), _) => {
                    debug!("Writing image to button");
                    let image = image::DynamicImage::ImageRgb8(
                        image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?)
                            .ok_or_else(|| anyhow::anyhow!("Couldn't extract image buffer"))?,
                    );
                    let image = image.resize_exact(
                        image_format.size.0 as u32,
                        image_format.size.1 as u32,
                        image::imageops::FilterType::Lanczos3,
                    );
                    // Convert the image into the EXACT format needed for the remote device
                    let image = elgato_streamdeck::images::convert_image(kind, image)?;
                    // Send this to the satellite
                    streamdeck_device.set_button_image(key, image).await?;
                }
                (None, Some(lcd_key)) => {
                    debug!("Writing image to LCD panel");

                    let (lcd_width, lcd_height) = kind.lcd_strip_size().unwrap_or((0, 0));
                    let (lcd_width, lcd_height) = (lcd_width as u32, lcd_height as u32);

                    let image = image::DynamicImage::ImageRgb8(
                        image::ImageBuffer::from_vec(120, 120, keystate.bitmap()?).unwrap(),
                    );
                    // resize image to the height
                    let image = image.resize(
                        lcd_height,
                        lcd_height,
                        image::imageops::FilterType::Gaussian,
                    );
                    let button_x_offset = (lcd_key as u32 - 8) * ((lcd_width - image.width()) / 3);

                    // Convert the image into the EXACT format needed for the remote device
                    let image = elgato_streamdeck::images::convert_image(kind, image)?;
                    streamdeck_device
                        .set_lcd_image(
                            button_x_offset.try_into()?,
                            lcd_height.try_into()?,
                            lcd_height.try_into()?,
                            image,
                        )
                        .await?;
                }
                _ => {
                    debug!("Key out of range {:?}", keystate);
                }
            }
        }
        Command::Brightness(brightness) => {
            streamdeck_device.set_brightness(brightness.brightness).await?;
        }
        Command::Unknown(_) => todo!(),
    }

    Ok(())
}




/// Commands that can be sent to the device
#[derive(Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Pong,
    KeyPress(&'a str),
    KeyRotate(&'a str),
    Begin(Versions<'a>),
    AddDevice(AddDevice<'a>),
    KeyState(KeyState<'a>),
    Brightness(Brightness<'a>),
    Unknown(&'a str),
}

impl Command<'_> {
    /// Parse the incoming line of data into a command.
    /// This will return an error if the command is not
    /// formatted as expected.
    pub fn parse(in_data: &str) -> Result<Command<'_>> {
        let data = in_data;
        // command is up to the first space.  Don't use split_once because
        // there may not be a space to split on.
        let command = data
            .split(' ')
            .next()
            .ok_or_else(|| anyhow::anyhow!("No command"))?;

        // strip command from data.  This will always succeed
        let data = data
            .get(command.len()..)
            .ok_or_else(|| anyhow::anyhow!("Dev Error: this must succeed"))?;

        // shortcut
        match command {
            "PONG" => return Ok(Command::Pong),
            "KEY-PRESS" => return Ok(Command::KeyPress(data)),
            "KEY-ROTATE" => return Ok(Command::KeyRotate(data)),
            _ => {}
        }

        // annoying!, ADD-DEVICE has an extra OK value that doesn't match the key=value format
        // so strip that off if it's there.
        let (data, ok_or_err) = if command == "ADD-DEVICE" {
            // eat whitespace
            let data = data.trim_start();
            // the OK or ERR will be seperated by a space.
            let (ok_or_err, data) = data
                .split_once(' ')
                .ok_or_else(|| anyhow::anyhow!("Dev Error: this must succeed ADD-DEVICE"))?;
            // eat whitespace
            let data = data.trim_start();
            (data, ok_or_err)
        } else {
            (data, "")
        };

        // parse key values specially.  This handles quotes, escapes,
        // and other nonsense.  Returns a map of key value pairs (but
        // optimized to be as zero-copy as possible).
        let mut key_values = keyvalue::ParseMap::try_from(data)
            .map_err(|e| anyhow::anyhow!("Error parsing key values: {}", e))?;

        // helper function to get a value from the key value map (reduces code-noise below)
        // get is consuming from the container, so at the end, we should have consumed all
        // values.
        let mut get = |key| key_values.get(key);

        // switch on the command strings to parse the data into the
        // appropriate command.
        let res = match command {
            "PONG" => Command::Pong,
            "BEGIN" => Command::Begin(Versions {
                companion_version: get("CompanionVersion")?,
                api_version: get("ApiVersion")?,
            }),
            "KEY-STATE" => Command::KeyState(KeyState {
                device: get("DEVICEID")?,
                key: get("KEY")?.parse()?,
                button_type: get("TYPE")?,
                bitmap_base64: get("BITMAP")?,
                pressed: get("PRESSED")?.as_str() == "true",
            }),
            "ADD-DEVICE" => Command::AddDevice(AddDevice {
                success: ok_or_err == "OK",
                device_id: get("DEVICEID")?,
            }),
            "BRIGHTNESS" => Command::Brightness(Brightness {
                device: get("DEVICEID")?,
                brightness: get("VALUE")?.parse()?,
            }),
            _ => Command::Unknown(command),
        };

        // we should have consumed all values
        if !key_values.is_empty() {
            Err(anyhow::anyhow!(
                "Dev Error: Unparsed key values: {:?} from command: {:?}",
                key_values,
                in_data
            ))
        } else {
            Ok(res)
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct KeyState<'a> {
    pub device: StringOrStr<'a>,
    pub key: u8,
    pub button_type: StringOrStr<'a>,
    pub bitmap_base64: StringOrStr<'a>,
    pub pressed: bool,
}
impl KeyState<'_> {
    pub fn bitmap(&self) -> Result<Vec<u8>> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(self.bitmap_base64.as_ref().as_bytes())?;
        Ok(data)
    }
}

impl std::fmt::Debug for KeyState<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyState")
            .field("device", &self.device)
            .field("key", &self.key)
            .field("button_type", &self.button_type)
            .field("len(bitmap_base64)", &self.bitmap_base64.len())
            .field("pressed", &self.pressed)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Brightness<'a> {
    pub device: StringOrStr<'a>,
    pub brightness: u8,
}

#[derive(Debug, PartialEq, Eq)]
pub struct AddDevice<'a> {
    pub success: bool,
    pub device_id: StringOrStr<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Versions<'a> {
    pub companion_version: StringOrStr<'a>,
    pub api_version: StringOrStr<'a>,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pong_command() {
        const DATA: &str = "PONG";
        let command = Command::parse(DATA).unwrap();
        assert_eq!(command, Command::Pong);
    }

    #[test]
    fn test_begin() {
        const DATA: &str = "BEGIN CompanionVersion=3.99.0+6259-develop-a48ec073 ApiVersion=1.5.1";
        let command = Command::parse(DATA).unwrap();
        assert_eq!(
            command,
            Command::Begin(Versions {
                companion_version: "3.99.0+6259-develop-a48ec073".into(),
                api_version: "1.5.1".into()
            })
        );
    }

    #[test]
    fn test_adddevice() {
        const DATA: &str = "ADD-DEVICE OK DEVICEID=\"JohnAughey\"";
        let command = Command::parse(DATA).unwrap();
        assert_eq!(
            command,
            Command::AddDevice(AddDevice {
                success: true,
                device_id: "JohnAughey".into()
            })
        );
    }

    #[test]
    fn test_keystate() {
        const DATA: &str =
            "KEY-STATE DEVICEID=JohnAughey KEY=14 TYPE=BUTTON  BITMAP=rawdata PRESSED={true,false}";
        let command = Command::parse(DATA).unwrap();
        assert_eq!(
            command,
            Command::KeyState(KeyState {
                device: "JohnAughey".into(),
                key: 14,
                button_type: "BUTTON".into(),
                bitmap_base64: "rawdata".into(),
                pressed: false
            })
        );
    }

    #[test]
    fn test_add_device_command() {
        const DATA: &str = "ADD-DEVICE OK DEVICEID=\"JohnAughey\"";
        let command = Command::parse(DATA).unwrap();
        assert_eq!(
            command,
            Command::AddDevice(AddDevice {
                success: true,
                device_id: "JohnAughey".into()
            })
        );

        const DATA_ERR: &str = "ADD-DEVICE Err DEVICEID=\"JohnAughey\"";
        let command = Command::parse(DATA_ERR).unwrap();
        assert_eq!(
            command,
            Command::AddDevice(AddDevice {
                success: false,
                device_id: "JohnAughey".into()
            })
        );
    }
}
