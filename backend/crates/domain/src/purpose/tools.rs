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
        let name = raw.trim().to_ascii_lowercase();
        let mut bytes = name.bytes();
        let starts_with_letter = bytes.next().is_some_and(|b| b.is_ascii_lowercase());
        let rest_ok = bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if starts_with_letter && rest_ok && name.len() <= Self::MAX {
            Ok(Self(name))
        } else {
            Err(ToolError::InvalidName)
        }
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
        let description = description.trim();
        if description.chars().count() > Self::MAX_DESCRIPTION {
            return Err(ToolError::InvalidDescription);
        }
        Ok(Self {
            name,
            kind,
            description: description.to_owned(),
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
        if params.len() > Self::MAX {
            return Err(ToolError::TooManyInputs { max: Self::MAX });
        }
        if let Some(twice) = first_repeat(params.iter().map(ToolParam::name)) {
            return Err(ToolError::DuplicateName {
                name: twice.as_str().to_owned(),
            });
        }
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
        parse_url(raw.trim()).map_err(ToolError::InvalidUrl)
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
    /// Longest name.
    pub const MAX: usize = 64;

    pub fn parse(raw: &str) -> Result<Self, ToolError> {
        let name = raw.trim();
        let mut bytes = name.bytes();
        let starts_with_letter = bytes.next().is_some_and(|b| b.is_ascii_alphabetic());
        let rest_ok = bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if starts_with_letter && rest_ok && name.len() <= Self::MAX {
            Ok(Self(name.to_owned()))
        } else {
            Err(ToolError::InvalidSecretRef)
        }
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
        let description = spec.description.trim();
        if !(1..=Self::MAX_DESCRIPTION).contains(&description.chars().count()) {
            return Err(ToolError::InvalidDescription);
        }
        if !(Self::MIN_TIMEOUT_MS..=Self::MAX_TIMEOUT_MS).contains(&spec.timeout_ms) {
            return Err(ToolError::TimeoutOutOfRange {
                min: Self::MIN_TIMEOUT_MS,
                max: Self::MAX_TIMEOUT_MS,
            });
        }
        Ok(Self {
            name: spec.name,
            description: description.to_owned(),
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
        if http.len() > Self::MAX_HTTP {
            return Err(ToolError::TooManyTools {
                max: Self::MAX_HTTP,
            });
        }
        if let Some(reserved) = http
            .iter()
            .find(|tool| BuiltinTool::parse(tool.name.as_str()).is_ok())
        {
            return Err(ToolError::ReservedName {
                name: reserved.name.as_str().to_owned(),
            });
        }
        if let Some(twice) = first_repeat(http.iter().map(HttpTool::name)) {
            return Err(ToolError::DuplicateName {
                name: twice.as_str().to_owned(),
            });
        }
        let on: Vec<BuiltinTool> = builtins.into_iter().collect();
        let builtins = BuiltinTool::ALL
            .into_iter()
            .filter(|tool| tool.locked() || on.contains(tool))
            .collect();
        Ok(Self { builtins, http })
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
    /// Entries are trimmed and lower-cased; blank ones and repeats are skipped. Each is a
    /// DNS host name like a tool URL's, or `*.` and one.
    pub fn new<S: AsRef<str>>(entries: impl IntoIterator<Item = S>) -> Result<Self, ToolError> {
        let mut kept: Vec<String> = Vec::new();
        for entry in entries {
            let entry = entry.as_ref().trim().to_ascii_lowercase();
            if entry.is_empty() || kept.contains(&entry) {
                continue;
            }
            let host = entry.strip_prefix("*.").unwrap_or(&entry);
            if dns_host(host).is_err() {
                return Err(ToolError::InvalidHostPattern);
            }
            kept.push(entry);
        }
        Ok(Self { entries: kept })
    }

    /// Whether `host` (lower case, no port) may be called: it is an entry, or a subdomain of
    /// a `*.` entry's domain.
    #[must_use]
    pub fn allows(&self, host: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| match entry.strip_prefix('*') {
                Some(suffix) => host.len() > suffix.len() && host.ends_with(suffix),
                None => entry == host,
            })
    }

    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

/// The first value that occurs twice, in order.
fn first_repeat<'a, T: PartialEq + 'a>(values: impl Iterator<Item = &'a T>) -> Option<&'a T> {
    let mut seen: Vec<&T> = Vec::new();
    for value in values {
        if seen.contains(&value) {
            return Some(value);
        }
        seen.push(value);
    }
    None
}

/// Characters a URL may carry besides letters and digits (RFC 3986), without `#`.
const URL_PUNCTUATION: &str = "-._~:/?[]@!$&'()*+,;=%";

/// Checks a tool URL: `https://`, no fragment or user info, a DNS host, an optional port.
fn parse_url(raw: &str) -> Result<ToolUrl, UrlError> {
    if raw.chars().count() > ToolUrl::MAX {
        return Err(UrlError::TooLong);
    }
    let rest = match raw.get(..8) {
        Some(scheme) if scheme.eq_ignore_ascii_case("https://") => raw.get(8..).unwrap_or(""),
        _ => return Err(UrlError::NotHttps),
    };
    if rest.contains('#') {
        return Err(UrlError::Fragment);
    }
    if !rest
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || URL_PUNCTUATION.contains(c))
    {
        return Err(UrlError::InvalidCharacter);
    }
    let end = rest.find(['/', '?']).unwrap_or(rest.len());
    let (authority, path) = rest.split_at(end);
    if authority.contains('@') {
        return Err(UrlError::UserInfo);
    }
    if authority.starts_with('[') {
        return Err(UrlError::IpLiteral);
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    if let Some(port) = port {
        let number: u16 = port.parse().map_err(|_| UrlError::InvalidPort)?;
        if number == 0 || !port.bytes().all(|b| b.is_ascii_digit()) {
            return Err(UrlError::InvalidPort);
        }
    }
    let host = host.to_ascii_lowercase();
    dns_host(&host)?;
    let mut url = format!("https://{host}");
    if let Some(port) = port {
        url.push(':');
        url.push_str(port);
    }
    url.push_str(path);
    Ok(ToolUrl { url, host })
}

/// Longest host name.
const MAX_HOST: usize = 253;
/// Longest label of a host name.
const MAX_LABEL: usize = 63;

/// A lower-case DNS host name with at least two labels, not an IP address.
fn dns_host(host: &str) -> Result<(), UrlError> {
    let label_ok = |label: &str| {
        (1..=MAX_LABEL).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    if host.is_empty() || host.len() > MAX_HOST || !host.split('.').all(label_ok) {
        return Err(UrlError::InvalidHost);
    }
    // No top-level domain is numeric: a numeric last label is an IPv4 address in some form.
    if host
        .rsplit('.')
        .next()
        .is_some_and(|last| last.bytes().all(|b| b.is_ascii_digit()))
    {
        return Err(UrlError::IpLiteral);
    }
    if !host.contains('.') {
        return Err(UrlError::SingleLabelHost);
    }
    Ok(())
}
