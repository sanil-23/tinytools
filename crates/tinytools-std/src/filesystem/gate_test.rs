//! Tests for how a call's workspace descriptor scopes the gate.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use super::gate::{FsGate, gate_for_context};
use super::test_support::{TestGate, WorkspaceContext};

#[tokio::test]
async fn without_a_descriptor_the_same_gate_is_used() {
    let dir = tempfile::tempdir().unwrap();
    let gate: Arc<dyn FsGate> = TestGate::supervised(dir.path().to_path_buf());
    let resolved = gate_for_context(&gate, None, "file_read");
    assert!(Arc::ptr_eq(&gate, &resolved));
}

#[tokio::test]
async fn a_descriptor_roots_the_gate_at_the_isolated_workspace() {
    let home = tempfile::tempdir().unwrap();
    let isolated = tempfile::tempdir().unwrap();
    std::fs::write(isolated.path().join("seen.txt"), "x").unwrap();
    let gate: Arc<dyn FsGate> = TestGate::supervised(home.path().to_path_buf());

    let context = WorkspaceContext::at(isolated.path());
    let scoped = gate_for_context(&gate, Some(&context), "file_read");

    assert_eq!(scoped.action_dir(), isolated.path());
    let resolved = scoped.validate_path("seen.txt").await.unwrap();
    assert_eq!(
        resolved,
        isolated.path().join("seen.txt").canonicalize().unwrap()
    );
    // The original gate keeps its own root and refuses the isolated file.
    assert!(gate.validate_path("seen.txt").await.is_err());
}
