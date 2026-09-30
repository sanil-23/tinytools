use super::*;
use crate::network::test_support::{DEFAULT_LIMITS, TestHtml, TestNetGate};

fn test_security() -> Arc<TestNetGate> {
    TestNetGate::supervised()
}

fn fetch(
    gate: Arc<TestNetGate>,
    allowed: Vec<String>,
    max: Option<usize>,
    timeout: Option<u64>,
) -> WebFetchTool {
    WebFetchTool::new(
        gate,
        allowed,
        max,
        timeout,
        DEFAULT_LIMITS,
        Arc::new(TestHtml),
    )
}

#[test]
fn web_fetch_name_and_schema() {
    let tool = fetch(test_security(), vec!["example.com".into()], None, None);
    assert_eq!(tool.name(), "web_fetch");
    let schema = tool.parameters_schema();
    assert!(schema["properties"]["url"].is_object());
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("url"))
    );
}

#[test]
fn a_host_can_opt_the_schema_into_extra_optional_arguments() {
    let tool = fetch(test_security(), vec!["example.com".into()], None, None).with_schema_property(
        "summary_focus",
        json!({"type": "string", "description": "What you need."}),
    );
    let schema = tool.parameters_schema();
    assert_eq!(schema["properties"]["summary_focus"]["type"], "string");
    assert!(
        !schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("summary_focus"))
    );
    // Without the opt-in the schema is exactly the base one.
    let plain = fetch(test_security(), vec![], None, None).parameters_schema();
    assert!(plain["properties"].get("summary_focus").is_none());
}

#[test]
fn zero_and_none_limits_fall_back_to_defaults() {
    // Callers wire these from `[http_request]`; a stale `Some(0)` is a
    // 0-byte cap (empty bodies) and a 0-second timeout (instant failure).
    // Both `None` and `Some(0)` must coerce to the shared schema defaults.
    let defaults = DEFAULT_LIMITS;
    let from_zero = fetch(
        test_security(),
        vec!["example.com".into()],
        Some(0),
        Some(0),
    );
    assert_eq!(from_zero.max_bytes, defaults.max_response_size);
    assert_eq!(from_zero.timeout_secs, defaults.timeout_secs);
    assert_ne!(from_zero.timeout_secs, 0);
    assert_ne!(from_zero.max_bytes, 0);

    let from_none = fetch(test_security(), vec!["example.com".into()], None, None);
    assert_eq!(from_none.max_bytes, defaults.max_response_size);
    assert_eq!(from_none.timeout_secs, defaults.timeout_secs);
}

#[test]
fn nonzero_limits_are_preserved() {
    let tool = fetch(
        test_security(),
        vec!["example.com".into()],
        Some(4096),
        Some(15),
    );
    assert_eq!(tool.max_bytes, 4096);
    assert_eq!(tool.timeout_secs, 15);
}

#[tokio::test]
async fn web_fetch_rejects_disallowed_domain() {
    let tool = fetch(test_security(), vec!["example.com".into()], None, None);
    let result = tool
        .execute(json!({ "url": "https://evil.test/path" }))
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.output().contains("URL rejected"));
}

#[tokio::test]
async fn web_fetch_rejects_invalid_url() {
    let tool = fetch(test_security(), vec!["example.com".into()], None, None);
    let result = tool.execute(json!({ "url": "not-a-url" })).await.unwrap();
    assert!(result.is_error);
}

#[tokio::test]
async fn web_fetch_blocked_under_local_only_privacy_mode() {
    // Privacy epic S7 (#4441): under LocalOnly the fetch is refused with a
    // `[policy-blocked]` result before any URL validation / network.
    let tool = fetch(
        TestNetGate::local_only(),
        vec!["example.com".into()],
        None,
        None,
    );
    let result = tool
        .execute(json!({ "url": "https://example.com/data" }))
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(
        result.output().contains("[policy-blocked]"),
        "got: {}",
        result.output()
    );
    assert!(
        result.output().contains("Local-only"),
        "got: {}",
        result.output()
    );
}

#[test]
fn test_web_fetch_truncation_utf8() {
    // Mock body with multi-byte char exactly at budget
    let body = "Hello 🦀 World"; // 🦀 is at index 6-9
    let max_bytes = 8;
    // Should truncate at index 6
    let cut = floor_char_boundary(body, max_bytes);
    assert_eq!(cut, 6);
    assert_eq!(&body[..cut], "Hello ");
}

// --- content extraction ----------------------------------------------------

fn html(body: &str, content_type: Option<&str>) -> bool {
    is_html(&TestHtml, body, content_type)
}

#[test]
fn an_explicit_html_content_type_selects_markdown_conversion() {
    assert!(html("<p>hi</p>", Some("text/html; charset=utf-8")));
    assert!(html("<p>hi</p>", Some("application/xhtml+xml")));
}

#[test]
fn an_explicit_non_html_content_type_is_taken_at_its_word() {
    // A JSON API that happens to quote markup must come back verbatim —
    // the server said what it sent, so we don't second-guess it by sniffing.
    let body = r#"{"html": "<div><p>one</p><span>two</span><a href="/x">two</a></div>"}"#;
    assert!(!html(body, Some("application/json")));
    assert!(!html("<p>x</p>", Some("text/plain")));
}

#[test]
fn a_missing_content_type_falls_back_to_content_detection() {
    assert!(html(
        "<!DOCTYPE html><html><body><p>hi</p></body></html>",
        None
    ));
    assert!(!html("# Just a README\n\nSome prose.\n", None));
}

#[test]
fn an_empty_content_type_does_not_veto_detection() {
    assert!(html("<!DOCTYPE html><html><body>x</body></html>", Some("")));
}

#[test]
fn the_schema_offers_the_raw_escape_hatch() {
    let tool = fetch(test_security(), vec![], None, None);
    let schema = tool.parameters_schema();
    assert!(
        schema["properties"].get("raw").is_some(),
        "a caller must be able to opt out of conversion: {schema}"
    );
    assert!(
        tool.description().contains("Markdown"),
        "the model needs to know what it will get back: {}",
        tool.description()
    );
}

#[test]
fn the_declared_cap_is_sized_for_extracted_markdown_not_raw_markup() {
    let tool = fetch(test_security(), vec![], None, None);
    let cap = tool
        .max_result_size_chars()
        .expect("web_fetch declares a cap");
    assert!(
        (8_000..=32_000).contains(&cap),
        "cap should sit in the same range as Hermes (15k chars) and Codex \
         (~10k tokens) budget for one result, got {cap}"
    );
}

#[test]
fn html_is_converted_through_the_host_extractor_only_when_it_is_html() {
    let body = "<!DOCTYPE html><html><body><p>hi</p></body></html>";
    assert!(html(body, None));
    assert_eq!(TestHtml.to_markdown(body), "hi");
}

#[tokio::test]
async fn execute_blocks_when_rate_limited() {
    let tool = fetch(
        TestNetGate::with(crate::network::test_support::AutonomyLevel::Supervised, 0),
        vec!["example.com".into()],
        None,
        None,
    );
    let result = tool
        .execute(json!({ "url": "https://example.com/data" }))
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.output().contains("Rate limit exceeded"));
}
