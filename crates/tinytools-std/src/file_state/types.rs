//! Core types for the file state coordinator.

use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use tokio::sync::Mutex;

/// Snapshot of a single file read by an agent.
#[derive(Debug, Clone)]
pub struct ReadStamp {
    /// Filesystem mtime at the moment of the read.
    pub mtime: SystemTime,
    /// Monotonic clock timestamp taken just before the read's I/O began, so
    /// any write that lands during the read orders after it.
    pub timestamp: Instant,
    /// Whether the read was partial (paginated / offset+limit).
    pub partial: bool,
}

/// Per-path write metadata.
#[derive(Debug, Clone)]
pub(crate) struct WriteStamp {
    /// Agent identity that performed the write.
    pub writer: String,
    /// Monotonic clock timestamp of the write.
    pub timestamp: Instant,
}

/// Process-global coordinator that tracks file reads and writes across
/// all agents in the process. Thread-safe via `RwLock`.
#[derive(Debug)]
pub struct FileStateCoordinator {
    /// Per-agent, per-resolved-path read stamps.
    /// Key: `(agent_id, canonical_path)`.
    pub(crate) reads: RwLock<HashMap<(String, PathBuf), ReadStamp>>,

    /// Per-resolved-path write stamp (last writer wins). Staleness checks
    /// compare against this.
    pub(crate) writes: RwLock<HashMap<PathBuf, WriteStamp>>,

    /// Every resolved path each agent has ever written. Unlike `writes`, a
    /// later write by another agent does not erase an earlier writer, so
    /// [`FileStateCoordinator::paths_written_by`] can still attribute it.
    pub(crate) written_paths: RwLock<HashMap<String, HashSet<PathBuf>>>,

    /// Per-resolved-path async mutex for serialising read-modify-write
    /// sections (used by `edit` and `apply_patch`).
    pub(crate) path_locks: RwLock<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl Default for FileStateCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl FileStateCoordinator {
    #[must_use]
    /// Create an empty coordinator.
    pub fn new() -> Self {
        Self {
            reads: RwLock::new(HashMap::new()),
            writes: RwLock::new(HashMap::new()),
            written_paths: RwLock::new(HashMap::new()),
            path_locks: RwLock::new(HashMap::new()),
        }
    }

    /// Return the set of resolved paths that `parent_agent_id` has read
    /// but were subsequently written by a different agent.
    pub fn stale_reads_for_parent(&self, parent_agent_id: &str) -> Vec<PathBuf> {
        let reads = self.reads.read();
        let writes = self.writes.read();
        let mut stale = Vec::new();
        for ((agent_id, path), read_stamp) in reads.iter() {
            if agent_id != parent_agent_id {
                continue;
            }
            if let Some(ws) = writes.get(path)
                && ws.writer != parent_agent_id
                && ws.timestamp > read_stamp.timestamp
            {
                stale.push(path.clone());
            }
        }
        stale.sort();
        stale.dedup();
        stale
    }

    /// Collect every path written by each agent in `agent_ids`, keyed by
    /// agent. A path appears under every listed agent that ever wrote it,
    /// even when a different agent wrote it afterwards. Paths are sorted;
    /// agents with no writes are absent.
    #[must_use]
    pub fn paths_written_by(&self, agent_ids: &[String]) -> HashMap<String, Vec<PathBuf>> {
        let written_paths = self.written_paths.read();
        agent_ids
            .iter()
            .filter_map(|agent_id| {
                let paths = written_paths.get(agent_id)?;
                let mut paths: Vec<PathBuf> = paths.iter().cloned().collect();
                paths.sort();
                Some((agent_id.clone(), paths))
            })
            .collect()
    }
}
