# contracts/rule-packs/

Versioned compliance rule packs, one per use case and jurisdiction: `<id>.v<version>.toml`,
where `<id>` is `<jurisdiction>-<use case>` in lower case (`us-ca-booking`). Each pack holds the
locked texts a plan's start step opens with, word for word and in order (AI disclosure first,
then the recording notice), the opt-out phrases and the confirmation the agent says word for
word when the caller opts out (separate from the opening), the claims the agent must never
make, and the retention periods.

A pack is published with a version and never edited in place; a change is a new file with the
next version. The `plan` contract (`contracts/contracts.toml`) pins the version 1 files. Agents
pin a version, so a new version applies only to agents that move to it.

`vak-services` embeds the files at build time and the domain checks them when they load
(`backend/crates/services/tests/contract_plan_rule_packs.rs`). Version 1 covers every use case
in `US` and `US-CA`. This is product research, not legal advice: see
`docs/research/04-compliance-and-market.md`.
