//! The canonical encoding of a purpose definition, hashed to name exactly what a draft or a
//! version holds. Like the agent draft's, it is versioned (`vak.purpose.v1`) and
//! length-prefixed, so no value can forge another field. Changing it changes every stored
//! hash: bump the version tag.

use super::{EscalationAction, PurposeDefinition};

/// `key=<length>:<value>` lines, in a fixed order, after the version line.
pub(super) fn bytes(definition: &PurposeDefinition) -> Vec<u8> {
    let mut bytes = b"vak.purpose.v1\n".to_vec();
    let mut line = |key: &str, value: &str| {
        bytes.extend_from_slice(format!("{key}={}:{value}\n", value.len()).as_bytes());
    };
    line("name", definition.name().as_str());
    line("description", definition.description());
    line("compliance", definition.compliance().as_str());
    line("persona", definition.persona());
    let flow = definition.flow();
    line("flow.nodes", &flow.nodes().len().to_string());
    for (i, node) in flow.nodes().iter().enumerate() {
        line(&format!("flow.node.{i}.id"), node.id().as_str());
        line(&format!("flow.node.{i}.kind"), node.kind().as_str());
        line(&format!("flow.node.{i}.label"), node.label());
        line(&format!("flow.node.{i}.text"), node.text());
    }
    line("flow.edges", &flow.edges().len().to_string());
    for (i, edge) in flow.edges().iter().enumerate() {
        line(&format!("flow.edge.{i}.id"), edge.id().as_str());
        line(&format!("flow.edge.{i}.from"), edge.from().as_str());
        line(&format!("flow.edge.{i}.to"), edge.to().as_str());
        line(&format!("flow.edge.{i}.condition"), edge.condition());
    }
    let tools = definition.tools();
    let builtins: Vec<&str> = tools.builtins().iter().map(|tool| tool.name()).collect();
    line("tools.builtins", &builtins.join(","));
    line("tools.http", &tools.http().len().to_string());
    for (i, tool) in tools.http().iter().enumerate() {
        let key = |field: &str| format!("tools.http.{i}.{field}");
        line(&key("name"), tool.name().as_str());
        line(&key("description"), tool.description());
        line(&key("method"), tool.method().as_str());
        line(&key("url"), tool.url().as_str());
        line(&key("timeout_ms"), &tool.timeout_ms().to_string());
        line(
            &key("secret"),
            tool.secret().map_or("", |secret| secret.as_str()),
        );
        line(&key("confirm"), &tool.confirm().to_string());
        line(&key("inputs"), &tool.inputs().params().len().to_string());
        for (j, param) in tool.inputs().params().iter().enumerate() {
            let key = |field: &str| format!("tools.http.{i}.input.{j}.{field}");
            line(&key("name"), param.name().as_str());
            line(&key("kind"), param.kind().as_str());
            line(&key("description"), param.description());
            line(&key("required"), &param.required().to_string());
        }
    }
    let guardrails = definition.guardrails();
    list(&mut line, "guardrails.topics", guardrails.allowed_topics());
    list(&mut line, "guardrails.never", guardrails.never());
    line(
        "guardrails.escalations",
        &guardrails.escalations().len().to_string(),
    );
    for (i, rule) in guardrails.escalations().iter().enumerate() {
        line(&format!("guardrails.escalation.{i}.when"), rule.when());
        line(
            &format!("guardrails.escalation.{i}.action"),
            rule.action().as_str(),
        );
        if let EscalationAction::EndCall { message } = rule.action() {
            line(&format!("guardrails.escalation.{i}.message"), message);
        }
    }
    line(
        "guardrails.max_call_minutes",
        &guardrails.max_call_minutes().to_string(),
    );
    let never_ask: Vec<&str> = guardrails
        .never_ask()
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    line("guardrails.never_ask", &never_ask.join(","));
    let additions = definition.additions();
    list(&mut line, "compliance.opt_out", additions.opt_out_phrases());
    list(&mut line, "compliance.claims", additions.forbidden_claims());
    let days = |days: Option<u32>| days.map_or_else(String::new, |days| days.to_string());
    line(
        "compliance.recordings_days",
        &days(additions.recordings_days()),
    );
    line(
        "compliance.transcripts_days",
        &days(additions.transcripts_days()),
    );
    bytes
}

/// A count line, then one line per item.
fn list(line: &mut impl FnMut(&str, &str), key: &str, items: &[String]) {
    line(key, &items.len().to_string());
    for (i, item) in items.iter().enumerate() {
        line(&format!("{key}.{i}"), item);
    }
}
