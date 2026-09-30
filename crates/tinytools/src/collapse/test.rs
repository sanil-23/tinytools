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
    assert!(!any_external_effect(&[CollapsedAction {
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

/// A member that classifies per call: `force` raises it to `Write` and `send`
/// makes it effectful. It answers `Dangerous` if the dispatch key reaches it,
/// so a test can tell the key was stripped first.
struct ArgAware;

#[async_trait]
impl Tool for ArgAware {
    fn name(&self) -> &str {
        "arg_aware"
    }
    fn description(&self) -> &str {
        "classifies per call"
    }
    fn parameters_schema(&self) -> Value {
        json!({"type": "object", "properties": {"force": {"type": "boolean"}}})
    }
    fn permission_level_with_args(&self, args: &Value) -> PermissionLevel {
        if args.get("action").is_some() {
            PermissionLevel::Dangerous
        } else if args["force"] == json!(true) {
            PermissionLevel::Write
        } else {
            PermissionLevel::ReadOnly
        }
    }
    fn external_effect_with_args(&self, args: &Value) -> bool {
        args["send"] == json!(true)
    }
    async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult::success("ok"))
    }
}

#[test]
fn the_argument_free_permission_is_the_least_member() {
    // The `Tool` contract: a multi-action tool must not be statically hidden
    // from a caller entitled to its read-only half.
    let read = stub("r", json!({}), PermissionLevel::ReadOnly, false);
    let execute = stub("x", json!({}), PermissionLevel::Execute, false);
    let actions = [
        CollapsedAction {
            action: "x",
            tool: &execute,
        },
        CollapsedAction {
            action: "r",
            tool: &read,
        },
    ];
    assert_eq!(minimum_permission(&actions), PermissionLevel::ReadOnly);
    assert_eq!(minimum_permission(&[]), PermissionLevel::None);
}

#[test]
fn the_per_call_permission_is_the_selected_members_own_answer() {
    let aware = ArgAware;
    let execute = stub("x", json!({}), PermissionLevel::Execute, false);
    let actions = [
        CollapsedAction {
            action: "aware",
            tool: &aware,
        },
        CollapsedAction {
            action: "x",
            tool: &execute,
        },
    ];
    assert_eq!(
        permission_for_args(&actions, &json!({"action": "aware"})),
        PermissionLevel::ReadOnly
    );
    assert_eq!(
        permission_for_args(&actions, &json!({"action": "aware", "force": true})),
        PermissionLevel::Write
    );
    assert_eq!(
        permission_for_args(&actions, &json!({"action": "x"})),
        PermissionLevel::Execute
    );
}

#[test]
fn a_call_selecting_no_member_gets_the_strictest_permission() {
    let aware = ArgAware;
    let execute = stub("x", json!({}), PermissionLevel::Execute, false);
    let actions = [
        CollapsedAction {
            action: "aware",
            tool: &aware,
        },
        CollapsedAction {
            action: "x",
            tool: &execute,
        },
    ];
    assert_eq!(
        permission_for_args(&actions, &json!({"action": "nope"})),
        PermissionLevel::Execute
    );
    assert_eq!(
        permission_for_args(&actions, &json!({})),
        PermissionLevel::Execute
    );
}

#[test]
fn the_per_call_external_effect_reaches_a_member_that_classifies_per_call() {
    // `ArgAware` leaves the argument-free `external_effect` at `false`, so an
    // aggregate of static answers would wave an effectful call past the gate.
    let aware = ArgAware;
    let actions = [CollapsedAction {
        action: "aware",
        tool: &aware,
    }];
    assert!(!any_external_effect(&actions));
    assert!(external_effect_for_args(
        &actions,
        &json!({"action": "aware", "send": true})
    ));
    assert!(!external_effect_for_args(
        &actions,
        &json!({"action": "aware"})
    ));
}

#[test]
fn a_call_selecting_no_member_gets_the_aggregate_external_effect() {
    let dirty = stub("d", json!({}), PermissionLevel::ReadOnly, true);
    let actions = [CollapsedAction {
        action: "d",
        tool: &dirty,
    }];
    assert!(external_effect_for_args(
        &actions,
        &json!({"action": "nope"})
    ));
}

#[test]
fn conflicting_definitions_of_a_shared_property_are_all_kept() {
    let a = stub(
        "a",
        json!({"type": "object", "properties": {"id": {"type": "string"}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let b = stub(
        "b",
        json!({"type": "object", "properties": {"id": {"type": "integer", "minimum": 1}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let forward = [
        CollapsedAction {
            action: "a",
            tool: &a,
        },
        CollapsedAction {
            action: "b",
            tool: &b,
        },
    ];
    let reversed = [forward[1], forward[0]];
    let merged = merge_action_schemas(&forward);
    let alternatives = merged["properties"]["id"]["anyOf"].as_array().unwrap();
    assert_eq!(alternatives.len(), 2);
    assert!(alternatives.contains(&json!({"type": "string"})));
    assert!(alternatives.contains(&json!({"type": "integer", "minimum": 1})));
    // Neither member's constraints depend on which came first.
    let reordered = merge_action_schemas(&reversed);
    let reordered_alternatives = reordered["properties"]["id"]["anyOf"].as_array().unwrap();
    assert_eq!(reordered_alternatives.len(), 2);
    assert!(reordered_alternatives.contains(&json!({"type": "string"})));
}

#[test]
fn definitions_differing_only_in_description_are_one_property() {
    let a = stub(
        "a",
        json!({"type": "object", "properties": {"id": {"type": "string", "description": "A."}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let b = stub(
        "b",
        json!({"type": "object", "properties": {"id": {"type": "string", "description": "B."}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let actions = [
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
    assert!(merged["properties"]["id"].get("anyOf").is_none());
    assert_eq!(merged["properties"]["id"]["type"], json!("string"));
}

#[test]
fn a_member_property_named_action_cannot_replace_the_discriminator() {
    let clash = stub(
        "clash",
        json!({"type": "object", "properties": {"action": {"type": "integer"}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let actions = [CollapsedAction {
        action: "clash",
        tool: &clash,
    }];
    let merged = merge_action_schemas(&actions);
    assert_eq!(merged["properties"]["action"]["type"], json!("string"));
    assert_eq!(merged["properties"]["action"]["enum"], json!(["clash"]));
}

#[test]
fn validation_accepts_a_well_formed_family() {
    let a = stub("a", json!({}), PermissionLevel::ReadOnly, false);
    let b = stub("b", json!({}), PermissionLevel::Write, false);
    let actions = [
        CollapsedAction {
            action: "a",
            tool: &a,
        },
        CollapsedAction {
            action: "b",
            tool: &b,
        },
    ];
    assert_eq!(validate_actions(&actions), Ok(()));
}

#[test]
fn validation_rejects_an_empty_family() {
    assert_eq!(validate_actions(&[]), Err(CollapseError::Empty));
    assert_eq!(
        CollapseError::Empty.to_string(),
        "a collapsed tool needs at least one action"
    );
}

#[test]
fn validation_rejects_a_duplicated_action() {
    let a = stub("a", json!({}), PermissionLevel::ReadOnly, false);
    let actions = [
        CollapsedAction {
            action: "a",
            tool: &a,
        },
        CollapsedAction {
            action: "a",
            tool: &a,
        },
    ];
    let err = validate_actions(&actions).unwrap_err();
    assert_eq!(
        err,
        CollapseError::DuplicateAction {
            action: "a".to_string()
        }
    );
    assert_eq!(err.to_string(), "action 'a' is declared more than once");
}

#[test]
fn validation_rejects_a_member_declaring_the_reserved_property() {
    let clash = stub(
        "clash",
        json!({"type": "object", "properties": {"action": {"type": "string"}}}),
        PermissionLevel::ReadOnly,
        false,
    );
    let actions = [CollapsedAction {
        action: "clash",
        tool: &clash,
    }];
    let err = validate_actions(&actions).unwrap_err();
    assert_eq!(
        err,
        CollapseError::ReservedProperty {
            action: "clash".to_string()
        }
    );
    assert!(err.to_string().contains("reserved for dispatch"));
}

#[test]
fn the_debug_form_names_the_member_tool() {
    let a = stub("member", json!({}), PermissionLevel::ReadOnly, false);
    let entry = CollapsedAction {
        action: "a",
        tool: &a,
    };
    assert_eq!(
        format!("{entry:?}"),
        r#"CollapsedAction { action: "a", tool: "member" }"#
    );
}
