#![doc = include_str!("../README.md")]

// exposed modules
pub mod cache;
pub mod entry;

// lifted types to the top level
pub use crate::cache::Cache;
pub use crate::entry::CacheExpiration;
