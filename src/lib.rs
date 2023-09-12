pub use anyhow::Result;
use clap::Parser;
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

/// Parses a line into command and data fields
pub fn command_data(line: &str) -> Option<(&str, &str)> {
    let command = line.split(" ").next()?;
    let data = line.get((command.len() + 1)..)?;
    Some((command, data))
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
