//! Contract `agent-provisioning` v1 (domain): creating an agent needs only a name, a purpose
//! (use case) and a voice; every other field gets a default; invalid input is a typed error;
//! the draft has a deterministic content hash; and a new agent's setup progress is 25 %.
//!
//! Contract file: never edit it once it is tagged `contract/agent-provisioning/v1`. To change
//! the contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "the fixture helper outside #[test] functions fails the test on invalid input"
)]

use vak_domain::catalog::{LANGUAGES, VOICES};
use vak_domain::{
    AgentDraft, AgentName, AgentNameError, DraftPatch, Jurisdiction, JurisdictionError, Language,
    LanguageError, ModelProfile, ProgressFacts, SetupProgress, SetupStep, TurnDetection,
    TurnDetectionError, UseCase, UseCaseError, VoiceId, VoiceIdError,
};

fn front_desk() -> AgentDraft {
    AgentDraft::new(
        AgentName::parse("Front desk").expect("valid name"),
        UseCase::parse("support").expect("valid use case"),
        VoiceId::parse("af_heart").expect("valid voice"),
    )
}

#[test]
fn a_draft_needs_only_a_name_a_use_case_and_a_voice() {
    let draft = front_desk();

    let persona = draft.persona();
    assert_eq!(persona.name().as_str(), "Front desk");
    assert_eq!(persona.use_case(), UseCase::Support);
    assert_eq!(persona.voice().as_str(), "af_heart");
}

#[test]
fn every_other_field_gets_a_default() {
    let draft = front_desk();

    assert_eq!(draft.persona().language().as_str(), "en-US");
    assert_eq!(draft.persona().language(), Language::default());
    assert_eq!(draft.jurisdiction().as_str(), "US");
    assert_eq!(draft.jurisdiction(), &Jurisdiction::default());
    assert_eq!(draft.rule_pack().id(), "us-support");
    assert_eq!(draft.rule_pack().version(), 1);
    assert_eq!(draft.settings().model(), ModelProfile::Balanced);
    assert_eq!(draft.settings().turn_detection(), TurnDetection::default());
    assert_eq!(TurnDetection::default().silence_ms(), 500);
    assert!(TurnDetection::default().barge_in());
}

#[test]
fn the_rule_pack_follows_the_use_case_and_the_jurisdiction() {
    let patch = DraftPatch {
        use_case: Some(UseCase::Booking),
        jurisdiction: Some(Jurisdiction::parse("us-ca").expect("valid jurisdiction")),
        ..DraftPatch::default()
    };

    let draft = front_desk().apply(&patch);

    assert_eq!(draft.persona().use_case(), UseCase::Booking);
    assert_eq!(draft.jurisdiction().as_str(), "US-CA");
    assert_eq!(draft.rule_pack().id(), "us-ca-booking");
    assert_eq!(draft.rule_pack().version(), 1);
}

#[test]
fn a_patch_changes_only_the_fields_it_sets() {
    let original = front_desk();
    let patch = DraftPatch {
        name: Some(AgentName::parse("After hours").expect("valid name")),
        ..DraftPatch::default()
    };

    let patched = original.apply(&patch);

    assert_eq!(patched.persona().name().as_str(), "After hours");
    assert_eq!(patched.persona().voice(), original.persona().voice());
    assert_eq!(patched.rule_pack(), original.rule_pack());
    assert_eq!(original.apply(&DraftPatch::default()), original);
}

#[test]
fn names_are_trimmed_and_validated() {
    assert_eq!(
        AgentName::parse("  Front desk \n").map(|name| name.as_str().to_owned()),
        Ok("Front desk".to_owned())
    );
    assert_eq!(AgentName::parse(""), Err(AgentNameError::Empty));
    assert_eq!(AgentName::parse(" \t "), Err(AgentNameError::Empty));
    assert!(AgentName::parse(&"a".repeat(AgentName::MAX)).is_ok());
    assert_eq!(
        AgentName::parse(&"a".repeat(AgentName::MAX + 1)),
        Err(AgentNameError::TooLong { max: 80 })
    );
    assert_eq!(
        AgentName::parse("Front\u{0}desk"),
        Err(AgentNameError::ControlCharacter)
    );
}

#[test]
fn use_cases_are_a_short_fixed_list_parsed_liberally() {
    let names: Vec<&str> = UseCase::ALL
        .iter()
        .map(|use_case| use_case.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "support",
            "service",
            "booking",
            "survey",
            "telemarketing",
            "custom"
        ]
    );
    assert!(UseCase::ALL.len() <= 7, "Hick: at most 7 purposes");
    assert_eq!(UseCase::parse(" Booking "), Ok(UseCase::Booking));
    assert_eq!(UseCase::parse("TELEMARKETING"), Ok(UseCase::Telemarketing));
    assert_eq!(UseCase::parse("sales"), Err(UseCaseError::Unknown));
    assert_eq!(UseCase::parse(""), Err(UseCaseError::Unknown));
}

