//! # Gateway
//! The gateway is a TCP server that accepts connections from leaf satellite devices
//! and connects to a companion app.  The gateway forwards commands from the companion
//! app to the satellite device and forwards commands from the satellite device to the
//! companion app.
//! 
//! The protocol between the gateway and the companion app is a binary protocol with the
//! images encoded in the format expected by the leaf device.  This allows the leaf device
//! to be a simple device that just forwards the commands to the connected Streamdeck hardware.
//! 
//! The intent of the gateway is to allow the leaf satellite devices to be trivially implemented
//! and could be implmeneted on a microcontroller with a TCP stack and a USB connection to the
//! Streamdeck hardware.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use traits::Result;
use clap::Parser;

/// The command line arguments for the gateway
#[derive(Parser)]
pub struct Cli {
    /// The host to connect to for the companion app
    #[arg(long)]
    pub companion_host: String,
    /// The port to connect to for the companion app
    #[arg(short, long)]
    pub companion_port: u16,
    /// The port to listen on for leaf satellite connections
    #[arg(long)]
    pub listen_port: u16,
    /// Address to listen on for leaf satellite connections
    #[arg(long)]
    #[clap(default_value = "0.0.0.0")]
    pub listen_address: String,
}
