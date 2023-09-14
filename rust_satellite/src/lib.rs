pub use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};

pub mod keyvalue;
use keyvalue::StringOrStr;

#[derive(Serialize, Deserialize)]
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

#[derive(Parser)]
pub struct Cli {
    #[arg(long)]
    pub host: String,
    #[arg(short, long)]
    pub port: u16,
}

/// Commands that can be sent to the device
#[derive(Debug, PartialEq, Eq)]
pub enum Command<'a> {
    Pong,
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
    pub fn parse(data: &str) -> Result<Command<'_>> {
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

        // annoying!, ADD-DEVICE has an extra OK value that doesn't match the key=value format
        // so strip that off if it's there.
        let (data, ok) = if command == "ADD-DEVICE" {
            // eat whitespace
            let data = data.trim_start();
            // the OK or ERR will be seperated by a space.
            let (ok, data) = data
                .split_once(' ')
                .ok_or_else(|| anyhow::anyhow!("Dev Error: this must succeed ADD-DEVICE"))?;
            // eat whitespace
            let data = data.trim_start();
            (data, ok)
        } else {
            (data, "")
        };

        // parse key values specially.  This handles quotes, escapes,
        // and other nonsense.  Returns a map of key value pairs (but
        // optimized to be as zero-copy as possible).
        let key_values = keyvalue::ParseMap::try_from(data)
            .map_err(|e| anyhow::anyhow!("Error parsing key values: {}", e))?;

        // helper function to get a value from the key value map (reduces noise)
        let get = |key| key_values.get(key);

        // switch on the command strings to parse the data into the
        // appropriate command.
        Ok(match command {
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
                pressed: get("PRESSED")?.as_ref() == "true",
            }),
            "ADD-DEVICE" => Command::AddDevice(AddDevice {
                success: ok == "OK",
                device_id: get("DEVICEID")?,
            }),
            "BRIGHTNESS" => Command::Brightness(Brightness {
                device: get("DEVICEID")?,
                brightness: get("VALUE")?.parse()?,
            }),
            _ => Command::Unknown(command),
        })
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

#[derive(Debug, PartialEq, Eq)]
pub struct DeviceMsg {
    pub device_id: String,
    pub product_name: String,
    pub keys_total: u32,
    pub keys_per_row: u32,
}
impl DeviceMsg {
    pub fn device_msg(&self) -> String {
        format!("DEVICEID={} PRODUCT_NAME=\"{}\" KEYS_TOTAL={}, KEYS_PER_ROW={} BITMAPS=120 COLORS=0 TEXT=0",
            self.device_id, self.product_name, self.keys_total, self.keys_per_row)
    }
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
