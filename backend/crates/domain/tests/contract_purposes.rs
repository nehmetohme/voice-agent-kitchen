//! Contract `purposes` v1 (domain): purposes, their versions, tools, guardrails and
//! tighten-only compliance, and how agents pin them.
//!
//! - The six use cases are built-in system purposes at version 1, with fixed IDs. Each equals
//!   today's agent exactly: its starting flow opened with any pack's locked texts is today's
//!   template, it adds nothing to its pack, and an agent on it gets today's instructions. They
//!   are read-only.
//! - A purpose has a draft and numbered versions. A version never changes; saving needs a
//!   change since the latest version and a draft without problems (flow graph rules, tool
//!   hosts on the allow-list). A duplicate starts as a draft copy without a version.
//! - Compliance is tighten-only (property-based): the effective policy keeps the pack's
//!   locked texts, opt-out confirmation and audit period, keeps every pack phrase and claim,
//!   and only adds phrases and claims or shortens recording and transcript retention.
//! - Tools: the built-in catalog, with `mark_do_not_call` always on; custom HTTP tools with
//!   LLM-safe names, https URLs with DNS hosts, a capped timeout, a secret by name only, and
//!   a small input schema. The host allow-list matches exactly or by `*.` wildcard.
//! - Guardrails: topics, never-rules, escalation rules and limits are checked.
//! - Agents pin `(purpose, version)`. A new agent pins its use case's system purpose v1 and
//!   keeps its content hash; a chosen version sets the use case and rule pack and is part of
//!   the hash. Updating to a newer version says what changes and keeps an edited plan unless
//!   the operator replaces it. A purpose compiles its guardrails into the instructions.
//!
//! Contract file: never edit it once it is tagged `contract/purposes/v1`. To change the
//! contract, add a new version.
#![expect(
    clippy::expect_used,
    reason = "fixture helpers outside #[test] functions fail the test on invalid input"
)]

use jiff::Timestamp;
use jiff::civil::date;
use proptest::prelude::*;
use vak_domain::{
    AgentDraft, AgentName, BuiltinTool, ComplianceAdditions, ComplianceAdditionsSpec,
    ComplianceError, DraftField, DraftPatch, DraftPlan, EffectiveCompliance, EscalationAction,
    EscalationRule, GuardrailError, Guardrails, GuardrailsSpec, HttpMethod, HttpTool, HttpToolSpec,
    Jurisdiction, LockedText, NodeId, NodeKind, ParamKind, PersonalData, Plan, PlanEdge, PlanError,
    PlanNode, PlanOnUpdate, Purpose, PurposeChange, PurposeDefinition, PurposeError, PurposeId,
    PurposeName, PurposeProblem, PurposeRef, PurposeSpec, PurposeTools, PurposeUpdate,
    PurposeVersion, Retention, RulePack, RulePackCatalog, RulePackRef, RulePackSpec, SecretRef,
    SessionScript, ToolError, ToolHosts, ToolInputs, ToolName, ToolParam, ToolUrl, UrlError,
    UseCase, VoiceId,
};

const DISCLOSURE: &str = "Hello, you're speaking with an AI assistant.";
const NOTICE: &str = "This call is recorded and transcribed.";
const CONFIRMATION: &str = "Understood. We won't contact you again. Goodbye.";
/// Today's (pre-purpose) content hash of the "Front desk" support draft.
const FRONT_DESK_HASH: &str =
    "sha256:b96a68e1c320cc47ccc101a2eb155166b15672919a1a93e9de6403241c1d6b52";
const SYSTEM_IDS: [&str; 6] = [
    "019a0000-0000-7000-8000-000000000001",
    "019a0000-0000-7000-8000-000000000002",
    "019a0000-0000-7000-8000-000000000003",
    "019a0000-0000-7000-8000-000000000004",
    "019a0000-0000-7000-8000-000000000005",
    "019a0000-0000-7000-8000-000000000006",
];
const LABELS: [&str; 6] = [
    "Customer support",
    "Customer service",
    "Appointment booking",
    "Survey",
    "Outbound sales",
    "Something else",
];

fn at(second: i64) -> Timestamp {
    Timestamp::from_second(1_793_000_000 + second).expect("valid time")
}

fn locked(texts: &[(&str, &str)]) -> Vec<LockedText> {
    texts
        .iter()
        .map(|(key, text)| LockedText::new(key, text).expect("valid locked text"))
        .collect()
}

fn pack_for(use_case: UseCase, jurisdiction: &str) -> RulePack {
    let jurisdiction = Jurisdiction::parse(jurisdiction).expect("valid jurisdiction");
    RulePack::new(RulePackSpec {
        id: RulePackRef::for_agent(use_case, &jurisdiction)
            .id()
            .to_owned(),
        version: 1,
        jurisdiction,
        use_case,
        effective: date(2026, 11, 1),
        locked: locked(&[
            (LockedText::AI_DISCLOSURE, DISCLOSURE),
            (LockedText::RECORDING_NOTICE, NOTICE),
        ]),
        opt_out_phrases: vec!["stop calling me".to_owned(), "unsubscribe".to_owned()],
        opt_out_confirmation: CONFIRMATION.to_owned(),
        forbidden_claims: vec!["Saying it is a human.".to_owned()],
        retention: Retention::new(180, 365, 1_826).expect("valid retention"),
    })
    .expect("valid pack")
}

fn catalog() -> RulePackCatalog {
    let packs = UseCase::ALL
        .into_iter()
        .flat_map(|use_case| [pack_for(use_case, "US"), pack_for(use_case, "US-CA")])
        .collect();
    RulePackCatalog::new(packs).expect("each pack once")
}

fn draft(name: &str, use_case: UseCase) -> AgentDraft {
    AgentDraft::new(
        AgentName::parse(name).expect("valid name"),
        use_case,
        VoiceId::parse("af_heart").expect("valid voice"),
    )
}

fn name(raw: &str) -> PurposeName {
    PurposeName::parse(raw).expect("valid purpose name")
}

fn purpose_id(raw: &str) -> PurposeId {
    PurposeId::parse(raw).expect("valid purpose ID")
}

fn tool_name(raw: &str) -> ToolName {
    ToolName::parse(raw).expect("valid tool name")
}

fn url(raw: &str) -> ToolUrl {
    ToolUrl::parse(raw).expect("valid tool URL")
}

fn hosts(entries: &[&str]) -> ToolHosts {
    ToolHosts::new(entries.iter().copied()).expect("valid allow-list")
}

/// A booking tool on `address`.
fn http_tool(tool: &str, address: &str) -> HttpTool {
    let date = ToolParam::new(
        tool_name("date"),
        ParamKind::String,
        "The day, as YYYY-MM-DD",
        true,
    )
    .expect("valid input");
    HttpTool::new(HttpToolSpec {
        name: tool_name(tool),
        description: "Book a free slot in the clinic's calendar.".to_owned(),
        inputs: ToolInputs::new(vec![date]).expect("valid inputs"),
        method: HttpMethod::Post,
        url: url(address),
        timeout_ms: HttpTool::DEFAULT_TIMEOUT_MS,
        secret: Some(SecretRef::parse("CALENDAR_API_KEY").expect("valid secret name")),
        confirm: true,
    })
    .expect("valid tool")
}

