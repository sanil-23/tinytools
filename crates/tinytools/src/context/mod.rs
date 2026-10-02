//! The run-scoped seam between a tool and the harness driving it.

mod types;

pub use types::ToolRunContext;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
