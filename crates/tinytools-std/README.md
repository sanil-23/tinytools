# tinytools-std

Host-independent building blocks for agent tools, extracted from OpenHuman.

| Module | What it is |
| --- | --- |
| `file_state` | Process-wide read/write stamps so parallel agents detect stale or partial reads before overwriting a file. The host decides whether the guard is on (`init_global(enabled)`); read tools pass `record_read` an `Instant` captured before their I/O. |
| `url_guard` | URL validation with SSRF checks for outbound network tools. `validate_url_with_dns_check` returns a `ValidatedUrl` whose vetted `addrs` the caller must pin its HTTP client to (e.g. `reqwest`'s `resolve_to_addrs`); re-resolving the hostname reopens DNS rebinding. |
| `detect_tools` | `find_on_path` and the read-only `detect_tools` tool. |

No enforcement of host policy lives here; the crate only supplies mechanisms.
