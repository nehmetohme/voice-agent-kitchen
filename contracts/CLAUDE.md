# contracts/: the agreements between parts

## Responsibility

A change here can break the other side. Treat every file as a published interface.

| Path | What | Edit? |
|---|---|---|
| `openapi.json` | REST API contract, **generated** from the Rust code (utoipa in `vak-api`) by `vak-server openapi`. OpenAPI 3.1, sorted keys. | **Never by hand.** Change the backend, run `just openapi`, commit it with the regenerated frontend client. |
| `contracts.toml` | Contract-test manifest: `contract_globs` and one `[features.<name>]` per contract with its `version` and `paths` | Yes, to register a contract or bump a version (never lower one) |
| `contracts.just` | Recipes: drift checks, `oasdiff`, `contract-check`, `contract-status`, `contract-pin`, `contract-push` | Yes |
| `serving/` | `stt.json`, `tts.json`: JSON Schemas of the Databricks STT/TTS invocation bodies (`serving/README.md`) | Only with a version bump: the `model-shim` contract tag pins both |
| `rule-packs/` | Compliance rule packs, `<id>.v<version>.toml` (`rule-packs/README.md`), loaded by `vak-services` | Add a file for a new version; the `plan` contract tag pins the v1 files |
| `media/` | `vak.media.v1.json`, `examples.json`: the media WebSocket's events (`media/README.md`) | Only with a version bump: the `voice-core` contract tag pins both |
| `decisions/` | Decision questions (`decisions/README.md`): what the voice core and test runs ask | Placeholders until their milestones |

