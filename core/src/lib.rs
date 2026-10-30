//! Arogya Swasthya core: the encrypted record store and its keys. No UI; the
//! Tauri shell and tests drive it.

pub mod ccda;
pub mod fhir;
pub mod files;
pub mod import;
pub mod keys;
pub mod shl;
pub mod store;

pub use store::{Error, Result, Store};
