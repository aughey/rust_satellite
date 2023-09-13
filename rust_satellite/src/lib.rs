use std::collections::HashMap;

pub use anyhow::Result;
use clap::Parser;
use nom::{
    branch::alt,
    bytes::complete::{tag, take, take_while1},
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

enum StringOrStr<'a> {
    String(String),
    Str(&'a str),
}

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

fn str_to_key_value(data: &str) -> IResult<&str, HashMap<String, StringOrStr>> {
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

    Ok((head, key_values))
}

trait Parse<'a> {
    fn parse(data: &'a str) -> IResult<&str, Self>
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
impl<'a> Command<'a> {
    pub fn parse(data: &'a str) -> Result<Self> {
        let command = data
            .split(" ")
            .next()
            .ok_or_else(|| anyhow::anyhow!("No command"))?;
        let data = data
            .get((command.len() + 1)..)
            .ok_or_else(|| anyhow::anyhow!("No data found")); // There might not be data, that's ok
        let res = match command {
            "PONG" => Self::Pong,
            "BEGIN" => Self::Begin(iresult_unwrap(Versions::parse(data?))?),
            "KEY-STATE" => Self::KeyState(iresult_unwrap(KeyState::parse(data?))?),
            "ADD-DEVICE" => Self::AddDevice(iresult_unwrap(AddDevice::parse(data?))?),
            "BRIGHTNESS" => Self::Brightness(iresult_unwrap(Brightness::parse(data?))?),
            _ => Self::Unknown(command),
        };
        Ok(res)
    }
}

#[derive(PartialEq, Eq)]
pub struct KeyState<'a> {
    pub device: &'a str,
    pub key: u8,
    pub button_type: &'a str,
    pub bitmap_base64: &'a str,
    pub pressed: bool,
}
impl KeyState<'_> {
    pub fn bitmap(&self) -> Result<Vec<u8>> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(self.bitmap_base64.as_bytes())?;
        Ok(data)
    }
}
impl<'a> Parse<'a> for KeyState<'a> {
    fn parse(data: &'a str) -> IResult<&str, Self> {
        // String looks like
        // DEVICEID=JohnAughey KEY=14 TYPE=BUTTON  BITMAP=rawdata PRESSED={true,false}
        let (data, _) = tag("DEVICEID=")(data)?;
        let (data, device) = nom::bytes::complete::take_until(" ")(data)?;
        let (data, _) = tag(" KEY=")(data)?;
        let (data, key) = nom::bytes::complete::take_until(" ")(data)?;

        let (data, _) = tag(" TYPE=")(data)?;
        let (data, button_type) = nom::bytes::complete::take_until(" ")(data)?;
        let (data, _) = multispace0(data)?;
        let (data, _) = tag("BITMAP=")(data)?;
        let (data, bitmap_base64) = nom::bytes::complete::take_until(" ")(data)?;
        let (data, _) = tag(" PRESSED=")(data)?;
        let (data, pressed) = nom::bytes::complete::take_till(|_| false)(data)?;
        let pressed = pressed == "true";

        let key = key.parse().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(data, nom::error::ErrorKind::Digit))
        })?;

        Ok((
            data,
            Self {
                device,
                key,
                button_type,
                bitmap_base64,
                pressed,
            },
        ))
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
    pub device: &'a str,
    pub brightness: u8,
}
impl<'a> Parse<'a> for Brightness<'a> {
    fn parse(data: &'a str) -> IResult<&str, Self> {
        // String looks like:
        // DEVICEID=JohnAughey VALUE=100
        let (data, _) = tag("DEVICEID=")(data)?;
        let (data, device) = nom::bytes::complete::take_until(" ")(data)?;
        let (data, _) = tag(" VALUE=")(data)?;
        let (data, brightness) = nom::bytes::complete::take_till(|_| false)(data)?;

        let brightness = brightness.parse().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(data, nom::error::ErrorKind::Digit))
        })?;
        Ok((data, Self { brightness, device }))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AddDevice<'a> {
    pub success: bool,
    pub device_id: &'a str,
}
impl<'a> Parse<'a> for AddDevice<'a> {
    fn parse(data: &'a str) -> IResult<&str, Self> {
        // String looks like:
        // OK DEVICEID="value"
        // or
        // ERROR
        // parse either "OK " or "ERROR "
        let (data, success) = alt((tag("OK "), tag("ERROR ")))(data)?;
        let (data, _) = tag("DEVICEID=\"")(data)?;
        let (data, device_id) = nom::bytes::complete::take_till(|c| c == '"')(data)?;
        let (data, _) = tag("\"")(data)?;
        Ok((
            data,
            Self {
                success: success == "OK ",
                device_id,
            },
        ))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Versions<'a> {
    pub companion_version: &'a str,
    pub api_version: &'a str,
}
impl<'a> Parse<'a> for Versions<'a> {
    fn parse(data: &'a str) -> IResult<&str, Self> {
        // String looks like:
        // CompanionVersion=3.99.0+6259-develop-a48ec073 ApiVersion=1.5.1

        // use nom to parse
        let (data, _) = tag("CompanionVersion=")(data)?;
        let (data, companion_version) = nom::bytes::complete::take_until(" ")(data)?;
        let (data, _) = tag(" ApiVersion=")(data)?;
        let (data, api_version) = nom::bytes::complete::take_till(|_| false)(data)?;

        IResult::Ok((
            data,
            Self {
                companion_version,
                api_version,
            },
        ))
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
                companion_version: "3.99.0+6259-develop-a48ec073",
                api_version: "1.5.1"
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
                device_id: "JohnAughey"
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
                device: "JohnAughey",
                key: 14,
                button_type: "BUTTON",
                bitmap_base64: "rawdata",
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
}
