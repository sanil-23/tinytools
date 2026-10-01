//! The trait every agent capability implements.

mod types;

pub use types::{Tool, ToolExposure};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