/// `base` with `change` applied to its parts.
fn changed(base: &PurposeDefinition, change: impl FnOnce(&mut PurposeSpec)) -> PurposeDefinition {
    let mut spec = base.to_spec();
    change(&mut spec);
    PurposeDefinition::new(spec).expect("valid definition")
}

fn duplicate_of(use_case: UseCase, as_name: &str) -> Purpose {
    let id = purpose_id("019a5a5a-0000-7000-8000-000000000001");
    Purpose::duplicate(
        id,
        PurposeVersion::system(use_case).definition(),
        name(as_name),
        at(0),
    )
}

/// A step labelled after its ID.
fn step(id: &str, kind: NodeKind, text: &str) -> PlanNode {
    PlanNode::new(
        NodeId::parse(id).expect("valid ID"),
        kind,
        &format!("Step {id}"),
        text,
    )
    .expect("valid step")
}

fn path(id: &str, from: &str, to: &str, condition: &str) -> PlanEdge {
    PlanEdge::new(
        vak_domain::EdgeId::parse(id).expect("valid ID"),
        NodeId::parse(from).expect("valid ID"),
        NodeId::parse(to).expect("valid ID"),
        condition,
    )
    .expect("valid path")
}

// ---------------------------------------------------------------------------------------
// System purposes
// ---------------------------------------------------------------------------------------

#[test]
fn the_six_use_cases_are_system_purposes_at_version_1_with_fixed_ids() {
    for ((use_case, id), label) in UseCase::ALL.into_iter().zip(SYSTEM_IDS).zip(LABELS) {
        let version = PurposeVersion::system(use_case);
        let reference = version.reference();

        assert_eq!(reference, PurposeRef::system(use_case), "{use_case}");
        assert_eq!(reference.id(), purpose_id(id), "{use_case}");
        assert_eq!(reference.id(), PurposeId::system(use_case));
        assert_eq!(reference.id().system_use_case(), Some(use_case));
        assert_eq!(reference.version(), 1);
        assert!(reference.is_system());
        let definition = version.definition();
        assert_eq!(definition.name().as_str(), label);
        assert_eq!(definition.compliance(), use_case);
        assert_eq!(definition.tools().builtins(), [BuiltinTool::MarkDoNotCall]);
        assert_eq!(definition.tools().http(), []);
        assert_eq!(definition.guardrails(), &Guardrails::default());
        assert_eq!(definition.additions(), &ComplianceAdditions::default());
        assert_eq!(version.choice().compliance(), use_case);
        assert_eq!(version.choice().reference(), reference);

        let purpose = Purpose::system(use_case);
        assert!(purpose.is_system());
        assert_eq!(purpose.latest_version(), Some(1));
        assert_eq!(purpose.latest(), Some(&version));
        assert_eq!(purpose.draft(), definition);
        assert!(!purpose.has_unsaved_changes());
    }
    let custom = purpose_id("019a5a5a-0000-7000-8000-000000000001");
    assert_eq!(custom.system_use_case(), None);
    assert!(
        !PurposeRef::new(custom, 3)
            .expect("valid reference")
            .is_system()
    );
    assert_eq!(
        PurposeRef::new(custom, 0),
        Err(PurposeError::InvalidVersion)
    );
}

#[test]
fn a_system_purposes_flow_is_todays_template_once_opened_with_the_packs_texts() {
    let openings = [
        Vec::new(),
        locked(&[(LockedText::AI_DISCLOSURE, DISCLOSURE)]),
        locked(&[
            (LockedText::AI_DISCLOSURE, DISCLOSURE),
            (LockedText::RECORDING_NOTICE, NOTICE),
            ("opt_out_notice", "Say stop at any time to opt out."),
        ]),
    ];
    for use_case in UseCase::ALL {
        let definition = PurposeVersion::system(use_case).definition().clone();

        assert_eq!(
            definition.flow(),
            &Plan::template(use_case, &[]),
            "{use_case}"
        );
        assert_eq!(definition.flow().validate_graph(), Ok(()), "{use_case}");
        for texts in &openings {
            assert_eq!(
                definition.opened_flow(texts),
                Plan::template(use_case, texts),
                "{use_case}"
            );
        }
    }
}

#[test]
fn a_system_purpose_adds_nothing_to_its_pack() {
    for use_case in UseCase::ALL {
        let pack = pack_for(use_case, "US-CA");
        let effective = EffectiveCompliance::of(
            &pack,
            PurposeVersion::system(use_case).definition().additions(),
        );

        assert_eq!(effective.rule_pack(), pack.reference());
        assert_eq!(effective.locked_texts(), pack.locked_texts());
        assert_eq!(effective.opt_out_phrases(), pack.opt_out_phrases());
        assert_eq!(
            effective.opt_out_confirmation(),
            pack.opt_out_confirmation()
        );
        assert_eq!(effective.forbidden_claims(), pack.forbidden_claims());
        assert_eq!(effective.retention(), pack.retention());
    }
}

#[test]
fn an_agent_on_a_system_purpose_gets_todays_instructions() {
    for use_case in UseCase::ALL {
        let agent = draft("Ava", use_case);
        let system = PurposeVersion::system(use_case);

        let script = SessionScript::for_agent(&agent, system.definition());

        assert_eq!(script, SessionScript::for_draft(&agent), "{use_case}");
        assert_eq!(script.max_call_minutes(), 30);
    }
    let booking = SessionScript::for_draft(&draft("Ava", UseCase::Booking));
    assert_eq!(
        booking.instructions(),
        "You are Ava, an AI voice assistant. Your job: book, move and cancel appointments. \
         Speak English. You are an AI: never claim to be human, and say so if asked. Never \
         make up facts, prices or availability; offer to take a message instead."
    );
}

#[test]
fn system_purposes_are_read_only() {
    let purpose = Purpose::system(UseCase::Support);
    let edited = changed(purpose.draft(), |spec| {
        spec.description = "Edited".to_owned();
    });

    assert_eq!(
        purpose.update_draft(edited, at(1)),
        Err(PurposeError::ReadOnly)
    );
    assert_eq!(
        purpose.save_version(&ToolHosts::default(), at(1)),
        Err(PurposeError::ReadOnly)
    );
}

// ---------------------------------------------------------------------------------------
// Drafts and versions
// ---------------------------------------------------------------------------------------

#[test]
fn a_duplicate_is_a_renamed_draft_copy_without_a_version() {
    let source = PurposeVersion::system(UseCase::Booking);

    let copy = duplicate_of(UseCase::Booking, "Clinic bookings");

    assert!(!copy.is_system());
    assert_eq!(copy.latest_version(), None);
    assert_eq!(copy.latest(), None);
    assert!(copy.has_unsaved_changes());
    assert_eq!(copy.draft().name().as_str(), "Clinic bookings");
    assert_eq!(
        copy.draft(),
        &source.definition().renamed(name("Clinic bookings"))
    );
    assert_eq!(copy.created_at(), at(0));
    assert_eq!(copy.updated_at(), at(0));
}

