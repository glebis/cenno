//! Mac-side CloudKit relay writer.
//!
//! Bridges `ask_user` / `ask_sequence` results to the iCloud.app.cenno
//! CloudKit container so the Watch/iPhone companion app can display prompts
//! and surface answers when the user is away from the Mac.
//!
//! Both public functions are fire-and-forget: the Swift Task runs in the
//! background and CloudKit errors are non-fatal (logged to stderr only).

use std::ffi::CString;

extern "C" {
    fn cenno_relay_write_prompt(
        prompt_id: *const std::os::raw::c_char,
        payload_json: *const std::os::raw::c_char,
        targets: *const std::os::raw::c_char,
        grace_secs: i64,
        timeout_secs: i64,
    );

    fn cenno_relay_update_state(
        prompt_id: *const std::os::raw::c_char,
        state: *const std::os::raw::c_char,
        answer_json: *const std::os::raw::c_char, // nullable
    );
}

/// Publish a newly-registered prompt to CloudKit so the companion can pick it up.
/// `payload_json` is the full `AskRequest` serialised as JSON. `targets` is the
/// resolved routing string (`"iphone:fallback,ipad:mirror"`, empty = no
/// companion device eligible); `grace_secs` is the fallback delay devices apply.
pub fn write_prompt(prompt_id: &str, payload_json: &str, targets: &str, grace_secs: u64, timeout_secs: u64) {
    // No eligible companion device → nothing to publish.
    if targets.is_empty() {
        return;
    }
    let Ok(id) = CString::new(prompt_id) else { return };
    let Ok(payload) = CString::new(payload_json) else { return };
    let Ok(targets_c) = CString::new(targets) else { return };
    unsafe {
        cenno_relay_write_prompt(
            id.as_ptr(),
            payload.as_ptr(),
            targets_c.as_ptr(),
            grace_secs as i64,
            timeout_secs as i64,
        );
    }
}

/// Mark a prompt as answered or timed out in CloudKit.
/// `answer_json` is the serialised `AskResponse::Answered` payload (None for timed_out).
pub fn update_state(prompt_id: &str, state: &str, answer_json: Option<&str>) {
    let Ok(id) = CString::new(prompt_id) else { return };
    let Ok(st) = CString::new(state) else { return };

    let answer_c = answer_json.and_then(|s| CString::new(s).ok());
    let answer_ptr = answer_c
        .as_ref()
        .map(|c| c.as_ptr())
        .unwrap_or(std::ptr::null());

    unsafe {
        cenno_relay_update_state(id.as_ptr(), st.as_ptr(), answer_ptr);
    }
}
