//! The host's say over every network tool call.
//!
//! The tools in this module reach the internet, but they do not decide whether
//! they are *allowed* to. Whether the agent may act at all, how many actions it
//! has left this hour, whether an outbound request needs a human's approval,
//! whether a privacy mode forbids leaving the device, and how the process-wide
//! proxy is applied to a client all belong to the host whose threat model and
//! configuration they depend on. A tool asks a [`NetGate`] instead.
//!
//! Like [`crate::filesystem::FsGate`], the trait carries no decision *types* of
//! its own, only booleans, strings and the `reqwest` builder the tool was
//! already holding, so a host maps its own policy onto it without translating
//! a vocabulary.

/// The host policy a network tool consults before it acts.
///
/// Implementations must be cheap to call: the tools ask on every invocation.
/// All methods take `&self`; an implementation that counts actions keeps that
/// state behind interior mutability.
pub trait NetGate: std::fmt::Debug + Send + Sync {
    /// Whether the host permits the agent to act at all right now.
    ///
    /// `false` blocks the tool before it touches the network.
    fn can_act(&self) -> bool;

    /// Whether the action budget is already spent, without consuming any of it.
    fn is_rate_limited(&self) -> bool;

    /// Consume one unit of the action budget.
    ///
    /// Returns `false` when the call is over budget and must be refused.
    fn record_action(&self) -> bool;

    /// Whether an outbound request must be confirmed by a human before it runs.
    ///
    /// Drives the tools' `external_effect_with_args` flag, which routes the
    /// call through the host's approval flow.
    fn network_needs_approval(&self) -> bool;

    /// The refusal message when the host's privacy mode forbids contacting
    /// `host`, or `None` when the request may proceed.
    ///
    /// Asked before URL validation and DNS, so a refused request leaks nothing,
    /// not even a lookup.
    fn local_only_block(&self, host: &str) -> Option<String>;

    /// Tell the host that a request is about to leave the device for `host`.
    ///
    /// Observe-only: called after enforcement has already let the request
    /// through. `has_body` is true when a request body rides along and
    /// `has_headers` when caller-supplied headers do.
    fn disclose(&self, host: &str, has_body: bool, has_headers: bool);

    /// Apply the host's process-wide proxy settings for `service` to a client
    /// builder the tool has already configured.
    fn prepare_client(
        &self,
        service: &str,
        builder: reqwest::ClientBuilder,
    ) -> reqwest::ClientBuilder;

    /// A ready-made client for `service` with the given total and connect
    /// timeouts, proxied and TLS-configured the way the host does it.
    fn timeout_client(
        &self,
        service: &str,
        timeout_secs: u64,
        connect_timeout_secs: u64,
    ) -> reqwest::Client;
}

/// The host of `url`, or `"unknown"` when it does not parse or has none.
///
/// The value handed to [`NetGate::local_only_block`] and
/// [`NetGate::disclose`].
pub(crate) fn host_of(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Response-size and timeout limits a tool falls back to when it is
/// constructed with a `0` (or absent) value.
///
/// A `0` byte cap truncates every body to nothing and a `0` second timeout
/// fails every request instantly, so neither is ever a meaningful limit; it
/// only shows up when a stale configuration slips through. The host owns what
/// the sensible default is and passes it in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpLimits {
    /// Response-body cap, in bytes.
    pub max_response_size: usize,
    /// Whole-request timeout, in seconds.
    pub timeout_secs: u64,
}
