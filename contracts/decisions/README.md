# contracts/decisions/

Platform decision questions for the System One decision model: opt-out, escalation and call outcome. They will live here as `*.toml`, checked by domain unit tests and the offline eval set. There are no files yet; see `../CLAUDE.md`.

## What the voice core asks today (M4a)

One question, defined in code (`backend/crates/pipeline/src/routing.rs`) until files here replace it:

| ID | Type | Instructions |
|---|---|---|
| `opt_out` (`OPT_OUT_QUESTION`) | yes/no (System One `noul`) | "Is the caller asking not to be called or contacted again, or to be removed from a calling list?" |

- **When:** the pipeline asks it about every caller turn, in one decision request with the last four turns, in parallel with the LLM.
- **Threshold:** a probability of yes at or above 0.3 (recall first) ends the session with opt-out layer `decision`. The reply waits up to 300 ms for the answer; a later answer, within the 2 s deadline, still ends the session.
- **Never a no:** a timeout or a failure is reported as `status: timeout` or `error` in the `decision` event, without a probability. The phrase list and the LLM's `mark_do_not_call` tool still apply.

Plan routing (M4b) adds a `branch` choice over the current node's outgoing edges to the same request.

## Rules every question follows

These come from the domain types in `backend/crates/domain/src/decision.rs`:

- **ID** (`QuestionId::parse`): lowercase ASCII letters, digits and underscores, starting with a letter, 1 to 64 characters, such as `opt_out` or `branch2`.
- **Kinds** (`Question`): `yes_no(instructions)`, answered with the probability of yes, or `choice(instructions, options)`, with 1 to 255 options whose labels are unique and 1 to 128 characters long. Each option may have a description.
- **Answers** (`Answer::fits`): an answer must have its question's type, with every probability from 0 to 1. A choice must name a known label, with exactly one probability per option. An answer that doesn't fit is a protocol error, never a decision.
