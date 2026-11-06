# contracts/decisions/

Platform decision questions for the System One decision model: opt-out, escalation and call outcome. They will live here as `*.toml`, checked by domain unit tests and the offline eval set. There are no files yet; see `../CLAUDE.md`.

## What the voice core asks today (M4a)

One question, defined in code (`backend/crates/pipeline/src/routing.rs`) until files here replace it:

| ID | Type | Instructions |
|---|---|---|
| `opt_out` (`OPT_OUT_QUESTION`) | yes/no (System One `noul`) | "Is the caller asking not to be called or contacted again, or to be removed from a calling list?" |

- **When:** the pipeline asks it about every caller turn, in one decision request with the last four turns, in parallel with the LLM.
- **Thresholds: confirm when unsure.** A probability of yes at or above **0.8** (the decision threshold) ends the session with opt-out layer `decision`. From **0.3** (the confirm threshold, recall first) up to 0.8 is the **confirm band**: instead of its reply, the agent asks the check question "Just to confirm, do you want us to stop contacting you?", and the caller's next turn decides. That answer is judged with the question in context and the same 0.8: at or above it the session ends with the locked confirmation (layer `decision`); below it the call goes on, without a second check. Below 0.3 nothing happens. The reply waits up to 300 ms for the answer; a later answer, within the 2 s deadline, still ends the session, or, in the band, cuts the playing reply for the check question. The pipeline's behavior is in `backend/crates/pipeline/CLAUDE.md` (The confirm band), and the contract `opt-out-confirm` v1 pins it.
- **Never a no:** a timeout or a failure is reported as `status: timeout` or `error` in the `decision` event, without a probability. The phrase list and the LLM's `mark_do_not_call` tool still apply, in or out of a check, and a check question that can't be synthesized ends the call as an opt-out.
- **Where the values live:** the thresholds are `OptOutPolicy`'s defaults and the check question is `vak_domain::DEFAULT_OPT_OUT_CHECK_QUESTION`, platform defaults for every agent. M4b moves them, with the opt-out phrases and the confirmation, into the rule packs (as new pack versions: the v1 files are pinned).

### Why 0.3 and 0.8 (live scores, 2026-11-03)

The first live test call, a survey, ended as a false opt-out: "It's a one." scored 0.343, above the old single threshold of 0.3. Replayed against the local decision model (Kev-4B Q4_K_M, llama-server `/v1/systemone`, `noul`, the instructions above, the pipeline's four-turn state):

| Caller said, after "How would you rate your overall experience with our service today, on a scale from 1 to 10?" | `opt_out` p (owner's replay) | p (this replay: disclosure, "Hi.", the question, the answer) |
|---|---|---|
| "It's a one." | 0.343 | 0.600 |
| "Eight." | 0.051 | 0.127 |
| "It was terrible, honestly." | 0.170 | 0.333 |
| "I'm not interested, leave me alone." | | 0.798 |
| "No thanks, and please stop calling me." | 0.73 (smoke fixture) | 0.955 |
| "Don't ever call me again." | | 0.953 |
| "Please remove my number." | | 0.953 |
| "I don't want to hear from you people again." | | 0.965 |
| "Please stop calling me." | 0.909 | 0.963 |
| "Take me off your list." | 0.957 | 0.984 |

Survey answers land anywhere from 0.05 to 0.6 depending on the context, so no single threshold separates them from real requests, which mostly score above 0.9. The band asks about everything in between, the ambiguous "leave me alone" included.

After the check question (state: the survey question, "It's a one.", the check question, the answer):

| Answer | p |
|---|---|
| "Yes." | 0.944 |
| "Yes, please." | 0.949 |
| "Yeah, stop." | 0.937 |
| "Um, I guess." | 0.941 |
| "What?" | 0.703 |
| "Sorry, what was that?" | 0.658 |
| "Two." | 0.613 |
| "No." | 0.554 |
| "No, go on." | 0.457 |
| "Just keep going." | 0.138 |

With the explicit question in context every answer scores higher, a no included, so the answer turn keeps the 0.8 threshold: a lower one (say 0.5) would end calls on "No." or "What?". The other wording tried, "Just to check, should we stop contacting you? Yes or no?", scored the same within 0.04 except "No, go on." (0.286) and "No." (0.421).

Plan routing (M4b) adds a `branch` choice over the current node's outgoing edges to the same request.

## Rules every question follows

These come from the domain types in `backend/crates/domain/src/decision.rs`:

- **ID** (`QuestionId::parse`): lowercase ASCII letters, digits and underscores, starting with a letter, 1 to 64 characters, such as `opt_out` or `branch2`.
- **Kinds** (`Question`): `yes_no(instructions)`, answered with the probability of yes, or `choice(instructions, options)`, with 1 to 255 options whose labels are unique and 1 to 128 characters long. Each option may have a description.
- **Answers** (`Answer::fits`): an answer must have its question's type, with every probability from 0 to 1. A choice must name a known label, with exactly one probability per option. An answer that doesn't fit is a protocol error, never a decision.
