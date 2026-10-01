//! Host-reported regressions: parser behaviors an embedding host pinned with
//! its own tests before the parsers lived here.

use super::parse;
use crate::types::CallSource;

#[test]
fn very_large_arguments_still_parse() {
    let large_arg = "x".repeat(100_000);
    let response = format!(
        r#"<tool_call>{{"name":"echo","arguments":{{"message":"{large_arg}"}}}}</tool_call>"#
    );
    let (_text, calls) = parse(&response);
    assert_eq!(calls.len(), 1, "large arguments should still parse");
    assert_eq!(calls[0].name, "echo");
}

#[test]
fn special_characters_in_arguments_survive() {
    let response = r#"<tool_call>{"name":"echo","arguments":{"message":"hello \"world\" <>&'\n\t"}}</tool_call>"#;
    let (_text, calls) = parse(response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "echo");
    assert_eq!(calls[0].arguments["message"], "hello \"world\" <>&'\n\t");
}

#[test]
fn cross_alias_closing_tags_are_recovered() {
    let response =
        "<toolcall>\n{\"name\": \"shell\", \"arguments\": {\"command\": \"date\"}}\n</tool_call>";
    let (text, calls) = parse(response);
    assert_eq!(text.len(), 0);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "shell");
}

#[test]
fn raw_tool_json_without_a_wrapper_is_not_a_call() {
    // SECURITY: JSON that merely resembles a call, with no wrapper, must not
    // execute; otherwise injected content could mimic a tool call.
    let response = "Sure, creating the file now.\n{\"name\": \"file_write\", \"arguments\": {\"path\": \"hello.py\", \"content\": \"print('hello')\"}}";
    let (text, calls) = parse(response);
    assert!(text.contains("Sure, creating the file now."));
    assert!(calls.is_empty(), "raw JSON without wrappers must not parse");
}

#[test]
fn an_empty_tool_result_block_is_not_a_call() {
    let response = "I'll run that command.\n<tool_result name=\"shell\">\n\n</tool_result>\nDone.";
    let (text, calls) = parse(response);
    assert!(text.contains("Done."));
    assert_eq!(calls.len(), 0);
}

#[test]
fn an_empty_tool_calls_array_is_returned_as_text() {
    let response = r#"{"content": "Hello", "tool_calls": []}"#;
    let (text, calls) = parse(response);
    assert!(text.contains("Hello"));
    assert_eq!(calls.len(), 0);
}

#[test]
fn invoke_tag_with_a_json_body_parses_like_tool_call() {
    let input = "Some text\n<invoke>{\"name\":\"echo\",\"arguments\":{\"value\":\"hi\"}}</invoke>\ntrailing";
    let (text, calls) = parse(input);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "echo");
    assert_eq!(calls[0].arguments, serde_json::json!({"value": "hi"}));
    assert!(text.contains("Some text"));
    assert!(text.contains("trailing"));
}

#[test]
fn invoke_attribute_form_does_not_leak_markup() {
    let input =
        "Sure.\n<invoke name=\"echo\">\n<parameter name=\"value\">hi</parameter>\n</invoke>\ndone";
    let (text, calls) = parse(input);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].arguments, serde_json::json!({"value": "hi"}));
    assert!(text.contains("Sure.") && text.contains("done"));
    assert!(!text.contains("<invoke") && !text.contains("<parameter"));
}

#[test]
fn invoke_attribute_form_scalar_policy_and_empty_names() {
    let input = concat!(
        "<invoke name=\"search\">\n",
        "<parameter name=\"query\">rust parsers</parameter>\n",
        "<parameter name=\"limit\">5</parameter>\n",
        "<parameter name=\"fuzzy\">true</parameter>\n",
        "<parameter name=\"\">ignored</parameter>\n",
        "</invoke>"
    );
    let (_text, calls) = parse(input);
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0].arguments,
        serde_json::json!({"query": "rust parsers", "limit": 5, "fuzzy": true})
    );
}

#[test]
fn invoke_without_a_name_attribute_is_not_a_call() {
    let input = "<invoke foo=\"bar\">\n<parameter name=\"v\">hi</parameter>\n</invoke>";
    let (_text, calls) = parse(input);
    assert_eq!(calls.len(), 0);
}

#[test]
fn tool_call_json_and_invoke_attribute_blocks_mix_in_source_order() {
    let input = concat!(
        "<tool_call>{\"name\":\"first\",\"arguments\":{\"a\":1}}</tool_call>\n",
        "<invoke name=\"second\">\n<parameter name=\"b\">two</parameter>\n</invoke>"
    );
    let (_text, calls) = parse(input);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].name, "first");
    assert_eq!(calls[0].arguments, serde_json::json!({"a": 1}));
    assert_eq!(calls[1].name, "second");
    assert_eq!(calls[1].arguments, serde_json::json!({"b": "two"}));
}

#[test]
fn markdown_fence_with_a_json_body_parses() {
    let input = "preamble\n```tool_call\n{\"name\":\"ping\",\"arguments\":{}}\n```\npostamble";
    let (text, calls) = parse(input);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "ping");
    assert_ne!(calls[0].source, CallSource::Native);
    assert!(text.contains("preamble") && text.contains("postamble"));
}
