//! # leaf
//! 
//! A leaf is a remote device that has an attached streamdeck.  The term "leaf"
//! is distinguished from the term "satellite" to indicate that the leaf will
//! connect to a gateway and the gateway will connect to the companion app.
//! 
//! In the companion terminology, a "satellite" application connects directly
//! to the companion app and communicates over an ascii protocol.  In contrast,
//! a leaf program is designed to use minimal resources and be poetntially
//! implemented on a microcontroller.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

pub use traits::Result;
use clap::Parser;

/// Command line options for a leaf program
#[derive(Parser)]
pub struct Cli {
    /// IP address of the gateway
    #[arg(long)]
    pub gateway_host: String,
    /// Port number of the gateway
    #[arg(short, long)]
    pub gateway_port: u16,
}
