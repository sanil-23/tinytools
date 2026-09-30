#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::*;

#[test]
fn name_and_permission() {
    let tool = DetectToolsTool::new();
    assert_eq!(tool.name(), "detect_tools");
    assert_eq!(tool.permission_level(), PermissionLevel::ReadOnly);
}

#[tokio::test]
async fn missing_tool_reported_missing() {
    let tool = DetectToolsTool::new();
    let result = tool
        .execute(json!({ "tools": ["definitely_not_a_real_binary_xyz_123"] }))
        .await
        .unwrap();
    assert!(!result.is_error);
    let payload: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
    assert_eq!(payload["probed"], 1);
    assert_eq!(payload["available"].as_array().unwrap().len(), 0);
    assert_eq!(
        payload["missing"].as_array().unwrap()[0],
        "definitely_not_a_real_binary_xyz_123"
    );
}

#[tokio::test]
async fn available_plus_missing_equals_probed() {
    let tool = DetectToolsTool::new();
    let result = tool
        .execute(json!({ "tools": ["sh", "definitely_not_a_real_binary_xyz_123"] }))
        .await
        .unwrap();
    let payload: serde_json::Value = serde_json::from_str(&result.output()).unwrap();
    let avail = payload["available"].as_array().unwrap().len();
    let miss = payload["missing"].as_array().unwrap().len();
    assert_eq!(avail + miss, 2);
}

#[test]
fn bare_name_is_probed_unchanged_without_pathext() {
    assert_eq!(candidate_file_names("git", None), vec!["git".to_string()]);
}

#[test]
fn extensionless_name_gets_each_pathext_extension() {
    assert_eq!(
        candidate_file_names("git", Some(".EXE;.CMD")),
        vec!["git.EXE".to_string(), "git.CMD".to_string()]
    );
}

#[test]
fn name_already_carrying_a_pathext_extension_is_probed_unchanged() {
    // Windows must probe `git.exe`, not `git.exe.EXE`.
    assert_eq!(
        candidate_file_names("git.exe", Some(".EXE;.CMD;.BAT")),
        vec!["git.exe".to_string()]
    );
    assert_eq!(
        candidate_file_names("build.Cmd", Some(".EXE;.CMD")),
        vec!["build.Cmd".to_string()]
    );
}

#[test]
fn name_with_a_non_pathext_extension_still_gets_pathext_appended() {
    assert_eq!(
        candidate_file_names("python3.11", Some(".EXE")),
        vec!["python3.11.EXE".to_string()]
    );
}

#[test]
fn empty_pathext_entries_are_ignored() {
    assert_eq!(
        candidate_file_names("git", Some(".EXE;;")),
        vec!["git.EXE".to_string()]
    );
    assert_eq!(
        candidate_file_names("git", Some("")),
        vec!["git".to_string()]
    );
}
