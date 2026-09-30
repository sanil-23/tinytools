//! Building blocks for collapsing a family of tools into one action-dispatched
//! tool.
//!
//! # Why collapse
//!
//! Every tool on the wire costs its name, its description and its full
//! parameter schema on every request. A family of six CRUD tools over one
//! resource pays that six times to say almost the same thing: `cron_list`,
//! `cron_add`, `cron_update`, `cron_remove`, `cron_run` and `cron_runs` were
//! 3,938 bytes between them, and four of the six are a `job_id` and nothing
//! else. One action-dispatched `cron` tool serves all six.
//!
//! # The two rules that make this safe
//!
//! Collapsing merges tools that the security layer had been judging
//! separately, and getting that wrong is how a token optimisation becomes a
//! privilege bug. So:
//!
//! 1. **The parameter schema is merged from the members, never retyped.** A
//!    hand-written union drifts the moment a member gains a field, and the
//!    drift is silent: the model is told about a parameter the implementation
//!    ignores, or not told about one it needs. [`merge_action_schemas`] derives
//!    it from the same `parameters_schema()` the members serve.
//! 2. **Permission is per action, and the argument-free answer is the
//!    strictest.** [`Tool::permission_level`] has no arguments, so a collapsed
//!    tool cannot answer it honestly; it returns the strictest level any member
//!    requires, and [`Tool::permission_level_with_args`] gives the exact one
//!    once the action is known. A caller that ignores the arguments therefore
//!    over-restricts rather than under-restricts.
//!
//! A collapsed tool uses [`external_effect_for_action`] at the host's
//! argument-aware approval point. Its argument-less declaration remains a
//! conservative summary via [`any_external_effect`].

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::{PermissionLevel, Tool};

/// One member of a collapsed family: the action name the model passes, and the
/// tool that serves it.
#[derive(Clone, Copy)]
pub struct CollapsedAction<'a> {
    /// The `action` value the model passes to select this member.
    pub action: &'static str,
    /// The member tool that serves the action.
    pub tool: &'a dyn Tool,
}

impl std::fmt::Debug for CollapsedAction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollapsedAction")
            .field("action", &self.action)
            .field("tool", &self.tool.name())
            .finish()
    }
}

