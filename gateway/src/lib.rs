pub use anyhow::Result;

use clap::Parser;
pub use bin_comm::stream_utils;





#[derive(Parser)]
pub struct Cli {
    #[arg(long)]
    pub host: String,
    #[arg(short, long)]
    pub port: u16,
    #[arg(long)]
    pub listen_port: u16,
    #[arg(long)]
    #[clap(default_value = "0.0.0.0")]
    pub listen_address: String,
}



