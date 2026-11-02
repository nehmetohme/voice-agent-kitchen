use std::time::Duration;

use serde_json::json;
use vak_domain::{AgentName, UseCase, VoiceId};
use vak_pipeline::Engines;
use vak_testkit::media::{Sent, fake_transport};
use vak_testkit::providers::{LlmStep, ScriptedDecisions, ScriptedLlm, ScriptedStt, ScriptedTts};
use vak_testkit::repositories::InMemoryAgentRepo;

use super::*;
use crate::agents::CreateAgent;

fn ada() -> Actor {
    Actor::new("7364019283746501", "ada@example.com").expect("valid")
}

fn bob() -> Actor {
    Actor::new("1111", "bob@example.com").expect("valid")
}

struct Fakes {
    stt: Arc<ScriptedStt>,
    llm: Arc<ScriptedLlm>,
}

fn engines() -> (Engines, Fakes) {
    let stt = Arc::new(ScriptedStt::new());
    let llm = Arc::new(ScriptedLlm::new());
    let engines = Engines::new(
        stt.clone(),
        llm.clone(),
        Arc::new(ScriptedTts::new()),
        Arc::new(ScriptedDecisions::new()),
    );
    (engines, Fakes { stt, llm })
}

async fn service(settings: VoiceSettings) -> (VoiceService, AgentService, AgentId, Fakes) {
    let agents = AgentService::new(Arc::new(InMemoryAgentRepo::default()));
    let agent = agents
        .create(
            &ada(),
            CreateAgent {
                name: AgentName::parse("Ava").expect("valid"),
                use_case: UseCase::Booking,
                voice: VoiceId::parse("af_bella").expect("valid"),
                language: None,
                jurisdiction: None,
                model: None,
                turn_detection: None,
            },
        )
        .await
        .expect("created");
    let (engines, fakes) = engines();
    let voice = VoiceService::with_settings(agents.clone(), Some(engines), settings);
    (voice, agents, agent.id(), fakes)
}

#[tokio::test(start_paused = true)]
async fn a_ticket_opens_once_and_only_for_its_operator() {
    let (voice, _, agent, _) = service(VoiceSettings::default()).await;

    let ticket = voice.start_session(&ada(), agent).await.expect("a ticket");

    assert_eq!(
        ticket.media_path,
        format!("/sessions/{}/media", ticket.session_id)
    );
    assert!(matches!(
        voice.open_media(&bob(), &ticket.session_id),
        Err(OpenMediaError::NotFound)
    ));
    assert!(voice.open_media(&ada(), &ticket.session_id).is_ok());
    assert!(matches!(
        voice.open_media(&ada(), &ticket.session_id),
        Err(OpenMediaError::NotFound)
    ));
    assert!(matches!(
        voice.open_media(&ada(), "no-such-session"),
        Err(OpenMediaError::NotFound)
    ));
}

#[tokio::test(start_paused = true)]
async fn a_ticket_expires() {
    let (voice, _, agent, _) = service(VoiceSettings::default()).await;
    let stale = voice.start_session(&ada(), agent).await.expect("a ticket");

    tokio::time::advance(Duration::from_secs(61)).await;
    let fresh = voice.start_session(&ada(), agent).await.expect("a ticket");

    assert!(matches!(
        voice.open_media(&ada(), &stale.session_id),
        Err(OpenMediaError::NotFound)
    ));
    assert!(voice.open_media(&ada(), &fresh.session_id).is_ok());
}

#[tokio::test(start_paused = true)]
async fn sessions_need_an_active_agent_and_configured_models() {
    let (voice, agents, agent, _) = service(VoiceSettings::default()).await;
    let unknown = AgentId::parse("01920000-0000-7000-8000-00000000beef").expect("an id");

    assert!(matches!(
        voice.start_session(&ada(), unknown).await,
        Err(StartSessionError::NotFound)
    ));
    agents.archive(&ada(), agent).await.expect("archived");
    assert!(matches!(
        voice.start_session(&ada(), agent).await,
        Err(StartSessionError::Archived)
    ));

    let without_models = VoiceService::new(agents, None);
    assert!(matches!(
        without_models.start_session(&ada(), agent).await,
        Err(StartSessionError::VoiceUnavailable)
    ));
    assert!(matches!(
        without_models.open_media(&ada(), "any"),
        Err(OpenMediaError::NotFound)
    ));
}

#[tokio::test(start_paused = true)]
async fn the_live_session_limit_holds_until_a_session_ends() {
    let settings = VoiceSettings {
        max_live_sessions: 1,
        ..VoiceSettings::default()
    };
    let (voice, _, agent, _) = service(settings).await;
    let first = voice.start_session(&ada(), agent).await.expect("a ticket");
    let second = voice.start_session(&ada(), agent).await.expect("a ticket");

    let running = voice.open_media(&ada(), &first.session_id).expect("opened");
    assert!(matches!(
        voice.start_session(&ada(), agent).await,
        Err(StartSessionError::Busy)
    ));
    assert!(matches!(
        voice.open_media(&ada(), &second.session_id),
        Err(OpenMediaError::Busy)
    ));

    drop(running);
    assert!(voice.start_session(&ada(), agent).await.is_ok());
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_speaks_the_agent_script_and_records_its_latency() {
    let (voice, _, agent, fakes) = service(VoiceSettings::default()).await;
    fakes
        .stt
        .push_reply("Can I book a table?", Duration::from_millis(40));
    fakes
        .llm
        .push_reply([LlmStep::text("Sure, for when?", Duration::from_millis(100))]);
    let ticket = voice.start_session(&ada(), agent).await.expect("a ticket");
    let session = voice
        .open_media(&ada(), &ticket.session_id)
        .expect("opened");
    let (transport, client) = fake_transport();
    let running = tokio::spawn(session.run(transport));

    client.send(json!({"type": "session.start"})).await;
    client
        .silence_until("the disclosure", |log| has_end(log, 0))
        .await;
    client.speak(Duration::from_millis(600)).await;
    client
        .silence_until("the reply", |log| has_end(log, 2))
        .await;
    client.send(json!({"type": "session.end"})).await;

    assert_eq!(running.await.expect("ran"), SessionEndReason::ClientEnded);
    let log = client.log();
    let Sent::Event { event, .. } = &log[0] else {
        panic!("session.started first")
    };
    assert_eq!(event["session_id"], ticket.session_id);
    let disclosure = log.iter().find_map(|sent| match sent {
        Sent::Event { event, .. }
            if event["type"] == "transcript.final" && event["turn_id"] == 0 =>
        {
            event["text"].as_str().map(str::to_owned)
        }
        _ => None,
    });
    assert_eq!(
        disclosure.as_deref(),
        Some("Hi, this is Ava, an automated AI assistant. This call may be recorded.")
    );
    let prompt = &fakes.llm.calls()[0].messages[0].1;
    assert!(prompt.contains("book, move and cancel appointments"));
    let mouth_to_ear = voice
        .latency()
        .into_iter()
        .find(|stage| stage.stage.as_str() == "mouth_to_ear")
        .expect("a mouth-to-ear histogram");
    assert_eq!(mouth_to_ear.count, 1);
}

fn has_end(log: &[Sent], turn: u32) -> bool {
    log.iter().any(|sent| {
        matches!(sent, Sent::Event { event, .. }
            if event["type"] == "agent.audio.end" && event["turn_id"] == turn)
    })
}
