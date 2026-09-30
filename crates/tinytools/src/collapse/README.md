# Collapsed actions

This module combines a family of related tools behind one action-dispatched
tool. It derives the combined parameter schema from the member tools, validates
that action names and schemas are safe to combine, and provides helpers for
dispatching classification decisions to the selected member.

## Public surface

- [`CollapsedAction`](types.rs) pairs a stable action name with its tool.
- [`validate_actions`](mod.rs) rejects empty families, duplicate action names,
  and member schemas that use the reserved `action` property.
- [`merge_action_schemas`](mod.rs) unions member properties and namespaces
  member-local `$defs` and draft-07 `definitions` so local references remain
  valid after merging.
- Permission and external-effect helpers expose the static minimum/maximum
  classifications and select the member-specific classification for a call.
- [`CollapseError`](types.rs) describes invalid action families.

## Schema and classification constraints

The merged schema requires only the `action` discriminator. A union cannot
express that a property is required for one action but optional for another, so
the selected member remains responsible for validating its own arguments.
Conflicting definitions for a shared property are preserved as `anyOf`
alternatives. Member-local definition names include the action in the merged
namespace to prevent cross-member collisions.

The module describes classifications; it does not enforce permissions or run
tools. The host must use the argument-aware classification helpers at its
enforcement point and dispatch execution to the matching member.
