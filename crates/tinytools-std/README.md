# tinytools-std

Host-independent building blocks for agent tools, extracted from OpenHuman.

| Module | What it is |
| --- | --- |
| `file_state` | Process-wide read/write stamps so parallel agents detect stale or partial reads before overwriting a file. The host decides whether the guard is on (`init_global(enabled)`). |
| `url_guard` | URL validation with SSRF checks; DNS validation returns addresses callers must pin for the connection. |
| `detect_tools` | `find_on_path` and the read-only `detect_tools` tool. |

No enforcement of host policy lives here; the crate only supplies mechanisms.
