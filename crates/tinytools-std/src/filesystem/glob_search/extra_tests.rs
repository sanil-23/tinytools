//! Budget, root-validation, truncation and policy-filter tests for `glob`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::*;
use crate::filesystem::test_support::{TestGate, WorkspaceContext, WrapGate};
use tempfile::TempDir;

#[test]
fn glob_is_concurrency_safe() {
    let dir = TempDir::new().unwrap();
    let tool = GlobTool::new(TestGate::supervised(dir.path().to_path_buf()));
    assert!(tool.is_concurrency_safe(&json!({})));
}

#[tokio::test]
async fn rate_limit_and_exhausted_budget_are_refused() {
    let dir = TempDir::new().unwrap();
    let limited = GlobTool::new(TestGate::with(
        dir.path().to_path_buf(),
        crate::filesystem::test_support::AutonomyLevel::Supervised,
        0,
    ));
    let r = limited.execute(json!({"pattern": "*"})).await.unwrap();
    assert!(r.output().contains("too many actions in the last hour"));

    let gate = WrapGate::new(TestGate::supervised(dir.path().to_path_buf()))
        .no_budget()
        .arc();
    let r = GlobTool::new(gate)
        .execute(json!({"pattern": "*"}))
        .await
        .unwrap();
    assert!(r.output().contains("action budget exhausted"));
}

#[tokio::test]
async fn a_root_that_vanishes_after_validation_is_reported() {
    let dir = TempDir::new().unwrap();
    let gate = WrapGate::new(TestGate::supervised(dir.path().to_path_buf()))
        .fixed_path(dir.path().join("gone"))
        .arc();
    let r = GlobTool::new(gate)
        .execute(json!({"pattern": "*"}))
        .await
        .unwrap();
    assert!(r.is_error);
    assert!(r.output().contains("is not accessible"));
}

#[tokio::test]
async fn a_missing_action_dir_falls_back_to_its_raw_path() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    let gate = WrapGate::new(TestGate::supervised(dir.path().to_path_buf()))
        .action_dir_override(dir.path().join("does-not-exist"))
        .fixed_path(dir.path().to_path_buf())
        .arc();
    let r = GlobTool::new(gate)
        .execute(json!({"pattern": "*.txt"}))
        .await
        .unwrap();
    assert!(!r.is_error, "{}", r.output());
    assert!(r.output().contains("a.txt"));
}

#[tokio::test]
async fn results_report_truncation() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    std::fs::write(dir.path().join("b.txt"), "").unwrap();
    let tool = GlobTool::new(TestGate::supervised(dir.path().to_path_buf()));
    let ctx = WorkspaceContext::at(dir.path());
    let r = tool
        .execute_with_context(
            json!({"pattern": "*.txt", "max_results": 1}),
            ToolCallOptions::default(),
            Some(&ctx),
        )
        .await
        .unwrap();
    assert!(
        r.output().starts_with("1 match(es) (truncated at 1)"),
        "{}",
        r.output()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn hits_the_gate_refuses_are_filtered_out() {
    let dir = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "s").unwrap();
    std::fs::write(dir.path().join("ok.txt"), "").unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        dir.path().join("link.txt"),
    )
    .unwrap();
    let tool = GlobTool::new(TestGate::supervised(dir.path().to_path_buf()));
    let r = tool.execute(json!({"pattern": "*.txt"})).await.unwrap();
    assert!(r.output().contains("ok.txt"));
    assert!(!r.output().contains("link.txt"), "{}", r.output());
}