#[test]
fn versions_are_numbered_from_1_and_never_change() {
    let allowed = ToolHosts::default();
    let copy = duplicate_of(UseCase::Booking, "Clinic bookings");

    let (saved, first) = copy.save_version(&allowed, at(1)).expect("first version");

    assert_eq!(first.reference().id(), copy.id());
    assert_eq!(first.reference().version(), 1);
    assert_eq!(first.definition(), copy.draft());
    assert_eq!(first.created_at(), at(1));
    assert_eq!(saved.latest_version(), Some(1));
    assert!(!saved.has_unsaved_changes());
    assert_eq!(
        saved.save_version(&allowed, at(2)),
        Err(PurposeError::Unchanged { version: 1 })
    );

    let edited = changed(saved.draft(), |spec| {
        spec.description = "Books, moves and cancels clinic appointments.".to_owned();
    });
    let saved = saved
        .update_draft(edited.clone(), at(3))
        .expect("a custom purpose")
        .expect("a change");
    assert!(saved.has_unsaved_changes());
    assert_eq!(saved.updated_at(), at(3));
    assert_eq!(
        saved.latest(),
        Some(&first),
        "editing leaves version 1 alone"
    );
    assert_eq!(
        saved.update_draft(edited, at(4)),
        Ok(None),
        "no change, nothing to store"
    );

    let (saved, second) = saved.save_version(&allowed, at(5)).expect("second version");

    assert_eq!(second.reference().version(), 2);
    assert_eq!(
        second.definition().description(),
        "Books, moves and cancels clinic appointments."
    );
    assert_eq!(first.reference().version(), 1);
    assert_eq!(first.definition().description(), "");
    assert_eq!(saved.latest(), Some(&second));
    assert_ne!(
        first.definition().content_hash(),
        second.definition().content_hash()
    );
}

#[test]
fn a_draft_with_problems_can_be_stored_but_not_saved_as_a_version() {
    let copy = duplicate_of(UseCase::Custom, "Lonely steps");
    let broken = changed(copy.draft(), |spec| {
        spec.flow = Plan::new(
            vec![
                step("start", NodeKind::Start, "Hi."),
                step("help", NodeKind::Say, "Help."),
            ],
            vec![path("p1", "start", "help", "The caller answers")],
        )
        .expect("well-formed");
        spec.tools = PurposeTools::new(
            [BuiltinTool::EndCall],
            vec![http_tool(
                "book_slot",
                "https://calendar.example.com/v1/slots",
            )],
        )
        .expect("valid tools");
    });

    let stored = copy
        .update_draft(broken, at(1))
        .expect("a custom purpose")
        .expect("a change");

    let expected = vec![
        PurposeProblem::Flow(PlanError::MissingExit {
            node: NodeId::parse("help").expect("valid ID"),
        }),
        PurposeProblem::HostNotAllowed {
            tool: tool_name("book_slot"),
            host: "calendar.example.com".to_owned(),
        },
    ];
    assert_eq!(stored.problems(&ToolHosts::default()), expected);
    assert_eq!(
        stored.save_version(&ToolHosts::default(), at(2)),
        Err(PurposeError::Invalid { problems: expected })
    );
    let allowed = hosts(&["calendar.example.com"]);
    assert_eq!(
        stored.problems(&allowed),
        vec![PurposeProblem::Flow(PlanError::MissingExit {
            node: NodeId::parse("help").expect("valid ID"),
        })]
    );
}

#[test]
fn a_starting_flow_is_checked_by_the_graph_rules_not_the_locked_texts() {
    let flow = Plan::new(
        vec![
            step("start", NodeKind::Start, "Hi there."),
            step("bye", NodeKind::End, "Bye."),
        ],
        vec![path("p1", "start", "bye", "The caller is done")],
    )
    .expect("well-formed");
    let pack = pack_for(UseCase::Custom, "US");

    assert_eq!(flow.validate_graph(), Ok(()));
    assert!(flow.validate(&pack).is_err(), "no locked texts yet");
    let opened = flow.opened_with(pack.locked_texts());
    assert_eq!(opened.validate(&pack), Ok(()));
    assert_eq!(
        opened.nodes().first().map(PlanNode::text),
        Some(format!("{DISCLOSURE} {NOTICE} Hi there.").as_str())
    );
    let unreachable = Plan::new(
        vec![
            step("start", NodeKind::Start, "Hi."),
            step("bye", NodeKind::End, "Bye."),
        ],
        vec![],
    )
    .expect("well-formed");
    assert_eq!(
        unreachable.validate_graph(),
        Err(vec![
            PlanError::UnreachableNode {
                node: NodeId::parse("bye").expect("valid ID")
            },
            PlanError::MissingExit {
                node: NodeId::parse("start").expect("valid ID")
            },
        ])
    );
}

#[test]
fn equal_definitions_hash_equal_and_every_part_changes_the_hash() {
    let base = PurposeVersion::system(UseCase::Support)
        .definition()
        .renamed(name("Base"));
    let variants = [
        changed(&base, |spec| spec.name = name("Other")),
        changed(&base, |spec| spec.description = "Desc".to_owned()),
        changed(&base, |spec| spec.compliance = UseCase::Service),
        changed(&base, |spec| spec.persona = "Be brief.".to_owned()),
        changed(&base, |spec| {
            spec.flow = Plan::template(UseCase::Custom, &[]);
        }),
        changed(&base, |spec| {
            spec.tools = PurposeTools::new([BuiltinTool::EndCall], vec![]).expect("valid");
        }),
        changed(&base, |spec| {
            spec.guardrails = Guardrails::new(GuardrailsSpec {
                allowed_topics: vec!["Orders".to_owned()],
                never: vec![],
                escalations: vec![],
                max_call_minutes: 30,
                never_ask: vec![],
            })
            .expect("valid");
        }),
        changed(&base, |spec| {
            spec.additions = ComplianceAdditions::new(ComplianceAdditionsSpec {
                recordings_days: Some(30),
                ..ComplianceAdditionsSpec::default()
            })
            .expect("valid");
        }),
    ];

    assert_eq!(base.content_hash(), base.clone().content_hash());
    let hash = base.content_hash();
    assert!(hash.as_str().starts_with("sha256:"));
    for (at, variant) in variants.iter().enumerate() {
        assert_ne!(variant.content_hash(), hash, "variant {at}");
    }
}

#[test]
fn descriptions_names_and_personas_are_checked() {
    assert_eq!(
        PurposeName::parse("  Clinic bookings ").map(|name| name.as_str().to_owned()),
        Ok("Clinic bookings".to_owned())
    );
    assert!(PurposeName::parse(" ").is_err());
    assert!(PurposeName::parse(&"a".repeat(PurposeName::MAX + 1)).is_err());
    assert!(PurposeName::parse("Bell\u{7}").is_err());
    let base = PurposeVersion::system(UseCase::Support)
        .definition()
        .clone();
    let mut spec = base.to_spec();
    spec.description = "d".repeat(PurposeDefinition::MAX_DESCRIPTION + 1);
    assert_eq!(
        PurposeDefinition::new(spec),
        Err(PurposeError::DescriptionTooLong { max: 500 })
    );
    let mut spec = base.to_spec();
    spec.persona = "p".repeat(PurposeDefinition::MAX_PERSONA + 1);
    assert_eq!(
        PurposeDefinition::new(spec),
        Err(PurposeError::PersonaTooLong { max: 2_000 })
    );
    let trimmed = changed(&base, |spec| {
        spec.description = "  Helps customers.  ".to_owned();
        spec.persona = " Be kind.\r\nBe brief. ".to_owned();
    });
    assert_eq!(trimmed.description(), "Helps customers.");
    assert_eq!(trimmed.persona(), "Be kind.\nBe brief.");
}

