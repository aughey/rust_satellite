//! # Traits
//! 
//! This crate contains the traits that are used by the companion and device crates.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

/// re-export anyhow
pub use anyhow;
/// re-export anyhow::Result
pub use anyhow::Result;
/// re-export the async_trait
pub use async_trait::async_trait;
/// export the companion interface
pub mod companion;
/// export the device interface
pub mod device;
