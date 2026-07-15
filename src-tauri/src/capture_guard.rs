//! Mandatory security boundary for all captured screen context.

use crate::config::CaptureConfig;
use regex::Regex;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, OnceLock,
};

const DEFAULT_DENIED_BUNDLES: &[&str] = &[
    "com.1password.1password",
    "com.1password.1password7",
    "com.bitwarden.desktop",
    "org.keepassxc.keepassxc",
    "com.apple.keychainaccess",
];
const REDACTED: &str = "[REDACTED SECRET]";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureSource {
    Accessibility,
    ScreenCapture,
    Ocr,
    ActivitySample,
}

#[derive(Debug, Clone)]
pub struct CaptureInput {
    pub source: CaptureSource,
    pub bundle_id: Option<String>,
    pub host: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct GuardedCapture {
    pub source: CaptureSource,
    pub bundle_id: Option<String>,
    pub host: Option<String>,
    pub captured_content: Option<String>,
    pub redaction_count: usize,
    pub untrusted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureBlocked {
    CaptureDisabled,
    DeniedBundle,
    DeniedHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureSnapshot {
    pub enabled: bool,
    pub active: bool,
}

struct CaptureStateInner {
    enabled: AtomicBool,
    active: AtomicUsize,
    on_change: Arc<dyn Fn(CaptureSnapshot) + Send + Sync>,
}

#[derive(Clone)]
pub struct CaptureState(Arc<CaptureStateInner>);

impl CaptureState {
    pub fn new(enabled: bool, on_change: impl Fn(CaptureSnapshot) + Send + Sync + 'static) -> Self {
        Self(Arc::new(CaptureStateInner {
            enabled: AtomicBool::new(enabled),
            active: AtomicUsize::new(0),
            on_change: Arc::new(on_change),
        }))
    }

    pub fn is_enabled(&self) -> bool {
        self.0.enabled.load(Ordering::Acquire)
    }

    pub fn is_active(&self) -> bool {
        self.0.active.load(Ordering::Acquire) > 0
    }

    pub fn snapshot(&self) -> CaptureSnapshot {
        CaptureSnapshot {
            enabled: self.is_enabled(),
            active: self.is_active(),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if self.0.enabled.swap(enabled, Ordering::AcqRel) != enabled {
            (self.0.on_change)(self.snapshot());
        }
    }

    pub fn begin(&self) -> Result<ActiveCapture, CaptureBlocked> {
        if !self.is_enabled() {
            return Err(CaptureBlocked::CaptureDisabled);
        }
        let previous = self.0.active.fetch_add(1, Ordering::AcqRel);
        if !self.is_enabled() {
            self.0.active.fetch_sub(1, Ordering::AcqRel);
            return Err(CaptureBlocked::CaptureDisabled);
        }
        if previous == 0 {
            (self.0.on_change)(self.snapshot());
        }
        Ok(ActiveCapture {
            state: self.clone(),
            live: true,
        })
    }
}

pub struct ActiveCapture {
    state: CaptureState,
    live: bool,
}

impl std::fmt::Debug for ActiveCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActiveCapture")
            .field("live", &self.live)
            .finish()
    }
}

impl Drop for ActiveCapture {
    fn drop(&mut self) {
        if self.live && self.state.0.active.fetch_sub(1, Ordering::AcqRel) == 1 {
            (self.state.0.on_change)(self.state.snapshot());
        }
    }
}

pub fn guard(
    input: CaptureInput,
    policy: &CaptureConfig,
    capture_enabled: bool,
) -> Result<GuardedCapture, CaptureBlocked> {
    if !capture_enabled {
        return Err(CaptureBlocked::CaptureDisabled);
    }

    if input.bundle_id.as_deref().is_some_and(|bundle| {
        DEFAULT_DENIED_BUNDLES.contains(&bundle)
            || policy
                .denylist_bundles
                .iter()
                .any(|denied| denied == bundle)
    }) {
        return Err(CaptureBlocked::DeniedBundle);
    }

    if input.host.as_deref().is_some_and(|host| {
        policy
            .denylist_hosts
            .iter()
            .any(|rule| host_matches(host, rule))
    }) {
        return Err(CaptureBlocked::DeniedHost);
    }

    let (captured_content, redaction_count) = match input.text {
        Some(text) if policy.redaction_enabled() => {
            let (text, count) = redact(&text);
            (Some(text), count)
        }
        text => (text, 0),
    };

    Ok(GuardedCapture {
        source: input.source,
        bundle_id: input.bundle_id,
        host: input.host,
        captured_content,
        redaction_count,
        untrusted: true,
    })
}

fn host_matches(host: &str, rule: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    let rule = rule.trim().trim_end_matches('.').to_ascii_lowercase();
    !rule.is_empty() && (host == rule || host.ends_with(&format!(".{rule}")))
}

fn redactors() -> &'static [Regex] {
    static REDACTORS: OnceLock<Vec<Regex>> = OnceLock::new();
    REDACTORS.get_or_init(|| {
        [
            r"(?s)-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----.*?-----END (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
            r"\bAKIA[0-9A-Z]{16}\b",
            r"\beyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
            r"\bsk-[A-Za-z0-9_-]{20,}\b",
        ]
        .into_iter()
        .map(|pattern| Regex::new(pattern).expect("capture redaction regex must compile"))
        .collect()
    })
}

fn redact(text: &str) -> (String, usize) {
    let mut output = text.to_string();
    let mut count = 0;
    for regex in redactors() {
        count += regex.find_iter(&output).count();
        output = regex.replace_all(&output, REDACTED).into_owned();
    }
    (output, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(bundle: &str, host: &str, text: &str) -> CaptureInput {
        CaptureInput {
            source: CaptureSource::Accessibility,
            bundle_id: Some(bundle.into()),
            host: Some(host.into()),
            text: Some(text.into()),
        }
    }

    #[test]
    fn disabled_precedes_denylist_and_redaction() {
        let cfg = CaptureConfig {
            denylist_bundles: vec!["com.secret".into()],
            ..Default::default()
        };
        assert_eq!(
            guard(input("com.secret", "x.test", "sk-secret"), &cfg, false),
            Err(CaptureBlocked::CaptureDisabled)
        );
    }

    #[test]
    fn bundle_and_host_denials_return_no_content() {
        let cfg = CaptureConfig {
            denylist_bundles: vec!["com.secret".into()],
            denylist_hosts: vec!["private.example".into()],
            ..Default::default()
        };
        assert_eq!(
            guard(input("com.secret", "safe.test", "do not leak"), &cfg, true),
            Err(CaptureBlocked::DeniedBundle)
        );
        assert_eq!(
            guard(
                input("com.safe", "mail.private.example", "do not leak"),
                &cfg,
                true
            ),
            Err(CaptureBlocked::DeniedHost)
        );
    }

    #[test]
    fn redacts_high_confidence_secrets_and_marks_output_untrusted() {
        let cfg = CaptureConfig::default();
        let raw = "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.signature AKIAABCDEFGHIJKLMNOP sk-abcdefghijklmnopqrstuvwxyz123456";
        let guarded = guard(input("com.safe", "safe.test", raw), &cfg, true).unwrap();
        let text = guarded.captured_content.unwrap();
        assert!(!text.contains("AKIA"));
        assert!(!text.contains("eyJhbGci"));
        assert!(!text.contains("sk-"));
        assert_eq!(guarded.redaction_count, 3);
        assert!(guarded.untrusted);
    }

    #[test]
    fn matching_is_precise_and_redaction_can_be_disabled() {
        let cfg = CaptureConfig {
            denylist_hosts: vec!["private.example".into()],
            redaction: Some(false),
            ..Default::default()
        };
        let raw = "ask-item sk-example short token AKIAABCDEFGHIJKLMNOP";
        let guarded = guard(input("com.safe", "notprivate.example", raw), &cfg, true).unwrap();
        assert_eq!(guarded.captured_content.as_deref(), Some(raw));
        assert_eq!(guarded.redaction_count, 0);
    }

    #[test]
    fn blocked_serialization_never_contains_captured_text() {
        let encoded = serde_json::to_string(&CaptureBlocked::DeniedHost).unwrap();
        assert_eq!(encoded, r#""denied_host""#);
        assert!(!encoded.contains("captured_content"));
    }

    #[test]
    fn kill_switch_blocks_begin_and_drop_clears_activity() {
        let state = CaptureState::new(true, |_| {});
        assert!(!state.is_active());
        let lease = state.begin().unwrap();
        assert!(state.is_active());
        drop(lease);
        assert!(!state.is_active());
        state.set_enabled(false);
        assert_eq!(state.begin().unwrap_err(), CaptureBlocked::CaptureDisabled);
    }

    #[test]
    fn overlapping_reads_keep_indicator_active_until_last_drop() {
        let state = CaptureState::new(true, |_| {});
        let first = state.begin().unwrap();
        let second = state.begin().unwrap();
        drop(first);
        assert!(state.is_active());
        drop(second);
        assert!(!state.is_active());
    }
}
