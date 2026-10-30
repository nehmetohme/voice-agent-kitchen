# vak-domain: pure types and rules

## Responsibility

This crate holds the domain model and nothing else:

- entities, value objects and IDs
- the plan graph and its validation
- rule packs and decision types
- lifecycle state machines (agent, version, test run, session)
- `TurnTimeline` and the stage timestamps that metrics are derived from

It is the bottom of the stack, and every other layer speaks its types.

## Dependencies

- **May depend on:** no workspace crate.
- **External, normal and build:** only `serde` (derive), `thiserror`, `uuid` (v7 IDs) and `jiff` (time). This is the `external` list in `layers.toml`.
- **External, dev only:** additionally `proptest` and `insta`.
- **Must not depend on:** tokio or any async runtime, sqlx, axum, reqwest, tracing exporters, any adapter, or any other `vak-*` crate.
- `cargo xtask check-layers` fails on anything else.

## Conventions

- **No I/O, no async, no clocks, no randomness.** Callers pass in `now: jiff::Timestamp` and IDs, so every rule is deterministic.
- **Pure rules:** every rule is a pure function or method that returns `Result<T, DomainError>`.
- **Errors:** they are `thiserror` enums of rule violations, with no I/O error types. Name the rule in the variant: `PlanError::UnreachableNode { node }`.
- **Validated types:** make invalid states unrepresentable with newtypes and constructors that validate (`AgentName::parse`). Fields stay private.
- **IDs:** typed wrappers over `uuid::Uuid` v7 (`AgentId`, `VersionId`), never bare `Uuid` in signatures.
- **Serde:** DTOs live in `vak-api`, not here. Serde derives exist only for persisted snapshots, such as published versions stored as JSONB.
- **Panics:** never use `unwrap`, `expect` or `panic!`. Index safely (`get`) and return errors.
- **Visibility:** `pub(crate)` is the default; export only what other layers need.

## Testing

- Unit tests sit next to the code in `#[cfg(test)] mod tests`.
- Property tests (`proptest`) cover state machines, schedule windows, plan validation and round-trips.
- Tests need no fakes, containers or async runtime. If one does, the code is in the wrong layer.

## Coverage floor

**95%** line coverage (`coverage.toml`). Mutation testing (`cargo mutants --in-diff`) runs nightly on this crate as a report.

## Example

This shows the pattern; the type doesn't exist yet:

```rust
/// Display name of an agent: 1-80 characters after trimming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentName(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentNameError {
    #[error("agent name is empty")]
    Empty,
    #[error("agent name is longer than {max} characters")]
    TooLong { max: usize },
}

impl AgentName {
    pub const MAX: usize = 80;

    pub fn parse(raw: &str) -> Result<Self, AgentNameError> {
        let name = raw.trim();
        match name.chars().count() {
            0 => Err(AgentNameError::Empty),
            n if n > Self::MAX => Err(AgentNameError::TooLong { max: Self::MAX }),
            _ => Ok(Self(name.to_owned())),
        }
    }
}
```
