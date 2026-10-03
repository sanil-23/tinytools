//! `contains_call_markup`: telling text that is really a tool call apart from
//! an answer.

use crate::parse::contains_call_markup;

#[test]
fn glm_fallback_keeps_fenced_text_within_a_kept_range() {
    let input = "discard\nintro\n```text\nshell/command>example\n```\nshell/command>real";
    let start = input.find("intro").unwrap();
    let (text, calls) = super::super::parse_unprotected_glm(input, &[start..input.len()]);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments, serde_json::json!({"command": "real"}));
    assert!(text.starts_with("intro\n```text\nshell/command>example\n```"));
    assert!(!text.contains("discard"));
    assert!(!text.contains("shell/command>real"));
}

/// What `DeepSeek` V4 returned in place of a summary when its request declared
/// no tools (captured from the `OpenHuman` harness benchmark).
const DSML: &str = "<｜｜DSML｜｜ calls>\n<｜｜DSML｜｜ invoke name=\"shell\">\n\
<｜｜DSML｜｜ parameter name=\"command\" string=\"true\">cd /app && cat src/lib.rs</｜｜DSML｜｜ parameter>\n\
</｜｜DSML｜｜ invoke>\n</｜｜DSML｜｜ calls>";

#[test]
fn a_dsml_call_is_markup() {
    assert!(contains_call_markup(DSML));
    assert!(contains_call_markup(&format!(
        "Let me check first.\n\n{DSML}"
    )));
}

#[test]
fn tagged_and_invoke_calls_are_markup() {
    assert!(contains_call_markup(
        "<tool_call>{\"name\":\"shell\",\"arguments\":{\"command\":\"ls\"}}</tool_call>"
    ));
    assert!(contains_call_markup(
        "<invoke name=\"read\"><parameter name=\"path\">a.rs</parameter></invoke>"
    ));
}

#[test]
fn a_block_that_does_not_decode_still_counts() {
    // Opened and never closed: truncated mid-call.
    assert!(contains_call_markup(
        "<｜｜DSML｜｜ calls>\n<｜｜DSML｜｜ invoke name=\"shell\">\n<｜｜DSML｜｜ parameter name=\"command\""
    ));
}

#[test]
fn prose_a_bare_json_answer_and_a_quoted_example_are_not_markup() {
    assert!(!contains_call_markup(
        "## Goal\nShip the parser.\n\n## Active State\nConfig is {\"retries\": 3}."
    ));
    assert!(!contains_call_markup(
        "{\"name\": \"shell\", \"arguments\": {\"command\": \"ls\"}}"
    ));
    assert!(!contains_call_markup(
        "The format is:\n```xml\n<invoke name=\"shell\"><parameter name=\"command\">ls</parameter></invoke>\n```"
    ));
    assert!(!contains_call_markup("```text\nshell/command>ls\n```"));
    assert!(!contains_call_markup(""));
}
