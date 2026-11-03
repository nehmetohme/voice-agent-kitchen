//! Every scripted fake passes its port's conformance suite, so the suites are proven against
//! at least one implementation and the fakes behave like the real adapters.

use std::time::Duration;

use vak_testkit::conformance::{
    decision_conformance, llm_conformance, llm_tool_conformance, stt_conformance, tts_conformance,
};
use vak_testkit::providers::{
    DecisionReply, LlmStep, ScriptedDecisions, ScriptedLlm, ScriptedStt, ScriptedTts,
};

const QUICK: Duration = Duration::from_millis(20);

#[tokio::test(start_paused = true)]
async fn scripted_stt_conforms() {
    let stt = ScriptedStt::new();
    stt.push_reply(
        "Hi, I would like to book a table for two tomorrow at seven.",
        QUICK,
    );
    stt.push_reply("", QUICK);

    stt_conformance(&stt).await;

    assert_eq!(stt.calls().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn scripted_tts_conforms() {
    let tts = ScriptedTts::new().with_delay(QUICK);

    tts_conformance(&tts).await;

    assert_eq!(tts.calls().len(), 3, "both clauses and the blank text");
}

#[tokio::test(start_paused = true)]
async fn scripted_llm_conforms() {
    let llm = ScriptedLlm::new();
    llm.push_reply([
        LlmStep::text("Hello", QUICK),
        LlmStep::text(" there!", QUICK),
    ]);
    llm.push_reply([
        LlmStep::text("One, two,", QUICK),
        LlmStep::text(" three", Duration::from_secs(5)),
    ]);
    llm.push_reply([LlmStep::text("Hi!", QUICK)]);
    llm.push_reply([LlmStep::tool_call("mark_do_not_call", "{}", QUICK)]);

    llm_conformance(&llm).await;
    llm_tool_conformance(&llm).await;

    let calls = llm.calls();
    assert_eq!(calls.len(), 4);
    assert!(
        calls[1].cancelled_at.is_some(),
        "the dropped stream is recorded"
    );
    assert!(calls[0].completed && calls[2].completed && calls[3].completed);
    assert_eq!(calls[3].tools, ["mark_do_not_call"]);
}

#[tokio::test(start_paused = true)]
async fn scripted_decisions_conform() {
    let decisions = ScriptedDecisions::new();
    decisions.push(DecisionReply::yes_no("opt_out", 0.92).with_choice("intent", "opt_out"));
    decisions.push(DecisionReply::yes_no("opt_out", 0.03).with_choice("intent", "book"));

    decision_conformance(&decisions).await;

    let calls = decisions.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].questions, ["opt_out", "intent"]);
    assert_eq!(calls[0].turns[1].0, "caller");
}
