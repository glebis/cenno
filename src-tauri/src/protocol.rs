use serde::{Deserialize, Serialize};

// JsonSchema derives on the request-side types exist for rmcp's `ask_user`
// tool: rmcp tool parameter structs must implement schemars::JsonSchema.
// Wire shape (serde attrs) is unchanged.

/// Visual flow theme — selects the surface hue (see docs/design/TOKENS.md).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Flow { Mood, Question, Ema, Reminder, Ambient }

/// Multi-step progress (drives dot pagination in EMA-style flows).
///
/// Deserialization clamps agent-supplied values to a sane range so the dot
/// pagination can never render nonsense: `total` is forced to ≥1 and `step`
/// into `1..=total` (e.g. `{step:5,total:3}` → `{3,3}`, `{step:0,...}` → `1`).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(from = "ProgressWire")]
pub struct Progress { pub step: u32, pub total: u32 }

/// Raw wire shape for [`Progress`] before clamping (see the `From` impl).
#[derive(Deserialize, schemars::JsonSchema)]
struct ProgressWire { step: u32, total: u32 }

impl From<ProgressWire> for Progress {
    fn from(w: ProgressWire) -> Self {
        let total = w.total.max(1);
        let step = w.step.clamp(1, total);
        Progress { step, total }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Text,
    Voice,
    VoiceText,
    Choice,
    Scale,
    Confirm,
    None,
    /// A credential the agent must never read. The value goes to sops-encrypted
    /// storage and the caller receives a [`SecretReceipt`], not the secret.
    Secret,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InputSpec {
    #[serde(default = "default_kind")]
    pub kind: InputKind,
    /// `kind: "secret"` only — the name the secret is stored under
    /// (`~/.cenno/secrets/<name>.sops.json`) and the handle the caller uses with
    /// `cenno secret run` / `cenno secret get`. Required for secret prompts;
    /// validated by [`crate::secret::validate_name`] before the panel opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_name: Option<String>,
    /// `kind: "secret"` only — a built-in format id from
    /// [`crate::secret_formats::FORMATS`]. Drives the live "this does not look
    /// like an OpenAI key" caption. Advisory: a mismatch warns, never blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_format: Option<String>,
    /// `kind: "secret"` only — a human label shown above the field, e.g.
    /// "Stripe secret key (test mode)".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_label: Option<String>,
}
fn default_kind() -> InputKind {
    InputKind::Text
}
impl Default for InputSpec {
    fn default() -> Self {
        Self { kind: default_kind(), secret_name: None, secret_format: None, secret_label: None }
    }
}

/// Agents routinely double-escape newlines — writing `\\n` into a JSON string
/// so the panel renders a literal backslash-n instead of a line break. The
/// panel is the last stop before a human reads the text, so we repair it here
/// rather than expecting every caller to get its escaping right. Only `\n`,
/// `\r\n` and `\t` are unescaped; other backslash sequences are left alone.
pub fn unescape_literals(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('n') => {
                chars.next();
                out.push('\n');
            }
            Some('t') => {
                chars.next();
                out.push('\t');
            }
            // "\r\n" collapses to a single newline; a bare "\r" also becomes one.
            Some('r') => {
                chars.next();
                if chars.peek() == Some(&'\\') {
                    let mut probe = chars.clone();
                    probe.next();
                    if probe.peek() == Some(&'n') {
                        chars.next();
                        chars.next();
                    }
                }
                out.push('\n');
            }
            _ => out.push(c),
        }
    }
    out
}

fn de_unescaped<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    Ok(unescape_literals(&s))
}

