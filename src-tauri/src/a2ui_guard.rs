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
//!
//! Component props are then checked one type at a time in `check_component`;
//! today only `Image.url`, which rejects remote URLs and data URIs the webview
//! cannot render (see `check_image_url`).

use serde_json::Value;

/// Maximum total components across all updateComponents messages.
pub const MAX_COMPONENTS: usize = 200;
/// Maximum serialized size of the whole payload (256 KiB).
pub const MAX_BYTES: usize = 256 * 1024;
/// The only catalog the webview registers (src/a2ui/catalog.tsx).
pub const CATALOG_ID: &str = "cenno:catalog/v1";

const MESSAGE_KEYS: [&str; 3] = ["createSurface", "updateComponents", "updateDataModel"];

/// Inline raster formats the webview can render, mirroring `DATA_URI_OK` in
/// src/a2ui/image.ts. SVG is deliberately absent: `data:image/svg+xml` is an
/// XSS vector.
const DATA_URI_OK: [&str; 4] = ["png", "jpeg", "gif", "webp"];

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

/// Does the part of a `data:` URI after `data:image/` have an allowed raster
/// MIME followed by exactly `;base64,`?
fn is_renderable_data_uri(rest: &str) -> bool {
    for mime in DATA_URI_OK {
        if let Some(tail) = rest.strip_prefix(mime) {
            return tail.starts_with(";base64,");
        }
    }
    false
}

/// The one rule for an `Image.url`, shared by every component that carries an
/// image reference (today `Image.url`, later `ImageChoice` options).
///
/// Anything that is neither a `data:` URI nor a remote URL is treated as a
/// local path and passes: the `load_image` command enforces the path rules
/// (absolute, image extension, size cap, magic bytes) at read time, and the
/// guard must not stat files.
fn check_image_url(url: &str) -> Result<(), String> {
    if let Some(rest) = url.strip_prefix("data:") {
        if rest
            .strip_prefix("image/")
            .is_some_and(is_renderable_data_uri)
        {
            return Ok(());
        }
        return Err(format!(
            "malformed data: URI \"{url}\" — expected \
             data:image/<png|jpeg|gif|webp>;base64,… (lowercase MIME, base64 \
             payload); SVG and other types are not renderable"
        ));
    }
    if url.starts_with("https:") || url.starts_with("http:") {
        return Err(format!(
            "remote images are not allowed (\"{url}\") — cenno never fetches; embed \
             the image as a data: URI or pass a local file path"
        ));
    }
    Ok(())
}

/// Per-component prop rules. Called for every entry of every
/// `updateComponents.components` array.
///
/// Unknown component names — and entries that are not objects — return
/// `Ok(())`: the renderer drops what it cannot build, as it always has. Each
/// arm below covers exactly one component type.
fn check_component(component: &Value) -> Result<(), String> {
    let Some(obj) = component.as_object() else {
        return Ok(());
    };
    match obj.get("component").and_then(Value::as_str) {
        Some("Image") => {
            let label = obj
                .get("id")
                .and_then(Value::as_str)
                .map_or_else(|| "Image".to_string(), |id| format!("Image \"{id}\""));
            let url = obj.get("url").and_then(Value::as_str).ok_or_else(|| {
                format!(
                    "{label}: Image.url must be a string (a data: URI or a local file \
                     path), got {} — add it or remove the component",
                    obj.get("url").map_or("nothing", type_name)
                )
            })?;
            check_image_url(url).map_err(|err| format!("{label}: {err}"))
        }
        _ => Ok(()),
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

            for component in components {
                check_component(component)?;
            }
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

    // ---- Image.url (Task 4) ----

    /// Envelope carrying the standard root Column plus one extra component
    /// under test, so each case exercises the component walk.
    fn envelope_with(component: Value) -> Value {
        json!([
            create_surface(),
            { "version": "v0.9", "updateComponents": { "surfaceId": "main",
                "components": [
                    { "id": "root", "component": "Column", "children": ["c"] },
                    component
                ] } }
        ])
    }

    #[test]
    fn https_image_rejects_with_the_fix() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "https://example.com/a.png" })))
            .unwrap_err();
        assert!(err.contains("remote images are not allowed"), "{err}");
        assert!(err.contains("data: URI"), "{err}");
    }

    #[test]
    fn http_image_rejects_too() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "http://example.com/a.png" })))
            .unwrap_err();
        assert!(err.contains("remote images are not allowed"), "{err}");
        assert!(err.contains("http://example.com/a.png"), "must quote the url: {err}");
    }

    #[test]
    fn malformed_data_uri_rejects() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "data:text/html;base64,AAAA" })))
            .unwrap_err();
        assert!(err.contains("data:image/"), "{err}");
    }

    #[test]
    fn every_bad_data_uri_shape_rejects() {
        for url in [
            "data:image/svg+xml;base64,AAAA", // XSS vector
            "data:image/PNG;base64,AAAA",    // uppercase mime
            "data:image/png,AAAA",           // not base64
            "data:image/png;base64",         // no payload separator
            "data:,AAAA",
        ] {
            let err = validate_a2ui(&envelope_with(json!({
                "id": "c", "component": "Image", "url": url })))
                .unwrap_err();
            assert!(err.contains("data:image/"), "{url} must be rejected: {err}");
        }
    }

    #[test]
    fn local_path_and_good_data_uri_pass() {
        for url in [
            "/Users/x/a.png",
            "data:image/png;base64,AAAA",
            "data:image/jpeg;base64,AAAA",
            "data:image/gif;base64,AAAA",
            "data:image/webp;base64,AAAA",
        ] {
            validate_a2ui(&envelope_with(json!({
                "id": "c", "component": "Image", "url": url })))
                .unwrap();
        }
    }

    #[test]
    fn image_without_a_string_url_rejects() {
        for bad in [json!(null), json!(7), json!(["a.png"])] {
            let err = validate_a2ui(&envelope_with(json!({
                "id": "c", "component": "Image", "url": bad })))
                .unwrap_err();
            assert!(err.contains("Image.url must be a string"), "{err}");
        }
        // Absent url is the same error.
        let err = validate_a2ui(&envelope_with(json!({ "id": "c", "component": "Image" })))
            .unwrap_err();
        assert!(err.contains("Image.url must be a string"), "{err}");
    }

    #[test]
    fn image_error_names_the_component_id() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "hero", "component": "Image", "url": "https://example.com/a.png" })))
            .unwrap_err();
        assert!(err.contains("hero"), "must name the node to fix: {err}");
    }

    #[test]
    fn components_without_an_image_url_are_untouched() {
        // Unknown component names stay the renderer's problem, as before.
        for component in [
            json!({ "id": "c", "component": "Text", "text": "hi" }),
            json!({ "id": "c", "component": "Future" }),
            json!({ "id": "c" }),
            json!("not an object"),
        ] {
            validate_a2ui(&envelope_with(component)).unwrap();
        }
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
