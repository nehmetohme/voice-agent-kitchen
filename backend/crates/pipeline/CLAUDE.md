# vak-pipeline: real-time voice pipeline

## Responsibility

This crate runs the real-time voice path, with one session actor task per call: `run_session(SessionConfig, Engines, MediaTransport) -> SessionOutcome`. It also plays the same reply logic in text for test runs: `run_text_session` (below).

Today (M4a):

- **Disclosure first.** Nothing is sent before the client's `session.start`. The session answers `session.started`, then synthesizes the locked disclosure word for word and plays it as turn 0, which the caller can't interrupt. If that synthesis fails, the session ends with `Error`.
- **Caller turns.** Audio arrives as 20 ms frames (the transport's `InputFramer` cuts them), and the VAD (default `EnergyVad`; `SessionConfig::with_vad`) classifies each one. Muted audio counts as silence. The turn detector keeps 200 ms of audio from before the speech, ignores clicks under three speech frames, caps an utterance at 30 s and ends it after the end-of-turn silence. STT runs per utterance, and transcripts are released in speech order even when STT answers out of order.
- **Replies.** Each caller turn is checked against the opt-out phrases, and the decision model gets its `opt_out` question in parallel with the LLM. The LLM's text is cut into clauses, at most two clauses are synthesized ahead of playback, and the audio is paced out at one 24 kHz frame per 20 ms. A reply waits up to `decision_wait` (default 300 ms) for the opt-out answer before it plays. A failed stage sends an `error` event (`provider_unavailable`) and the session goes on.
- **Barge-in.** Speaking over a reply for the barge-in threshold (default 300 ms; `TurnPolicy`) cancels it at once. The session sends `agent.interrupted`, then a `transcript.final` with `interrupted: true` and only the words the caller heard. Speaking while a reply is still being prepared cancels it too, and the next reply answers both turns. `TurnPolicy::without_barge_in` lets the agent always finish.
- **Three-layer opt-out.** Any one layer ends the session:
  1. the phrase list (`vak_domain::OptOutPhrases`)
  2. the decision model's yes/no `opt_out` question, at or above the decision threshold (default 0.8). From the confirm threshold (default 0.3) up to it, the agent asks first: see [The confirm band](#the-confirm-band)
  3. the LLM calling the `mark_do_not_call` tool, which every LLM request offers

  The session then cancels the reply and the pending decisions, speaks the locked confirmation as the next agent turn (uninterruptible), and sends `session.ended` with `reason: opt_out` and the layer. A decision that times out or fails is reported as `status: timeout` or `error`, never as a no. An answer that comes after the wait but before the deadline (default 2 s) still ends the session.
- **Ending.** The session also ends on the client's `session.end` or a closed transport (`ClientEnded`), and after `max_duration` (default 30 minutes; `Timeout`).
- **Latency.** When a reply's first audio goes out, its domain `TurnTimeline` gives its `StageLatencies`. The session sends them as a `latency` event, keeps them in the outcome's `TurnRecord`, and passes them to `SessionObserver::reply_latencies`, where `vak-services` keeps the histograms. Each opt-out decision's time goes to `SessionObserver::decision_latency`. The target is P50 ≤ 1.2 s and P95 ≤ 2.0 s mouth-to-ear.
- `transcript.partial` and `plan.node` are not sent yet.

### The confirm band

Confirm when unsure (`opt-out-confirm` v1; the thresholds and the live scores behind them are in `contracts/decisions/README.md`). An `opt_out` probability below the confirm threshold (0.3) does nothing, one at or above the decision threshold (0.8) ends the session at once, and one in between makes the agent ask the check question (`OptOutPolicy::with_check_question`, default `vak_domain::DEFAULT_OPT_OUT_CHECK_QUESTION`: "Just to confirm, do you want us to stop contacting you?"). The phrase list and the LLM tool still end the session at once.

- **The reply is replaced.** An in-band score within the decision wait drops the reply unspoken: its clause syntheses stop (its `tts` token), but its LLM stream runs on as `listening`, only so that `mark_do_not_call` still ends the session, until the stream ends, the next reply starts (a one-slot LLM would queue behind it) or the session opts out. A **late** in-band score cuts the playing reply the way barge-in does (`agent.interrupted`, then the words heard, which is all the history keeps) the moment the question is synthesized, and the question starts in the same tick, so there is no gap; a reply that ends first isn't cut. The media protocol has no other way to drop the browser's queued audio, so it is unchanged. Until the question starts (`CheckState::Synthesizing`, `Ready`) no reply starts or plays, and it waits for the disclosure.
- **The check question** (`TurnKind::Check`) is spoken word for word and uninterruptible. It enters the history when it starts, so speech over it is its answer, judged with the question in context. Its `transcript.final` is the question. It answers the pending caller turns, so it sends a `latency` event and keeps a `TurnRecord` like a reply, with `llm_ttft_ms: null` (after a cut reply, only `tts_first_audio_ms`).
- **The answer** is the first caller turn after it starts (`CheckState::Asked`), and its decision settles the check. At or above the decision threshold the session ends (layer `decision`, the locked confirmation); anything else, a timeout or failure included, carries on with the LLM's reply, which saw the question, without asking again. The answer keeps the decision threshold, not a lower one: the question in context lifts every answer ("No." 0.55, "What?" 0.70, "Yes." 0.94). A later in-band turn may ask again; one check at a time.
- **Recall never drops below the old behavior:** a check question that can't be synthesized sends an `error` event and ends the session as an opt-out (layer `decision`), as any score from 0.3 did before; so does a blank one. An opt-out during a check cancels a question not yet spoken; one being spoken finishes before the confirmation.
- **M4b** moves the question, the confirmation, the phrases and the thresholds into new rule pack versions (the v1 files are pinned by `plan`). Until then they are platform defaults: `vak_domain` constants that `SessionScript` hands to `vak-services`, and `OptOutPolicy`'s defaults.

| Module | Holds |
|---|---|
| `session` | The actor: turn state, playback, barge-in, opt-out and its check question |
| `stages` | Model calls as child tasks reporting `StageEvent`s |
| `detector` | `TurnDetector`: VAD per frame, utterances, speech runs for barge-in |
| `chunker` | `ClauseChunker`. The first clause ends at a sentence end, at a comma, semicolon or colon after four words, or after twelve words. Later clauses end at sentence ends or after 25 words. A boundary counts only once whitespace follows it |
| `playback` | `Playing`: one agent turn's clauses, cut into 20 ms frames, and which words were sent; `TurnKind` (disclosure, reply, check question, confirmation) |
| `routing` | `Router`, the seam for plan routing (below) |
| `config` | `Engines`, `SessionConfig`, `TurnPolicy`, `OptOutPolicy` (phrases, confirmation, check question, decision and confirm thresholds, decision wait and deadline), `StageDeadlines` (STT, LLM first token and TTS; 10 s each by default), `SessionObserver` |
| `outcome` | `SessionOutcome` (end reason, turns) and `TurnRecord` |
| `text` | Text-mode conversations for test runs (below): `run_text_session`, `TextSessionConfig`, `TextEngines`, `TextDeadlines`, the `SimulatedCaller` trait, `LlmCaller`, `CallerTurn`, `TextOutcome` |

### Text mode (M5)

`run_text_session(&TextSessionConfig, &TextEngines, &mut dyn SimulatedCaller) -> TextOutcome` plays one conversation line by line, for test runs. The agent side is the voice session's, through the same `Router`: the locked disclosure first, the instructions, history and tools, and the three opt-out layers with the confirm band (`OptOutPolicy`). There is no STT, TTS, VAD, pacing or barge-in, so a conversation is as fast as its model calls; the audio actor is unchanged.

- **Per caller line:** the phrase list first (a match ends it before any model call), then the `opt_out` decision and the LLM reply in parallel (`tokio::join!`). The decision is always awaited, up to its deadline: in text there is no reply to hold back. At or above the decision threshold, or on `mark_do_not_call`, the agent says the locked confirmation and it ends (`OptOut`); in the confirm band the check question replaces the reply and the next line answers it, never twice in a row. A decision that times out or fails is never a no.
- **The caller:** `LlmCaller` sends the scenario's instructions as the system message, the agent's lines as `user` and its own as `assistant`, and offers `end_call` (`CALLER_TOOL`) to hang up; a blank reply hangs up too. Any `SimulatedCaller` works (tests use their own).
- **Endings:** the caller hangs up (`ClientEnded`; its last words are recorded, not answered), an opt-out, the turn cap (`Timeout`), or a model failure (`Error`).
- **Deadlines and sampling:** every LLM request has a first-token deadline and the whole reply a total one (`TextDeadlines`), and samples greedily with the config's seed; the decision has its policy's deadline. Dropping the future drops every request in flight.
- Audio-mode simulation (TTS for the caller, STT for the agent) is a later option.

**The routing seam (M4b).** `Router` builds each turn's decision request and LLM request:

- **Decision request:** the last four turns and the platform question `OPT_OUT_QUESTION` (`opt_out`).
- **LLM request:** the agent's prompt plus fixed voice rules, the last 40 messages, the `OPT_OUT_TOOL` (`mark_do_not_call`) and at most 200 tokens.

Plan routing replaces these inputs with the plan node's. It adds a `branch` choice to the same decision request, so it costs no extra round trip, and swaps in the node's prompt and tools. The actor only calls `Router`, so it doesn't change.

Later: plan execution (M4b), model VADs and turn detection on inference threads, and persisting call turns through a sink trait that services provide (test runs store their text transcripts already).

## Dependencies

- **May depend on:** `vak-providers`, `vak-domain`, `vak-audio`. Dev-only: `vak-testkit`.
- **External crates:** `tokio` (`macros`, `rt`, `sync`, `time`), `tokio-util` (`CancellationToken`) and `tracing`. Dev-only: `jsonschema`, `serde_json`, and tokio's `test-util`. Later: a channel crate for inference threads (`crossbeam-channel`, `flume` or `rtrb`).
- **Must not depend on:** repositories or repositories-pg, services, api, any adapter, axum, sqlx, reqwest. Services reach into a session only through `SessionConfig` and `SessionObserver`.

## Conventions

- **The actor owns the turn state.** It runs a `biased` `select!` over the client's input, the stage reports, the 20 ms pacing tick, the end-of-turn timer, the opt-out decision's wait and the session limit.
- **Stage I/O runs in child tasks** (`stages`), spawned with a child `CancellationToken` each. A task drops the engine's future, or the LLM stream, as soon as its token is cancelled, which aborts the request. A `tokio::time::timeout` at the stage's deadline is the backstop for an engine that ignores its own. When the session ends, cancelling its root token stops every task.
- **Bounded channels only.** Each edge declares its overflow policy in a comment next to the channel:

  | Edge | Capacity | Policy |
  |---|---|---|
  | client → session (media inbound) | 64 items | Audio is dropped when full (`PeerSender::push_audio`); events wait |
  | session → client (media outbound) | 256 items | The actor awaits `send` (backpressure) |
  | stage tasks → actor | `STAGE_CAPACITY` = 64 | Tasks await `send`. The actor drains it continuously, and a task sends at most a few events per stage |

- **Barge-in:** each reply has a child `CancellationToken`, and its clause syntheses get children of its `tts` token, itself a child (so dropping a reply for the check question stops them and keeps the LLM stream). Stage reports carry their reply or utterance ID, and reports from a cancelled reply are ignored. An interrupted turn's `Playing` is dropped, so none of its frames go out after it.
- **Locked lines:** the disclosure, the opt-out check question and the opt-out confirmation are spoken word for word, and the caller can't interrupt them.
- **No blocking in async code:** stay under about 100 µs between `.await`s.
  - ONNX inference (later) runs on dedicated `std::thread` workers.
  - `spawn_blocking` is only for occasional work.
  - Never use `std::thread::sleep`; clippy disallows it.
- **No per-frame allocation, `format!` or tracing span.**
- **Pacing:** `tokio::time::interval(20 ms)` with `MissedTickBehavior::Delay`, after a three-frame head start for each agent turn, so the browser's playback never starves.
- **Latency metrics:** every reply's stage timings are derived from the domain `TurnTimeline` and reported to the `SessionObserver`.
- **Logging:** never log transcripts or audio. The session logs its ID at open and end, and a warning for each failed stage.

## Testing

- **Deterministic simulation:** `#[tokio::test(flavor = "current_thread", start_paused = true)]`.
  - testkit's `fake_transport()` gives a `FakeClient` that sends one 20 ms frame of 16 kHz audio every 20 ms of tokio time (a loud square wave for speech, zeros for silence) and records every server message with its virtual time.
  - `LevelVad` detects speech deterministically.
  - Scripted STT, LLM, TTS and decision fakes answer after virtual delays and record their calls.
- **Assertions:** they run on the timestamped event log. For example, barge-in sends `agent.interrupted` within one frame and cancels the LLM request, and transcripts keep speech order. Every event a session sends must validate against `contracts/media/vak.media.v1.json`.
- **Contracts (`voice-core` v1):**
  - `tests/contract_voice_core_pipeline.rs`: disclosure first, barge-in, turn order, stage timings.
  - `tests/contract_voice_core_opt_out.rs`: each layer alone, the confirmation, and a slow or failing decision model never suppressing an opt-out. It configures the decision threshold 0.5 and scores 0.0 and 0.9, so the confirm band changes none of it.
  - Both share `tests/contract_voice_core/mod.rs`.
- **Contract (`opt-out-confirm` v1):** `tests/contract_opt_out_confirm.rs`, on the same shared setup with the default thresholds: the band's edges, the check question replacing the reply (and its latency event), the late cut, a yes ending the call and anything else carrying on without a second check, speech over the question as its answer, the LLM tool while a check is pending, and a check question that can't be synthesized.
- **Confirm band edges** (`tests/session_confirm.rs`, not a contract): a blank check question, a configured floor, an answer whose decision times out, a later in-band turn asking again, an opt-out while the question plays, and the dropped reply's LLM stream (runs to its end unspoken; cancelled once the caller answers).
- **Failure and limit paths** (`tests/session_failures.rs`, not a contract; it reuses the contracts' setup as is): the disclosure's synthesis failing ends the session, a failed STT or LLM (or an LLM past its deadline) leaves the turn unanswered and the call going, a failed clause is skipped, an opt-out still ends the call when its confirmation can't be spoken, the time limit, audio that isn't PCM16, and the latencies the observer is told.
- **Unit tests** next to the code cover the chunker, the turn detector, playback and routing.
- **Text mode** (`tests/text_session.rs`, not a contract): the endings, the opt-out layers and the confirm band, deadlines, and the caller's requests, with scripted engines. The `test-runs` v1 contract (`vak-services`) covers it through test runs.
- **Inline inference in simulations,** behind its trait. Paused time doesn't wait for our OS threads.
- **CPU cost per frame:** criterion or gungraun benchmarks, not unit tests.

## Coverage floor

**90%** line coverage (`coverage.toml`).

## Example

How a stage task stops on barge-in (`src/stages.rs`):

```rust
let result = tokio::select! {
    biased;
    // Cancelled: dropping `call` aborts the request.
    () = cancel.cancelled() => return,
    result = within(deadline, call) => result,
};
```
