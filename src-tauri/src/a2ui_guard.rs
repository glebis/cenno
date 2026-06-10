//! Boundary validation for native `a2ui` payloads passed to `ask_user`.
//!
//! The web renderer ignores the `version` field entirely and silently drops
//! messages it cannot parse (spike finding,
//! docs/superpowers/research/2026-06-a2ui-react-spike.md), so this guard is
//! the first line of protection: it rejects malformed payloads at the MCP
//! boundary with actionable errors the agent can correct from, instead of
//! the UI showing a blank panel that parks the prompt for the full
//! timeout_s. (Second layer: PromptPanel falls back to the desugared prompt
//! if a guard-passing payload still fails to build or render.)
//!
//! Shape validated: the desugar envelope (src/a2ui/desugar.ts) — a JSON
//! array of message objects, each carrying exactly one of createSurface /
//! updateComponents / updateDataModel, with version "v0.9" when present.
//! Additionally: at least one createSurface is required, every createSurface
//! must target the one catalog cenno ships, and every updateComponents must
//! be an object carrying a components array — payloads violating any of
//! these render nothing on the web side.

use serde_json::Value;

/// Maximum total components across all updateComponents messages.
pub const MAX_COMPONENTS: usize = 200;
/// Maximum serialized size of the whole payload (256 KiB).
pub const MAX_BYTES: usize = 256 * 1024;
/// The only catalog the webview registers (src/a2ui/catalog.tsx).
pub const CATALOG_ID: &str = "cenno:catalog/v1";

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
        // serde_json::Value always serializes (string keys only, and NaN /
        // infinity are unrepresentable in Value::Number).
        .expect("serde_json::Value serializes infallibly")
        .len();
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

    let mut create_surface_count: usize = 0;
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
                     (the renderer ignores the version field entirely, so a mismatch \
                     signals a payload built for another A2UI revision) — omit version \
                     or set it to \"v0.9\""
                ));
            }
        }

        if let Some(create) = obj.get("createSurface") {
            create_surface_count += 1;
            let create_obj = create.as_object().ok_or_else(|| {
                format!(
                    "message [{i}].createSurface must be an object with surfaceId and \
                     catalogId, got {}",
                    type_name(create)
                )
            })?;
            match create_obj.get("catalogId").and_then(Value::as_str) {
                Some(CATALOG_ID) => {}
                other => {
                    return Err(format!(
                        "message [{i}].createSurface.catalogId must be \"{CATALOG_ID}\" \
                         (the only catalog cenno ships), got {}",
                        other.map_or_else(
                            || create_obj
                                .get("catalogId")
                                .map_or("nothing".to_string(), |v| type_name(v).to_string()),
                            |s| format!("\"{s}\"")
                        )
                    ))
                }
            }
        }

        if let Some(update) = obj.get("updateComponents") {
            let update_obj = update.as_object().ok_or_else(|| {
                format!(
                    "message [{i}].updateComponents must be an object with a \
                     \"components\" array, got {}",
                    type_name(update)
                )
            })?;
            let components = update_obj.get("components").ok_or_else(|| {
                format!(
                    "message [{i}].updateComponents is missing \"components\"; it must \
                     be an array of component objects"
                )
            })?;
            let components = components.as_array().ok_or_else(|| {
                format!(
                    "message [{i}].updateComponents.components must be an array of \
                     component objects, got {}",
                    type_name(components)
                )
            })?;
            total_components += components.len();
        }
    }

    if create_surface_count == 0 {
        return Err(format!(
            "payload contains no createSurface message; include one \
             ({{\"createSurface\": {{\"surfaceId\": ..., \"catalogId\": \
             \"{CATALOG_ID}\"}}}}) or the renderer has no surface to draw"
        ));
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

    fn create_surface() -> Value {
        json!({ "createSurface": { "surfaceId": "main", "catalogId": CATALOG_ID } })
    }

    /// Minimal valid envelope mirroring what src/a2ui/desugar.ts emits.
    fn minimal_envelope() -> Value {
        json!([
            {
                "version": "v0.9",
                "createSurface": { "surfaceId": "main", "catalogId": CATALOG_ID }
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
        let payload = json!([create_surface()]);
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
            "createSurface": { "surfaceId": "main", "catalogId": CATALOG_ID },
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
    fn create_surface_must_be_an_object() {
        let err = validate_a2ui(&json!([{ "createSurface": "main" }])).unwrap_err();
        assert!(err.contains("must be an object"), "{err}");
        assert!(err.contains("a string"), "must name what was given: {err}");
    }

    #[test]
    fn wrong_catalog_id_rejects_naming_the_real_one() {
        let payload = json!([
            { "createSurface": { "surfaceId": "main", "catalogId": "other:catalog/v2" } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("cenno:catalog/v1"), "must name the only catalog: {err}");
        assert!(err.contains("other:catalog/v2"), "must name what was given: {err}");
    }

    #[test]
    fn missing_catalog_id_rejects() {
        let payload = json!([
            { "createSurface": { "surfaceId": "main" } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("cenno:catalog/v1"), "{err}");
        assert!(err.contains("nothing"), "must say the field is absent: {err}");
    }

    #[test]
    fn payload_without_create_surface_rejects() {
        let payload = json!([
            { "updateComponents": { "surfaceId": "main", "components": [
                { "id": "root", "component": "Column", "children": [] }
            ] } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("no createSurface"), "{err}");
        assert!(err.contains("cenno:catalog/v1"), "must show the fix: {err}");
    }

    #[test]
    fn update_components_not_an_object_rejects() {
        let payload = json!([create_surface(), { "updateComponents": null }]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("updateComponents"), "{err}");
        assert!(err.contains("must be an object"), "{err}");
        assert!(err.contains("null"), "must name what was given: {err}");
    }

    #[test]
    fn update_components_missing_components_rejects() {
        let payload = json!([create_surface(), { "updateComponents": { "surfaceId": "main" } }]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("missing \"components\""), "{err}");
        assert!(err.contains("array of component objects"), "{err}");
    }

    #[test]
    fn update_components_components_not_an_array_rejects() {
        for bad in [json!(null), json!("root"), json!({ "id": "root" })] {
            let payload = json!([
                create_surface(),
                { "updateComponents": { "surfaceId": "main", "components": bad } }
            ]);
            let err = validate_a2ui(&payload).unwrap_err();
            assert!(err.contains("components must be an array"), "{err}");
        }
    }

    #[test]
    fn component_count_at_limit_passes() {
        let components: Vec<Value> = (0..MAX_COMPONENTS)
            .map(|i| json!({ "id": format!("c{i}"), "component": "Text", "text": "x" }))
            .collect();
        let payload = json!([
            create_surface(),
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
            create_surface(),
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
            create_surface(),
            { "updateComponents": { "surfaceId": "main", "components": [
                { "id": "t", "component": "Text", "text": big }
            ] } }
        ]);
        let err = validate_a2ui(&payload).unwrap_err();
        assert!(err.contains("256 KiB"), "must state the limit: {err}");
    }
}
