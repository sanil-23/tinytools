//! Host-independent building blocks for agent tools.
//!
//! `tinytools` is the vocabulary a tool is written against and deliberately
//! carries no behavior. This crate holds the small, reusable mechanisms that
//! several hosts' tools share, none of which encodes a host's policy:
//!
//! - [`file_state`] — cross-agent read/write stamps and per-path locks, so a
//!   sibling agent's edit is noticed before a stale overwrite.
//! - [`url_guard`] — URL validation with SSRF and DNS-rebinding checks.
//! - [`detect_tools`] — `PATH` probing and the read-only `detect_tools` tool.

pub mod detect_tools;
pub mod file_state;
pub mod url_guard;
