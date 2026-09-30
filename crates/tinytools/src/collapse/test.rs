//! Tests for the action-collapse building blocks.

#![allow(clippy::unwrap_used, clippy::unnecessary_literal_bound)]

use super::*;
use crate::ToolResult;
use async_trait::async_trait;

struct Stub {
    name: &'static str,
    schema: Value,
    permission: PermissionLevel,
    external: bool,
}

#[async_trait]
impl Tool for Stub {
    fn name(&self) -> &str {
        self.name
    }
    fn description(&self) -> &str {
        "stub"
    }
    fn parameters_schema(&self) -> Value {
        self.schema.clone()
    }
    fn permission_level(&self) -> PermissionLevel {
        self.permission
    }
    fn external_effect(&self) -> bool {
        self.external
    }
    fn external_effect_with_args(&self, _args: &Value) -> bool {
        self.external
    }
    async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult::success("ok"))
    }
}

fn stub(name: &'static str, schema: Value, permission: PermissionLevel, external: bool) -> Stub {
    Stub {
        name,
        schema,
        permission,
        external,
    }
}

#[test]
fn the_union_carries_every_members_properties() {
    let list = stub(
        "list",
        json!({"type": "object", "properties": {}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let runs = stub(
        "runs",
        json!({"type": "object", "properties": {
            "job_id": {"type": "string"},
            "limit": {"type": "integer", "description": "How many."}
        }}),
        PermissionLevel::ReadOnly,
        false,
    );
    let actions = vec![
        CollapsedAction {
            action: "list",
            tool: &list,
        },
        CollapsedAction {
            action: "runs",
            tool: &runs,
        },
    ];
    let merged = merge_action_schemas(&actions);
    let props = merged["properties"].as_object().unwrap();
    assert!(props.contains_key("action"));
    assert!(props.contains_key("job_id"));
    assert!(props.contains_key("limit"));
    assert_eq!(merged["required"], json!(["action"]));
}

#[test]
fn only_action_is_required_because_a_union_cannot_say_otherwise() {
    // `job_id` is required for `runs` and meaningless for `list`. Marking
    // it required here would make every `list` call invalid.
    let list = stub(
        "list",
        json!({"type": "object", "properties": {}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let runs = stub(
        "runs",
        json!({"type": "object", "properties": {"job_id": {"type": "string"}}, "required": ["job_id"]}),
        PermissionLevel::ReadOnly,
        false,
    );
    let actions = vec![
        CollapsedAction {
            action: "list",
            tool: &list,
        },
        CollapsedAction {
            action: "runs",
            tool: &runs,
        },
    ];
    assert_eq!(
        merge_action_schemas(&actions)["required"],
        json!(["action"])
    );
}

#[test]
fn a_property_only_some_actions_take_is_labelled_with_them() {
    let a = stub(
        "a",
        json!({"type": "object", "properties": {"shared": {"type": "string"}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let b = stub(
        "b",
        json!({"type": "object", "properties": {
            "shared": {"type": "string"},
            "only_b": {"type": "string", "description": "B's field."}
        }}),
        PermissionLevel::ReadOnly,
        false,
    );
    let actions = vec![
        CollapsedAction {
            action: "a",
            tool: &a,
        },
        CollapsedAction {
            action: "b",
            tool: &b,
        },
    ];
    let merged = merge_action_schemas(&actions);
    let props = &merged["properties"];
    assert_eq!(props["only_b"]["description"], json!("b: B's field."));
    // Taken by every action, so no prefix — it would be noise.
    assert!(props["shared"].get("description").is_none());
}

#[test]
fn permission_is_the_strictest_member_not_the_first() {
    let read = stub("r", json!({}), PermissionLevel::ReadOnly, false);
    let execute = stub("x", json!({}), PermissionLevel::Execute, false);
    let write = stub("w", json!({}), PermissionLevel::Write, false);
    let actions = vec![
        CollapsedAction {
            action: "r",
            tool: &read,
        },
        CollapsedAction {
            action: "x",
            tool: &execute,
        },
        CollapsedAction {
            action: "w",
            tool: &write,
        },
    ];
    assert_eq!(strictest_permission(&actions), PermissionLevel::Execute);
}

#[test]
fn external_effect_is_true_when_any_member_has_one() {
    let clean = stub("c", json!({}), PermissionLevel::ReadOnly, false);
    let dirty = stub("d", json!({}), PermissionLevel::ReadOnly, true);
    assert!(any_external_effect(&[CollapsedAction {
        action: "c",
        tool: &clean
    }]));
    assert!(any_external_effect(&[
        CollapsedAction {
            action: "c",
            tool: &clean
        },
        CollapsedAction {
            action: "d",
            tool: &dirty
        },
    ]));
}

#[test]
fn action_effect_resolution_uses_member_arguments_and_static_fallback_is_safe() {
    let clean = stub("c", json!({}), PermissionLevel::ReadOnly, false);
    let actions = [CollapsedAction {
        action: "c",
        tool: &clean,
    }];
    assert!(!external_effect_for_action(
        &actions,
        &json!({"action": "c"})
    ));
    assert!(external_effect_for_action(
        &actions,
        &json!({"action": "unknown"})
    ));
    assert!(any_external_effect(&actions));
    assert!(!any_external_effect(&[]));
}

#[test]
fn shared_property_types_are_exposed_as_schema_alternatives() {
    let text = stub(
        "text",
        json!({"properties": {"value": {"type": "string"}}}),
        PermissionLevel::None,
        false,
    );
    let number = stub(
        "number",
        json!({"properties": {"value": {"type": "number"}}}),
        PermissionLevel::None,
        false,
    );
    let actions = [
        CollapsedAction {
            action: "text",
            tool: &text,
        },
        CollapsedAction {
            action: "number",
            tool: &number,
        },
    ];
    let alternatives = merge_action_schemas(&actions)["properties"]["value"]["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|schema| schema["type"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        alternatives,
        std::collections::BTreeSet::from(["number", "string"])
    );
}

#[test]
fn the_dispatch_key_does_not_reach_the_member() {
    // Several members set `additionalProperties: false`.
    let args = json!({"action": "runs", "job_id": "j1"});
    assert_eq!(args_without_action(&args), json!({"job_id": "j1"}));
}

#[test]
fn an_unknown_action_names_the_valid_ones() {
    let a = stub("a", json!({}), PermissionLevel::ReadOnly, false);
    let actions = vec![CollapsedAction {
        action: "add",
        tool: &a,
    }];
    assert_eq!(
        unknown_action_message(&actions, Some("addd")),
        "unknown action 'addd' (expected add)"
    );
    assert_eq!(
        unknown_action_message(&actions, None),
        "missing required field `action` (expected add)"
    );
}

#[test]
fn resolves_actions_and_formats_debug_without_exposing_the_tool() {
    let a = stub("a", json!({}), PermissionLevel::Dangerous, false);
    let actions = [CollapsedAction {
        action: "add",
        tool: &a,
    }];
    assert!(resolve(&actions, "add").is_some());
    assert!(resolve(&actions, "missing").is_none());
    assert!(format!("{:?}", actions[0]).contains("tool: \"a\""));
    assert_eq!(strictest_permission(&actions), PermissionLevel::Dangerous);
}

#[test]
fn schema_merge_skips_missing_properties_and_non_object_property_specs() {
    let absent = stub("absent", json!({}), PermissionLevel::None, false);
    let malformed = stub(
        "malformed",
        json!({"properties": {"ignored": null}}),
        PermissionLevel::None,
        false,
    );
    let actions = [
        CollapsedAction {
            action: "absent",
            tool: &absent,
        },
        CollapsedAction {
            action: "malformed",
            tool: &malformed,
        },
    ];
    assert_eq!(
        merge_action_schemas(&actions)["properties"]["ignored"],
        Value::Null
    );
    assert_eq!(strictest_permission(&[]), PermissionLevel::None);
    assert_eq!(args_without_action(&json!([1, 2])), json!([1, 2]));
}
