//! What a voice session takes from an agent's draft and its pinned purpose version: its
//! locked opening line, its instructions, its voice, how it takes turns, its opt-out phrases
//! and how long a call may last.
//!
//! The purpose's persona, guardrails and tool descriptions are compiled into the LLM
//! instructions, so test calls follow them now; enforcing them with the decision engine and
//! running the tools is M4b. Until rule packs carry per-jurisdiction disclosure text and
//! opt-out phrases into calls (M4b), the platform defaults apply to every agent.

use crate::{
    AgentDraft, BuiltinTool, DEFAULT_OPT_OUT_CHECK_QUESTION, DEFAULT_OPT_OUT_CONFIRMATION,
    DEFAULT_OPT_OUT_PHRASES, PurposeDefinition, PurposeVersion,
};

/// The session-facing view of an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScript {
    disclosure: String,
    instructions: String,
    voice: &'static str,
    end_of_turn_silence_ms: u32,
    barge_in: bool,
    opt_out_phrases: Vec<String>,
    max_call_minutes: u32,
}

impl SessionScript {
    /// The script of an agent with this draft on its use case's system purpose: the
    /// instructions every agent had before purposes existed.
    #[must_use]
    pub fn for_draft(draft: &AgentDraft) -> Self {
        let system = PurposeVersion::system(draft.persona().use_case());
        Self::for_agent(draft, system.definition())
    }

    /// The script of an agent with this draft and its pinned purpose version: the purpose's
    /// persona follows who the agent is, and its guardrails, tool descriptions, extra opt-out
    /// phrases and call length are added. A system purpose gives exactly
    /// [`SessionScript::for_draft`].
    #[must_use]
    pub fn for_agent(draft: &AgentDraft, purpose: &PurposeDefinition) -> Self {
        let persona = draft.persona();
        let name = persona.name().as_str();
        let turns = draft.settings().turn_detection();
        let mut opt_out_phrases: Vec<String> = DEFAULT_OPT_OUT_PHRASES
            .iter()
            .map(|phrase| (*phrase).to_owned())
            .collect();
        for phrase in purpose.additions().opt_out_phrases() {
            if !opt_out_phrases.contains(phrase) {
                opt_out_phrases.push(phrase.clone());
            }
        }
        Self {
            // Who is calling, that it is an AI, and that the call may be recorded: first,
            // and word for word (research 04, C9 and C11).
            disclosure: format!(
                "Hi, this is {name}, an automated AI assistant. This call may be recorded."
            ),
            instructions: instructions(name, language_name(persona.language().as_str()), purpose),
            voice: persona.voice().as_str(),
            end_of_turn_silence_ms: turns.silence_ms(),
            barge_in: turns.barge_in(),
            opt_out_phrases,
            max_call_minutes: purpose.guardrails().max_call_minutes(),
        }
    }

    /// The locked opening line: spoken first, unchanged, uninterruptible.
    #[must_use]
    pub fn disclosure(&self) -> &str {
        &self.disclosure
    }

    /// The agent's instructions for the LLM.
    #[must_use]
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    /// The TTS voice ID.
    #[must_use]
    pub const fn voice(&self) -> &'static str {
        self.voice
    }

    /// Phrases that end the session when the caller says one: the platform's, then the
    /// purpose's extra ones.
    #[must_use]
    pub fn opt_out_phrases(&self) -> &[String] {
        &self.opt_out_phrases
    }

    /// The locked line said before an opt-out ends the session.
    #[must_use]
    pub fn opt_out_confirmation(&self) -> &'static str {
        DEFAULT_OPT_OUT_CONFIRMATION
    }

    /// The question the agent asks when the decision model is unsure whether the caller
    /// opted out.
    #[must_use]
    pub fn opt_out_check_question(&self) -> &'static str {
        DEFAULT_OPT_OUT_CHECK_QUESTION
    }

    #[must_use]
    pub const fn end_of_turn_silence_ms(&self) -> u32 {
        self.end_of_turn_silence_ms
    }

    #[must_use]
    pub const fn barge_in(&self) -> bool {
        self.barge_in
    }

    /// The longest call, in minutes: the purpose's limit.
    #[must_use]
    pub fn max_call_minutes(&self) -> u32 {
        self.max_call_minutes
    }
}

/// Who the agent is and how it behaves, then one line per guardrail the purpose sets. A
/// purpose without guardrails or tools adds nothing.
fn instructions(name: &str, language: &str, purpose: &PurposeDefinition) -> String {
    let persona = match purpose.persona() {
        "" => String::new(),
        persona => format!(" {persona}"),
    };
    let mut text = format!(
        "You are {name}, an AI voice assistant.{persona} Speak {language}. You are an AI: never \
         claim to be human, and say so if asked. Never make up facts, prices or availability; \
         offer to take a message instead."
    );
    let rules = guardrail_lines(purpose);
    if !rules.is_empty() {
        text.push_str("\n\n");
        text.push_str(&rules.join("\n"));
    }
    text
}