fn de_unescaped_opt<'de, D>(d: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = Option::<String>::deserialize(d)?;
    Ok(s.map(|s| unescape_literals(&s)))
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AskRequest {
    #[serde(deserialize_with = "de_unescaped")]
    pub title: String,
    #[serde(default, deserialize_with = "de_unescaped")]
    pub body_md: String,
    #[serde(default)]
    pub input: InputSpec,
    #[serde(default)]
    pub choices: Option<Vec<String>>,
    #[serde(default = "default_urgency")]
    pub urgency: Urgency,
    /// Seconds to wait for an answer. `None` (agent omitted it) defers to the
    /// configured default (`~/.cenno` defaults.timeout_s) and finally to
    /// [`DEFAULT_TIMEOUT_S`]. Resolve via [`AskRequest::timeout_secs`].
    #[serde(default)]
    pub timeout_s: Option<u64>,
    #[serde(default)]
    pub a2ui: Option<serde_json::Value>,
    /// Optional cross-device routing proposal: `"phone"`, `"ipad"`, `"watch"`.
    /// A *hint* only — the user's `~/.cenno` routing policy is authoritative and
    /// can ignore it (see `crate::routing`). Unknown/absent → no preference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_hint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<Flow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
    /// Optional short spoken summary for sound-out (voice-out). When set, the
    /// webview speaks this instead of the title+body — agents write a concise,
    /// ear-friendly line. Absent → fall back to reading the prompt itself.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "de_unescaped_opt"
    )]
    pub say: Option<String>,
    /// Open the panel silently regardless of urgency gating (voice-mute).
    /// The final mute state is echoed back on `Answered.muted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<bool>,
    /// Audio for the panel to play (a generated clip, a jingle). Autoplays
    /// unless the prompt is muted; pauses the countdown while it plays. One of
    /// `data:audio/…;base64,…` or an absolute local path (mp3/wav/m4a/ogg).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioSource>,
    /// Server-set: `audio` after validation, as a ready-to-decode `data:` URI.
    /// The webview never resolves paths or base64 itself — it decodes this.
    /// Filled in by the MCP layer, never read from tool arguments.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub audio_uri: Option<String>,
    /// Server-set: present only when `audio` was rejected as oversized, so the
    /// panel can say why it is silent. Never read from tool arguments.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub audio_oversize: Option<AudioOversizeNote>,
    /// Server-set: the clip's local file path when the agent supplied one, for
    /// the history row. Recorded here rather than read from `audio` at insert
    /// time because `audio` is consumed during validation — otherwise every
    /// prompt with a clip would persist a NULL path.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub audio_path: Option<String>,
    /// Name of the MCP client that asked (its `initialize` clientInfo, e.g.
    /// `claude-code`), shown in the panel header. Filled in by the server —
    /// never read from tool arguments (skip_deserializing) and hidden from the
    /// tool schema, so an agent can't impersonate another through `ask_user`.
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub agent: Option<String>,
}
fn default_urgency() -> Urgency {
    Urgency::Normal
}

/// Built-in fallback when neither the agent nor `~/.cenno` config sets a timeout.
pub const DEFAULT_TIMEOUT_S: u64 = 120;

impl AskRequest {
    /// Resolve the effective timeout: the agent's value, else the configured
    /// default, else [`DEFAULT_TIMEOUT_S`].
    pub fn timeout_secs(&self, config_default: Option<u64>) -> u64 {
        self.timeout_s.or(config_default).unwrap_or(DEFAULT_TIMEOUT_S)
    }

    /// True when this prompt collects a credential whose value must never reach
    /// the caller. Every layer that persists, relays, or serializes an answer
    /// branches on this (see `db::record_prompt`, `mcp::ask_user`, `relay`).
    pub fn is_secret(&self) -> bool {
        matches!(self.input.kind, InputKind::Secret)
    }
}

/// An agent-supplied audio payload for the panel to play: a generated voice
/// clip, a jingle, a music snippet.
///
/// One JSON string, either way: a value starting with `data:` is a data URI,
/// anything else an absolute local path (mp3/wav/m4a/ogg). Both are bounded and
/// magic-byte validated by `crate::audio` before the webview ever sees them.
///
/// Deliberately NOT a two-variant untagged enum: because both variants are a
/// bare `String`, serde would put every value into the first one, so the
/// distinction has to live in the value (the `data:` prefix), not the type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct AudioSource(pub String);

impl AudioSource {
    /// True when this is a `data:` URI rather than a file path.
    pub fn is_uri(&self) -> bool {
        crate::audio::is_data_uri(&self.0)
    }

