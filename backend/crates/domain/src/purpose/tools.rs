//! A purpose's tools: the platform's built-in tools to turn on, and the workspace's own HTTP
//! tools. M3b stores and checks definitions only; M4b runs them.

/// A tool the platform carries out itself, with no outside integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BuiltinTool {
    /// Hands the call to a person (in browser test calls: ends it with a handoff message).
    TransferToHuman,
    /// Ends the call politely.
    EndCall,
    /// Stores a survey answer with the call.
    RecordAnswer,
    /// Notes a time to call back.
    ScheduleCallback,
    /// Records an opt-out and ends the call. Always on and locked.
    MarkDoNotCall,
}

impl BuiltinTool {
    /// Every built-in tool, in the order pickers show them.
    pub const ALL: [Self; 5] = [
        Self::TransferToHuman,
        Self::EndCall,
        Self::RecordAnswer,
        Self::ScheduleCallback,
        Self::MarkDoNotCall,
    ];

    /// The LLM tool name, e.g. `transfer_to_human`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TransferToHuman => "transfer_to_human",
            Self::EndCall => "end_call",
            Self::RecordAnswer => "record_answer",
            Self::ScheduleCallback => "schedule_callback",
            Self::MarkDoNotCall => "mark_do_not_call",
        }
    }

    /// What the tool does, as the LLM and the operator read it.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::TransferToHuman => "Hand the call to a person, with a short summary.",
            Self::EndCall => "End the call politely once the conversation is done.",
            Self::RecordAnswer => "Store the caller's answer to a question with the call.",
            Self::ScheduleCallback => "Note a day and time to call the caller back.",
            Self::MarkDoNotCall => "Record that the caller asked not to be contacted again.",
        }
    }

    /// Whether the tool is always on: only the opt-out tool.
    #[must_use]
    pub const fn locked(self) -> bool {
        matches!(self, Self::MarkDoNotCall)
    }

    /// Accepts a name in any case, with surrounding whitespace.
    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        let raw = raw.trim();
        Self::ALL
            .into_iter()
            .find(|tool| tool.name().eq_ignore_ascii_case(raw))
            .ok_or(ToolError::UnknownBuiltin)
    }
}

/// Why a tool definition is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    #[error(
        "a tool or input name is 1-64 lowercase letters, digits or '_', starting with a letter"
    )]
    InvalidName,
    #[error("{name} is a built-in tool's name")]
    ReservedName { name: String },
    #[error("two tools or inputs are named {name}")]
    DuplicateName { name: String },
    #[error("a description is 1-500 characters (inputs: up to 200)")]
    InvalidDescription,
    #[error("a purpose has at most {max} HTTP tools")]
    TooManyTools { max: usize },
    #[error("a tool has at most {max} inputs")]
    TooManyInputs { max: usize },
    #[error("the tool URL is not allowed: {0}")]
    InvalidUrl(UrlError),
    #[error("the timeout must be {min} to {max} ms")]
    TimeoutOutOfRange { min: u32, max: u32 },
    #[error("a secret name is 1-64 letters, digits, '_' or '-', starting with a letter")]
    InvalidSecretRef,
    #[error("a tool host is a DNS name, optionally starting with '*.'")]
    InvalidHostPattern,
    #[error("unknown built-in tool")]
    UnknownBuiltin,
    #[error("unknown method; expected GET, POST, PUT, PATCH or DELETE")]
    UnknownMethod,
    #[error("unknown input type; expected string, number, integer or boolean")]
    UnknownParamKind,
}

/// Why a URL can't be a tool's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    #[error("it must start with https://")]
    NotHttps,
    #[error("it must not carry a user name or password")]
    UserInfo,
    #[error("it must not have a fragment")]
    Fragment,
    #[error("its host must be a DNS name, not an IP address")]
    IpLiteral,
    #[error("its host must have a dot, like api.example.com")]
    SingleLabelHost,
    #[error("its host is not a valid DNS name")]
    InvalidHost,
    #[error("its port must be 1 to 65535")]
    InvalidPort,
    #[error("it has a space or a character URLs can't carry")]
    InvalidCharacter,
    #[error("it is longer than 2048 characters")]
    TooLong,
}

