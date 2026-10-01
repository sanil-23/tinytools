use super::*;

fn call(id: &str) -> NativeToolCall {
    NativeToolCall {
        id: id.into(),
        name: "shell".into(),
        arguments: r#"{"command":"ls"}"#.into(),
        extra_content: None,
    }
}

#[test]
fn assistant_envelope_round_trips_byte_exact() {
    let text = encode_assistant_envelope(Some("on it"), &[call("c1"), call("c2")], None);
    let parsed = parse_canonical_assistant_envelope(&text).unwrap();
    assert_eq!(parsed.content, "on it");
    assert_eq!(parsed.tool_calls.len(), 2);
    assert_eq!(
        encode_assistant_envelope(Some(&parsed.content), &parsed.tool_calls, None),
        text
    );
}

#[test]
fn literal_envelope_shape_is_stable() {
    // Compared as values: key order depends on whether a downstream build
    // unifies serde_json's `preserve_order`, the shape does not.
    let value = |text: String| serde_json::from_str::<Value>(&text).unwrap();
    assert_eq!(
        value(encode_assistant_envelope(Some("hi"), &[call("c1")], None)),
        serde_json::json!({
            "content": "hi",
            "tool_calls": [{"id": "c1", "name": "shell", "arguments": "{\"command\":\"ls\"}"}]
        })
    );
    assert_eq!(
        value(encode_tool_envelope("c1", "ok")),
        serde_json::json!({"tool_call_id": "c1", "content": "ok"})
    );
}

#[test]
fn non_canonical_assistant_envelopes_stay_opaque() {
    for text in [
        encode_assistant_envelope(None, &[call("c1")], None),
        encode_assistant_envelope(Some("x"), &[call("c1")], Some("because")),
        r#"{"content":"x","tool_calls":[{"id":"c1","name":"n","arguments":"{}"}],"extra":1}"#
            .to_string(),
        "plain prose".to_string(),
        r#"{"content":"x","tool_calls":[]}"#.to_string(),
    ] {
        assert!(
            parse_canonical_assistant_envelope(&text).is_none(),
            "{text}"
        );
    }
}

#[test]
fn tool_envelope_round_trips_and_rejects_extras() {
    let text = encode_tool_envelope("c1", "result \"quoted\"");
    assert_eq!(
        parse_canonical_tool_envelope(&text),
        Some(("c1".into(), "result \"quoted\"".into()))
    );
    assert!(
        parse_canonical_tool_envelope(r#"{"tool_call_id":"c1","content":"a","name":"n"}"#)
            .is_none()
    );
    assert!(parse_canonical_tool_envelope("bare").is_none());
}

#[test]
fn image_parts_split_and_join_exactly() {
    for text in [
        "just text",
        "[OH_IMAGE:data:image/png;base64,AAAA]",
        "look [OH_IMAGE:data:image/png;base64,AAAA] and [OH_IMAGE:https://x/y.png]\ndone",
        "dangling [OH_IMAGE:data:no-close",
        "",
    ] {
        assert_eq!(join_image_parts(&split_image_parts(text)), text);
    }
    assert_eq!(
        split_image_parts("a[OH_IMAGE:u]b"),
        vec![
            ContentPart::Text("a".into()),
            ContentPart::Image("u".into()),
            ContentPart::Text("b".into())
        ]
    );
}