    /// The reference as the agent wrote it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Raised when the agent's audio was refused as too large.
///
/// Not an error for the agent: the question is still asked, just silently
/// without the clip, and the panel explains why so the user can go and look at
/// the file themselves. That is a better trade than losing the whole prompt to
/// a 12 MB export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AudioOversizeNote {
    /// Size the source claimed, in bytes.
    pub bytes: usize,
    /// The cap that refused it, in bytes.
    pub max: usize,
    /// Set when the source was a file path — the panel offers “Open in Finder”.
    /// `None` for data URIs: there is nothing on disk to reveal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// Answer dictated (possibly then edited) in a voice_text panel.
    VoiceText,
    Text,
    Choice,
    /// A credential captured by a `kind: "secret"` panel. The answer value is
    /// the secret itself; it is stored, never returned.
    Secret,
}

/// What the caller learns after a secret was captured: proof of storage plus
/// enough to tell two keys apart, and nothing usable.
///
/// The `hint` is deliberately lossy (first 6 + last 4 characters) — enough for
/// the user and agent to confirm "that is the key I pasted", never enough to
/// reconstruct it. There is intentionally NO field that can carry the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SecretReceipt {
    /// True once the value is encrypted at rest. Always true in this shape —
    /// the field exists so the wire form is self-describing.
    pub stored: bool,
    /// Name the secret is stored under (`cenno secret run <name>`).
    pub name: String,
    /// Storage backend, currently always `"sops"`.
    pub backend: String,
    /// The format id the panel validated against, if the caller asked for one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// `sk-pro…7a2f` — for human recognition only.
    pub hint: String,
    /// Absolute path of the ciphertext file, so the user can inspect or move it.
    pub path: String,
}

/// Result of an `ask_user` call.
///
/// Serialized `#[serde(untagged)]`: the two variants are discriminated by
/// their disjoint keys — `answer` (Answered) vs `answered` (TimedOut).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AskResponse {
    Answered {
        answer: String,
        via: Via,
        elapsed_s: f64,
        /// Final voice-mute state of the panel (user toggle wins over the
        /// request default). Absent on older frontends — callers treat
        /// missing as unknown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        muted: Option<bool>,
    },
    /// Invariant: `answered` is always `false` on the wire. The presence of
    /// this shape (`{"answered":false,"prompt_id":...}`) MEANS the prompt
    /// timed out; the field exists only because the wire format requires it.
    TimedOut { answered: bool, prompt_id: String },
    /// Result of a `kind: "secret"` prompt: the value was captured and encrypted,
    /// and the caller gets a receipt instead of the secret. Discriminated by the
    /// disjoint `stored` key (Answered has `answer`, TimedOut has `answered`).
    SecretStored { stored: bool, secret: SecretReceipt },
}

/// One `ask_sequence` call: N questions run back-to-back in a single panel.
/// `flow` is an optional default applied to any question that lacks its own.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SequenceRequest {
    pub questions: Vec<AskRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<Flow>,
}

/// Ordered answers for an `ask_sequence` call — one entry per question that
/// ran. A timeout ends the run early, so `answers` may be shorter than the
/// question list (the last entry is then the `TimedOut` shape).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SequenceResponse {
    pub answers: Vec<AskResponse>,
}

/// Per-prompt sequence marker attached to a `prompt` event so the frontend
/// knows a question belongs to an `ask_sequence` run and must NOT hide between
/// steps (it swaps content instead, hiding only after `last`). Absent for a
/// plain `ask_user` prompt.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SeqMeta {
    pub index: u32,
    pub total: u32,
    pub last: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ScreenContextRequest {
    pub include_visible_text: Option<bool>,
    pub max_chars: Option<u32>,
}

