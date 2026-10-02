//! Where a tool is allowed to operate.

mod types;

pub use types::{SandboxMode, WorkspaceDescriptor};

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
