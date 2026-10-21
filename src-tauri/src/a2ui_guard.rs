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
//! Component props are then checked one type at a time in `check_component`:
//! `Image.url`, which rejects remote URLs and data URIs the webview cannot
//! render (see `check_image_url`), and `ChoicePicker.displayStyle`, which must
//! be one of the two styles the catalog can render.

use serde_json::Value;

/// Maximum total components across all updateComponents messages.
pub const MAX_COMPONENTS: usize = 200;
/// Maximum serialized size of the whole payload (256 KiB).
pub const MAX_BYTES: usize = 256 * 1024;
/// The only catalog the webview registers (src/a2ui/catalog.tsx).
pub const CATALOG_ID: &str = "cenno:catalog/v1";
/// Maximum `Table.columns` (a panel is ~320-420px wide; a 5th column would
/// force horizontal scroll in a surface that must not scroll).
pub const MAX_TABLE_COLUMNS: usize = 4;
/// Maximum `Table.rows` (a panel's height is clamped to 240-560px by
/// `resize_panel`).
pub const MAX_TABLE_ROWS: usize = 12;

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
    // Schemes are case-insensitive (RFC 3986) and agents do emit `DATA:` and
    // `HTTPS:`. Lowercase only the scheme token for classification — never the
    // payload, and never the local path.
    let trimmed = url.trim();
    let scheme_len = trimmed.find(':').filter(|i| *i > 0);
    let probe = match scheme_len {
        Some(i) => format!("{}{}", trimmed[..i].to_ascii_lowercase(), &trimmed[i..]),
        None => trimmed.to_string(),
    };

    if let Some(rest) = probe.strip_prefix("data:") {
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
    if probe.starts_with("https:") || probe.starts_with("http:") {
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
        Some("ChoicePicker") => {
            // `displayStyle` picks the rendering: chips (cenno's default and
            // every payload shipped before the list view existed) or the
            // radio/checkbox list. The upstream zod schema only admits those
            // two, and the webview silently drops a component it cannot parse
            // — so an unknown value would blank the options instead of telling
            // the agent. `displayStyle` is a plain enum, NOT a DynamicString: a
            // data-model binding ({"path": …}) is equally unparseable, so it is
            // rejected here too rather than left to blank the panel.
            let Some(style) = obj.get("displayStyle") else {
                return Ok(());
            };
            let label = obj.get("id").and_then(Value::as_str).map_or_else(
                || "ChoicePicker".to_string(),
                |id| format!("ChoicePicker \"{id}\""),
            );
            match style.as_str() {
                Some("chips" | "checkbox") => Ok(()),
                Some(value) => Err(format!(
                    "{label}.displayStyle must be \"chips\" or \"checkbox\", got \"{value}\" \
                     — omit it to keep chips"
                )),
                None => Err(format!(
                    "{label}.displayStyle must be \"chips\" or \"checkbox\", got {} \
                     — it takes a literal string, never a data-model binding; omit it \
                     to keep chips",
                    type_name(style)
                )),
            }
        }
        Some("Table") => {
            // Display-only tables are capped because a panel is ~320-420px
            // wide and its height is clamped to 240-560px (resize_panel): a
            // fifth column or a thirteenth row forces scrolling in a surface
            // that must not scroll.
            //
            // Row-vs-header ASYMMETRY, deliberate: a row LONGER than `columns`
            // has no meaning and is rejected; a row SHORTER than `columns` is
            // allowed, because TableView pads the missing cells with an em
            // dash (a ragged last row is a normal table shape, a bulging one
            // is a bug). Do not "fix" one half without the other.
            let label = obj
                .get("id")
                .and_then(Value::as_str)
                .map_or_else(|| "Table".to_string(), |id| format!("Table \"{id}\""));
            let columns = obj
                .get("columns")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    format!(
                        "{label}: Table.columns must be an array of at least 1 column \
                         object ({{ \"label\": … }}), got {}",
                        obj.get("columns").map_or("nothing", type_name)
                    )
                })?;
            if columns.is_empty() {
                return Err(format!(
                    "{label}: has 0 columns; a table needs at least 1 column — add a \
                     column object ({{ \"label\": … }}) or drop the component"
                ));
            }
            if columns.len() > MAX_TABLE_COLUMNS {
                return Err(format!(
                    "{label}: has {} columns; the maximum is {MAX_TABLE_COLUMNS} — split \
                     it into two tables, or drop a column",
                    columns.len()
                ));
            }
            for (i, column) in columns.iter().enumerate() {
                let Some(col) = column.as_object() else {
                    return Err(format!(
                        "{label}: column {i} must be an object with a \"label\", got {}",
                        type_name(column)
                    ));
                };
                match col.get("align") {
                    None => {}
                    Some(Value::String(value)) if value == "start" || value == "end" => {}
                    Some(other) => {
                        return Err(format!(
                            "{label}: column {i} has align {other}; the allowed values are \
                             \"start\" (default) and \"end\" for numbers — omit it or pick one"
                        ))
                    }
                }
            }

            let rows = obj.get("rows").and_then(Value::as_array).ok_or_else(|| {
                format!(
                    "{label}: Table.rows must be an array of row arrays, got {}",
                    obj.get("rows").map_or("nothing", type_name)
                )
            })?;
            if rows.len() > MAX_TABLE_ROWS {
                return Err(format!(
                    "{label}: has {} rows; the maximum is {MAX_TABLE_ROWS} — split it into \
                     two tables, or send the rest in a follow-up prompt",
                    rows.len()
                ));
            }
            for (r, row) in rows.iter().enumerate() {
                let cells = row.as_array().ok_or_else(|| {
                    format!(
                        "{label}: row {r} must be an array of cell strings, got {}",
                        type_name(row)
                    )
                })?;
                if cells.len() > columns.len() {
                    return Err(format!(
                        "{label}: row {r} has {} cells but the header has {} columns; a row \
                         must not be longer than the header — drop the extra cell(s) or add \
                         the column (a SHORTER row is fine: missing cells render as an em \
                         dash)",
                        cells.len(),
                        columns.len()
                    ));
                }
                for (c, cell) in cells.iter().enumerate() {
                    // Numbers are stringified on render (the spec allows
                    // `["Pro", 9]`), so a scalar is fine; an object would be a
                    // data-model binding the display-only table cannot resolve.
                    if matches!(cell, Value::Object(_) | Value::Array(_)) {
                        return Err(format!(
                            "{label}: row {r} cell {c} is {}; cells hold literal strings \
                             or numbers, not data-model bindings — inline the value",
                            type_name(cell)
                        ));
                    }
                }
            }
            Ok(())
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
    fn uppercase_scheme_is_classified_by_the_guard_not_left_to_load_image() {
        // RFC 3986: schemes are case-insensitive. `DATA:` must be judged here —
        // falling through as a "local path" would defer a valid data URI to
        // load_image, which answers "not an absolute path".
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "DATA:IMAGE/PNG;base64,AA" })))
            .unwrap_err();
        assert!(err.contains("data:image/"), "{err}");

        let remote = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "HTTPS://x/a.png" })))
            .unwrap_err();
        assert!(remote.contains("remote images are not allowed"), "{remote}");

        // A good data URI keeps working whatever the scheme case.
        validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "Data:Image/PNG;base64,AA" })))
            .unwrap_err();
        validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Image", "url": "data:image/png;base64,AA" })))
            .unwrap();
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

    // ---- ChoicePicker.displayStyle (Task 5) ----

    #[test]
    fn choice_picker_unknown_display_style_rejects() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "ChoicePicker", "displayStyle": "radio",
            "options": [{ "label": "a", "value": "a" }] })))
        .unwrap_err();
        assert!(err.contains("displayStyle"), "{err}");
        assert!(err.contains("chips") && err.contains("checkbox"), "{err}");
    }

    #[test]
    fn choice_picker_display_style_error_names_the_value_and_the_node() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "mood", "component": "ChoicePicker", "displayStyle": "CHECKBOX",
            "options": [{ "label": "a", "value": "a" }] })))
        .unwrap_err();
        // The offending value is quoted back so the agent can grep for it.
        assert!(err.contains("\"CHECKBOX\""), "must quote the value: {err}");
        assert!(err.contains("mood"), "must name the node: {err}");
    }

    #[test]
    fn choice_picker_non_string_display_style_rejects() {
        // `displayStyle` is a plain enum, not a DynamicString: a data-model
        // binding is as unparseable as a boolean, and both blank the panel.
        for bad in [
            json!(true),
            json!(1),
            json!(["chips"]),
            json!({ "path": "/style" }),
        ] {
            let err = validate_a2ui(&envelope_with(json!({
                "id": "c", "component": "ChoicePicker", "displayStyle": bad,
                "options": [{ "label": "a", "value": "a" }] })))
            .unwrap_err();
            assert!(
                err.contains("displayStyle"),
                "{bad} must be rejected: {err}"
            );
            assert!(err.contains("chips") && err.contains("checkbox"), "{err}");
        }
    }

    #[test]
    fn choice_picker_known_display_styles_and_omission_pass() {
        for style in ["chips", "checkbox"] {
            validate_a2ui(&envelope_with(json!({
                "id": "c", "component": "ChoicePicker", "displayStyle": style,
                "options": [{ "label": "a", "value": "a" }] })))
            .unwrap();
        }
        // Omitted displayStyle keeps chips — the shipped payload shape.
        validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "ChoicePicker",
            "options": [{ "label": "a", "value": "a" }] })))
        .unwrap();
    }

    // ---- Table caps (Task 6) ----

    #[test]
    fn table_with_five_columns_rejects_naming_the_cap() {
        let cols: Vec<Value> = (0..5)
            .map(|i| json!({ "label": format!("c{i}") }))
            .collect();
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table", "columns": cols, "rows": [] })))
        .unwrap_err();
        assert!(
            err.contains("5 columns") && err.contains("maximum is 4"),
            "{err}"
        );
    }

    #[test]
    fn table_row_longer_than_the_header_rejects() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table",
            "columns": [{ "label": "a" }, { "label": "b" }],
            "rows": [["1", "2", "3"]] })))
        .unwrap_err();
        assert!(err.contains("row 0") && err.contains("3 cells"), "{err}");
    }

    #[test]
    fn table_over_twelve_rows_rejects() {
        let rows: Vec<Value> = (0..13).map(|i| json!([format!("{i}")])).collect();
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table",
            "columns": [{ "label": "n" }], "rows": rows })))
        .unwrap_err();
        assert!(err.contains("maximum is 12"), "{err}");
    }

    #[test]
    fn table_row_long_error_names_the_row_and_the_header_width() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "plan", "component": "Table",
            "columns": [{ "label": "a" }, { "label": "b" }],
            "rows": [["1", "2"], ["1", "2", "3"]] })))
        .unwrap_err();
        // The SECOND row is the offender: naming the index is the fix.
        assert!(err.contains("row 1"), "{err}");
        assert!(err.contains("3 cells"), "{err}");
        assert!(err.contains("2 columns"), "{err}");
        assert!(err.contains("plan"), "must name the node: {err}");
    }

    #[test]
    fn table_at_the_caps_passes() {
        let cols: Vec<Value> = (0..4)
            .map(|i| json!({ "label": format!("c{i}") }))
            .collect();
        let rows: Vec<Value> = (0..12)
            .map(|i| json!([format!("{i}"), "b", "c", "d"]))
            .collect();
        validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table", "columns": cols, "rows": rows })))
        .unwrap();
    }

    #[test]
    fn table_short_row_passes_because_the_renderer_pads_it() {
        // Asymmetry by design: a row LONGER than the header has no meaning
        // and is rejected; a row SHORTER than the header renders the missing
        // cells as an em dash, so it is allowed.
        validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table",
            "columns": [{ "label": "a" }, { "label": "b" }],
            "rows": [["1"]] })))
        .unwrap();
    }

    #[test]
    fn table_align_outside_start_end_rejects() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table",
            "columns": [{ "label": "a", "align": "center" }],
            "rows": [] })))
        .unwrap_err();
        assert!(err.contains("align"), "{err}");
        assert!(err.contains("center"), "must quote the value: {err}");
        assert!(
            err.contains("start") && err.contains("end"),
            "must give the fix: {err}"
        );
    }

    #[test]
    fn table_with_no_columns_rejects() {
        let err = validate_a2ui(&envelope_with(json!({
            "id": "c", "component": "Table", "columns": [], "rows": [] })))
        .unwrap_err();
        assert!(err.contains("at least 1 column"), "{err}");
    }
}
