//! Very small caching utility with async locking support.
//!
//! All interaction in this crate will be done through the `Cache` type,
//! so please see the the `cache` module for further instructions.
#![doc(html_root_url = "https://docs.rs/retainer/0.2.2")]

// exposed modules
pub mod cache;
pub mod entry;

// lifted types to the top level
pub use crate::cache::Cache;
pub use crate::entry::CacheEntry;
pub use crate::entry::CacheExpiration;
