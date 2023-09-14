use std::collections::HashMap;

pub use anyhow::Result;
use clap::Parser;
use nom::{
    branch::alt,
    bytes::complete::{tag, take},
    character::complete::multispace0,
    IResult,
};
use serde::{Deserialize, Serialize};

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

// returns the next character, or the subsequent characters if the first is a backslash
fn char_or_escaped_char(data: &str) -> IResult<&str, &str> {
    let (data, maybe_backslash) = take(1usize)(data)?;

    if data == "\\" {
        let (data, escaped_char) = take(1usize)(data)?;
        Ok((data, escaped_char))
    } else {
        Ok((data, maybe_backslash))
    }
}

#[derive(Debug, Clone)]
pub enum StringOrStr<'a> {
    String(String),
    Str(&'a str),
}
impl From<&str> for StringOrStr<'_> {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}
impl<'a> AsRef<str> for StringOrStr<'a> {
    fn as_ref(&self) -> &str {
        match self {
            Self::String(s) => s.as_ref(),
            Self::Str(s) => s,
        }
    }
}
impl StringOrStr<'_> {
    fn len(&self) -> usize {
        self.as_ref().len()
    }
}
impl PartialEq for StringOrStr<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for StringOrStr<'_> {}

// parse a quoted string, with escaped characters
fn quoted_string(data: &str) -> IResult<&str, StringOrStr> {
    // initial quote
    let (data, _) = tag("\"")(data)?;
    // char_or_escaped_char will return the next value.  Accumulate this until
    let mut head = data;
    let mut accum = String::new();
    loop {
        let (data, value) = char_or_escaped_char(head)?;
        head = data;
        if value == "\"" {
            break;
        }
        accum.push_str(value);
    }

    Ok((head, StringOrStr::String(accum)))
}

fn unquoted_string(data: &str) -> IResult<&str, StringOrStr> {
    let (data, value) = nom::bytes::complete::take_while(|c: char| !c.is_whitespace())(data)?;
    Ok((data, StringOrStr::Str(value)))
}

struct ParseMap<'a> {
    map: HashMap<String, StringOrStr<'a>>,
}
impl<'a> ParseMap<'a> {
    fn get(&self, key: &str) -> Result<StringOrStr<'a>> {
        if let Some(value) = self.map.get(key) {
            Ok(value.clone())
        } else {
            Err(anyhow::anyhow!("Key {} not found", key))
        }
    }

    #[cfg(test)]
    fn keys(&self) -> std::collections::hash_map::Keys<String, StringOrStr> {
        self.map.keys()
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.map.len()
    }
}

fn str_to_key_value<'a>(data: &str) -> IResult<&str, ParseMap> {
    let mut key_values = HashMap::new();

    let mut head = data;
    loop {
        // Check for empty
        if head.is_empty() {
            break;
        }
        // using nom, trim whitesapce
        let (data, _) = multispace0(head)?;
        // Check again just in case trailing whitespace
        if data.is_empty() {
            head = data;
            break;
        }
        // parse key, letters, numbers, underscores, dashes
        let (data, key) = nom::bytes::complete::take_while(|c: char| {
            c.is_ascii_alphanumeric() || c == '_' || c == '-'
        })(data)?;
        // parse =
        let (data, _) = tag("=")(data)?;
        // parse value, a quoted string or a non-quoted string with no whitespace
        let (data, value) = alt((quoted_string, unquoted_string))(data)?;
        // insert into map
        key_values.insert(key.to_string(), value);
        head = data;
    }

    Ok((head, ParseMap { map: key_values }))
}

trait Parse<'a> {
    fn parse(data: &'a ParseMap) -> Result<Self>
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
    pub fn parse<'a>(data: &'a str) -> Result<Command<'a>> {
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
        let key_values = str_to_key_value(data)
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
    fn parse(data: &'a ParseMap) -> Result<Self> {
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
    fn parse(data: &'a ParseMap) -> Result<Brightness<'a>> {
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
    fn parse(data: &'a ParseMap) -> Result<Self> {
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
    fn parse(data: &'a ParseMap) -> Result<Self> {
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
        let (_, key_values) = str_to_key_value(DATA).unwrap();
        let mut keys = key_values.keys().map(|s| s.to_owned()).collect::<Vec<_>>();
        keys.sort();

        assert_eq!(keys, vec!["BITMAP", "DEVICEID", "KEY", "PRESSED", "TYPE",]);
    }

    #[test]
    fn test_keyvalue_quoted_value() {
        const DATA: &str = "key=\"value\"";
        let (_, key_values) = str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_empty() {
        const DATA: &str = "  ";
        let (_, key_values) = str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 0);
    }

    #[test]
    fn test_keyvalue_parser_leading_space() {
        const DATA: &str = "  key=value";
        let (_, key_values) = str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_trailing_space() {
        const DATA: &str = "key=value  ";
        let (_, key_values) = str_to_key_value(DATA).unwrap();
        assert_eq!(key_values.len(), 1);
        assert_eq!(key_values.get("key").unwrap(), "value".into());
    }

    #[test]
    fn test_keyvalue_parser_multi_inbetween() {
        const DATA: &str = " key=value  foo=bar ";
        let (_, key_values) = str_to_key_value(DATA).unwrap();
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