The checker lives in `backend/xtask/src/contracts.rs`. `.github/contract-signers` (owner-only) holds the SSH public keys allowed to sign contract tags. It is kept but unused while signatures are off (see [Signatures](#signatures-off-for-now)).

## The contract workflow (TDD with contract tags)

1. **Write the contract tests first** in paths matching `contract_globs`: `**/tests/contract_*.rs`, `**/tests/contract_*/**`, `**/*.contract.test.ts(x)`, `**/*.contract.spec.ts` or `frontend/e2e/contract/**`.
2. **Register them** in `contracts.toml` under `[features.<name>]` with `version = 1`, then commit and push for review. Local gates warn "awaiting its contract tag: untagged contract file …: the coordinating session pins contract/<name>/v1 with `just contract-pin <name> 1` …" and pass. CI's strict check fails, so the PR can't merge until the tag exists: that is the approval gate.
3. **The coordinating session pins them.** In a clean checkout of the commit that holds the tests, `just contract-pin <name> 1` creates the unsigned annotated tag `contract/<name>/v1` at HEAD, and `just contract-push <name> 1` pushes exactly that tag. Task agents never create, move, delete or push tags.
4. **Implement.** The check stays green while every file in the tag's file set is byte-identical at HEAD. Never edit a tagged file.
5. **To change a contract,** bump `version` in `contracts.toml` first, then edit the files and commit. The coordinating session pins `contract/<name>/v<N+1>`.

## What `contract-check` enforces

Run as `cargo xtask contract-check --base <trusted ref> --head <ref>`. For each feature in HEAD's manifest:
- `contract/<name>/v<version>` exists and is an annotated tag object with that name. Signed and unsigned tags pin alike (see [Signatures](#signatures-off-for-now)).
- The version is not lower than the base's, and no feature the base has may disappear.
- The file set comes from the **tag's own** manifest (which must pin that version), and every file in it is blob-identical at HEAD. Blobs are compared, not ancestry, so squash merges pass.
- Every file at HEAD matching the base's or HEAD's `contract_globs` is covered by a valid tag.

**Two modes.** Findings fall into two classes:

| Class | Findings | Strict (CI: `contracts-check-ci`) | `--allow-pending` (local: `verify`, `verify-fast`, git hooks, Stop gate) |
|---|---|---|---|
| **Pending** | the tag for HEAD's version doesn't exist yet; an untagged contract file | fails | warns "awaiting its contract tag: … `just contract-pin <f> <v>` …" and passes |
| **Violation** | a tagged file changed or deleted; a lower version; a removed feature; a lightweight tag, a tag with the wrong name, a manifest that doesn't pin its version, or no files; with `--require-signatures`, a bad or unknown signature | fails | fails |

So an agent can commit and push contract tests for review, but nothing merges until the tag is pinned, and no gate ever tolerates a change to a pinned contract.

Defence in depth: CI runs the **base branch's copy** of the checker (`just contracts-check-ci`). The `contract-tags` ruleset lets only repository admins create, move or delete `refs/tags/contract/**`, so the coordinating session pushes with the owner's credentials. CODEOWNERS covers these paths. Claude Code hooks deny `git tag`, tag pushes and edits to tagged or generated files.

## Signatures (off for now)

The owner turned contract-tag signatures off, for now. Today:

- `contract-check` and `contract-status` don't verify signatures by default. They never run `git verify-tag` and never read `.github/contract-signers`.
- Signed and unsigned tags pin alike. The first four tags (`contract/{health,app-shell,model-shim,agent-provisioning}/v1`) are signed. `just contract-pin` makes unsigned ones.
- Nothing cryptographic ties a tag to the owner, so the tag ruleset, the hooks and the permission rules are what keep task agents from pinning.
- `.github/contract-signers` stays in place, unused.

`--require-signatures` brings back the signed checks exactly as they were:
- `git verify-tag` must accept each tag with the **base** ref's `.github/contract-signers`, so a key added only in HEAD doesn't count. An unsigned tag, or one signed by an unknown key, is a `TagUnverified` violation.
- **Bootstrap:** a base from before the contract system (no `contracts.toml` and no signers file) has no trusted keys. Then the run trusts HEAD's signers file, with a loud `BOOTSTRAP` warning. From the next PR on, only the base's copy counts.

To turn signatures back on:
1. Add `--require-signatures` to the `contract-check` calls in `contracts-check-ci` and `_contracts-check`, and optionally to `contract-status`. CI runs the base's checker binary, but with HEAD's recipe flags, so the flag takes effect in the PR that adds it.
2. Bring back a signing recipe. The old `contract-approve` is in git history: `git log -S contract-approve -- contracts/contracts.just`.
3. Every feature whose current tag is unsigned fails then. Bump each one and sign the new version.

Bootstrap of the checker itself (any mode): when the base has no `xtask contract-check`, `contracts-check-ci` runs HEAD's copy with a loud `BOOTSTRAP` warning.

## Recipes

Run them from the repo root.

| Recipe | Does |
|---|---|
| `just contracts-verify` | OpenAPI drift, client drift (regenerates orval), `oasdiff breaking` against the base, then `contract-check --allow-pending` **last** |
| `just contracts-verify-fast` | The same without regenerating the frontend client |
| `just openapi` | Regenerate `openapi.json` and `frontend/src/shared/api/generated/` |
| `just contract-status` | Each feature with its version and tag state, plus untagged files |
| `just contract-pin <feature> <version>` | **Coordinating session only.** Creates the unsigned annotated tag `contract/<feature>/v<version>` at HEAD, with the message `<feature> v<version>`. Never pushes. Refuses an invalid name or version, a dirty tree, a HEAD manifest without that feature at that version, and a tag that already exists locally or on origin (`git ls-remote`). Never moves or deletes a tag. |
| `just contract-push <feature> <version>` | **Coordinating session only.** Pushes exactly `refs/tags/contract/<feature>/v<version>` to origin, and the pre-push hook runs as usual. Refuses a tag that doesn't exist locally, or one that origin has at another object. Exits 0 when origin already has the same object. Never forces. |
| `just contracts-check-ci <base> <head>` | CI: builds and runs the base ref's checker from a temporary worktree, **strict** |

The base defaults to `origin/main`. Set `VAK_CONTRACTS_BASE` to override it, and run `git fetch origin main` first so the base is current. CI needs the base commit and the `contract/*` tags fetched.

## Never

- Hand-edit `openapi.json` or `frontend/src/shared/api/generated/**`.
- Edit a file covered by a contract tag, or lower a version.
- Create, move, delete or push a tag by hand. Contract tags come only from `just contract-pin` and `just contract-push`, run by the coordinating session.
- Edit `.github/contract-signers`.

## Example

```sh
# A new contract for agent provisioning, in the feature's worktree:
$EDITOR backend/crates/services/tests/contract_agent_provisioning.rs
# contracts.toml:  [features.agent-provisioning]  version = 1
#                  paths = ["backend/crates/services/tests/contract_agent_provisioning.rs"]
git commit -am "test(contract): agent provisioning v1"
just contract-status        # agent-provisioning  1  contract/agent-provisioning/v1  tag not found

# Then the coordinating session, in a clean checkout of that commit:
just contract-pin agent-provisioning 1     # creates contract/agent-provisioning/v1 (unsigned)
just contract-push agent-provisioning 1    # pushes it; the pre-push hook runs `just verify`
```
