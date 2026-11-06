//! `LlmEngine`: streamed chat completions with tools.

use std::time::Duration;

use crate::{BoxFuture, ProviderError, SensitiveText};

/// Who wrote a chat message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    /// The chat-completions role name: `system`, `user` or `assistant`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}

/// One message of the conversation so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    role: Role,
    content: SensitiveText,
}

impl ChatMessage {
    #[must_use]
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: SensitiveText::new(content),
        }
    }

    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    #[must_use]
    pub fn content(&self) -> &str {
        self.content.as_str()
    }
}

/// A function the model may call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolSpec {
    name: String,
    description: String,
    parameters: String,
}

impl ToolSpec {
    /// A tool without parameters (`{"type":"object","properties":{}}`).
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: r#"{"type":"object","properties":{}}"#.to_owned(),
        }
    }

    /// The JSON Schema of the tool's arguments, as JSON text.
    #[must_use]
    pub fn with_parameters_json(self, parameters: impl Into<String>) -> Self {
        Self {
            parameters: parameters.into(),
            ..self
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn parameters_json(&self) -> &str {
        &self.parameters
    }
}

/// One chat completion to stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmRequest {
    messages: Vec<ChatMessage>,
    tools: Vec<ToolSpec>,
    max_tokens: Option<u32>,
    greedy_seed: Option<u64>,
    deadline: Duration,
}

impl LlmRequest {
    /// An empty conversation. The stream fails with [`ProviderError::Timeout`] when its
    /// first event doesn't arrive within `deadline` (time to first token).
    #[must_use]
    pub const fn new(deadline: Duration) -> Self {
        Self {
            messages: Vec::new(),
            tools: Vec::new(),
            max_tokens: None,
            greedy_seed: None,
            deadline,
        }
    }

    #[must_use]
    pub fn with_message(mut self, role: Role, content: impl Into<String>) -> Self {
        self.messages.push(ChatMessage::new(role, content));
        self
    }

    #[must_use]
    pub fn with_tool(mut self, tool: ToolSpec) -> Self {
        self.tools.push(tool);
        self
    }

    #[must_use]
    pub fn with_max_tokens(self, max_tokens: u32) -> Self {
        Self {
            max_tokens: Some(max_tokens),
            ..self
        }
    }

    /// Samples greedily (temperature 0) with `seed`, so the same request gets the same reply
    /// wherever the model honours it: test runs are reproducible. Without it, the model's
    /// own sampling defaults apply.
    #[must_use]
    pub fn with_greedy_sampling(self, seed: u64) -> Self {
        Self {
            greedy_seed: Some(seed),
            ..self
        }
    }

    #[must_use]
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    #[must_use]
    pub fn tools(&self) -> &[ToolSpec] {
        &self.tools
    }

    #[must_use]
    pub const fn max_tokens(&self) -> Option<u32> {
        self.max_tokens
    }

    /// The seed of greedy sampling (temperature 0), if the request pins it.
    #[must_use]
    pub const fn greedy_seed(&self) -> Option<u64> {
        self.greedy_seed
    }

    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }
}

/// A complete call of a tool the request offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCall {
    id: String,
    name: String,
    arguments: SensitiveText,
}

impl ToolCall {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments: SensitiveText::new(arguments),
        }
    }

    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The arguments as JSON text.
    #[must_use]
    pub fn arguments(&self) -> &str {
        self.arguments.as_str()
    }
}

/// One event of a streamed reply.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LlmEvent {
    /// The next piece of the reply's text, as it arrives.
    Text(SensitiveText),
    /// A tool call, once its arguments are complete.
    ToolCall(ToolCall),
    /// The reply is complete. Nothing follows it.
    Done,
}

/// A reply being streamed. Dropping it aborts the request, which frees the model for the
/// next one: barge-in relies on that.
pub trait LlmStream: Send {
    /// The next event, or `None` after [`LlmEvent::Done`] or an error. A stream that stalls
    /// fails with [`ProviderError::Timeout`].
    fn next_event(&mut self) -> BoxFuture<'_, Option<Result<LlmEvent, ProviderError>>>;
}

/// Streamed chat completions with tools.
pub trait LlmEngine: Send + Sync {
    /// Starts streaming the reply to `request`.
    fn stream(
        &self,
        request: LlmRequest,
    ) -> BoxFuture<'_, Result<Box<dyn LlmStream>, ProviderError>>;
}
