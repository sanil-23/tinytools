# Collapsed tools and standard helpers

**Status:** Implemented  
**Owner:** tinytools maintainers

## Problem

Hosts need to expose related tools as one action-dispatched tool without losing
the members' parameter schemas or per-call permission and effect decisions.
Standard helpers also need consistent contracts for PATH discovery, concurrent
file-state tracking, and URL validation before a host performs network I/O.

## Goals and non-goals

Goals are to define the public contracts for action collapse and the
`tinytools-std` helpers changed alongside it. This includes action validation
and schema merging, executable discovery on supported platforms, per-agent
read/write staleness tracking, and URL authority/DNS validation results.

These helpers describe data and classifications. They do not execute tools,
enforce permissions, perform HTTP requests, pin connections, or replace host
coordination around file mutation.

## Proposed behavior

- A collapsed family is non-empty, has unique action names, and reserves the
  `action` parameter for dispatch. Its merged schema preserves member
  properties and namespaces definitions and local references. Static
  permission is the minimum across members; static external-effect
  classification is conservative for any non-empty family. Argument-aware
  classification delegates to the selected member after removing `action`.
- PATH discovery returns executable candidates, accounting for `PATHEXT` on
  Windows and execute access on Unix.
- File-state records reads before I/O and retains write timestamps for
  staleness checks as well as every writer's path attribution.
- URL validation rejects malformed or ambiguous authorities, resolves the
  host, rejects non-global destinations (including translated IPv4), and
  returns the validated URL, host, and vetted socket addresses for the host to
  use when connecting.

## Invariants and constraints

- The vocabulary crate remains independent of harnesses, transports, runtimes,
  and native libraries.
- A schema merge must not introduce dangling references when namespacing
  definitions. Draft-07 `additionalItems` and schema-valued `dependencies`
  contain schemas; array-valued dependencies are property-name lists.
- Unknown actions receive conservative classifications and fail before a
  member executes.
- URL validation is not an end-to-end SSRF guarantee: callers must connect to
  vetted addresses, preserve the hostname for TLS and Host, and revalidate
  redirects.
- File-state tracking coordinates cooperating callers and does not authenticate
  agent identities or lock files by itself.

## Acceptance criteria

- Invalid action families are rejected, and merged schemas retain namespaced
  definitions with all local references rewritten.
- Per-call classification reaches only the selected member with the dispatch
  key removed.
- PATH lookup tests cover platform-specific candidate and executable behavior.
- File-state tests cover concurrent reads/writes, stale reads, and attribution
  for multiple writers of one path.
- URL tests reject private and local destinations while allowing public IPv4
  addresses synthesized through the well-known NAT64 prefix.
- Public APIs and their host-side operational constraints are documented.

## Open questions

None for the implemented contract. Hosts remain responsible for the
connection-pinning and locking sequences described above.

Implementation sequence: [collapsed-tools-and-standard-helpers plan](../plans/collapsed-tools-and-standard-helpers.md).
