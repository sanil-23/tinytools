# Collapsed tools and standard helpers implementation plan

Specification: [Collapsed tools and standard helpers](../specs/collapsed-tools-and-standard-helpers.md)

**Status:** Complete

## Goal and assumptions

Implement the public contracts in the linked specification across `tinytools`
and `tinytools-std`. The contracts are host-facing helpers; enforcement and
network/file operations remain in consuming hosts.

## Ordered tasks

1. **Define collapsed action contracts** in
   `crates/tinytools/src/collapse/`. Add tests first for invalid action sets,
   property unions, namespaced references, and per-call classification; then
   implement validation, schema merging, conservative static answers, and
   selected-member delegation. Update exports and module documentation.
2. **Define PATH discovery behavior** in
   `crates/tinytools-std/src/detect_tools/`. Add tests for candidate names and
   executable checks before implementing platform-aware discovery. Document
   the public entry point.
3. **Define file-state coordination** in
   `crates/tinytools-std/src/file_state/`. Add tests for reads captured before
   I/O, stale reads during concurrent writes, and attribution retained for
   multiple writers before updating the tracking implementation.
4. **Define URL validation behavior** in
   `crates/tinytools-std/src/url_guard/`. Add rejection/acceptance tests for
   authority parsing, DNS results, and vetted addresses before updating the
   validator. Document caller requirements for connection pinning and
   redirects.
5. **Verify the workspace** with formatting, Clippy, build, tests, and rustdoc.

## Completion checklist

- [x] Each behavior change has focused regression coverage.
- [x] Public exports and module/crate documentation describe the contracts.
- [x] Draft-07 schema references under `additionalItems` and object-valued
  `dependencies` are rewritten; dependency name arrays remain untouched.
- [x] The well-known NAT64 prefix permits public embedded IPv4 addresses and
  rejects non-global embedded IPv4 addresses.
- [x] `cargo fmt --all -- --check` passes.
- [x] `cargo clippy --all-targets --all-features -- -D warnings` passes.
- [x] `cargo build --all-targets --all-features` passes.
- [x] `cargo test --all-features` passes.
