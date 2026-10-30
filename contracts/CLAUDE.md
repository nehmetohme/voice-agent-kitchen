# contracts/: the agreements between parts

## Responsibility

A change here can break the other side. Treat every file as a published interface.

| Path | What | Edit? |
|---|---|---|
| `openapi.json` | REST API contract, **generated** from the Rust code (utoipa in `vak-api`) by `vak-server openapi`. OpenAPI 3.1, sorted keys. | **Never by hand.** Change the backend, run `just openapi`, commit it with the regenerated frontend client. |
| `contracts.toml` | Contract-test manifest: `contract_globs` and one `[features.<name>]` per contract with its `version` and `paths` | Yes, to register a contract or bump a version (never lower one) |
| `contracts.just` | Recipes: drift checks, `oasdiff`, `contract-check`, `contract-status`, `contract-approve` | Yes |
| `media/`, `serving/`, `decisions/`, `rule-packs/` | Media protocol, model-serving shapes, decision questions, compliance packs | Placeholders until their milestones |

Related: `.github/contract-signers` holds the SSH public keys allowed to sign contract tags (owner-only). The checker lives in `backend/xtask/src/contracts.rs`.

## The contract workflow (TDD with signed tags)

1. **Write the contract tests first** in paths matching `contract_globs`: `**/tests/contract_*.rs`, `**/tests/contract_*/**`, `**/*.contract.test.ts(x)`, `**/*.contract.spec.ts` or `frontend/e2e/contract/**`.
2. **Register them** in `contracts.toml` under `[features.<name>]` with `version = 1`, then commit. `contract-check` now fails with "untagged contract file …: ask the owner to run `just contract-approve <name> 1`". That failure is the approval gate.
3. **The owner reviews and signs:** `just contract-approve <name> 1` creates the signed tag `contract/<name>/v1` at HEAD and pushes it. Agents never create, move or push tags.
4. **Implement.** The check stays green while every file in the tag's file set is byte-identical at HEAD. Never edit a tagged file.
5. **To change a contract,** bump `version` in `contracts.toml` first, then edit the files, commit, and ask the owner to sign `contract/<name>/v<N+1>`.

## What `contract-check` enforces

Run as `cargo xtask contract-check --base <trusted ref> --head <ref>`. For each feature in HEAD's manifest:
- `contract/<name>/v<version>` exists, is a tag object with that name, and `git verify-tag` accepts it with the **base** ref's `.github/contract-signers`.
- The version is not lower than the base's, and no feature the base has may disappear.
- The file set comes from the **tag's own** manifest (which must pin that version), and every file in it is blob-identical at HEAD. Blobs are compared, not ancestry, so squash merges pass.
- Every file at HEAD matching the base's or HEAD's `contract_globs` is covered by a valid tag.

Defence in depth: CI runs the **base branch's copy** of the checker (`just contracts-check-ci`), tags are verified against the base's signers, CODEOWNERS covers these paths, and Claude Code hooks deny `git tag` and edits to tagged or generated files.

**Bootstrap:** a base from before the contract system (no `contracts.toml` and no signers file) has no trusted keys and no checker. Then the run trusts HEAD's signers file and checker copy, and says so with a loud `BOOTSTRAP` warning. From the next PR on, only the base's copies count.

## Recipes

Run them from the repo root.

| Recipe | Does |
|---|---|
| `just contracts-verify` | OpenAPI drift, client drift (regenerates orval), `oasdiff breaking` against the base, then `contract-check` **last**, reported on its own |
| `just contracts-verify-fast` | The same without regenerating the frontend client |
| `just openapi` | Regenerate `openapi.json` and `frontend/src/shared/api/generated/` |
| `just contract-status` | Each feature with its version and tag state, plus untagged files |
| `just contract-approve <feature> <version>` | **Owner only.** Interactive: clean tree, version matches HEAD's manifest, then `git tag -s` + `git push origin <tag>` |
| `just contracts-check-ci <base> <head>` | CI: builds and runs the base ref's checker from a temporary worktree |

The base defaults to `origin/main`. Set `VAK_CONTRACTS_BASE` to override it, and run `git fetch origin main` first so the base is current. CI needs the base commit and the `contract/*` tags fetched.

## Never

- Hand-edit `openapi.json` or `frontend/src/shared/api/generated/**`.
- Edit a file covered by a contract tag, or lower a version.
- Create, move, sign or push a tag, or edit `.github/contract-signers`.

## Example

```sh
# A new contract for agent provisioning, in the feature's worktree:
$EDITOR backend/crates/services/tests/contract_agent_provisioning.rs
# contracts.toml:  [features.agent-provisioning]  version = 1
#                  paths = ["backend/crates/services/tests/contract_agent_provisioning.rs"]
git commit -am "test(contract): agent provisioning v1"
just contract-status        # agent-provisioning  1  contract/agent-provisioning/v1  tag not found
```
