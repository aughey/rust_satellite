pub use anyhow::Result;
use clap::Parser;
use nom::{branch::alt, bytes::complete::tag, character::complete::multispace0, IResult};
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

pub struct KeyState<'a> {
    pub device: &'a str,
    pub key: u8,
    pub button_type: &'a str,
    pub bitmap_base64: &'a str,
    pub pressed: bool
}
impl KeyState<'_> {
    pub fn bitmap(&self) -> Result<Vec<u8>> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD_NO_PAD.decode(self.bitmap_base64.as_bytes())?;
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

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
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
