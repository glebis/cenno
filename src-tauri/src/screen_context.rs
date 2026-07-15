//! Guarded screen-context service and injectable AX reader boundary.

use std::{
    ffi::{c_char, c_void, CStr},
    sync::Arc,
};

use crate::{
    capture_guard::{self, CaptureBlocked, CaptureInput, CaptureSource, CaptureState},
    config::CaptureConfig,
    protocol::{
        RawScreenContext, ScreenContextBlockedReason, ScreenContextRequest, ScreenContextResponse,
        ScreenContextStatus,
    },
};

pub trait ScreenContextReader: Send + Sync {
    fn read(&self, request: &ScreenContextRequest) -> Result<RawScreenContext, String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SwiftScreenContextReader;

type ScreenContextCallback = extern "C" fn(*mut c_void, *const c_char);

#[cfg(target_os = "macos")]
extern "C" {
    fn cenno_screen_context_read(
        include_visible_text: i32,
        max_chars: u32,
        ctx: *mut c_void,
        callback: ScreenContextCallback,
    ) -> i32;
}

extern "C" fn screen_context_callback(ctx: *mut c_void, json: *const c_char) {
    if ctx.is_null() || json.is_null() {
        return;
    }
    let output = unsafe { &mut *(ctx.cast::<Option<String>>()) };
    *output = Some(
        unsafe { CStr::from_ptr(json) }
            .to_string_lossy()
            .into_owned(),
    );
}

fn decode_raw_json(json: &str) -> Result<RawScreenContext, String> {
    serde_json::from_str(json).map_err(|e| format!("invalid Swift screen-context JSON: {e}"))
}

impl ScreenContextReader for SwiftScreenContextReader {
    fn read(&self, request: &ScreenContextRequest) -> Result<RawScreenContext, String> {
        #[cfg(target_os = "macos")]
        {
            let mut output: Option<String> = None;
            let code = unsafe {
                cenno_screen_context_read(
                    i32::from(request.include_visible_text()),
                    request.bounded_max_chars(),
                    (&mut output as *mut Option<String>).cast(),
                    screen_context_callback,
                )
            };
            if code != 0 {
                return Err(format!(
                    "Swift screen-context reader failed with code {code}"
                ));
            }
            let json = output.ok_or_else(|| {
                "Swift screen-context reader returned without a callback".to_string()
            })?;
            decode_raw_json(&json)
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = request;
            Ok(RawScreenContext {
                status: ScreenContextStatus::AxUnavailable,
                app_name: None,
                bundle_id: None,
                window_title: None,
                url: None,
                host: None,
                focused_role: None,
                selected_text: None,
                visible_text: None,
                truncated: false,
            })
        }
    }
}

#[derive(Clone)]
pub struct ScreenContextServices {
    reader: Arc<dyn ScreenContextReader>,
    state: CaptureState,
    policy: CaptureConfig,
}

impl ScreenContextServices {
    pub fn new(
        reader: Arc<dyn ScreenContextReader>,
        state: CaptureState,
        policy: CaptureConfig,
    ) -> Self {
        Self {
            reader,
            state,
            policy,
        }
    }

    pub fn read_guarded(
        &self,
        request: &ScreenContextRequest,
    ) -> Result<ScreenContextResponse, String> {
        let _lease = match self.state.begin() {
            Ok(lease) => lease,
            Err(blocked) => return Ok(blocked_response(blocked)),
        };
        let raw = self.reader.read(request)?;
        let raw_json = serde_json::to_string(&raw)
            .map_err(|e| format!("serializing raw screen context: {e}"))?;
        let guarded = match capture_guard::guard(
            CaptureInput {
                source: CaptureSource::Accessibility,
                bundle_id: raw.bundle_id.clone(),
                host: raw.host.clone(),
                text: Some(raw_json),
            },
            &self.policy,
            &self.state,
        ) {
            Ok(guarded) => guarded,
            Err(blocked) => return Ok(blocked_response(blocked)),
        };
        let cleaned: RawScreenContext = serde_json::from_str(
            guarded
                .captured_content
                .as_deref()
                .ok_or_else(|| "capture guard returned no screen-context payload".to_string())?,
        )
        .map_err(|e| format!("decoding guarded screen context: {e}"))?;
        Ok(ScreenContextResponse {
            status: cleaned.status,
            app_name: cleaned.app_name,
            bundle_id: cleaned.bundle_id,
            window_title: cleaned.window_title,
            url: cleaned.url,
            focused_role: cleaned.focused_role,
            selected_text: cleaned.selected_text,
            visible_text: cleaned.visible_text,
            truncated: cleaned.truncated,
            blocked_reason: None,
            redaction_count: guarded.redaction_count,
            untrusted: guarded.untrusted,
        })
    }
}

fn blocked_response(blocked: CaptureBlocked) -> ScreenContextResponse {
    let blocked_reason = match blocked {
        CaptureBlocked::CaptureDisabled => ScreenContextBlockedReason::CaptureDisabled,
        CaptureBlocked::DeniedBundle => ScreenContextBlockedReason::DeniedBundle,
        CaptureBlocked::DeniedHost => ScreenContextBlockedReason::DeniedHost,
    };
    ScreenContextResponse {
        status: ScreenContextStatus::Blocked,
        app_name: None,
        bundle_id: None,
        window_title: None,
        url: None,
        focused_role: None,
        selected_text: None,
        visible_text: None,
        truncated: false,
        blocked_reason: Some(blocked_reason),
        redaction_count: 0,
        untrusted: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ScreenContextBlockedReason, ScreenContextStatus};
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FakeReader(Result<RawScreenContext, String>);

    impl ScreenContextReader for FakeReader {
        fn read(&self, _request: &ScreenContextRequest) -> Result<RawScreenContext, String> {
            self.0.clone()
        }
    }

    fn services(
        raw: RawScreenContext,
        policy: CaptureConfig,
        enabled: bool,
    ) -> ScreenContextServices {
        ScreenContextServices::new(
            Arc::new(FakeReader(Ok(raw))),
            CaptureState::new(enabled, |_| {}),
            policy,
        )
    }

    fn raw() -> RawScreenContext {
        RawScreenContext {
            status: ScreenContextStatus::Ok,
            app_name: Some("Notes".into()),
            bundle_id: Some("com.apple.Notes".into()),
            window_title: Some("note".into()),
            url: None,
            host: None,
            focused_role: Some("AXTextArea".into()),
            selected_text: Some("chosen".into()),
            visible_text: Some("visible".into()),
            truncated: false,
        }
    }

    #[test]
    fn whole_raw_payload_is_redacted_and_marked_untrusted() {
        let raw = RawScreenContext {
            window_title: Some("sk-abcdefghijklmnopqrstuvwxyz123456".into()),
            url: Some("https://safe.test/AKIAABCDEFGHIJKLMNOP".into()),
            host: Some("safe.test".into()),
            selected_text: Some("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.signature".into()),
            visible_text: Some("sk-zyxwvutsrqponmlkjihgfedcba654321".into()),
            ..raw()
        };
        let response = services(raw, CaptureConfig::default(), true)
            .read_guarded(&ScreenContextRequest::default())
            .unwrap();
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("AKIA"));
        assert!(!json.contains("eyJ"));
        assert!(!json.contains("sk-"));
        assert_eq!(response.redaction_count, 4);
        assert!(response.untrusted);
    }