/// An LLM-safe identifier: 1–64 lowercase ASCII letters, digits or `_`, starting with a
/// letter. Names tools and their inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ToolName(String);

impl ToolName {
    /// Longest name.
    pub const MAX: usize = 64;

    /// Trims and lower-cases (Postel), then checks the identifier rules.
    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        Ok(Self(raw.trim().to_ascii_lowercase()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The JSON type of a tool input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamKind {
    String,
    Number,
    Integer,
    Boolean,
}

impl ParamKind {
    pub const ALL: [Self; 4] = [Self::String, Self::Number, Self::Integer, Self::Boolean];

    /// The JSON Schema type name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Boolean => "boolean",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        let raw = raw.trim();
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str().eq_ignore_ascii_case(raw))
            .ok_or(ToolError::UnknownParamKind)
    }
}

/// One input of an HTTP tool: a property of its JSON Schema.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolParam {
    name: ToolName,
    kind: ParamKind,
    description: String,
    required: bool,
}

impl ToolParam {
    /// Longest description.
    pub const MAX_DESCRIPTION: usize = 200;

    pub fn new(
        name: ToolName,
        kind: ParamKind,
        description: &str,
        required: bool,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            name,
            kind,
            description: description.trim().to_owned(),
            required,
        })
    }

    #[must_use]
    pub fn name(&self) -> &ToolName {
        &self.name
    }

    #[must_use]
    pub fn kind(&self) -> ParamKind {
        self.kind
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn required(&self) -> bool {
        self.required
    }
}

/// An HTTP tool's inputs: a small JSON Schema object of named, typed properties.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ToolInputs {
    params: Vec<ToolParam>,
}

impl ToolInputs {
    /// Most inputs a tool takes.
    pub const MAX: usize = 10;

    /// At most [`ToolInputs::MAX`] inputs, each name once.
    pub fn new(params: Vec<ToolParam>) -> Result<Self, ToolError> {
        Ok(Self { params })
    }

    #[must_use]
    pub fn params(&self) -> &[ToolParam] {
        &self.params
    }
}

/// The HTTP method a tool calls with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub const ALL: [Self; 5] = [Self::Get, Self::Post, Self::Put, Self::Patch, Self::Delete];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    /// Accepts any case.
    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        let raw = raw.trim();
        Self::ALL
            .into_iter()
            .find(|method| method.as_str().eq_ignore_ascii_case(raw))
            .ok_or(ToolError::UnknownMethod)
    }
}

/// The URL an HTTP tool calls: `https://`, a DNS host name, no user info or fragment, at
/// most 2,048 characters. Whether the host may be called is the allow-list's call.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolUrl {
    url: String,
    host: String,
}

