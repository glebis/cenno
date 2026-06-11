use serde::{Deserialize, Serialize};

// JsonSchema derives on the request-side types exist for rmcp's `ask_user`
// tool: rmcp tool parameter structs must implement schemars::JsonSchema.
// Wire shape (serde attrs) is unchanged.

/// Visual flow theme — selects the surface hue (see docs/design/TOKENS.md).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Flow { Mood, Question, Ema, Reminder, Ambient }

/// Multi-step progress (drives dot pagination in EMA-style flows).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Progress { pub step: u32, pub total: u32 }

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
    /// Seconds to wait for an answer. `None` (agent omitted it) defers to the
    /// configured default (`~/.cenno` defaults.timeout_s) and finally to
    /// [`DEFAULT_TIMEOUT_S`]. Resolve via [`AskRequest::timeout_secs`].
    #[serde(default)]
    pub timeout_s: Option<u64>,
    #[serde(default)]
    pub a2ui: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<Flow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    /// Answer dictated (possibly then edited) in a voice_text panel.
    VoiceText,
    Text,
    Choice,
}

/// Result of an `ask_user` call.
///
/// Serialized `#[serde(untagged)]`: the two variants are discriminated by
/// their disjoint keys — `answer` (Answered) vs `answered` (TimedOut).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AskResponse {
    Answered { answer: String, via: Via, elapsed_s: f64 },
    /// Invariant: `answered` is always `false` on the wire. The presence of
    /// this shape (`{"answered":false,"prompt_id":...}`) MEANS the prompt
    /// timed out; the field exists only because the wire format requires it.
    TimedOut { answered: bool, prompt_id: String },
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

#[cfg(test)]
mod tests {
    use super::*;

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
                AskResponse::Answered { answer: "y".into(), via: Via::Text, elapsed_s: 1.0 },
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
        let resp = AskResponse::Answered { answer: "ok".into(), via: Via::Text, elapsed_s: 3.2 };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"answer":"ok","via":"text","elapsed_s":3.2}"#);
    }

    #[test]
    fn voice_text_via_serializes_snake_case() {
        let resp = AskResponse::Answered {
            answer: "dictated".into(),
            via: Via::VoiceText,
            elapsed_s: 4.0,
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
}
