//! Arogya Swasthya core: the encrypted record store and its keys. No UI; the
//! Tauri shell and tests drive it.

pub mod fhir;
pub mod files;
pub mod keys;
pub mod store;

pub use store::{Error, Result, Store};
