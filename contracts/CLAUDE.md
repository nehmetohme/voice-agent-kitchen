# contracts/: the agreements between parts

## Responsibility

A change here can break the other side. Treat every file as a published interface.

| Path | What | Edit? |
|---|---|---|
| `openapi.json` | REST API contract, **generated** from the Rust code (utoipa in `vak-api`) by `vak-server openapi`. OpenAPI 3.1, sorted keys. | **Never by hand.** Change the backend, run `just openapi`, commit it with the regenerated frontend client. |
| `contracts.toml` | Contract-test manifest: `contract_globs` and one `[features.<name>]` per contract with its `version` and `paths` | Yes, to register a contract or bump a version (never lower one) |
| `contracts.just` | Recipes: drift checks, `oasdiff`, `contract-check`, `contract-status`, `contract-approve` | Yes |
| `serving/` | `stt.json`, `tts.json`: JSON Schemas of the Databricks STT/TTS invocation bodies (`serving/README.md`) | Only with a version bump: the `model-shim` contract tag pins both |
| `media/`, `decisions/`, `rule-packs/` | Media protocol, decision questions, compliance packs | Placeholders until their milestones |

Related: `.github/contract-signers` holds the SSH public keys allowed to sign contract tags (owner-only). The checker lives in `backend/xtask/src/contracts.rs`.

## The contract workflow (TDD with signed tags)

1. **Write the contract tests first** in paths matching `contract_globs`: `**/tests/contract_*.rs`, `**/tests/contract_*/**`, `**/*.contract.test.ts(x)`, `**/*.contract.spec.ts` or `frontend/e2e/contract/**`.
2. **Register them** in `contracts.toml` under `[features.<name>]` with `version = 1`, then commit and push for review. Local gates warn "awaiting owner signature: untagged contract file …: ask the owner to run `just contract-approve <name> 1`" and pass. CI's strict check fails, so the PR can't merge: that is the approval gate.
3. **The owner reviews and signs:** `just contract-approve <name> 1` creates the signed tag `contract/<name>/v1` at HEAD and pushes it. Agents never create, move or push tags.
4. **Implement.** The check stays green while every file in the tag's file set is byte-identical at HEAD. Never edit a tagged file.
5. **To change a contract,** bump `version` in `contracts.toml` first, then edit the files, commit, and ask the owner to sign `contract/<name>/v<N+1>`.

## What `contract-check` enforces

Run as `cargo xtask contract-check --base <trusted ref> --head <ref>`. For each feature in HEAD's manifest:
- `contract/<name>/v<version>` exists, is a tag object with that name, and `git verify-tag` accepts it with the **base** ref's `.github/contract-signers`.
- The version is not lower than the base's, and no feature the base has may disappear.
- The file set comes from the **tag's own** manifest (which must pin that version), and every file in it is blob-identical at HEAD. Blobs are compared, not ancestry, so squash merges pass.
- Every file at HEAD matching the base's or HEAD's `contract_globs` is covered by a valid tag.

**Two modes.** Findings fall into two classes:

| Class | Findings | Strict (CI: `contracts-check-ci`) | `--allow-pending` (local: `verify`, `verify-fast`, git hooks, Stop gate) |
|---|---|---|---|
| **Pending approval** | the tag for HEAD's version doesn't exist yet; an untagged contract file | fails | warns "awaiting owner signature: … `just contract-approve <f> <v>`" and passes |
| **Violation** | a tagged file changed or deleted; an unsigned tag, or one signed by a key not in the base's signers (including a key added only in HEAD); a lower version; a removed feature; a tag with the wrong name, a manifest that doesn't pin its version, or no files | fails | fails |

So an agent can commit and push contract tests for review, but nothing merges until the owner signs, and no gate ever tolerates a change to a signed contract.

Defence in depth: CI runs the **base branch's copy** of the checker (`just contracts-check-ci`), tags are verified against the base's signers, CODEOWNERS covers these paths, and Claude Code hooks deny `git tag` and edits to tagged or generated files.

**Bootstrap:** a base from before the contract system (no `contracts.toml` and no signers file) has no trusted keys and no checker. Then the run trusts HEAD's signers file and checker copy, and says so with a loud `BOOTSTRAP` warning. From the next PR on, only the base's copies count.

## Recipes

Run them from the repo root.

| Recipe | Does |
|---|---|
| `just contracts-verify` | OpenAPI drift, client drift (regenerates orval), `oasdiff breaking` against the base, then `contract-check --allow-pending` **last** |
| `just contracts-verify-fast` | The same without regenerating the frontend client |
| `just openapi` | Regenerate `openapi.json` and `frontend/src/shared/api/generated/` |
| `just contract-status` | Each feature with its version and tag state, plus untagged files |
| `just contract-approve <feature> <version>` | **Owner only.** Interactive: clean tree, version matches HEAD's manifest, then `git tag -s` + `git push origin <tag>` |
| `just contracts-check-ci <base> <head>` | CI: builds and runs the base ref's checker from a temporary worktree, **strict** |

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