/// Build the collapsed `parameters_schema` from the members' own schemas.
///
/// The result is an object with `action` (a required enum over the member
/// names) plus the union of every member's properties. Property descriptions
/// are prefixed with the action they belong to — the convention `memory_tree`
/// and `todo` already use — so the model can tell which fields apply to the
/// action it picked.
///
/// Nothing is `required` beyond `action`. A union cannot express "required for
/// this action only", and marking a field required because one action needs it
/// would make every other action's call invalid. The members already validate
/// their own required arguments and return a useful error, so the check lives
/// where it can be specific rather than in a schema that has to be vague.
#[must_use]
pub fn merge_action_schemas(actions: &[CollapsedAction<'_>]) -> Value {
    let mut properties: BTreeMap<String, Vec<(&str, Value)>> = BTreeMap::new();
    // Track which actions mentioned each property so a shared field reads as
    // shared rather than as belonging to whichever action happened to be first.
    let mut owners: BTreeMap<String, Vec<&str>> = BTreeMap::new();

    for entry in actions {
        let schema = entry.tool.parameters_schema();
        let Some(props) = schema.get("properties").and_then(Value::as_object) else {
            continue;
        };
        for (name, spec) in props {
            owners.entry(name.clone()).or_default().push(entry.action);
            properties
                .entry(name.clone())
                .or_default()
                .push((entry.action, spec.clone()));
        }
    }

    // Rewrite each description to name its actions. Done in a second pass so
    // the prefix can list every owner, which the first pass does not yet know.
    let mut merged_properties = BTreeMap::new();
    for (name, specs) in properties {
        let owned_by = owners.get(&name).map_or(&[][..], Vec::as_slice);
        // A property every action takes needs no prefix — saying so would be
        // noise on every line.
        let needs_prefix = owned_by.len() != actions.len() && !owned_by.is_empty();
        let prefix = owned_by.join("/");
        let mut alternatives = Vec::new();
        for (action, mut spec) in specs {
            if needs_prefix {
                if let Some(object) = spec.as_object_mut() {
                    let existing = object
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let described = if existing.is_empty() {
                        format!("{action}: {prefix}")
                    } else {
                        format!("{action}: {existing}")
                    };
                    object.insert("description".to_string(), Value::String(described));
                }
            }
            if !alternatives.contains(&spec) {
                alternatives.push(spec);
            }
        }
        let merged = match alternatives.as_slice() {
            [only] => only.clone(),
            many => json!({"anyOf": many}),
        };
        // `spec` entries are deduplicated by schema; action ownership in the
        // descriptions above tells the model which alternative applies.
        merged_properties.insert(name, merged);
    }

    let enum_values: Vec<Value> = actions
        .iter()
        .map(|entry| Value::String(entry.action.to_string()))
        .collect();

    let mut merged = Map::new();
    merged.insert(
        "action".to_string(),
        json!({
            "type": "string",
            "enum": enum_values,
            "description": "Which operation to run."
        }),
    );
    for (name, spec) in merged_properties {
        merged.insert(name, spec);
    }

    json!({
        "type": "object",
        "properties": Value::Object(merged),
        "required": ["action"]
    })
}

/// The strictest permission level any member requires.
///
/// Used for the argument-free [`Tool::permission_level`], which cannot know
/// which action is coming. Over-restricting is the only safe direction.
#[must_use]
pub fn strictest_permission(actions: &[CollapsedAction<'_>]) -> PermissionLevel {
    actions
        .iter()
        .map(|entry| entry.tool.permission_level())
        .max_by_key(|level| permission_rank(*level))
        .unwrap_or(PermissionLevel::None)
}

/// Conservative argument-less answer: `true` whenever the family has a member.
///
/// The trait has no argument-less way to detect argument-dependent effects, so
/// a collapsed family cannot safely claim `false` here. Hosts should use
/// [`external_effect_for_action`] to get the exact per-call answer.
#[must_use]
pub fn any_external_effect(actions: &[CollapsedAction<'_>]) -> bool {
    // The static answer has no arguments to discriminate with. Conservatively
    // require approval whenever there is a member; the per-action helper below
    // gives the precise answer at the actual call boundary.
    !actions.is_empty()
}

/// Resolve whether the selected member has an external effect for these args.
///
/// Hosts must call this at the approval gate for collapsed tools, passing the
/// original model arguments so the member's argument-aware declaration runs.
#[must_use]
pub fn external_effect_for_action(actions: &[CollapsedAction<'_>], args: &Value) -> bool {
    let Some(action) = args.get("action").and_then(Value::as_str) else {
        return any_external_effect(actions);
    };
    let Some(member) = resolve(actions, action) else {
        return any_external_effect(actions);
    };
    member
        .tool
        .external_effect_with_args(&args_without_action(args))
}

/// Order the permission levels from least to most privileged.
///
/// `PermissionLevel` does derive `Ord` over explicit discriminants, so `.max()`
/// would work today. This exhaustive match is here for the day it gains a
/// variant: a new level would compile fine against `.max()` and silently take
/// whatever rank its discriminant implied, whereas here it is a compile error
/// until someone decides where it sits. Getting that wrong under-restricts a
/// collapsed tool, which is the failure this module exists to avoid.
fn permission_rank(level: PermissionLevel) -> u8 {
    match level {
        PermissionLevel::None => 0,
        PermissionLevel::ReadOnly => 1,
        PermissionLevel::Write => 2,
        PermissionLevel::Execute => 3,
        PermissionLevel::Dangerous => 4,
    }
}

/// Find the member serving `action`.
#[must_use]
pub fn resolve<'a>(
    actions: &'a [CollapsedAction<'a>],
    action: &str,
) -> Option<&'a CollapsedAction<'a>> {
    actions.iter().find(|entry| entry.action == action)
}

/// The error a collapsed tool returns for an unknown or missing action.
///
/// Lists the valid actions, because the model's next move after this message is
/// to guess, and a guess against a printed list is far more likely to be right.
#[must_use]
pub fn unknown_action_message(actions: &[CollapsedAction<'_>], got: Option<&str>) -> String {
    let valid = actions
        .iter()
        .map(|entry| entry.action)
        .collect::<Vec<_>>()
        .join("|");
    match got {
        Some(other) => format!("unknown action '{other}' (expected {valid})"),
        None => format!("missing required field `action` (expected {valid})"),
    }
}

/// Strip the dispatch key before forwarding to the member.
///
/// The members are the same tools that serve the legacy names, and several of
/// them set `"additionalProperties": false`; leaving `action` in the object
/// would be rejected by any validation they do.
#[must_use]
pub fn args_without_action(args: &Value) -> Value {
    match args.as_object() {
        Some(object) => {
            let mut cloned = object.clone();
            cloned.remove("action");
            Value::Object(cloned)
        }
        None => args.clone(),
    }
}

#[cfg(test)]
mod test;