impl ScreenContextRequest {
    pub fn include_visible_text(&self) -> bool { self.include_visible_text.unwrap_or(true) }
    pub fn bounded_max_chars(&self) -> u32 { self.max_chars.unwrap_or(8000).clamp(1, 8000) }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScreenContextStatus { Ok, PermissionDenied, AxUnavailable, Blocked }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScreenContextBlockedReason { CaptureDisabled, DeniedBundle, DeniedHost }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawScreenContext {
    pub status: ScreenContextStatus,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub host: Option<String>,
    pub focused_role: Option<String>,
    pub selected_text: Option<String>,
    pub visible_text: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
pub struct ScreenContextResponse {
    pub status: ScreenContextStatus,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
    pub window_title: Option<String>,
    pub url: Option<String>,
    pub focused_role: Option<String>,
    pub selected_text: Option<String>,
    pub visible_text: Option<String>,
    pub truncated: bool,
    pub blocked_reason: Option<ScreenContextBlockedReason>,
    pub redaction_count: usize,
    pub untrusted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_is_server_set_only() {
        // An agent can't claim another's name through tool arguments…
        let req: AskRequest =
            serde_json::from_str(r#"{"title":"t","agent":"someone-else"}"#).unwrap();
        assert_eq!(req.agent, None);
        // …nor see the field in the advertised tool schema…
        let schema = serde_json::to_string(&schemars::schema_for!(AskRequest)).unwrap();
        assert!(!schema.contains("\"agent\""));
        // …but once the server sets it, it reaches the webview.
        let req = AskRequest { agent: Some("claude-code".into()), ..req };
        assert_eq!(serde_json::to_value(&req).unwrap()["agent"], "claude-code");
    }

    #[test]
    fn screen_context_request_defaults_and_clamps_cost() {
        let default: ScreenContextRequest = serde_json::from_str("{}").unwrap();
        assert!(default.include_visible_text());
        assert_eq!(default.bounded_max_chars(), 8000);

        let zero: ScreenContextRequest = serde_json::from_str(r#"{"max_chars":0}"#).unwrap();
        assert_eq!(zero.bounded_max_chars(), 1);
        let huge: ScreenContextRequest = serde_json::from_str(r#"{"max_chars":50000}"#).unwrap();
        assert_eq!(huge.bounded_max_chars(), 8000);
    }

    #[test]
    fn screen_context_statuses_are_typed_success_shapes() {
        let response = ScreenContextResponse {
            status: ScreenContextStatus::PermissionDenied,
            app_name: None, bundle_id: None, window_title: None, url: None,
            focused_role: None, selected_text: None, visible_text: None,
            truncated: false, blocked_reason: None, redaction_count: 0,
            untrusted: true,
        };
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["status"], "permission_denied");
        assert_eq!(json["untrusted"], true);
        assert!(json["blocked_reason"].is_null());
    }

    #[test]
    fn sequence_request_roundtrips() {
        let json = r#"{"questions":[{"title":"a"},{"title":"b"}],"flow":"question"}"#;
        let req: SequenceRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.questions.len(), 2);
        assert_eq!(req.questions[0].title, "a");
        assert!(matches!(req.flow, Some(Flow::Question)));
        let back = serde_json::to_string(&req).unwrap();
        let round: SequenceRequest = serde_json::from_str(&back).unwrap();
        assert_eq!(round.questions.len(), 2);
    }

    #[test]
    fn sequence_request_empty_questions_is_valid() {
        let req: SequenceRequest = serde_json::from_str(r#"{"questions":[]}"#).unwrap();
        assert!(req.questions.is_empty());
        assert!(req.flow.is_none());
        // flow omitted from the wire when None.
        let back = serde_json::to_string(&req).unwrap();
        assert!(!back.contains("flow"));
    }

    #[test]
    fn sequence_response_serializes_ordered_answers() {
        let resp = SequenceResponse {
            answers: vec![
                AskResponse::Answered { answer: "y".into(), via: Via::Text, elapsed_s: 1.0, muted: None },
                AskResponse::TimedOut { answered: false, prompt_id: "p_1".into() },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(
            json,
            r#"{"answers":[{"answer":"y","via":"text","elapsed_s":1.0},{"answered":false,"prompt_id":"p_1"}]}"#
        );
    }

    #[test]
    fn ask_request_roundtrip_with_defaults() {
        let json = r#"{"title":"Check-in","body_md":"How is **focus**?"}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Check-in");
        assert!(matches!(req.input.kind, InputKind::Text));
        assert!(matches!(req.urgency, Urgency::Normal));
        // Omitted timeout → None, resolving to the built-in default (or config).
        assert_eq!(req.timeout_s, None);
        assert_eq!(req.timeout_secs(None), 120);
        assert_eq!(req.timeout_secs(Some(45)), 45); // config default applies
        let explicit: AskRequest =
            serde_json::from_str(r#"{"title":"t","timeout_s":30}"#).unwrap();
        assert_eq!(explicit.timeout_secs(Some(45)), 30); // agent value wins
        let back = serde_json::to_string(&req).unwrap();
        assert!(back.contains("\"urgency\":\"normal\""));
    }

    #[test]
    fn answered_response_serializes() {
        let resp = AskResponse::Answered { answer: "ok".into(), via: Via::Text, elapsed_s: 3.2, muted: None };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"answer":"ok","via":"text","elapsed_s":3.2}"#);
    }

    #[test]
    fn voice_text_via_serializes_snake_case() {
        let resp = AskResponse::Answered {
            answer: "dictated".into(),
            via: Via::VoiceText,
            elapsed_s: 4.0,
            muted: None,
        };
        assert_eq!(
            serde_json::to_string(&resp).unwrap(),
            r#"{"answer":"dictated","via":"voice_text","elapsed_s":4.0}"#
        );
    }

    #[test]
    fn timeout_response_serializes() {
        let resp = AskResponse::TimedOut { answered: false, prompt_id: "p_1".into() };
        assert_eq!(serde_json::to_string(&resp).unwrap(), r#"{"answered":false,"prompt_id":"p_1"}"#);
    }

    #[test]
    fn flow_and_progress_roundtrip() {
        let json = r#"{"title":"t","flow":"mood","progress":{"step":2,"total":3}}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        assert!(matches!(req.flow, Some(Flow::Mood)));
        assert_eq!(req.progress.as_ref().unwrap().step, 2);
        assert_eq!(req.progress.as_ref().unwrap().total, 3);
    }

    #[test]
    fn flow_and_progress_absent_from_wire_when_none() {
        let req: AskRequest = serde_json::from_str(r#"{"title":"t"}"#).unwrap();
        let back = serde_json::to_string(&req).unwrap();
        assert!(!back.contains("flow"));
        assert!(!back.contains("progress"));
    }

    #[test]
    fn progress_clamps_step_over_total() {
        let json = r#"{"title":"t","progress":{"step":5,"total":3}}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        let p = req.progress.unwrap();
        assert_eq!((p.step, p.total), (3, 3));
    }

    #[test]
    fn progress_clamps_step_below_one() {
        let json = r#"{"title":"t","progress":{"step":0,"total":5}}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        let p = req.progress.unwrap();
        assert_eq!((p.step, p.total), (1, 5));
    }

    #[test]
    fn progress_clamps_zero_total() {
        let json = r#"{"title":"t","progress":{"step":0,"total":0}}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        let p = req.progress.unwrap();
        assert_eq!((p.step, p.total), (1, 1));
    }
    #[test]
    fn literal_backslash_escapes_in_body_become_real_whitespace() {
        // The agent double-escaped: the JSON string literally contains
        // backslash-n, so a naive renderer prints "\n" at the user.
        let json = r#"{"title":"A\\nB","body_md":"one\\ntwo\\r\\nthree\\tfour","say":"x\\ny"}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "A\nB");
        assert_eq!(req.body_md, "one\ntwo\nthree\tfour");
        assert_eq!(req.say.as_deref(), Some("x\ny"));
    }

    #[test]
    fn real_newlines_and_other_escapes_survive_untouched() {
        assert_eq!(unescape_literals("a\nb"), "a\nb");
        assert_eq!(unescape_literals(r"C:\path"), r"C:\path");
        assert_eq!(unescape_literals(r"\d+"), r"\d+");
        assert_eq!(unescape_literals("no backslash"), "no backslash");
    }

}