// ---------------------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------------------

#[test]
fn the_built_in_catalog_has_the_opt_out_tool_always_on() {
    let names: Vec<&str> = BuiltinTool::ALL.iter().map(|tool| tool.name()).collect();
    assert_eq!(
        names,
        [
            "transfer_to_human",
            "end_call",
            "record_answer",
            "schedule_callback",
            "mark_do_not_call"
        ]
    );
    for tool in BuiltinTool::ALL {
        assert_eq!(BuiltinTool::parse(&tool.name().to_uppercase()), Ok(tool));
        assert_ne!(tool.description(), "");
        assert_eq!(tool.locked(), tool == BuiltinTool::MarkDoNotCall);
    }
    assert_eq!(
        BuiltinTool::parse("send_email"),
        Err(ToolError::UnknownBuiltin)
    );

    assert_eq!(
        PurposeTools::default().builtins(),
        [BuiltinTool::MarkDoNotCall]
    );
    let none = PurposeTools::new([], vec![]).expect("valid");
    assert_eq!(none.builtins(), [BuiltinTool::MarkDoNotCall]);
    let some = PurposeTools::new(
        [
            BuiltinTool::ScheduleCallback,
            BuiltinTool::TransferToHuman,
            BuiltinTool::TransferToHuman,
        ],
        vec![],
    )
    .expect("valid");
    assert_eq!(
        some.builtins(),
        [
            BuiltinTool::TransferToHuman,
            BuiltinTool::ScheduleCallback,
            BuiltinTool::MarkDoNotCall
        ]
    );
    assert!(some.has(BuiltinTool::MarkDoNotCall));
    assert!(!some.has(BuiltinTool::EndCall));
}

#[test]
fn tool_names_are_llm_safe_identifiers() {
    assert_eq!(
        ToolName::parse(" Book_Slot ").map(|name| name.as_str().to_owned()),
        Ok("book_slot".to_owned())
    );
    assert!(ToolName::parse(&format!("a{}", "b".repeat(ToolName::MAX - 1))).is_ok());
    for bad in [
        "",
        "1book",
        "_book",
        "book slot",
        "book-slot",
        "böok",
        "book.slot",
        &"a".repeat(ToolName::MAX + 1),
    ] {
        assert_eq!(ToolName::parse(bad), Err(ToolError::InvalidName), "{bad:?}");
    }
}

#[test]
fn tool_urls_are_https_with_a_dns_host() {
    let good = url("HTTPS://Calendar.Example.com:8443/v1/slots?clinic=7");
    assert_eq!(good.host(), "calendar.example.com");
    assert_eq!(
        good.as_str(),
        "https://calendar.example.com:8443/v1/slots?clinic=7"
    );
    assert_eq!(url("https://api.example.com").host(), "api.example.com");

    let refused = [
        ("http://api.example.com/x", UrlError::NotHttps),
        ("ftp://api.example.com", UrlError::NotHttps),
        ("api.example.com/x", UrlError::NotHttps),
        ("https://user:pass@api.example.com/x", UrlError::UserInfo),
        ("https://api.example.com/x#top", UrlError::Fragment),
        ("https://10.0.0.1/x", UrlError::IpLiteral),
        ("https://169.254.169.254/latest", UrlError::IpLiteral),
        ("https://[::1]/x", UrlError::IpLiteral),
        ("https://localhost/x", UrlError::SingleLabelHost),
        ("https://intranet:8080/x", UrlError::SingleLabelHost),
        ("https://-bad.example.com/x", UrlError::InvalidHost),
        ("https://bad_host.example.com/x", UrlError::InvalidHost),
        ("https:///x", UrlError::InvalidHost),
        ("https://api.example.com:0/x", UrlError::InvalidPort),
        ("https://api.example.com:70000/x", UrlError::InvalidPort),
        ("https://api.example.com:x/x", UrlError::InvalidPort),
        ("https://api.example.com/a b", UrlError::InvalidCharacter),
        ("https://api.example.com/{id}", UrlError::InvalidCharacter),
    ];
    for (raw, why) in refused {
        assert_eq!(
            ToolUrl::parse(raw),
            Err(ToolError::InvalidUrl(why)),
            "{raw}"
        );
    }
    let long = format!("https://api.example.com/{}", "a".repeat(ToolUrl::MAX));
    assert_eq!(
        ToolUrl::parse(&long),
        Err(ToolError::InvalidUrl(UrlError::TooLong))
    );
}

#[test]
fn http_tools_have_a_capped_timeout_a_description_and_a_named_secret() {
    let base = http_tool("book_slot", "https://calendar.example.com/v1/slots");
    let spec = |timeout_ms: u32, description: &str| HttpToolSpec {
        name: tool_name("book_slot"),
        description: description.to_owned(),
        inputs: ToolInputs::default(),
        method: HttpMethod::Get,
        url: url("https://calendar.example.com/v1/slots"),
        timeout_ms,
        secret: None,
        confirm: false,
    };

    assert_eq!(base.timeout_ms(), 3_000);
    assert_eq!(
        base.secret().map(SecretRef::as_str),
        Some("CALENDAR_API_KEY")
    );
    assert!(base.confirm());
    assert_eq!(base.method(), HttpMethod::Post);
    assert_eq!(base.url().host(), "calendar.example.com");
    for ok in [HttpTool::MIN_TIMEOUT_MS, HttpTool::MAX_TIMEOUT_MS] {
        assert!(HttpTool::new(spec(ok, "Find slots.")).is_ok(), "{ok}");
    }
    for bad in [99, 10_001, 0] {
        assert_eq!(
            HttpTool::new(spec(bad, "Find slots.")),
            Err(ToolError::TimeoutOutOfRange {
                min: 100,
                max: 10_000
            }),
            "{bad}"
        );
    }
    for bad in ["", "   ", &"d".repeat(HttpTool::MAX_DESCRIPTION + 1)] {
        assert_eq!(
            HttpTool::new(spec(3_000, bad)),
            Err(ToolError::InvalidDescription)
        );
    }
    assert_eq!(
        HttpTool::new(spec(3_000, "  Find slots. "))
            .expect("valid")
            .description(),
        "Find slots."
    );

    assert!(SecretRef::parse("calendar-key_2").is_ok());
    for bad in ["", "2fa", "has space", "a/b", "key=value", &"k".repeat(65)] {
        assert_eq!(
            SecretRef::parse(bad),
            Err(ToolError::InvalidSecretRef),
            "{bad:?}"
        );
    }
    for method in HttpMethod::ALL {
        assert_eq!(
            HttpMethod::parse(&method.as_str().to_lowercase()),
            Ok(method)
        );
    }
    assert_eq!(HttpMethod::parse("TRACE"), Err(ToolError::UnknownMethod));
}

