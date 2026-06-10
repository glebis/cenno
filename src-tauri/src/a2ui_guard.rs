//! Boundary validation for native `a2ui` payloads passed to `ask_user`.
//!
//! The web renderer SILENTLY ignores wrong/unknown `version` fields and
//! silently drops v0.8-shaped messages (spike finding,
//! docs/superpowers/research/2026-06-a2ui-react-spike.md), so this guard is
//! the ONLY protection: it rejects malformed payloads at the MCP boundary
//! with actionable errors the agent can correct from, instead of the UI
//! showing nothing.
//!
//! Shape validated: the desugar envelope (src/a2ui/desugar.ts) — a JSON array
//! of message objects, each carrying exactly one of createSurface /
//! updateComponents / updateDataModel, with version "v0.9" when present.

use serde_json::Value;

/// Maximum total components across all updateComponents messages.
pub const MAX_COMPONENTS: usize = 200;
/// Maximum serialized size of the whole payload (256 KiB).
pub const MAX_BYTES: usize = 256 * 1024;

const MESSAGE_KEYS: [&str; 3] = ["createSurface", "updateComponents", "updateDataModel"];

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Validate a native a2ui payload. `Err` strings are returned verbatim to the
/// agent, so each says what was wrong AND what is expected.
pub fn validate_a2ui(value: &Value) -> Result<(), String> {
    // Size cap first: applies regardless of shape, and protects the
    // shape checks below from pathological inputs.
    let size = serde_json::to_string(value)
        .map(|s| s.len())
        .map_err(|e| format!("payload is not serializable JSON: {e}"))?;
    if size > MAX_BYTES {
        return Err(format!(
            "payload is {size} bytes when serialized; the maximum is {MAX_BYTES} bytes \
             (256 KiB) — send a smaller surface"
        ));
    }

    let messages = value.as_array().ok_or_else(|| {
        format!(
            "expected a JSON array of A2UI v0.9 message objects \
             (createSurface / updateComponents / updateDataModel), got {}",
            type_name(value)
        )
    })?;

    let mut total_components: usize = 0;
    for (i, msg) in messages.iter().enumerate() {
        let obj = msg.as_object().ok_or_else(|| {
            format!(
                "message [{i}] must be an object with one of \
                 createSurface / updateComponents / updateDataModel, got {}",
                type_name(msg)
            )
        })?;

        let mut found: Vec<&str> = Vec::new();
        for key in obj.keys() {
            if MESSAGE_KEYS.contains(&key.as_str()) {
                found.push(key);
            } else if key != "version" {
                return Err(format!(
                    "message [{i}] has unknown key \"{key}\"; each message must carry \
                     exactly one of createSurface / updateComponents / updateDataModel \
                     (plus an optional version: \"v0.9\")"
                ));
            }
        }
        match found.len() {
            1 => {}
            0 => {
                return Err(format!(
                    "message [{i}] carries none of createSurface / updateComponents / \
                     updateDataModel; each message must carry exactly one"
                ))
            }
            _ => {
                return Err(format!(
                    "message [{i}] carries multiple message keys ({}); each message \
                     must carry exactly one of createSurface / updateComponents / \
                     updateDataModel",
                    found.join(", ")
                ))
            }
        }

        if let Some(version) = obj.get("version") {
            if version != "v0.9" {
                return Err(format!(
                    "message [{i}] has version {version}; only \"v0.9\" is supported \
                     (the renderer silently drops other versions) — omit version or \
                     set it to \"v0.9\""
                ));
            }
        }

        if let Some(update) = obj.get("updateComponents") {
            total_components += update
                .get("components")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
        }
    }

    if total_components > MAX_COMPONENTS {
        return Err(format!(
            "payload defines {total_components} components across updateComponents \
             messages; the maximum is {MAX_COMPONENTS}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Minimal valid envelope mirroring what src/a2ui/desugar.ts emits.
    fn minimal_envelope() -> Value {
        json!([
            {
                "version": "v0.9",
                "createSurface": { "surfaceId": "main", "catalogId": "cenno:catalog/v1" }
            },
            {
                "version": "v0.9",
                "updateComponents": {
                    "surfaceId": "main",
                    "components": [
                        { "id": "root", "component": "Column", "children": ["t"] },
                        { "id": "t", "component": "Text", "text": "hi" }
                    ]
                }
            },
            {
                "version": "v0.9",
                "updateDataModel": { "surfaceId": "main", "path": "/", "value": {} }
            }
        ])
    }

    #[test]
    fn valid_minimal_envelope_passes() {
        assert_eq!(validate_a2ui(&minimal_envelope()), Ok(()));
    }

    #[test]
    fn version_field_is_optional() {
        let payload = json!([
            { "createSurface": { "surfaceId": "main", "catalogId": "cenno:catalog/v1" } }
        ]);
        assert_eq!(validate_a2ui(&payload), Ok(()));
    }

    #[test]
    fn not_an_array_rejects() {
        let err = validate_a2ui(&json!({"createSurface": {}})).unwrap_err();
        assert!(err.contains("array"), "should name the expected shape: {err}");
        assert!(err.contains("object"), "should name what was given: {err}");
    }

    #[test]
    fn unknown_message_key_rejects_naming_it() {
        let payload = json!([
            { "deleteSurface": { "surfaceId": "main" } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("deleteSurface"), "must name the offender: {err}");
        assert!(err.contains("createSurface"), "must say what is expected: {err}");
    }

    #[test]
    fn message_with_no_known_key_rejects() {
        let err = validate_a2ui(&json!([{ "version": "v0.9" }])).unwrap_err();
        assert!(err.contains("exactly one"), "{err}");
    }

    #[test]
    fn message_with_two_known_keys_rejects() {
        let payload = json!([{
            "createSurface": { "surfaceId": "main", "catalogId": "c" },
            "updateDataModel": { "surfaceId": "main", "path": "/", "value": {} }
        }]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("exactly one"), "{err}");
    }

    #[test]
    fn version_v0_8_rejects() {
        let payload = json!([
            { "version": "v0.8", "createSurface": { "surfaceId": "main", "catalogId": "c" } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("v0.8"), "must name the bad version: {err}");
        assert!(err.contains("v0.9"), "must name the supported version: {err}");
    }

    #[test]
    fn component_count_at_limit_passes() {
        let components: Vec<Value> = (0..MAX_COMPONENTS)
            .map(|i| json!({ "id": format!("c{i}"), "component": "Text", "text": "x" }))
            .collect();
        let payload = json!([
            { "updateComponents": { "surfaceId": "main", "components": components } }
        ]);
        assert_eq!(validate_a2ui(&payload), Ok(()));
    }

    #[test]
    fn component_count_over_limit_rejects() {
        // 201 components, split across two messages to prove the cap is TOTAL.
        let first: Vec<Value> = (0..MAX_COMPONENTS)
            .map(|i| json!({ "id": format!("c{i}"), "component": "Text", "text": "x" }))
            .collect();
        let payload = json!([
            { "updateComponents": { "surfaceId": "main", "components": first } },
            { "updateComponents": { "surfaceId": "main", "components": [
                { "id": "extra", "component": "Text", "text": "x" }
            ] } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("201"), "must report the count: {err}");
        assert!(err.contains("200"), "must report the limit: {err}");
    }

    #[test]
    fn oversized_payload_rejects() {
        let big = "x".repeat(MAX_BYTES); // serialized size > 256 KiB with envelope overhead
        let payload = json!([
            { "updateComponents": { "surfaceId": "main", "components": [
                { "id": "t", "component": "Text", "text": big }
            ] } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("256 KiB"), "must state the limit: {err}");
    }
}
