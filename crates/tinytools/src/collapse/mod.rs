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
//! 2. **Classification is per action.** The argument-free answers follow the
//!    [`Tool`] contract for a multi-action tool: [`Tool::permission_level`] is
//!    the *minimum* any member requires ([`minimum_permission`]), so a caller
//!    who may run the read-only half is not statically shut out of the whole
//!    tool, and [`Tool::external_effect`] is `true` if any member's is
//!    ([`any_external_effect`]). The enforcement points are the
//!    argument-aware variants, and those delegate to the member the call
//!    selects — [`permission_for_args`] and [`external_effect_for_action`] — so
//!    a member that classifies per call keeps doing so behind the collapse.
//!    A call whose action resolves to no member falls back to the strictest
//!    answer, even though it will fail before any member runs.
//!
//! Call [`validate_actions`] once when building the collapsed tool: it rejects
//! an empty family, a duplicated action name, and a member that declares the
//! reserved `action` parameter.

use std::collections::{BTreeMap, HashSet};

use serde_json::{Map, Value, json};

use crate::PermissionLevel;
// Only named in the docs: the module's contract is stated against it.
#[cfg(doc)]
use crate::Tool;

mod types;

pub use types::{CollapseError, CollapsedAction};

/// The parameter a collapsed tool reserves to select the member.
const ACTION_KEY: &str = "action";

/// Check that `actions` can be served as one collapsed tool.
///
/// # Errors
///
/// Returns [`CollapseError::Empty`] when `actions` is empty,
/// [`CollapseError::DuplicateAction`] when two members share an action name,
/// and [`CollapseError::ReservedProperty`] when a member's schema declares a
/// property named `action`.
pub fn validate_actions(actions: &[CollapsedAction<'_>]) -> Result<(), CollapseError> {
    if actions.is_empty() {
        return Err(CollapseError::Empty);
    }
    let mut seen = HashSet::new();
    for entry in actions {
        if !seen.insert(entry.action) {
            return Err(CollapseError::DuplicateAction {
                action: entry.action.to_string(),
            });
        }
        let schema = entry.tool.parameters_schema();
        if schema
            .get("properties")
            .and_then(Value::as_object)
            .is_some_and(|props| props.contains_key(ACTION_KEY))
        {
            return Err(CollapseError::ReservedProperty {
                action: entry.action.to_string(),
            });
        }
    }
    Ok(())
}

/// Build the collapsed `parameters_schema` from the members' own schemas.
///
/// The result is an object with `action` (a required enum over the member
/// names) plus the union of every member's properties. Property descriptions
/// are prefixed with the action they belong to — the convention `memory_tree`
/// and `todo` already use — so the model can tell which fields apply to the
/// action it picked.
///
/// When members declare the same property with different schemas, neither is
/// dropped: the merged property is an `anyOf` over the distinct definitions,
/// so no action's constraints are lost and member order does not matter.
/// Definitions that differ only in their `description` count as the same.
///
/// The `action` discriminator always wins over a member property of the same
/// name; [`validate_actions`] reports such a member as an error.
///
/// Nothing is `required` beyond `action`. A union cannot express "required for
/// this action only", and marking a field required because one action needs it
/// would make every other action's call invalid. The members already validate
/// their own required arguments and return a useful error, so the check lives
/// where it can be specific rather than in a schema that has to be vague.
#[must_use]
pub fn merge_action_schemas(actions: &[CollapsedAction<'_>]) -> Value {
    // Every distinct definition of each property, in first-seen order.
    let mut definitions: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut merged_defs = Map::new();
    // Track which actions mentioned each property so a shared field reads as
    // shared rather than as belonging to whichever action happened to be first.
    let mut owners: BTreeMap<String, Vec<&str>> = BTreeMap::new();

    for entry in actions {
        let schema = entry.tool.parameters_schema();
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            for (name, spec) in props {
                if name == ACTION_KEY {
                    continue;
                }
                owners.entry(name.clone()).or_default().push(entry.action);
                let known = definitions.entry(name.clone()).or_default();
                let mut property = spec.clone();
                rewrite_local_refs(&mut property, entry.action);
                if !known
                    .iter()
                    .any(|existing| same_definition(existing, &property))
                {
                    known.push(property);
                }
            }
        }
        if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
            for (name, definition) in defs {
                let mut definition = definition.clone();
                rewrite_local_refs(&mut definition, entry.action);
                merged_defs.insert(namespace_definition(entry.action, name), definition);
            }
        }
    }

    let mut properties: BTreeMap<String, Value> = definitions
        .into_iter()
        .map(|(name, mut specs)| {
            let spec = if specs.len() == 1 {
                specs.remove(0)
            } else {
                json!({ "anyOf": specs })
            };
            (name, spec)
        })
        .collect();

    // Rewrite each description to name its actions. Done in a second pass so
    // the prefix can list every owner, which the first pass does not yet know.
    for (name, spec) in &mut properties {
        let Some(object) = spec.as_object_mut() else {
            continue;
        };
        let owned_by = owners.get(name).map_or(&[][..], Vec::as_slice);
        // A property every action takes needs no prefix — saying so would be
        // noise on every line.
        if owned_by.len() == actions.len() || owned_by.is_empty() {
            continue;
        }
        let prefix = owned_by.join("/");
        let existing = object
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let described = if existing.is_empty() {
            prefix
        } else {
            format!("{prefix}: {existing}")
        };
        object.insert("description".to_string(), Value::String(described));
    }

    let enum_values: Vec<Value> = actions
        .iter()
        .map(|entry| Value::String(entry.action.to_string()))
        .collect();

    let mut merged = Map::new();
    for (name, spec) in properties {
        merged.insert(name, spec);
    }
    // Inserted last so nothing a member declares can replace it.
    merged.insert(
        ACTION_KEY.to_string(),
        json!({
            "type": "string",
            "enum": enum_values,
            "description": "Which operation to run."
        }),
    );

    let mut result = json!({
        "type": "object",
        "properties": Value::Object(merged),
        "required": [ACTION_KEY]
    });
    if !merged_defs.is_empty() {
        result["$defs"] = Value::Object(merged_defs);
    }
    result
}