#[test]
fn tool_inputs_are_a_small_json_schema() {
    let param = |name: &str| {
        ToolParam::new(tool_name(name), ParamKind::Integer, "A number", false).expect("valid")
    };
    for kind in ParamKind::ALL {
        assert_eq!(ParamKind::parse(&kind.as_str().to_uppercase()), Ok(kind));
    }
    assert_eq!(ParamKind::parse("object"), Err(ToolError::UnknownParamKind));
    assert_eq!(
        ToolParam::new(
            tool_name("note"),
            ParamKind::String,
            &"d".repeat(ToolParam::MAX_DESCRIPTION + 1),
            false
        ),
        Err(ToolError::InvalidDescription)
    );
    let full: Vec<ToolParam> = (0..ToolInputs::MAX)
        .map(|i| param(&format!("p{i}")))
        .collect();
    assert_eq!(
        ToolInputs::new(full.clone()).map(|inputs| inputs.params().len()),
        Ok(10)
    );
    let mut too_many = full;
    too_many.push(param("extra"));
    assert_eq!(
        ToolInputs::new(too_many),
        Err(ToolError::TooManyInputs { max: 10 })
    );
    assert_eq!(
        ToolInputs::new(vec![param("day"), param("day")]),
        Err(ToolError::DuplicateName {
            name: "day".to_owned()
        })
    );
}

#[test]
fn a_purpose_has_at_most_ten_http_tools_each_named_once_and_not_like_a_built_in() {
    let tool = |name: &str| http_tool(name, "https://calendar.example.com/v1/slots");
    let ten: Vec<HttpTool> = (0..PurposeTools::MAX_HTTP)
        .map(|i| tool(&format!("tool_{i}")))
        .collect();

    assert!(PurposeTools::new([], ten.clone()).is_ok());
    let mut eleven = ten;
    eleven.push(tool("tool_x"));
    assert_eq!(
        PurposeTools::new([], eleven),
        Err(ToolError::TooManyTools { max: 10 })
    );
    assert_eq!(
        PurposeTools::new([], vec![tool("book"), tool("book")]),
        Err(ToolError::DuplicateName {
            name: "book".to_owned()
        })
    );
    assert_eq!(
        PurposeTools::new([], vec![tool("end_call")]),
        Err(ToolError::ReservedName {
            name: "end_call".to_owned()
        })
    );
}