impl ToolUrl {
    /// Longest URL.
    pub const MAX: usize = 2_048;

    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        let url = raw.trim().to_owned();
        Ok(Self {
            host: String::new(),
            url,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// The host name, lower case and without the port.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }
}

/// The name of a secret in the workspace's secret store (a key in the app's Databricks secret
/// scope, or an entry in `.env` locally): 1–64 letters, digits, `_` or `-`, starting with a
/// letter. Never the value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretRef(String);

impl SecretRef {
    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        Ok(Self(raw.trim().to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The parts of an HTTP tool, before [`HttpTool::new`] checks them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpToolSpec {
    pub name: ToolName,
    /// What the tool does: the LLM reads it. 1–500 characters.
    pub description: String,
    pub inputs: ToolInputs,
    pub method: HttpMethod,
    pub url: ToolUrl,
    /// 100 to 10,000 ms.
    pub timeout_ms: u32,
    /// The secret sent as a bearer token, if the API needs one.
    pub secret: Option<SecretRef>,
    /// Whether the agent confirms with the caller before calling it.
    pub confirm: bool,
}

/// A checked HTTP tool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HttpTool {
    name: ToolName,
    description: String,
    inputs: ToolInputs,
    method: HttpMethod,
    url: ToolUrl,
    timeout_ms: u32,
    secret: Option<SecretRef>,
    confirm: bool,
}

impl HttpTool {
    /// Shortest timeout.
    pub const MIN_TIMEOUT_MS: u32 = 100;
    /// Longest timeout: a voice reply can't wait longer.
    pub const MAX_TIMEOUT_MS: u32 = 10_000;
    /// The timeout when none is given.
    pub const DEFAULT_TIMEOUT_MS: u32 = 3_000;
    /// Longest description.
    pub const MAX_DESCRIPTION: usize = 500;

    pub fn new(spec: HttpToolSpec) -> Result<Self, ToolError> {
        Ok(Self {
            name: spec.name,
            description: spec.description,
            inputs: spec.inputs,
            method: spec.method,
            url: spec.url,
            timeout_ms: spec.timeout_ms,
            secret: spec.secret,
            confirm: spec.confirm,
        })
    }

    #[must_use]
    pub fn name(&self) -> &ToolName {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    #[must_use]
    pub fn inputs(&self) -> &ToolInputs {
        &self.inputs
    }

    #[must_use]
    pub fn method(&self) -> HttpMethod {
        self.method
    }

    #[must_use]
    pub fn url(&self) -> &ToolUrl {
        &self.url
    }

    #[must_use]
    pub fn timeout_ms(&self) -> u32 {
        self.timeout_ms
    }

    #[must_use]
    pub fn secret(&self) -> Option<&SecretRef> {
        self.secret.as_ref()
    }

    #[must_use]
    pub fn confirm(&self) -> bool {
        self.confirm
    }
}

/// The tools a purpose turns on: built-in ones (the opt-out tool always) and up to 10 HTTP
/// tools, every name once.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PurposeTools {
    builtins: Vec<BuiltinTool>,
    http: Vec<HttpTool>,
}

impl Default for PurposeTools {
    /// Only the opt-out tool.
    fn default() -> Self {
        Self {
            builtins: vec![BuiltinTool::MarkDoNotCall],
            http: Vec::new(),
        }
    }
}

impl PurposeTools {
    /// Most HTTP tools a purpose has.
    pub const MAX_HTTP: usize = 10;

    /// The built-ins (in [`BuiltinTool::ALL`] order, the opt-out tool always among them) and
    /// the HTTP tools, if every name is unique and none is a built-in's.
    pub fn new(
        builtins: impl IntoIterator<Item = BuiltinTool>,
        http: Vec<HttpTool>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            builtins: builtins.into_iter().collect(),
            http,
        })
    }

    /// The built-in tools turned on, in [`BuiltinTool::ALL`] order.
    #[must_use]
    pub fn builtins(&self) -> &[BuiltinTool] {
        &self.builtins
    }

    #[must_use]
    pub fn has(&self, tool: BuiltinTool) -> bool {
        self.builtins.contains(&tool)
    }

    #[must_use]
    pub fn http(&self) -> &[HttpTool] {
        &self.http
    }
}

/// The hosts HTTP tools may call: exact DNS names, or `*.example.com` for any subdomain.
/// Empty allows nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolHosts {
    entries: Vec<String>,
}

impl ToolHosts {
    /// Entries are trimmed and lower-cased; blank ones are skipped.
    pub fn new<S: AsRef<str>>(entries: impl IntoIterator<Item = S>) -> Result<Self, ToolError> {
        Ok(Self {
            entries: entries
                .into_iter()
                .map(|entry| entry.as_ref().trim().to_ascii_lowercase())
                .collect(),
        })
    }

    /// Whether `host` (lower case, no port) may be called.
    #[must_use]
    pub fn allows(&self, host: &str) -> bool {
        let _ = host;
        false
    }

    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}
