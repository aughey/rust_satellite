//! # rust_satellite
//!
//! This is a complete application that is a rust implementation of the
//! companion satellite protocol.  It is intended to be used on a device
//! that has an attached Streamdeck and connects directly to the companion
//! app.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use anyhow::Result;
use clap::Parser;

/// Command line argument for the satellite program
#[derive(Parser)]
pub struct Cli {
    /// hostname of the companion app
    #[arg(long)]
    pub companion_host: String,
    /// port number of the companion app (usually 16622)
    #[arg(short, long)]
    pub companion_port: u16,
}
