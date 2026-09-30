//! Types for the action-collapse building blocks.

use std::fmt;

use crate::Tool;

/// One member of a collapsed family: the action name the model passes, and the
/// tool that serves it.
#[derive(Clone, Copy)]
pub struct CollapsedAction<'a> {
    /// The `action` value the model passes to select this member.
    pub action: &'static str,
    /// The member tool that serves the action.
    pub tool: &'a dyn Tool,
}

impl fmt::Debug for CollapsedAction<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CollapsedAction")
            .field("action", &self.action)
            .field("tool", &self.tool.name())
            .finish()
    }
}

/// Why a set of [`CollapsedAction`]s cannot be served as one tool.
///
/// Returned by [`super::validate_actions`], which a host calls once when it
/// builds the collapsed tool rather than on every request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CollapseError {
    /// No members were given, so there is no action to dispatch to.
    Empty,
    /// Two members answer to the same action name; dispatch could only ever
    /// reach the first.
    DuplicateAction {
        /// The action name declared more than once.
        action: String,
    },
    /// A member declares a parameter named `action`, the key the collapsed
    /// tool reserves for dispatch. The member could never receive it, because
    /// [`super::args_without_action`] strips it before forwarding.
    ReservedProperty {
        /// The action whose member declares the reserved property.
        action: String,
    },
}

impl fmt::Display for CollapseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("a collapsed tool needs at least one action"),
            Self::DuplicateAction { action } => {
                write!(f, "action '{action}' is declared more than once")
            }
            Self::ReservedProperty { action } => write!(
                f,
                "the member serving '{action}' declares a parameter named `action`, \
                 which is reserved for dispatch"
            ),
        }
    }
}

impl std::error::Error for CollapseError {}
