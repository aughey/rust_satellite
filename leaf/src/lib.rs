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