/// Namespace member-local JSON Schema definitions so merged properties keep
/// resolving their references without collisions between actions.
fn namespace_definition(action: &str, name: &str) -> String {
    format!("a{}_{}d{}_{}", action.len(), action, name.len(), name)
}

/// Rewrite references only where JSON Schema expects a subschema. Values
/// inside `const`, `enum`, `default`, and `examples` are instance data.
fn rewrite_local_refs(value: &mut Value, action: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref")
                && let Some(pointer) = reference.strip_prefix("#/$defs/")
                && let (token, suffix) = pointer.split_once('/').unwrap_or((pointer, ""))
                && let Some(name) = decode_pointer_token(token)
            {
                let namespaced = namespace_definition(action, &name);
                let suffix = if suffix.is_empty() {
                    String::new()
                } else {
                    format!("/{suffix}")
                };
                *reference = format!("#/$defs/{}{suffix}", encode_pointer_token(&namespaced));
            }
            for key in [
                "$defs",
                "definitions",
                "properties",
                "patternProperties",
                "dependentSchemas",
            ] {
                if let Some(Value::Object(schemas)) = object.get_mut(key) {
                    for schema in schemas.values_mut() {
                        rewrite_local_refs(schema, action);
                    }
                }
            }
            for key in [
                "additionalProperties",
                "unevaluatedProperties",
                "propertyNames",
                "items",
                "contains",
                "not",
                "if",
                "then",
                "else",
                "unevaluatedItems",
                "contentSchema",
            ] {
                if let Some(schema) = object.get_mut(key) {
                    rewrite_local_refs(schema, action);
                }
            }
            for key in ["allOf", "anyOf", "oneOf"] {
                if let Some(Value::Array(schemas)) = object.get_mut(key) {
                    for schema in schemas {
                        rewrite_local_refs(schema, action);
                    }
                }
            }
        }
        Value::Array(values) => values
            .iter_mut()
            .for_each(|schema| rewrite_local_refs(schema, action)),
        _ => {}
    }
}