    #[test]
    fn denylist_and_kill_switch_return_content_free_blocked_shapes() {
        let disabled = services(raw(), CaptureConfig::default(), false)
            .read_guarded(&ScreenContextRequest::default())
            .unwrap();
        assert_eq!(disabled.status, ScreenContextStatus::Blocked);
        assert_eq!(
            disabled.blocked_reason,
            Some(ScreenContextBlockedReason::CaptureDisabled)
        );
        assert!(disabled.app_name.is_none() && disabled.visible_text.is_none());

        let denied = services(
            raw(),
            CaptureConfig {
                denylist_bundles: vec!["com.apple.Notes".into()],
                ..Default::default()
            },
            true,
        )
        .read_guarded(&ScreenContextRequest::default())
        .unwrap();
        assert_eq!(
            denied.blocked_reason,
            Some(ScreenContextBlockedReason::DeniedBundle)
        );
        assert!(denied.window_title.is_none() && denied.selected_text.is_none());
    }

    #[test]
    fn absence_statuses_remain_successful_untrusted_results() {
        for status in [
            ScreenContextStatus::PermissionDenied,
            ScreenContextStatus::AxUnavailable,
        ] {
            let response = services(
                RawScreenContext { status, ..raw() },
                CaptureConfig::default(),
                true,
            )
            .read_guarded(&ScreenContextRequest::default())
            .unwrap();
            assert_eq!(response.status, status);
            assert!(response.untrusted);
        }
    }

    #[test]
    fn capture_lease_returns_idle_after_reader_error() {
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let observed = snapshots.clone();
        let state = CaptureState::new(true, move |snapshot| {
            observed.lock().unwrap().push(snapshot);
        });
        let services = ScreenContextServices::new(
            Arc::new(FakeReader(Err("AX failed".into()))),
            state.clone(),
            CaptureConfig::default(),
        );
        assert_eq!(
            services
                .read_guarded(&ScreenContextRequest::default())
                .unwrap_err(),
            "AX failed"
        );
        assert!(!state.is_active());
        let seen = snapshots.lock().unwrap();
        assert_eq!(seen.first().map(|s| s.active), Some(true));
        assert_eq!(seen.last().map(|s| s.active), Some(false));
    }

    struct ToggleOffReader {
        state: CaptureState,
        raw: RawScreenContext,
    }

    impl ScreenContextReader for ToggleOffReader {
        fn read(&self, _request: &ScreenContextRequest) -> Result<RawScreenContext, String> {
            self.state.set_enabled(false);
            Ok(self.raw.clone())
        }
    }

    #[test]
    fn toggle_off_during_ax_read_blocks_final_release() {
        let state = CaptureState::new(true, |_| {});
        let services = ScreenContextServices::new(
            Arc::new(ToggleOffReader {
                state: state.clone(),
                raw: raw(),
            }),
            state,
            CaptureConfig::default(),
        );
        let response = services
            .read_guarded(&ScreenContextRequest::default())
            .unwrap();
        assert_eq!(response.status, ScreenContextStatus::Blocked);
        assert_eq!(
            response.blocked_reason,
            Some(ScreenContextBlockedReason::CaptureDisabled)
        );
        assert!(response.visible_text.is_none());
    }

    #[test]
    fn swift_json_decodes_into_raw_contract() {
        let raw = decode_raw_json(
            r#"{"status":"ok","app_name":"Notes","bundle_id":"com.apple.Notes","window_title":"N","url":null,"host":null,"focused_role":"AXTextArea","selected_text":"chosen","visible_text":"body","truncated":false}"#,
        ).unwrap();
        assert_eq!(raw.status, ScreenContextStatus::Ok);
        assert_eq!(raw.bundle_id.as_deref(), Some("com.apple.Notes"));
        assert_eq!(raw.selected_text.as_deref(), Some("chosen"));
    }
}
