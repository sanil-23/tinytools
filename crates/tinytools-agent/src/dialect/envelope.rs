//! The native replay envelopes and the inline image marker, in one place.
//!
//! A host that keeps provider history as flat `{role, content}` rows carries a
//! native tool round inside `content`: an assistant row that made calls is the
//! JSON `{"content", "tool_calls"}`, a tool result is `{"tool_call_id",
//! "content"}`, and a pasted image rides in a user row as an `[OH_IMAGE:<url>]`
//! marker. This module is the single owner of those encodings, so the dialect,
//! a host's row bridge and a durable transcript writer cannot drift apart.
//!
//! The canonical parsers ([`parse_canonical_assistant_envelope`],
//! [`parse_canonical_tool_envelope`]) accept only a value whose re-encoding is
//! **byte-identical** to the input. A storage layer can therefore lift an
//! envelope into typed fields and rebuild the exact original string later, and
//! anything non-canonical stays opaque text instead of being silently altered.

use super::types::NativeToolCall;
use serde_json::Value;

/// Prefix of the private inline-image marker (`[OH_IMAGE:<url>]`).
pub const IMAGE_MARKER_PREFIX: &str = "[OH_IMAGE:";

/// A native assistant tool-call envelope, decoded.
#[derive(Debug, Clone, PartialEq)]
pub struct AssistantEnvelope {
    /// The visible text (empty when the envelope's `content` was null/absent).
    pub content: String,
    /// The calls the assistant made; never empty for a parsed envelope.
    pub tool_calls: Vec<NativeToolCall>,
    /// A `reasoning_content` carried inside the envelope, if any.
    pub reasoning_content: Option<String>,
}

/// One piece of a user row that mixes text and inline images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentPart {
    /// Literal text.
    Text(String),
    /// An image reference (the marker's payload, verbatim).
    Image(String),
}

/// Encode the `{content, tool_calls[, reasoning_content]}` assistant envelope.
///
/// `text` is `None` for an assistant turn that said nothing (serialized as
/// `null`, as [`NativeDialect`](super::NativeDialect) always has).
#[must_use]
pub fn encode_assistant_envelope(
    text: Option<&str>,
    tool_calls: &[NativeToolCall],
    reasoning_content: Option<&str>,
) -> String {
    let mut payload = serde_json::json!({
        "content": text,
        "tool_calls": tool_calls,
    });
    if let Some(reasoning) = reasoning_content {
        payload["reasoning_content"] = Value::String(reasoning.to_string());
    }
    payload.to_string()
}

/// Encode the `{tool_call_id, content}` tool-result envelope.
#[must_use]
pub fn encode_tool_envelope(tool_call_id: &str, content: &str) -> String {
    serde_json::json!({
        "tool_call_id": tool_call_id,
        "content": content,
    })
    .to_string()
}

/// Lenient parse of a native assistant envelope: any JSON object with a
/// non-empty, well-formed `tool_calls` array. A missing or non-string `content`
/// reads as empty. `None` for anything else (plain prose, JSON-looking text).
#[must_use]
pub fn parse_assistant_envelope(text: &str) -> Option<AssistantEnvelope> {
    let value: Value = serde_json::from_str(text).ok()?;
    let obj = value.as_object()?;
    let calls = obj.get("tool_calls")?;
    if calls.as_array().is_none_or(Vec::is_empty) {
        return None;
    }
    let tool_calls: Vec<NativeToolCall> = serde_json::from_value(calls.clone()).ok()?;
    if tool_calls.is_empty() {
        return None;
    }
    Some(AssistantEnvelope {
        content: obj
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        tool_calls,
        reasoning_content: obj
            .get("reasoning_content")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// Lenient parse of a native tool-result envelope into `(tool_call_id,
/// content)`. `None` for a bare tool message.
#[must_use]
pub fn parse_tool_envelope(text: &str) -> Option<(String, String)> {
    let value: Value = serde_json::from_str(text).ok()?;
    let obj = value.as_object()?;
    let id = obj.get("tool_call_id")?.as_str()?.to_string();
    let content = obj
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some((id, content))
}

/// Strict parse: the envelope **and** it re-encodes to exactly `text` (string
/// `content`, no reasoning, nothing else in the object).
#[must_use]
pub fn parse_canonical_assistant_envelope(text: &str) -> Option<AssistantEnvelope> {
    let parsed = parse_assistant_envelope(text)?;
    if parsed.reasoning_content.is_some() {
        return None;
    }
    let again = encode_assistant_envelope(Some(&parsed.content), &parsed.tool_calls, None);
    (again == text).then_some(parsed)
}

/// Strict parse of a tool-result envelope; see
/// [`parse_canonical_assistant_envelope`].
#[must_use]
pub fn parse_canonical_tool_envelope(text: &str) -> Option<(String, String)> {
    let (id, content) = parse_tool_envelope(text)?;
    (encode_tool_envelope(&id, &content) == text).then_some((id, content))
}

/// Split `text` at its [`IMAGE_MARKER_PREFIX`] markers, in source order. An
/// unterminated marker stays text. [`join_image_parts`] is the exact inverse.
#[must_use]
pub fn split_image_parts(text: &str) -> Vec<ContentPart> {
    let mut parts = Vec::new();
    let mut pending = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(IMAGE_MARKER_PREFIX) {
        let after = &rest[start + IMAGE_MARKER_PREFIX.len()..];
        let Some(end) = after.find(']') else {
            break;
        };
        pending.push_str(&rest[..start]);
        if !pending.is_empty() {
            parts.push(ContentPart::Text(std::mem::take(&mut pending)));
        }
        parts.push(ContentPart::Image(after[..end].to_string()));
        rest = &after[end + 1..];
    }
    pending.push_str(rest);
    if !pending.is_empty() {
        parts.push(ContentPart::Text(pending));
    }
    parts
}

/// Join parts back into marker form: text verbatim, each image as
/// `[OH_IMAGE:<url>]`.
#[must_use]
pub fn join_image_parts(parts: &[ContentPart]) -> String {
    let mut out = String::new();
    for part in parts {
        match part {
            ContentPart::Text(text) => out.push_str(text),
            ContentPart::Image(url) => {
                out.push_str(IMAGE_MARKER_PREFIX);
                out.push_str(url);
                out.push(']');
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
}
