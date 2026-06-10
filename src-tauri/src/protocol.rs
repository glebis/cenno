use serde::{Deserialize, Serialize};

// JsonSchema derives on the request-side types exist for rmcp's `ask_user`
// tool: rmcp tool parameter structs must implement schemars::JsonSchema.
// Wire shape (serde attrs) is unchanged.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Urgency {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum InputKind {
    Text,
    Voice,
    VoiceText,
    Choice,
    Scale,
    Confirm,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InputSpec {
    #[serde(default = "default_kind")]
    pub kind: InputKind,
}
fn default_kind() -> InputKind {
    InputKind::Text
}
impl Default for InputSpec {
    fn default() -> Self {
        Self { kind: default_kind() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AskRequest {
    pub title: String,
    #[serde(default)]
    pub body_md: String,
    #[serde(default)]
    pub input: InputSpec,
    #[serde(default)]
    pub choices: Option<Vec<String>>,
    #[serde(default = "default_urgency")]
    pub urgency: Urgency,
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
    #[serde(default)]
    pub a2ui: Option<serde_json::Value>,
}
fn default_urgency() -> Urgency {
    Urgency::Normal
}
fn default_timeout() -> u64 {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Via {
    Voice,
    Text,
    Choice,
}

/// Result of an `ask_user` call.
///
/// Serialized `#[serde(untagged)]`: the two variants are discriminated by
/// their disjoint keys — `answer` (Answered) vs `answered` (TimedOut).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AskResponse {
    Answered { answer: String, via: Via, elapsed_s: f64 },
    /// Invariant: `answered` is always `false` on the wire. The presence of
    /// this shape (`{"answered":false,"prompt_id":...}`) MEANS the prompt
    /// timed out; the field exists only because the wire format requires it.
    TimedOut { answered: bool, prompt_id: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ask_request_roundtrip_with_defaults() {
        let json = r#"{"title":"Check-in","body_md":"How is **focus**?"}"#;
        let req: AskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.title, "Check-in");
        assert!(matches!(req.input.kind, InputKind::Text));
        assert!(matches!(req.urgency, Urgency::Normal));
        assert_eq!(req.timeout_s, 120);
        let back = serde_json::to_string(&req).unwrap();
        assert!(back.contains("\"urgency\":\"normal\""));
    }

    #[test]
    fn answered_response_serializes() {
        let resp = AskResponse::Answered { answer: "ok".into(), via: Via::Text, elapsed_s: 3.2 };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"answer":"ok","via":"text","elapsed_s":3.2}"#);
    }

    #[test]
    fn timeout_response_serializes() {
        let resp = AskResponse::TimedOut { answered: false, prompt_id: "p_1".into() };
        assert_eq!(serde_json::to_string(&resp).unwrap(), r#"{"answered":false,"prompt_id":"p_1"}"#);
    }
}