#[test]
fn voices_and_languages_come_from_the_catalog() {
    assert!(!VOICES.is_empty() && VOICES.len() <= 7);
    for voice in VOICES {
        assert_eq!(VoiceId::parse(voice.id).map(|id| id.as_str()), Ok(voice.id));
        assert!(
            LANGUAGES
                .iter()
                .any(|language| language.code == voice.language)
        );
    }
    assert_eq!(
        VoiceId::parse(" AF_HEART ").map(|id| id.as_str()),
        Ok("af_heart")
    );
    assert_eq!(VoiceId::parse("robot_9000"), Err(VoiceIdError::Unknown));

    assert_eq!(
        LANGUAGES.first().map(|language| language.code),
        Some("en-US")
    );
    assert_eq!(
        Language::parse("en_gb").map(|language| language.as_str()),
        Ok("en-GB")
    );
    assert_eq!(Language::parse("xx-XX"), Err(LanguageError::Unsupported));
}

#[test]
fn jurisdictions_are_region_codes() {
    assert_eq!(
        Jurisdiction::parse(" us-ca ").map(|code| code.as_str().to_owned()),
        Ok("US-CA".to_owned())
    );
    assert_eq!(
        Jurisdiction::parse("UK").map(|code| code.as_str().to_owned()),
        Ok("UK".to_owned())
    );
    assert_eq!(
        Jurisdiction::parse("not a place"),
        Err(JurisdictionError::Invalid)
    );
    assert_eq!(Jurisdiction::parse(""), Err(JurisdictionError::Invalid));
}

#[test]
fn settings_are_validated() {
    assert_eq!(ModelProfile::parse("Fast"), Ok(ModelProfile::Fast));
    assert!(ModelProfile::parse("gpt").is_err());
    assert!(TurnDetection::new(TurnDetection::MIN_SILENCE_MS, false).is_ok());
    assert!(TurnDetection::new(TurnDetection::MAX_SILENCE_MS, true).is_ok());
    assert_eq!(
        TurnDetection::new(50, true),
        Err(TurnDetectionError::SilenceOutOfRange {
            min: 200,
            max: 2_000
        })
    );
    assert_eq!(
        TurnDetection::new(5_000, true),
        Err(TurnDetectionError::SilenceOutOfRange {
            min: 200,
            max: 2_000
        })
    );
}

#[test]
fn the_content_hash_is_deterministic_and_covers_every_field() {
    let draft = front_desk();
    let hash = draft.content_hash();

    assert_eq!(hash, front_desk().content_hash());
    let hex = hash
        .as_str()
        .strip_prefix("sha256:")
        .expect("sha256: prefix");
    assert_eq!(hex.len(), 64);
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    );

    let patches = [
        DraftPatch {
            name: Some(AgentName::parse("Other").expect("valid")),
            ..DraftPatch::default()
        },
        DraftPatch {
            use_case: Some(UseCase::Survey),
            ..DraftPatch::default()
        },
        DraftPatch {
            voice: Some(VoiceId::parse("bm_george").expect("valid")),
            ..DraftPatch::default()
        },
        DraftPatch {
            language: Some(Language::parse("en-GB").expect("valid")),
            ..DraftPatch::default()
        },
        DraftPatch {
            jurisdiction: Some(Jurisdiction::parse("UK").expect("valid")),
            ..DraftPatch::default()
        },
        DraftPatch {
            model: Some(ModelProfile::Fast),
            ..DraftPatch::default()
        },
        DraftPatch {
            turn_detection: Some(TurnDetection::new(800, false).expect("valid")),
            ..DraftPatch::default()
        },
    ];
    for patch in patches {
        assert_ne!(draft.apply(&patch).content_hash(), hash, "{patch:?}");
    }
}

#[test]
fn a_new_agent_has_finished_provision_and_is_25_percent_set_up() {
    let progress = SetupProgress::derive(&front_desk(), &ProgressFacts::default());

    let steps: Vec<(SetupStep, bool)> = progress
        .steps()
        .iter()
        .map(|step| (step.step(), step.done()))
        .collect();
    assert_eq!(
        steps,
        [
            (SetupStep::Provision, true),
            (SetupStep::Plan, false),
            (SetupStep::Test, false),
            (SetupStep::Publish, false),
        ]
    );
    assert_eq!(progress.percent(), 25);
    assert_eq!(progress.next_step(), Some(SetupStep::Plan));
}
