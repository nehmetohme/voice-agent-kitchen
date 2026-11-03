//! Reading a chat-completions event stream: SSE lines in, `LlmEvent`s out.

use std::collections::VecDeque;
use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;
use vak_providers::{BoxFuture, LlmEvent, LlmStream, ProviderError, SensitiveText, ToolCall};

use crate::{protocol, transport_error};

/// A tool call being put together from its deltas.
#[derive(Debug, Default)]
struct PartialCall {
    id: String,
    name: String,
    arguments: String,
}

/// What the lines read so far amount to.
#[derive(Debug, Default)]
pub(crate) struct Parser {
    line: Vec<u8>,
    calls: Vec<PartialCall>,
    finished: bool,
}

impl Parser {
    /// Feeds bytes; pushes the events they complete onto `events`.
    pub(crate) fn feed(
        &mut self,
        bytes: &[u8],
        events: &mut VecDeque<LlmEvent>,
    ) -> Result<(), ProviderError> {
        for byte in bytes {
            if *byte == b'\n' {
                let line = std::mem::take(&mut self.line);
                self.line(&line, events)?;
            } else {
                self.line.push(*byte);
            }
        }
        Ok(())
    }

    /// The stream ended: whatever is pending completes the reply.
    pub(crate) fn end(&mut self, events: &mut VecDeque<LlmEvent>) -> Result<(), ProviderError> {
        let line = std::mem::take(&mut self.line);
        self.line(&line, events)?;
        self.finish(events);
        Ok(())
    }

    pub(crate) fn finished(&self) -> bool {
        self.finished
    }

    fn line(&mut self, line: &[u8], events: &mut VecDeque<LlmEvent>) -> Result<(), ProviderError> {
        let line = std::str::from_utf8(line).map_err(|_| protocol("the stream is not UTF-8"))?;
        let Some(data) = line.trim_end_matches('\r').strip_prefix("data:") else {
            // Comments, event names, ids and blank separators carry nothing we use.
            return Ok(());
        };
        let data = data.trim();
        if self.finished || data.is_empty() {
            return Ok(());
        }
        if data == "[DONE]" {
            self.finish(events);
            return Ok(());
        }
        let chunk: Value =
            serde_json::from_str(data).map_err(|_| protocol("an event is not JSON"))?;
        if chunk.get("error").is_some() {
            return Err(protocol("the server reported an error in the stream"));
        }
        let choice = &chunk["choices"][0];
        if let Some(content) = choice["delta"]["content"].as_str()
            && !content.is_empty()
        {
            events.push_back(LlmEvent::Text(SensitiveText::new(content)));
        }
        for delta in choice["delta"]["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let index = delta["index"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .unwrap_or(self.calls.len().saturating_sub(1));
            if index >= self.calls.len() {
                self.calls.resize_with(index + 1, PartialCall::default);
            }
            let call = &mut self.calls[index];
            if let Some(id) = delta["id"].as_str() {
                id.clone_into(&mut call.id);
            }
            if let Some(name) = delta["function"]["name"].as_str() {
                call.name.push_str(name);
            }
            if let Some(arguments) = delta["function"]["arguments"].as_str() {
                call.arguments.push_str(arguments);
            }
        }
        if choice["finish_reason"].is_string() {
            self.flush_calls(events);
        }
        Ok(())
    }

    fn flush_calls(&mut self, events: &mut VecDeque<LlmEvent>) {
        for call in self.calls.drain(..) {
            if call.name.is_empty() {
                continue;
            }
            let arguments = if call.arguments.trim().is_empty() {
                "{}".to_owned()
            } else {
                call.arguments
            };
            events.push_back(LlmEvent::ToolCall(ToolCall::new(
                call.id, call.name, arguments,
            )));
        }
    }

    fn finish(&mut self, events: &mut VecDeque<LlmEvent>) {
        if self.finished {
            return;
        }
        self.flush_calls(events);
        events.push_back(LlmEvent::Done);
        self.finished = true;
    }
}

/// A reply being streamed from the server.
pub(crate) struct ChatStream {
    response: reqwest::Response,
    parser: Parser,
    events: VecDeque<LlmEvent>,
    first_content_by: Instant,
    idle_timeout: Duration,
    content_seen: bool,
    failed: bool,
}

impl ChatStream {
    pub(crate) fn new(
        response: reqwest::Response,
        first_content_by: Instant,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            response,
            parser: Parser::default(),
            events: VecDeque::new(),
            first_content_by,
            idle_timeout,
            content_seen: false,
            failed: false,
        }
    }

    async fn next(&mut self) -> Option<Result<LlmEvent, ProviderError>> {
        loop {
            if let Some(event) = self.events.pop_front() {
                self.content_seen = true;
                return Some(Ok(event));
            }
            if self.failed || self.parser.finished() {
                return None;
            }
            // Until the first content, the request's deadline; after it, the idle timeout.
            let wait_until = if self.content_seen {
                Instant::now() + self.idle_timeout
            } else {
                self.first_content_by
            };
            let chunk = tokio::time::timeout_at(wait_until, self.response.chunk()).await;
            let fed = match chunk {
                Err(_) => Err(ProviderError::Timeout),
                Ok(Err(error)) => Err(transport_error(&error)),
                Ok(Ok(Some(bytes))) => self.parser.feed(&bytes, &mut self.events),
                Ok(Ok(None)) => self.parser.end(&mut self.events),
            };
            if let Err(error) = fed {
                self.failed = true;
                return Some(Err(error));
            }
        }
    }
}

impl LlmStream for ChatStream {
    fn next_event(&mut self) -> BoxFuture<'_, Option<Result<LlmEvent, ProviderError>>> {
        Box::pin(self.next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Vec<LlmEvent>, ProviderError> {
        let mut parser = Parser::default();
        let mut events = VecDeque::new();
        parser.feed(text.as_bytes(), &mut events)?;
        parser.end(&mut events)?;
        Ok(events.into())
    }

    #[test]
    fn content_and_tool_calls_become_events() {
        let events = parse(
            ": keep-alive\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\r\n\
             \n\
             data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"t\"}}]}}]}\n\
             data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\
             data: [DONE]\n\
             data: {\"choices\":[{\"delta\":{\"content\":\"late\"}}]}\n",
        )
        .unwrap();
        assert_eq!(
            events,
            [
                LlmEvent::Text(SensitiveText::new("Hi")),
                LlmEvent::ToolCall(ToolCall::new("c1", "t", "{}")),
                LlmEvent::Done,
            ]
        );
    }

    #[test]
    fn a_stream_without_done_still_completes_and_errors_are_protocol_errors() {
        assert_eq!(
            parse("data: {\"choices\":[{\"delta\":{\"content\":\"x\"}}]}").unwrap(),
            [LlmEvent::Text(SensitiveText::new("x")), LlmEvent::Done]
        );
        for broken in [
            "data: {nope\n",
            "data: {\"error\":{\"message\":\"overloaded\"}}\n",
            "data: \u{0}\n",
        ] {
            assert!(
                matches!(parse(broken), Err(ProviderError::Protocol(_))),
                "{broken}"
            );
        }
        let mut parser = Parser::default();
        assert!(matches!(
            parser.feed(&[0xff, b'\n'], &mut VecDeque::new()),
            Err(ProviderError::Protocol(_))
        ));
    }
}