/// Decode one JSON Pointer token, leaving malformed escape sequences intact.
fn decode_pointer_token(token: &str) -> Option<String> {
    let mut decoded = String::with_capacity(token.len());
    let mut chars = token.chars();
    while let Some(ch) = chars.next() {
        if ch != '~' {
            decoded.push(ch);
            continue;
        }
        match chars.next()? {
            '0' => decoded.push('~'),
            '1' => decoded.push('/'),
            _ => return None,
        }
    }
    Some(decoded)
}

/// Escape a definition name for its JSON Pointer token.
fn encode_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// Whether two property schemas constrain the same thing, ignoring the
/// human-facing `description`.
fn same_definition(a: &Value, b: &Value) -> bool {
    match (a.as_object(), b.as_object()) {
        (Some(a), Some(b)) => {
            let strip = |object: &Map<String, Value>| {
                let mut object = object.clone();
                object.remove("description");
                object
            };
            strip(a) == strip(b)
        }
        _ => a == b,
    }
}

/// The least privilege any member requires.
///
/// The answer for the argument-free [`Tool::permission_level`], which the
/// [`Tool`] contract defines as the minimum over a multi-action tool's actions
/// so a caller entitled to the read-only half is not statically blocked. The
/// exact per-call level comes from [`permission_for_args`].
#[must_use]
pub fn minimum_permission(actions: &[CollapsedAction<'_>]) -> PermissionLevel {
    actions
        .iter()
        .map(|entry| entry.tool.permission_level())
        .min_by_key(|level| permission_rank(*level))
        .unwrap_or(PermissionLevel::None)
}

/// The strictest permission level any member requires.
///
/// The fallback [`permission_for_args`] uses when the call selects no member.
#[must_use]
pub fn strictest_permission(actions: &[CollapsedAction<'_>]) -> PermissionLevel {
    actions
        .iter()
        .map(|entry| entry.tool.permission_level())
        .max_by_key(|level| permission_rank(*level))
        .unwrap_or(PermissionLevel::None)
}

/// The answer for [`Tool::permission_level_with_args`]: the selected member's
/// own argument-aware level, asked with the dispatch key stripped.
///
/// A call whose `action` is missing or unknown gets
/// [`strictest_permission`] — over-restricting is the only safe direction when
/// the member is not known.
#[must_use]
pub fn permission_for_args(actions: &[CollapsedAction<'_>], args: &Value) -> PermissionLevel {
    match selected(actions, args) {
        Some(entry) => entry
            .tool
            .permission_level_with_args(&args_without_action(args)),
        None => strictest_permission(actions),
    }
}

/// `true` when any member has an external effect.
///
/// The conservative argument-free answer: `true` whenever the family has a
/// member, because this form cannot inspect call arguments.
#[must_use]
pub fn any_external_effect(actions: &[CollapsedAction<'_>]) -> bool {
    !actions.is_empty()
}

/// Resolve the selected member's external-effect classification for a call.
/// Missing or unknown actions use the conservative [`any_external_effect`].
#[must_use]
pub fn external_effect_for_action(actions: &[CollapsedAction<'_>], args: &Value) -> bool {
    external_effect_for_args(actions, args)
}

/// The answer for [`Tool::external_effect_with_args`]: the selected member's
/// own argument-aware answer, asked with the dispatch key stripped.
///
/// A call whose `action` is missing or unknown gets [`any_external_effect`];
/// such a call fails before any member runs.
#[must_use]
pub fn external_effect_for_args(actions: &[CollapsedAction<'_>], args: &Value) -> bool {
    match selected(actions, args) {
        Some(entry) => entry
            .tool
            .external_effect_with_args(&args_without_action(args)),
        None => any_external_effect(actions),
    }
}

/// The member a call's `action` argument selects, if any.
fn selected<'a>(
    actions: &'a [CollapsedAction<'a>],
    args: &Value,
) -> Option<&'a CollapsedAction<'a>> {
    let action = args.get(ACTION_KEY).and_then(Value::as_str)?;
    resolve(actions, action)
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
            cloned.remove(ACTION_KEY);
            Value::Object(cloned)
        }
        None => args.clone(),
    }
}

#[cfg(test)]
mod test;
