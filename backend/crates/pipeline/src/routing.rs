//! What each reply is built from: the LLM's instructions, history and tools, and the
//! decision questions asked about each caller turn.
//!
//! **The seam for plan routing (M4b).** Today every turn uses the agent's one prompt and the
//! platform's `opt_out` question. Plan routing replaces this module's inputs with the plan's
//! current node: [`Router::questions`] gains a `branch` choice over the node's outgoing edges
//! (asked in the same decision request, so it costs no extra round trip), the node's prompt
//! and tools replace the agent prompt in [`Router::llm_request`], and the session moves
//! between nodes on the decision's answer (an LLM tool call when its confidence is low). The
//! session actor only calls these three methods, so it does not change.

use std::time::Duration;

use vak_domain::{Question, QuestionId, Speaker};
use vak_providers::{DecisionRequest, LlmRequest, Role, ToolSpec};

/// The platform's opt-out question ID.
pub(crate) const OPT_OUT_QUESTION: &str = "opt_out";
/// The LLM tool through which a caller's opt-out reaches the session.
pub(crate) const OPT_OUT_TOOL: &str = "mark_do_not_call";

const OPT_OUT_INSTRUCTIONS: &str = "Is the caller asking not to be called or contacted again, \
     or to be removed from a calling list?";
const OPT_OUT_TOOL_DESCRIPTION: &str = "Record that the caller asked not to be called or \
     contacted again. This ends the call: use it as soon as the caller asks, and say nothing else.";
/// How the agent talks on a call, whatever its persona.
const VOICE_RULES: &str = "You are on a live phone call. Answer in one or two short spoken \
     sentences, in plain words, without lists, markdown or emojis. If the caller asks not to \
     be called or contacted again, call the mark_do_not_call tool.";
/// Turns the decision model sees: the recent ones are what matter, and short state is faster
/// and more accurate.
const DECISION_TURNS: usize = 4;
/// Messages the LLM sees, besides its instructions.
const HISTORY_MESSAGES: usize = 40;
/// Enough for two short sentences.
const MAX_REPLY_TOKENS: u32 = 200;

/// One line of the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub(crate) speaker: Speaker,
    pub(crate) text: String,
}

pub(crate) struct Router {
    system_prompt: String,
    opt_out: QuestionId,
}

impl Router {
    pub(crate) fn new(system_prompt: &str) -> Option<Self> {
        let system_prompt = if system_prompt.trim().is_empty() {
            VOICE_RULES.to_owned()
        } else {
            format!("{}\n\n{VOICE_RULES}", system_prompt.trim())
        };
        Some(Self {
            system_prompt,
            opt_out: QuestionId::parse(OPT_OUT_QUESTION).ok()?,
        })
    }

    /// The questions asked about every caller turn, in one decision request.
    pub(crate) fn questions(&self) -> Vec<(QuestionId, Question)> {
        vec![(self.opt_out.clone(), Question::yes_no(OPT_OUT_INSTRUCTIONS))]
    }

    /// The decision request about the latest caller turn.
    pub(crate) fn decision_request(&self, history: &[Line], deadline: Duration) -> DecisionRequest {
        let recent = history.len().saturating_sub(DECISION_TURNS);
        let request = history[recent..]
            .iter()
            .fold(DecisionRequest::new(deadline), |request, line| {
                request.with_turn(line.speaker, line.text.as_str())
            });
        self.questions()
            .into_iter()
            .fold(request, |request, (id, question)| {
                request.with_question(id, question)
            })
    }

    /// The LLM request for the next reply: instructions, the conversation, the tools.
    pub(crate) fn llm_request(&self, history: &[Line], deadline: Duration) -> LlmRequest {
        let recent = history.len().saturating_sub(HISTORY_MESSAGES);
        history[recent..]
            .iter()
            .fold(
                LlmRequest::new(deadline).with_message(Role::System, self.system_prompt.as_str()),
                |request, line| {
                    let role = match line.speaker {
                        Speaker::Caller => Role::User,
                        Speaker::Agent => Role::Assistant,
                    };
                    request.with_message(role, line.text.as_str())
                },
            )
            .with_tool(ToolSpec::new(OPT_OUT_TOOL, OPT_OUT_TOOL_DESCRIPTION))
            .with_max_tokens(MAX_REPLY_TOKENS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(speaker: Speaker, text: &str) -> Line {
        Line {
            speaker,
            text: text.to_owned(),
        }
    }

    #[test]
    fn the_llm_sees_the_agent_prompt_the_history_and_the_opt_out_tool() {
        let router = Router::new("You are Ava.").unwrap();
        let history = [line(Speaker::Agent, "Hi."), line(Speaker::Caller, "Hello.")];
        let request = router.llm_request(&history, Duration::from_secs(3));
        let messages: Vec<(Role, &str)> = request
            .messages()
            .iter()
            .map(|message| (message.role(), message.content()))
            .collect();
        assert_eq!(messages[0].0, Role::System);
        assert!(messages[0].1.starts_with("You are Ava.\n\n"));
        assert_eq!(
            messages[1..],
            [(Role::Assistant, "Hi."), (Role::User, "Hello.")]
        );
        assert_eq!(request.tools()[0].name(), OPT_OUT_TOOL);
        assert_eq!(request.max_tokens(), Some(MAX_REPLY_TOKENS));
        assert_eq!(request.deadline(), Duration::from_secs(3));
        let bare = Router::new("  ")
            .unwrap()
            .llm_request(&[], Duration::from_secs(1));
        assert_eq!(bare.messages()[0].content(), VOICE_RULES);
    }

    #[test]
    fn the_decision_model_sees_the_last_turns_and_the_opt_out_question() {
        let router = Router::new("").unwrap();
        let history: Vec<Line> = (0..6)
            .map(|i| line(Speaker::Caller, &format!("turn {i}")))
            .collect();
        let request = router.decision_request(&history, Duration::from_millis(300));
        let texts: Vec<&str> = request
            .turns()
            .iter()
            .map(|(_, text)| text.as_str())
            .collect();
        assert_eq!(texts, ["turn 2", "turn 3", "turn 4", "turn 5"]);
        assert_eq!(request.questions()[0].0.as_str(), OPT_OUT_QUESTION);
        assert_eq!(request.deadline(), Duration::from_millis(300));
    }
}
