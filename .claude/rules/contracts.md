---
paths:
  - "contracts/**"
  - "**/tests/contract_*.rs"
  - "**/tests/contract_*/**"
  - "**/*.contract.test.ts"
  - "**/*.contract.test.tsx"
  - "**/*.contract.spec.ts"
  - "**/e2e/contract/**"
---

# Contracts

- A change here can break the other side. Treat each file as a published interface.
- `contracts/openapi.json` is generated from the Rust API. Never edit it by hand: change the backend, then run `just openapi`.
- Contract tests come first. Write them, register them in `contracts/contracts.toml`, and commit them before the implementation.
- The owner signs `contract/<feature>/v<N>`. After that, a pinned file never changes. To change a contract, bump the feature's `version` in `contracts.toml` first, then edit, and the owner signs the new tag. Never lower a version.
- Never create, move or push tags.
