//! Which registered tools leave the wire: the host's half of
//! [`ToolExposure::Deferred`].
//!
//! # Why this exists
//!
//! Tool schemas are a fixed cost paid on every request, and most of that cost
//! is tools the model reaches for on a handful of turns a week.
//! [`ToolExposure::Deferred`] takes such a tool off the wire without removing
//! the capability: the harness advertises a search bridge (see
//! [`rank`](crate::rank)) in its place and the tool stays callable. This module
//! is the small piece a host needs to decide *what* is deferred: it subtracts
//! deferred and hidden tools from an advertised set and reports the deferred
//! names so the caller can keep them registered.
//!
//! Deferral is per-tool and is a property of the tool. It only ever subtracts
//! from a set the host and its security policy already decided, so it can never
//! widen the surface.

use std::collections::HashSet;

use crate::{Tool, ToolExposure};

/// The name an agent lists in `[tools] named` to opt a hand-written belt into
/// discovery. It is not a registered tool: the harness advertises its own
/// intrinsic `tool_search` whenever a run has a deferred tool, so the name in
/// a belt is a request for that, and the builder strips it from the allowlist.
pub const TOOL_SEARCH_NAME: &str = "tool_search";

/// Remove every [`ToolExposure::Deferred`] and [`ToolExposure::Hidden`] tool
/// from an agent's advertised set, returning the names of the deferred ones so
/// the caller can keep them registered.
///
/// Hidden tools are dropped and **not** returned: they are not searchable
/// either, by definition.
#[must_use]
pub fn strip_deferred_from_visible<S: std::hash::BuildHasher>(
    visible: &mut HashSet<String, S>,
    tools: &[Box<dyn Tool>],
) -> HashSet<String> {
    let mut deferred = HashSet::new();
    for tool in tools {
        let name = tool.name();
        if !visible.contains(name) {
            continue;
        }
        match tool.exposure() {
            ToolExposure::Direct => {}
            ToolExposure::Deferred => {
                visible.remove(name);
                deferred.insert(name.to_string());
            }
            ToolExposure::Hidden => {
                visible.remove(name);
            }
        }
    }
    deferred
}

/// Every [`ToolExposure::Deferred`] tool in `tools`, by name — the catalogue a
/// belt that opted into discovery can reach whether or not it named them.
#[must_use]
pub fn deferred_tool_names(tools: &[Box<dyn Tool>]) -> HashSet<String> {
    tools
        .iter()
        .filter(|tool| tool.exposure() == ToolExposure::Deferred)
        .map(|tool| tool.name().to_string())
        .collect()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod test;
