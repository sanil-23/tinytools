//! Availability and belt classification for a tool.

mod types;

pub use types::{ToolCategory, ToolScope};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
