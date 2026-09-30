//! Filesystem tools: read/write/edit/patch, grep/glob/list, diffs, CSV export,
//! git operations, linter and test runners, and the workspace memory index.
//!
//! Every tool that can touch a path takes an `Arc<dyn FsGate>`: the tool
//! performs the I/O, the host's gate decides whether it is allowed. The gate
//! trait ([`FsGate`]) is the whole seam; nothing here knows what an autonomy
//! level, an approval prompt or a workspace boundary is.

mod apply_patch;
mod csv_export;
mod edit_file;
mod file_read;
mod file_sink;
mod file_write;
mod gate;
mod git_operations;
mod glob_search;
mod grep;
mod list_files;
mod read_diff;
mod run_linter;
mod run_tests;
mod text;
mod update_memory_md;

#[cfg(test)]
mod test_support;

pub use apply_patch::ApplyPatchTool;
pub use csv_export::CsvExportTool;
pub use edit_file::EditFileTool;
pub use file_read::FileReadTool;
pub use file_write::FileWriteTool;
pub use gate::FsGate;
pub use git_operations::{GitOperationsTool, shell_git_env};
pub use glob_search::GlobTool;
pub use grep::GrepTool;
pub use list_files::ListFilesTool;
pub use read_diff::ReadDiffTool;
pub use run_linter::RunLinterTool;
pub use run_tests::RunTestsTool;
pub use update_memory_md::UpdateMemoryMdTool;