#[test]
fn the_allow_list_matches_hosts_exactly_or_by_subdomain_wildcard() {
    let allowed = hosts(&[" Calendar.Example.com ", "*.tools.test", ""]);

    assert_eq!(allowed.entries(), ["calendar.example.com", "*.tools.test"]);
    assert!(allowed.allows("calendar.example.com"));
    assert!(allowed.allows("api.tools.test"));
    assert!(allowed.allows("eu.api.tools.test"));
    for host in [
        "example.com",
        "evil.calendar.example.com",
        "calendar.example.com.evil.test",
        "tools.test",
        "xtools.test",
    ] {
        assert!(!allowed.allows(host), "{host}");
    }
    assert!(!ToolHosts::default().allows("calendar.example.com"));
    for bad in [
        "https://calendar.example.com",
        "10.0.0.1",
        "*.",
        "a b.com",
        "*",
    ] {
        assert_eq!(
            ToolHosts::new([bad]),
            Err(ToolError::InvalidHostPattern),
            "{bad}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Guardrails
// ---------------------------------------------------------------------------------------

fn guardrails(change: impl FnOnce(&mut GuardrailsSpec)) -> Result<Guardrails, GuardrailError> {
    let mut spec = GuardrailsSpec {
        allowed_topics: vec![],
        never: vec![],
        escalations: vec![],
        max_call_minutes: 30,
        never_ask: vec![],
    };
    change(&mut spec);
    Guardrails::new(spec)
}

#[test]
fn guardrails_default_to_no_limits_and_thirty_minute_calls() {
    let defaults = Guardrails::default();

    assert_eq!(defaults.allowed_topics(), [] as [String; 0]);
    assert_eq!(defaults.never(), [] as [String; 0]);
    assert_eq!(defaults.escalations(), []);
    assert_eq!(defaults.max_call_minutes(), 30);
    assert_eq!(defaults.never_ask(), []);
}

#[test]
fn topics_and_never_rules_are_trimmed_short_lists_without_repeats() {
    let checked = guardrails(|spec| {
        spec.allowed_topics = vec![
            " Booking appointments ".to_owned(),
            "booking APPOINTMENTS".to_owned(),
            "Opening hours".to_owned(),
        ];
        spec.never = vec!["Give medical advice".to_owned()];
    })
    .expect("valid");

    assert_eq!(
        checked.allowed_topics(),
        ["Booking appointments", "Opening hours"]
    );
    assert_eq!(checked.never(), ["Give medical advice"]);
    assert_eq!(
        guardrails(|spec| spec.never = vec!["  ".to_owned()]),
        Err(GuardrailError::EmptyItem)
    );
    assert_eq!(
        guardrails(|spec| spec.allowed_topics = vec!["t".repeat(Guardrails::MAX_ITEM + 1)]),
        Err(GuardrailError::ItemTooLong { max: 200 })
    );
    assert_eq!(
        guardrails(|spec| {
            spec.never = (0..=Guardrails::MAX_ITEMS)
                .map(|i| format!("Rule {i}"))
                .collect();
        }),
        Err(GuardrailError::TooManyItems { max: 20 })
    );
}

#[test]
fn escalation_rules_ask_a_question_and_act() {
    let human = EscalationRule::new(
        " Does the caller ask for a human? ",
        EscalationAction::Transfer,
    )
    .expect("valid");
    assert_eq!(human.when(), "Does the caller ask for a human?");
    assert_eq!(human.action(), &EscalationAction::Transfer);
    let goodbye = EscalationRule::new(
        "Is the caller abusive?",
        EscalationAction::EndCall {
            message: "  I'm ending this call now. Goodbye.  ".to_owned(),
        },
    )
    .expect("valid");
    assert_eq!(
        goodbye.action(),
        &EscalationAction::EndCall {
            message: "I'm ending this call now. Goodbye.".to_owned()
        }
    );
    assert_eq!(
        [
            EscalationAction::Transfer.as_str(),
            goodbye.action().as_str(),
            EscalationAction::Flag.as_str()
        ],
        ["transfer", "end_call", "flag"]
    );
    assert_eq!(
        EscalationRule::new(" ", EscalationAction::Flag),
        Err(GuardrailError::InvalidQuestion)
    );
    assert_eq!(
        EscalationRule::new(&"q".repeat(201), EscalationAction::Flag),
        Err(GuardrailError::InvalidQuestion)
    );
    assert_eq!(
        EscalationRule::new(
            "Is the caller abusive?",
            EscalationAction::EndCall {
                message: " ".to_owned()
            }
        ),
        Err(GuardrailError::InvalidMessage)
    );
    let rule = |i: usize| {
        EscalationRule::new(&format!("Question {i}?"), EscalationAction::Flag).expect("valid")
    };
    assert!(guardrails(|spec| spec.escalations = (0..5).map(rule).collect()).is_ok());
    assert_eq!(
        guardrails(|spec| spec.escalations = (0..6).map(rule).collect()),
        Err(GuardrailError::TooManyEscalations { max: 5 })
    );
}

#[test]
fn limits_cap_the_call_and_name_the_personal_data_never_to_ask_for() {
    for minutes in [1, 30] {
        assert!(guardrails(|spec| spec.max_call_minutes = minutes).is_ok());
    }
    for minutes in [0, 31] {
        assert_eq!(
            guardrails(|spec| spec.max_call_minutes = minutes),
            Err(GuardrailError::CallLengthOutOfRange { min: 1, max: 30 })
        );
    }
    let names: Vec<&str> = PersonalData::ALL.iter().map(|kind| kind.as_str()).collect();
    assert_eq!(
        names,
        [
            "card_number",
            "national_id",
            "password",
            "bank_account",
            "health"
        ]
    );
    for kind in PersonalData::ALL {
        assert_eq!(PersonalData::parse(&kind.as_str().to_uppercase()), Ok(kind));
        assert_ne!(kind.description(), "");
    }
    assert_eq!(
        PersonalData::parse("shoe_size"),
        Err(GuardrailError::UnknownPersonalData)
    );
    let checked = guardrails(|spec| {
        spec.never_ask = vec![
            PersonalData::Password,
            PersonalData::CardNumber,
            PersonalData::Password,
        ];
    })
    .expect("valid");
    assert_eq!(
        checked.never_ask(),
        [PersonalData::CardNumber, PersonalData::Password]
    );
}

// ---------------------------------------------------------------------------------------
// Compliance: tighten-only
// ---------------------------------------------------------------------------------------

#[test]
fn compliance_additions_are_checked() {
    let checked = ComplianceAdditions::new(ComplianceAdditionsSpec {
        opt_out_phrases: vec![" Leave Me Alone ".to_owned(), "leave me alone".to_owned()],
        forbidden_claims: vec![" Promising a refund. ".to_owned()],
        recordings_days: Some(30),
        transcripts_days: None,
    })
    .expect("valid");

    assert_eq!(checked.opt_out_phrases(), ["leave me alone"]);
    assert_eq!(checked.forbidden_claims(), ["Promising a refund."]);
    assert_eq!(checked.recordings_days(), Some(30));
    assert_eq!(checked.transcripts_days(), None);
    let spec = |change: fn(&mut ComplianceAdditionsSpec)| {
        let mut spec = ComplianceAdditionsSpec::default();
        change(&mut spec);
        ComplianceAdditions::new(spec)
    };
    assert_eq!(
        spec(|s| s.opt_out_phrases = vec![" ".to_owned()]),
        Err(ComplianceError::EmptyItem)
    );
    assert_eq!(
        spec(|s| s.opt_out_phrases = vec!["p".repeat(101)]),
        Err(ComplianceError::ItemTooLong { max: 100 })
    );
    assert_eq!(
        spec(|s| s.forbidden_claims = vec!["c".repeat(201)]),
        Err(ComplianceError::ItemTooLong { max: 200 })
    );
    assert_eq!(
        spec(|s| s.forbidden_claims = (0..21).map(|i| format!("Claim {i}")).collect()),
        Err(ComplianceError::TooManyItems { max: 20 })
    );
    assert_eq!(
        spec(|s| s.recordings_days = Some(0)),
        Err(ComplianceError::InvalidRetention)
    );
    assert_eq!(
        spec(|s| s.transcripts_days = Some(36_501)),
        Err(ComplianceError::InvalidRetention)
    );
}

#[test]
fn additions_tighten_the_pack() {
    let pack = pack_for(UseCase::Support, "US");
    let additions = ComplianceAdditions::new(ComplianceAdditionsSpec {
        opt_out_phrases: vec!["leave me alone".to_owned(), "Unsubscribe".to_owned()],
        forbidden_claims: vec!["Promising a refund.".to_owned()],
        recordings_days: Some(30),
        transcripts_days: Some(400),
    })
    .expect("valid");

    let effective = EffectiveCompliance::of(&pack, &additions);

    assert_eq!(effective.locked_texts(), pack.locked_texts());
    assert_eq!(effective.opt_out_confirmation(), CONFIRMATION);
    assert_eq!(
        effective.opt_out_phrases(),
        ["stop calling me", "unsubscribe", "leave me alone"]
    );
    assert_eq!(
        effective.forbidden_claims(),
        ["Saying it is a human.", "Promising a refund."]
    );
    let retention = effective.retention();
    assert_eq!(retention.recordings_days(), 30, "shortened");
    assert_eq!(
        retention.transcripts_days(),
        365,
        "a longer cap changes nothing"
    );
    assert_eq!(retention.audit_days(), 1_826, "never shorter");
}

fn packs() -> impl Strategy<Value = RulePack> {
    (
        proptest::sample::select(UseCase::ALL.to_vec()),
        proptest::collection::vec("[a-z]{1,8}( [a-z]{1,8}){0,2}", 1..4),
        proptest::collection::vec("[A-Z][a-z ]{0,20}\\.", 0..3),
        1_u32..2_000,
        1_u32..2_000,
        1_u32..4_000,
    )
        .prop_map(
            |(use_case, phrases, claims, recordings, transcripts, audit)| {
                let jurisdiction = Jurisdiction::default();
                RulePack::new(RulePackSpec {
                    id: RulePackRef::for_agent(use_case, &jurisdiction)
                        .id()
                        .to_owned(),
                    version: 1,
                    jurisdiction,
                    use_case,
                    effective: date(2026, 11, 1),
                    locked: locked(&[
                        (LockedText::AI_DISCLOSURE, DISCLOSURE),
                        (LockedText::RECORDING_NOTICE, NOTICE),
                    ]),
                    opt_out_phrases: phrases,
                    opt_out_confirmation: CONFIRMATION.to_owned(),
                    forbidden_claims: claims,
                    retention: Retention::new(recordings, transcripts, audit).expect("valid"),
                })
                .expect("valid pack")
            },
        )
}

fn additions() -> impl Strategy<Value = ComplianceAdditions> {
    (
        proptest::collection::vec("[a-z]{1,8}( [a-z]{1,8}){0,2}", 0..5),
        proptest::collection::vec("[A-Z][a-z ]{0,20}\\.", 0..5),
        proptest::option::of(1_u32..36_500),
        proptest::option::of(1_u32..36_500),
    )
        .prop_map(|(phrases, claims, recordings, transcripts)| {
            ComplianceAdditions::new(ComplianceAdditionsSpec {
                opt_out_phrases: phrases,
                forbidden_claims: claims,
                recordings_days: recordings,
                transcripts_days: transcripts,
            })
            .expect("valid additions")
        })
}

proptest! {
    #[test]
    fn compliance_is_tighten_only(pack in packs(), additions in additions()) {
        let effective = EffectiveCompliance::of(&pack, &additions);

        prop_assert_eq!(effective.rule_pack(), pack.reference());
        prop_assert_eq!(effective.locked_texts(), pack.locked_texts());
        prop_assert_eq!(effective.opt_out_confirmation(), pack.opt_out_confirmation());
        let phrases = effective.opt_out_phrases();
        prop_assert_eq!(phrases.get(..pack.opt_out_phrases().len()), Some(pack.opt_out_phrases()));
        for phrase in additions.opt_out_phrases() {
            prop_assert!(phrases.contains(phrase), "{phrase} applies");
        }
        let claims = effective.forbidden_claims();
        prop_assert_eq!(claims.get(..pack.forbidden_claims().len()), Some(pack.forbidden_claims()));
        for claim in additions.forbidden_claims() {
            prop_assert!(claims.contains(claim), "{claim} applies");
        }
        let (kept, limit) = (effective.retention(), pack.retention());
        prop_assert!(kept.recordings_days() <= limit.recordings_days());
        prop_assert!(kept.transcripts_days() <= limit.transcripts_days());
        prop_assert_eq!(kept.audit_days(), limit.audit_days());
        if let Some(cap) = additions.recordings_days() {
            prop_assert_eq!(kept.recordings_days(), cap.min(limit.recordings_days()));
        }
        if let Some(cap) = additions.transcripts_days() {
            prop_assert_eq!(kept.transcripts_days(), cap.min(limit.transcripts_days()));
        }
    }
}

// ---------------------------------------------------------------------------------------
// Agents and purposes
// ---------------------------------------------------------------------------------------

#[test]
fn a_new_agent_pins_its_use_cases_system_purpose_and_keeps_todays_hash() {
    let front_desk = draft("Front desk", UseCase::Support);

    assert_eq!(front_desk.purpose(), &PurposeRef::system(UseCase::Support));
    assert_eq!(front_desk.content_hash().as_str(), FRONT_DESK_HASH);

    let survey = front_desk.apply(&DraftPatch {
        use_case: Some(UseCase::Survey),
        ..DraftPatch::default()
    });
    assert_eq!(survey.purpose(), &PurposeRef::system(UseCase::Survey));
    assert!(
        front_desk
            .changed_fields(&survey)
            .contains(&DraftField::Purpose)
    );
    assert_eq!(DraftField::Purpose.as_str(), "purpose");
    let back = survey.apply(&DraftPatch {
        use_case: Some(UseCase::Support),
        ..DraftPatch::default()
    });
    assert_eq!(back.content_hash().as_str(), FRONT_DESK_HASH);
}

#[test]
fn choosing_a_purpose_version_sets_the_use_case_rule_pack_and_hash() {
    let copy = duplicate_of(UseCase::Booking, "Clinic bookings");
    let (_, version) = copy
        .save_version(&ToolHosts::default(), at(1))
        .expect("version 1");
    let front_desk = draft("Front desk", UseCase::Support).apply(&DraftPatch {
        jurisdiction: Some(Jurisdiction::parse("US-CA").expect("valid")),
        plan: Some(Plan::template(UseCase::Support, &[])),
        ..DraftPatch::default()
    });

    let chosen = front_desk.apply(&DraftPatch {
        purpose: Some(version.choice()),
        ..DraftPatch::default()
    });

    assert_eq!(chosen.purpose(), &version.reference());
    assert_eq!(chosen.persona().use_case(), UseCase::Booking);
    assert_eq!(chosen.rule_pack().id(), "us-ca-booking");
    assert_eq!(chosen.plan(), front_desk.plan(), "a saved plan stays");
    assert_eq!(
        front_desk.changed_fields(&chosen),
        [
            DraftField::UseCase,
            DraftField::RulePack,
            DraftField::Purpose
        ]
    );
    assert_ne!(chosen.content_hash(), front_desk.content_hash());
    let restored = AgentDraft::restore(
        chosen.persona().clone(),
        chosen.jurisdiction().clone(),
        chosen.rule_pack().clone(),
        *chosen.settings(),
        chosen.plan().cloned(),
    )
    .with_purpose(version.reference());
    assert_eq!(restored, chosen);
    assert_eq!(restored.content_hash(), chosen.content_hash());
    let other_version = PurposeRef::new(version.reference().id(), 2).expect("valid");
    assert_ne!(
        restored.clone().with_purpose(other_version).content_hash(),
        chosen.content_hash(),
        "the pinned version is part of the hash"
    );
}

#[test]
fn until_a_plan_is_saved_the_agent_shows_its_versions_starting_flow() {
    let packs = catalog();
    let copy = duplicate_of(UseCase::Custom, "Front door");
    let flow = Plan::new(
        vec![
            step("start", NodeKind::Start, "Welcome to the clinic."),
            step("bye", NodeKind::End, "Goodbye."),
        ],
        vec![path("p1", "start", "bye", "The caller is done")],
    )
    .expect("well-formed");
    let copy = copy
        .update_draft(
            changed(copy.draft(), |spec| spec.flow = flow.clone()),
            at(1),
        )
        .expect("custom")
        .expect("changed");
    let (_, version) = copy
        .save_version(&ToolHosts::default(), at(2))
        .expect("version 1");
    let agent = draft("Front door", UseCase::Custom).apply(&DraftPatch {
        purpose: Some(version.choice()),
        ..DraftPatch::default()
    });

    let shown = DraftPlan::for_purpose(&agent, &packs, version.definition());

    assert!(!shown.saved());
    let pack = packs.get(agent.rule_pack()).expect("the pack ships");
    assert_eq!(shown.plan(), &flow.opened_with(pack.locked_texts()));
    assert_eq!(shown.validation(), &Ok(()));
    let system = DraftPlan::for_purpose(
        &draft("Desk", UseCase::Support),
        &packs,
        PurposeVersion::system(UseCase::Support).definition(),
    );
    assert_eq!(
        system,
        DraftPlan::of(&draft("Desk", UseCase::Support), &packs)
    );
}

/// A custom booking purpose at version 1, and version 2 with `change`.
fn two_versions(change: impl FnOnce(&mut PurposeSpec)) -> (PurposeVersion, PurposeVersion) {
    let allowed = hosts(&["calendar.example.com"]);
    let copy = duplicate_of(UseCase::Booking, "Clinic bookings");
    let (saved, first) = copy.save_version(&allowed, at(1)).expect("version 1");
    let saved = saved
        .update_draft(changed(saved.draft(), change), at(2))
        .expect("custom")
        .expect("changed");
    let (_, second) = saved.save_version(&allowed, at(3)).expect("version 2");
    (first, second)
}

fn on(version: &PurposeVersion) -> AgentDraft {
    draft("Clinic desk", UseCase::Booking).apply(&DraftPatch {
        purpose: Some(version.choice()),
        ..DraftPatch::default()
    })
}

#[test]
fn an_update_says_what_changes_and_follows_the_purpose_without_a_saved_plan() {
    let packs = catalog();
    let (first, second) = two_versions(|spec| {
        spec.description = "Clinic appointments.".to_owned();
        spec.compliance = UseCase::Service;
        spec.guardrails = Guardrails::new(GuardrailsSpec {
            allowed_topics: vec!["Appointments".to_owned()],
            never: vec![],
            escalations: vec![],
            max_call_minutes: 10,
            never_ask: vec![],
        })
        .expect("valid");
    });
    let agent = on(&first);

    let update = PurposeUpdate::new(&agent, &first, &second, &packs, false).expect("an update");

    assert_eq!(update.from(), first.reference());
    assert_eq!(update.to(), second.reference());
    assert_eq!(
        update.changes(),
        [
            PurposeChange::Description,
            PurposeChange::ComplianceClass {
                from: UseCase::Booking,
                to: UseCase::Service
            },
            PurposeChange::Guardrails,
        ]
    );
    let areas: Vec<&str> = update.changes().iter().map(PurposeChange::area).collect();
    assert_eq!(areas, ["description", "compliance_class", "guardrails"]);
    assert_eq!(update.plan(), PlanOnUpdate::FollowsPurpose);
    assert!(!update.plan_edited());
    let moved = agent.apply(update.patch());
    assert_eq!(moved.purpose(), &second.reference());
    assert_eq!(moved.persona().use_case(), UseCase::Service);
    assert_eq!(moved.rule_pack().id(), "us-service");
    assert_eq!(moved.plan(), None);
    assert_eq!(
        PurposeUpdate::new(&agent, &first, &first, &packs, false),
        Err(PurposeError::SameVersion)
    );
    let other = PurposeVersion::system(UseCase::Booking);
    assert_eq!(
        PurposeUpdate::new(&agent, &first, &other, &packs, false),
        Err(PurposeError::OtherPurpose)
    );
}

#[test]
fn an_unedited_saved_plan_moves_to_the_new_flow() {
    let packs = catalog();
    let (first, second) = two_versions(|spec| spec.flow = Plan::template(UseCase::Custom, &[]));
    let agent = on(&first);
    let pack = packs.get(agent.rule_pack()).expect("the pack ships");
    let agent = agent.apply(&DraftPatch {
        plan: Some(first.definition().opened_flow(pack.locked_texts())),
        ..DraftPatch::default()
    });

    let update = PurposeUpdate::new(&agent, &first, &second, &packs, false).expect("an update");

    assert_eq!(update.changes(), [PurposeChange::Flow]);
    assert_eq!(update.plan(), PlanOnUpdate::Replaced);
    assert!(!update.plan_edited());
    let moved = agent.apply(update.patch());
    assert_eq!(
        moved.plan(),
        Some(&second.definition().opened_flow(pack.locked_texts()))
    );
}

#[test]
fn an_edited_plan_is_kept_unless_the_operator_replaces_it() {
    let packs = catalog();
    let (first, second) = two_versions(|spec| {
        spec.flow = Plan::template(UseCase::Custom, &[]);
        spec.persona = "Be brief and warm.".to_owned();
    });
    let agent = on(&first);
    let pack = packs.get(agent.rule_pack()).expect("the pack ships");
    let edited = Plan::template(UseCase::Survey, pack.locked_texts());
    let agent = agent.apply(&DraftPatch {
        plan: Some(edited.clone()),
        ..DraftPatch::default()
    });

    let kept = PurposeUpdate::new(&agent, &first, &second, &packs, false).expect("an update");
    let replaced = PurposeUpdate::new(&agent, &first, &second, &packs, true).expect("an update");

    assert_eq!(
        kept.changes(),
        [PurposeChange::Persona, PurposeChange::Flow]
    );
    assert!(kept.plan_edited());
    assert_eq!(kept.plan(), PlanOnUpdate::KeptEdits);
    assert_eq!(agent.apply(kept.patch()).plan(), Some(&edited));
    assert_eq!(agent.apply(kept.patch()).purpose(), &second.reference());
    assert!(replaced.plan_edited());
    assert_eq!(replaced.plan(), PlanOnUpdate::ReplacedEdits);
    assert_eq!(
        agent.apply(replaced.patch()).plan(),
        Some(&second.definition().opened_flow(pack.locked_texts()))
    );
    assert_eq!(
        [
            PlanOnUpdate::FollowsPurpose,
            PlanOnUpdate::Replaced,
            PlanOnUpdate::KeptEdits,
            PlanOnUpdate::ReplacedEdits
        ]
        .map(PlanOnUpdate::as_str),
        ["follows", "replaced", "kept", "replaced_edits"]
    );
}

#[test]
fn a_purpose_compiles_its_guardrails_tools_and_limits_into_the_script() {
    let base = PurposeVersion::system(UseCase::Booking)
        .definition()
        .clone();
    let purpose = changed(&base, |spec| {
        spec.persona = "Your job: book clinic appointments, kindly and briefly.".to_owned();
        spec.tools = PurposeTools::new(
            [BuiltinTool::TransferToHuman],
            vec![http_tool(
                "book_slot",
                "https://calendar.example.com/v1/slots",
            )],
        )
        .expect("valid");
        spec.guardrails = Guardrails::new(GuardrailsSpec {
            allowed_topics: vec![
                "Booking appointments".to_owned(),
                "Opening hours".to_owned(),
            ],
            never: vec!["Give medical advice".to_owned()],
            escalations: vec![
                EscalationRule::new(
                    "Does the caller ask for a human?",
                    EscalationAction::Transfer,
                )
                .expect("valid"),
            ],
            max_call_minutes: 8,
            never_ask: vec![PersonalData::CardNumber, PersonalData::NationalId],
        })
        .expect("valid");
        spec.additions = ComplianceAdditions::new(ComplianceAdditionsSpec {
            opt_out_phrases: vec!["leave me alone".to_owned()],
            forbidden_claims: vec!["Promising a refund.".to_owned()],
            ..ComplianceAdditionsSpec::default()
        })
        .expect("valid");
    });
    let agent = draft("Ava", UseCase::Booking);

    let script = SessionScript::for_agent(&agent, &purpose);
    let text = script.instructions();

    assert!(
        text.starts_with(
            "You are Ava, an AI voice assistant. Your job: book clinic appointments, kindly and \
         briefly. Speak English."
        ),
        "{text}"
    );
    assert!(
        !text.contains("book, move and cancel appointments"),
        "{text}"
    );
    for expected in [
        "Booking appointments",
        "Opening hours",
        "Give medical advice",
        "Promising a refund.",
        "payment card numbers",
        "social security or other national ID numbers",
        "transfer_to_human",
        "book_slot",
        "Book a free slot in the clinic's calendar.",
    ] {
        assert!(text.contains(expected), "{expected} in {text}");
    }
    assert!(
        !text.contains("mark_do_not_call: "),
        "the pipeline adds the opt-out tool"
    );
    assert!(
        !text.contains("CALENDAR_API_KEY"),
        "never a secret name in a prompt"
    );
    assert!(
        !text.contains("calendar.example.com"),
        "never a URL in a prompt"
    );
    assert_eq!(script.max_call_minutes(), 8);
    let phrases = script.opt_out_phrases();
    assert_eq!(
        phrases
            .get(..vak_domain::DEFAULT_OPT_OUT_PHRASES.len())
            .map(<[String]>::to_vec),
        Some(
            vak_domain::DEFAULT_OPT_OUT_PHRASES
                .iter()
                .map(|phrase| (*phrase).to_owned())
                .collect()
        )
    );
    assert!(phrases.contains(&"leave me alone".to_owned()));
    assert_eq!(
        script.disclosure(),
        SessionScript::for_draft(&agent).disclosure()
    );
}
