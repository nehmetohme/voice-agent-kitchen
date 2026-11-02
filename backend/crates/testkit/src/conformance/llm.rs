//! The `LlmEngine` contract.

use std::time::Duration;

use vak_providers::{LlmEngine, LlmEvent, LlmRequest, Role, ToolSpec};

const DEADLINE: Duration = Duration::from_secs(20);
/// More events than any test reply produces: a stream that goes on longer never ends.
const MAX_EVENTS: usize = 4_096;

/// Checks the `LlmEngine` contract for text replies:
///
/// 1. A reply streams as one or more `Text` events and then exactly one `Done`, which is
///    the last event; the text is not blank, and no tool call appears when no tool is
///    offered.
/// 2. Dropping a stream after its first event frees the engine: the next request still
///    streams a complete reply within its deadline. (A one-slot local server stays busy
///    when a dropped request isn't aborted.)
///
/// # Panics
/// When `engine` breaks a rule.
#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
pub async fn llm_conformance(engine: &dyn LlmEngine) {
    let events = collect(engine, greeting(), "rule 1").await;
    let text = text_of(&events);
    assert!(!text.trim().is_empty(), "LLM rule 1: the reply has no text");
    assert!(
        matches!(events.first(), Some(LlmEvent::Text(_))),
        "LLM rule 1: the reply should start with text, got {:?}",
        events.first()
    );
    assert_done_once_at_the_end(&events, "rule 1");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, LlmEvent::ToolCall(_))),
        "LLM rule 1: a tool call without any tool offered"
    );

    let long = LlmRequest::new(DEADLINE)
        .with_message(
            Role::User,
            "Count from one to two hundred in words, separated by commas.",
        )
        .with_max_tokens(512);
    let mut stream = engine
        .stream(long)
        .await
        .unwrap_or_else(|error| panic!("LLM rule 2: the long request failed: {error}"));
    match stream.next_event().await {
        Some(Ok(LlmEvent::Text(_))) => {}
        other => panic!("LLM rule 2: the long reply should start with text, got {other:?}"),
    }
    drop(stream);
    let events = collect(engine, greeting(), "rule 2").await;
    assert!(
        !text_of(&events).trim().is_empty(),
        "LLM rule 2: the request after a dropped stream has no text"
    );
    assert_done_once_at_the_end(&events, "rule 2");
}

/// Checks the `LlmEngine` contract for tool calls:
///
/// 1. Offered a `mark_do_not_call` tool and told to call it when the caller asks not to be
///    contacted, the engine answers "Take me off your list, and don't ever phone me again."
///    with a `ToolCall` named `mark_do_not_call` whose arguments are a JSON object.
/// 2. The reply ends with exactly one `Done`.
///
/// # Panics
/// When `engine` breaks a rule.
#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
pub async fn llm_tool_conformance(engine: &dyn LlmEngine) {
    let request = LlmRequest::new(DEADLINE)
        .with_message(
            Role::System,
            "You are a phone agent for a restaurant. When the caller asks not to be called \
             or contacted again, call the mark_do_not_call tool. Otherwise reply in one short \
             sentence.",
        )
        .with_message(
            Role::User,
            "Take me off your list, and don't ever phone me again.",
        )
        .with_tool(ToolSpec::new(
            "mark_do_not_call",
            "Record that the caller asked not to be contacted again, and end the call.",
        ))
        .with_max_tokens(64);
    let events = collect(engine, request, "tool rule 1").await;
    let call = events
        .iter()
        .find_map(|event| match event {
            LlmEvent::ToolCall(call) => Some(call),
            _ => None,
        })
        .unwrap_or_else(|| panic!("LLM tool rule 1: no tool call in {events:?}"));
    assert_eq!(
        call.name(),
        "mark_do_not_call",
        "LLM tool rule 1: the wrong tool"
    );
    let arguments: serde_json::Value = serde_json::from_str(call.arguments())
        .unwrap_or_else(|error| panic!("LLM tool rule 1: the arguments are not JSON: {error}"));
    assert!(
        arguments.is_object(),
        "LLM tool rule 1: the arguments are not a JSON object"
    );
    assert_done_once_at_the_end(&events, "tool rule 2");
}

fn greeting() -> LlmRequest {
    LlmRequest::new(DEADLINE)
        .with_message(
            Role::System,
            "You are a terse assistant. Answer in at most five words.",
        )
        .with_message(Role::User, "Say hello to me.")
        .with_max_tokens(32)
}

/// Every event of the reply to `request`, until the stream ends.
#[expect(clippy::panic, reason = "a broken rule fails the calling test")]
async fn collect(engine: &dyn LlmEngine, request: LlmRequest, rule: &str) -> Vec<LlmEvent> {
    let mut stream = engine
        .stream(request)
        .await
        .unwrap_or_else(|error| panic!("LLM {rule}: the request failed: {error}"));
    let mut events = Vec::new();
    while let Some(event) = stream.next_event().await {
        events.push(event.unwrap_or_else(|error| panic!("LLM {rule}: the stream failed: {error}")));
        assert!(
            events.len() <= MAX_EVENTS,
            "LLM {rule}: the stream never ends"
        );
    }
    events
}

fn text_of(events: &[LlmEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            LlmEvent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn assert_done_once_at_the_end(events: &[LlmEvent], rule: &str) {
    let done = events
        .iter()
        .filter(|event| matches!(event, LlmEvent::Done))
        .count();
    assert_eq!(done, 1, "LLM {rule}: expected exactly one Done, got {done}");
    assert!(
        matches!(events.last(), Some(LlmEvent::Done)),
        "LLM {rule}: Done must be the last event, got {:?}",
        events.last()
    );
}
