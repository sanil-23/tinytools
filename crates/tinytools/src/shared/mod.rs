//! Stub.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::result::ToolResult;
use crate::tool::Tool;

/// Stub.
pub struct SharedTool(Arc<dyn Tool>);

impl std::fmt::Debug for SharedTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SharedTool").finish()
    }
}

impl SharedTool {
    /// Stub.
    #[must_use]
    pub fn new(inner: Arc<dyn Tool>) -> Self {
        Self(inner)
    }
}

/// Stub.
#[must_use]
pub fn owned_belt(_shared: &[Arc<dyn Tool>]) -> Vec<Box<dyn Tool>> {
    Vec::new()
}

/// Stub.
#[must_use]
pub fn share_belt(_belt: Vec<Box<dyn Tool>>) -> Vec<Arc<dyn Tool>> {
    Vec::new()
}

#[async_trait]
impl Tool for SharedTool {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn description(&self) -> &str {
        self.0.description()
    }

    fn parameters_schema(&self) -> Value {
        self.0.parameters_schema()
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        self.0.execute(args).await
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
