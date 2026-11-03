//! What a voice session takes from an agent's draft: its locked opening line, its
//! instructions, its voice and how it takes turns.
//!
//! Until rule packs (M3) carry per-jurisdiction disclosure text and opt-out phrases, and
//! plans (M4b) carry per-node prompts, these defaults apply to every agent.

use crate::{AgentDraft, DEFAULT_OPT_OUT_CONFIRMATION, DEFAULT_OPT_OUT_PHRASES, UseCase};

/// The session-facing view of an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionScript {
    disclosure: String,
    instructions: String,
    voice: &'static str,
    end_of_turn_silence_ms: u32,
    barge_in: bool,
}

impl SessionScript {
    /// The script of an agent with this draft.
    #[must_use]
    pub fn for_draft(draft: &AgentDraft) -> Self {
        let persona = draft.persona();
        let name = persona.name().as_str();
        let turns = draft.settings().turn_detection();
        Self {
            // Who is calling, that it is an AI, and that the call may be recorded: first,
            // and word for word (research 04, C9 and C11).
            disclosure: format!(
                "Hi, this is {name}, an automated AI assistant. This call may be recorded."
            ),
            instructions: format!(
                "You are {name}, an AI voice assistant. Your job: {}. Speak {}. You are an AI: \
                 never claim to be human, and say so if asked. Never make up facts, prices or \
                 availability; offer to take a message instead.",
                purpose(persona.use_case()),
                language_name(persona.language().as_str()),
            ),
            voice: persona.voice().as_str(),
            end_of_turn_silence_ms: turns.silence_ms(),
            barge_in: turns.barge_in(),
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

    /// Phrases that end the session when the caller says one.
    #[must_use]
    pub fn opt_out_phrases(&self) -> &'static [&'static str] {
        &DEFAULT_OPT_OUT_PHRASES
    }

    /// The locked line said before an opt-out ends the session.
    #[must_use]
    pub fn opt_out_confirmation(&self) -> &'static str {
        DEFAULT_OPT_OUT_CONFIRMATION
    }

    #[must_use]
    pub const fn end_of_turn_silence_ms(&self) -> u32 {
        self.end_of_turn_silence_ms
    }

    #[must_use]
    pub const fn barge_in(&self) -> bool {
        self.barge_in
    }
}

const fn purpose(use_case: UseCase) -> &'static str {
    match use_case {
        UseCase::Support => "answer customers' questions and help solve their problems",
        UseCase::Service => "handle account and order requests: changes, status and billing",
        UseCase::Booking => "book, move and cancel appointments",
        UseCase::Survey => "ask the survey's questions one at a time and note the answers",
        UseCase::Telemarketing => {
            "present the offer briefly and honestly, and respect a no at once"
        }
        UseCase::Custom => "help the caller with what they need",
    }
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
    use crate::{AgentName, DraftPatch, TurnDetection, VoiceId};

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
        assert_eq!(script.opt_out_phrases(), DEFAULT_OPT_OUT_PHRASES.as_slice());
        assert_eq!(script.opt_out_confirmation(), DEFAULT_OPT_OUT_CONFIRMATION);

        let patient = draft(UseCase::Survey).apply(&DraftPatch {
            turn_detection: Some(TurnDetection::new(1_200, false).unwrap()),
            ..DraftPatch::default()
        });
        let script = SessionScript::for_draft(&patient);
        assert_eq!(script.end_of_turn_silence_ms(), 1_200);
        assert!(!script.barge_in());
    }

    #[test]
    fn every_use_case_has_a_purpose() {
        for use_case in UseCase::ALL {
            assert!(!purpose(use_case).is_empty(), "{use_case:?}");
        }
        assert_eq!(language_name("en-GB"), "English");
        assert_eq!(language_name("fr-FR"), "the caller's language");
    }
}
