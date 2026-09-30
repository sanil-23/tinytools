# tinytools-std

Host-independent building blocks for agent tools, extracted from OpenHuman.

| Module | What it is |
| --- | --- |
| `file_state` | Process-wide read/write stamps so parallel agents detect stale or partial reads before overwriting a file. The host decides whether the guard is on (`init_global(enabled)`). |
| `url_guard` | URL validation with SSRF and DNS-rebinding checks for outbound network tools. |
| `filesystem` | The file and repository tools: `file_read`, `file_write`, `edit_file`, `apply_patch`, `grep`, `glob`, `list_files`, `csv_export`, `read_diff`, `git_operations`, `run_linter`, `run_tests`, `update_memory_md`. Every tool that can touch a path takes an `Arc<dyn FsGate>`: the tool does the I/O, the host's gate (autonomy, workspace boundary, approvals, action budget) decides whether it may. |
| `detect_tools` | `find_on_path` and the read-only `detect_tools` tool. |

No enforcement of host policy lives here; the crate only supplies mechanisms. The `filesystem` tools' name, description and JSON Schema are pinned by the fixtures in `src/filesystem/fixtures/`.
