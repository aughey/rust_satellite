pub use anyhow::Result;
use clap::Parser;
use nom::IResult;
use serde::{Deserialize, Serialize};

mod keyvalue;
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

trait Parse<'a> {
    fn parse(data: &'a keyvalue::ParseMap) -> Result<Self>
    where
        Self: Sized;
}

pub fn iresult_unwrap<D, R>(value: IResult<D, R>) -> Result<R> {
    match value {
        Ok((_, value)) => Ok(value),
        Err(_) => Err(anyhow::anyhow!("nom Parse error")),
    }
}

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
    pub fn parse(data: &str) -> Result<Command<'_>> {
        // command is up to the first space
        let command = data
            .split(' ')
            .next()
            .ok_or_else(|| anyhow::anyhow!("No command"))?;

        // strip command from data.  This will always succeed
        let data = data
            .get(command.len()..)
            .ok_or_else(|| anyhow::anyhow!("Dev Error: this must succeed"))?;

        let (data, ok) = if command == "ADD-DEVICE" {
            // annoying!, it has an extra OK value that doesn't match the key=value format
            // eat whitespace
            let data = data.trim_start();
            let ok = data
                .split(' ')
                .next()
                .ok_or_else(|| anyhow::anyhow!("Dev Error: this must succeed ADD-DEVICE"))?;
            let data = data
                .get(ok.len()..)
                .ok_or_else(|| anyhow::anyhow!("Dev Error: this must succeed ADD-DEVICE"))?;
            (data, ok)
        } else {
            (data, "")
        };

        // parse key values
        let key_values = keyvalue::str_to_key_value(data)
            .map_err(|e| anyhow::anyhow!("Error parsing key values: {}", e))?
            .1;

        let res = match command {
            "PONG" => Command::Pong,
            "BEGIN" => Command::Begin(Versions {
                companion_version: key_values.get("CompanionVersion")?,
                api_version: key_values.get("ApiVersion")?,
            }),
            "KEY-STATE" => Command::KeyState(KeyState {
                device: key_values.get("DEVICEID")?,
                key: key_values.get("KEY")?.as_ref().parse()?,
                button_type: key_values.get("TYPE")?,
                bitmap_base64: key_values.get("BITMAP")?,
                pressed: key_values.get("PRESSED")?.as_ref() == "true",
            }),
            "ADD-DEVICE" => Command::AddDevice(AddDevice {
                success: ok == "OK",
                device_id: key_values.get("DEVICEID")?,
            }),
            "BRIGHTNESS" => Command::Brightness(Brightness {
                device: key_values.get("DEVICEID")?,
                brightness: key_values.get("VALUE")?.as_ref().parse()?,
            }),
            _ => Command::Unknown(command),
        };
        Ok(res)
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
impl<'a> Parse<'a> for KeyState<'a> {
    fn parse(data: &'a keyvalue::ParseMap) -> Result<Self> {
        Ok(Self {
            device: data.get("DEVICEID")?,
            key: data.get("KEY")?.as_ref().parse()?,
            button_type: data.get("TYPE")?,
            bitmap_base64: data.get("BITMAP")?,
            pressed: data.get("PRESSED")?.as_ref() == "true",
        })
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
impl<'a> Parse<'a> for Brightness<'a> {
    fn parse(data: &'a keyvalue::ParseMap) -> Result<Brightness<'a>> {
        Ok(Self {
            device: data.get("DEVICEID")?,
            brightness: data.get("VALUE")?.as_ref().parse()?,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AddDevice<'a> {
    pub success: bool,
    pub device_id: StringOrStr<'a>,
}
impl<'a> Parse<'a> for AddDevice<'a> {
    fn parse(data: &'a keyvalue::ParseMap) -> Result<Self> {
        Ok(Self {
            success: data.get("OK")?.as_ref() == "true",
            device_id: data.get("DEVICEID")?,
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Versions<'a> {
    pub companion_version: StringOrStr<'a>,
    pub api_version: StringOrStr<'a>,
}
impl<'a> Parse<'a> for Versions<'a> {
    fn parse(data: &'a keyvalue::ParseMap) -> Result<Self> {
        // String looks like:
        // CompanionVersion=3.99.0+6259-develop-a48ec073 ApiVersion=1.5.1
        Ok(Self {
            companion_version: data.get("CompanionVersion")?,
            api_version: data.get("ApiVersion")?,
        })
    }
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
    fn test_keyvalue_parser() {
        const DATA: &str =
            "DEVICEID=JohnAughey KEY=14 TYPE=BUTTON  BITMAP=rawdata PRESSED={true,false}";
        let (_, key_values) = keyvalue::str_to_key_value(DATA).unwrap();
        let mut keys = key_values.keys().map(|s| s.to_owned()).collect::<Vec<_>>();
        keys.sort();

        assert_eq!(keys, vec!["BITMAP", "DEVICEID", "KEY", "PRESSED", "TYPE",]);
    }

    #[test]
    fn test_keyvalue_quoted_value() {
        const DATA: &str = "key=\"value\"";
        let (_, key_values) = keyvalue::str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_empty() {
        const DATA: &str = "  ";
        let (_, key_values) = keyvalue::str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 0);
    }

    #[test]
    fn test_keyvalue_parser_leading_space() {
        const DATA: &str = "  key=value";
        let (_, key_values) = keyvalue::str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_trailing_space() {
        const DATA: &str = "key=value  ";
        let (_, key_values) = keyvalue::str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_multi_inbetween() {
        const DATA: &str = " key=value  foo=bar ";
        let (_, key_values) = keyvalue::str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 2);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
        assert_eq!(key_values.get("foo").unwrap(), "bar".into());
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
    }
}