fn guardrail_lines(purpose: &PurposeDefinition) -> Vec<String> {
    let guardrails = purpose.guardrails();
    let mut lines = Vec::new();
    if !guardrails.allowed_topics().is_empty() {
        lines.push(format!(
            "Only help with: {}. If the caller asks about anything else, say briefly that you \
             can't help with that on this call, and steer back.",
            guardrails.allowed_topics().join("; ")
        ));
    }
    let never: Vec<&str> = guardrails
        .never()
        .iter()
        .chain(purpose.additions().forbidden_claims())
        .map(String::as_str)
        .collect();
    if !never.is_empty() {
        lines.push(format!(
            "Never do or say any of these: {}.",
            never.join("; ")
        ));
    }
    if !guardrails.never_ask().is_empty() {
        let kinds: Vec<&str> = guardrails
            .never_ask()
            .iter()
            .map(|kind| kind.description())
            .collect();
        lines.push(format!(
            "Never ask the caller for {}. If they start to give one, stop them politely and say \
             you don't need it.",
            kinds.join(", ")
        ));
    }
    let tools = purpose.tools();
    let mut actions: Vec<String> = tools
        .builtins()
        .iter()
        .filter(|tool| **tool != BuiltinTool::MarkDoNotCall)
        .map(|tool| format!("{} ({})", tool.name(), tool.description()))
        .collect();
    actions.extend(
        tools
            .http()
            .iter()
            .map(|tool| format!("{} ({})", tool.name().as_str(), tool.description())),
    );
    if !actions.is_empty() {
        lines.push(format!(
            "Actions you can offer, not connected in test calls yet, so say what you would do \
             instead: {}.",
            actions.join("; ")
        ));
    }
    let confirm: Vec<&str> = tools
        .http()
        .iter()
        .filter(|tool| tool.confirm())
        .map(|tool| tool.name().as_str())
        .collect();
    if !confirm.is_empty() {
        lines.push(format!(
            "Ask the caller before you use {}.",
            confirm.join(" or ")
        ));
    }
    lines
}

fn language_name(tag: &str) -> &'static str {
    if tag.starts_with("en") {
        "English"
    } else {
        "the caller's language"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentName, DraftPatch, TurnDetection, UseCase, VoiceId};

    fn draft(use_case: UseCase) -> AgentDraft {
        AgentDraft::new(
            AgentName::parse("Ava").unwrap(),
            use_case,
            VoiceId::parse("bf_emma").unwrap(),
        )
    }

    #[test]
    fn the_script_names_the_agent_as_an_ai_and_follows_its_settings() {
        let script = SessionScript::for_draft(&draft(UseCase::Booking));
        assert_eq!(
            script.disclosure(),
            "Hi, this is Ava, an automated AI assistant. This call may be recorded."
        );
        assert!(
            script
                .instructions()
                .starts_with("You are Ava, an AI voice assistant.")
        );
        assert!(
            script
                .instructions()
                .contains("book, move and cancel appointments")
        );
        assert!(script.instructions().contains("Speak English"));
        assert!(script.instructions().contains("never claim to be human"));
        assert_eq!(script.voice(), "bf_emma");
        assert_eq!(script.end_of_turn_silence_ms(), 500);
        assert!(script.barge_in());
        assert_eq!(script.opt_out_phrases(), DEFAULT_OPT_OUT_PHRASES);
        assert_eq!(script.opt_out_confirmation(), DEFAULT_OPT_OUT_CONFIRMATION);
        assert_eq!(
            script.opt_out_check_question(),
            DEFAULT_OPT_OUT_CHECK_QUESTION
        );
        assert_eq!(script.max_call_minutes(), 30);

        let patient = draft(UseCase::Survey).apply(&DraftPatch {
            turn_detection: Some(TurnDetection::new(1_200, false).unwrap()),
            ..DraftPatch::default()
        });
        let script = SessionScript::for_draft(&patient);
        assert_eq!(script.end_of_turn_silence_ms(), 1_200);
        assert!(!script.barge_in());
    }

    #[test]
    fn a_purpose_without_a_persona_still_says_who_the_agent_is() {
        let system = PurposeVersion::system(UseCase::Custom);
        let mut spec = system.definition().to_spec();
        spec.persona = String::new();
        let silent = PurposeDefinition::new(spec).unwrap();

        let script = SessionScript::for_agent(&draft(UseCase::Custom), &silent);

        assert!(
            script
                .instructions()
                .starts_with("You are Ava, an AI voice assistant. Speak English.")
        );
        assert_eq!(language_name("en-GB"), "English");
        assert_eq!(language_name("fr-FR"), "the caller's language");
    }
}
