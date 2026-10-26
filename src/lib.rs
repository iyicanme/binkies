//! Parser and exporters for the BINKIES-MDM 3D model format (see `docs/MDM_FORMAT.md`).

pub mod container;
pub mod geometry;
pub mod obj;
mod reader;
pub mod scene;
#[cfg(target_arch = "wasm32")]
mod wasm;
pub mod zip;

pub use reader::{Error, Result};
