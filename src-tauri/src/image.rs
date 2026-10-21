//! Bounded loader that turns ONE local image file into a `data:` URI.
//!
//! Agents may point a panel prompt at an image on disk (a screenshot, a chart
//! they just generated, a mockup). The release CSP is `img-src 'self' data:`,
//! so the webview can only render `data:` URIs — hence this command. It is a
//! deliberately small read surface, and the order below is the design:
//!
//! 1. expand a leading `~`, then require an absolute path — no relative
//!    paths, so nothing is resolved against an unexpected working directory;
//! 2. extension allowlist — checked *before* reading, so a `notes.txt` or a
//!    `.png`-less path is rejected without touching its contents;
//! 3. size cap — checked from metadata *before* reading, so a huge file
//!    never lands in memory;
//! 4. read;
//! 5. magic-byte sniff — the extension alone is not evidence that the bytes
//!    are really that image format.
//!
//! There is no network access anywhere here and no other read surface: one
//! absolute path, one bounded file.

use base64::Engine;

/// Cap used when the caller does not ask for one: 512 KiB.
const DEFAULT_CAP_BYTES: u64 = 512 * 1024;
/// Smallest cap we honour, so a caller cannot make the read surface trivial: 1 KiB.
const MIN_CAP_BYTES: u64 = 1024;
/// Largest cap we honour: 4 MiB. Bigger images belong in a downscaled copy.
const MAX_CAP_BYTES: u64 = 4096 * 1024;

/// Every extension we accept, for error messages.
const ALLOWED_EXTENSIONS: &str = "png, jpg, jpeg, gif or webp";

/// Map a lowercased file extension to the MIME type we advertise for it.
///
/// Private to this module but kept as its own function because it is the one
/// piece of validation later code (the TS helper's payload checks) mirrors.
fn mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// Human label for a sniff failure, e.g. `PNG` in `not a valid PNG: …`.
fn type_label(mime: &str) -> &'static str {
    match mime {
        "image/png" => "PNG",
        "image/jpeg" => "JPEG",
        "image/gif" => "GIF",
        "image/webp" => "WEBP",
        _ => "image",
    }
}

/// Does `bytes` start with the signature that `mime` promises?
fn magic_bytes_match(mime: &str, bytes: &[u8]) -> bool {
    match mime {
        "image/png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "image/gif" => bytes.starts_with(b"GIF8"),
        "image/webp" => {
            bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
        }
        _ => false,
    }
}

/// `None` → 512 KiB; `Some(n)` → clamped into `1 KiB ..= 4 MiB`.
pub fn clamp_cap(requested: Option<u64>) -> u64 {
    match requested {
        None => DEFAULT_CAP_BYTES,
        Some(n) => n.clamp(MIN_CAP_BYTES, MAX_CAP_BYTES),
    }
}

/// One decimal, and the smaller of KB/MB so the number stays readable.
fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b < MB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / MB)
    }
}

/// Expand a leading `~` to `$HOME` so `~/shot.png` works before the
/// absolute-path check.
fn expand_tilde(path: &str) -> Result<String, String> {
    if path == "~" || path.starts_with("~/") {
        let home = std::env::var("HOME")
            .map_err(|_| "cannot expand ~: HOME is not set — use an absolute path".to_string())?;
        Ok(format!("{home}{}", &path[1..]))
    } else {
        Ok(path.to_string())
    }
}

/// Read one local image and return `(mime, bytes)`.
///
/// Deliberately takes no `AppHandle` so tests run without a Tauri runtime.
/// `max_bytes` is expected to be pre-clamped by [`clamp_cap`] (or be a
/// deliberate internal limit); it is the hard ceiling for this read.
pub fn load_image_bytes(path: &str, max_bytes: u64) -> Result<(String, Vec<u8>), String> {
    let path = expand_tilde(path)?;

    if !std::path::Path::new(&path).is_absolute() {
        return Err(format!(
            "not an absolute path: {path}; cenno loads local images by absolute path only \
             (e.g. /Users/you/shot.png, or ~/shot.png)"
        ));
    }

    // 2. Extension allowlist — before any read.
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let mime = match ext.as_deref() {
        Some(e) => mime_for_extension(e).ok_or_else(|| {
            format!(
                "not an image: {path} ends in .{e}; cenno only loads {ALLOWED_EXTENSIONS} — \
                 convert the file or point at an image"
            )
        })?,
        None => {
            return Err(format!(
                "not an image: {path} has no file extension; cenno only loads \
                 {ALLOWED_EXTENSIONS} — point at an image file"
            ))
        }
    };

    // 3. Existence + size — still before any read.
    let meta = std::fs::metadata(&path).map_err(|_| {
        format!("file not found: {path} does not exist — check the path the agent supplied")
    })?;
    if !meta.is_file() {
        return Err(format!(
            "not a regular file: {path} is not a readable file; cenno loads single image files \
             by absolute path"
        ));
    }
    let size = meta.len();
    if size > max_bytes {
        return Err(format!(
            "too large: {path} is {} but the cap is {} — raise max_bytes (up to {}) or \
             downscale the image",
            human_size(size),
            human_size(max_bytes),
            human_size(MAX_CAP_BYTES)
        ));
    }

    // 4. Read, bounded by the cap we already checked.
    let bytes = std::fs::read(&path).map_err(|e| {
        format!("could not read {path}: {e} — check that the file is readable by cenno")
    })?;

    // 5. The bytes must actually be the format the extension claims.
    if !magic_bytes_match(mime, &bytes) {
        return Err(format!(
            "not a valid {}: {path} has a matching extension but its first bytes are not a {} \
             signature — the file is mislabeled or corrupt; re-export it",
            type_label(mime),
            type_label(mime)
        ));
    }

    Ok((mime.to_string(), bytes))
}

/// Read one local image and return a `data:<mime>;base64,<…>` URI the webview
/// can render under `img-src 'self' data:`.
pub fn data_uri(path: &str, max_bytes: Option<u64>) -> Result<String, String> {
    let (mime, bytes) = load_image_bytes(path, clamp_cap(max_bytes))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

/// Tauri command: read ONE local image file and return it as a `data:` URI.
///
/// `max_bytes` is optional and clamped to 1 KiB..=4 MiB (default 512 KiB).
#[tauri::command]
pub fn load_image(path: String, max_bytes: Option<u64>) -> Result<String, String> {
    data_uri(&path, max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("cenno-image-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0];

    #[test]
    fn accepts_a_real_png_and_returns_a_data_uri() {
        let p = write_tmp("ok.png", PNG);
        let uri = data_uri(p.to_str().unwrap(), None).unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn missing_file_names_the_reason() {
        let err = data_uri("/definitely/not/here.png", None).unwrap_err();
        assert!(err.contains("file not found"), "{err}");
    }

    #[test]
    fn rejects_a_non_image_extension_before_reading() {
        let p = write_tmp("notes.txt", b"hello");
        let err = data_uri(p.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not an image"), "{err}");
        assert!(err.contains(".txt"), "{err}");
    }

    #[test]
    fn rejects_a_directory() {
        let dir = std::env::temp_dir().join("cenno-image-tests");
        let err = data_uri(dir.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not an image") || err.contains("not a regular file"), "{err}");
    }

    #[test]
    fn rejects_a_relative_path() {
        let err = data_uri("relative/a.png", None).unwrap_err();
        assert!(err.contains("absolute"), "{err}");
    }

    #[test]
    fn rejects_an_oversized_file_naming_both_sizes() {
        let mut big = PNG.to_vec();
        big.resize(2048, 0);
        let p = write_tmp("big.png", &big);
        let err = data_uri(p.to_str().unwrap(), Some(1024)).unwrap_err();
        assert!(err.contains("too large"), "{err}");
        assert!(err.contains("2.0 KB") && err.contains("1.0 KB"), "{err}");
    }

    #[test]
    fn rejects_bytes_that_disagree_with_the_extension() {
        let p = write_tmp("liar.png", b"not a png at all");
        let err = data_uri(p.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not a valid PNG"), "{err}");
    }

    #[test]
    fn clamps_the_cap_to_the_documented_range() {
        // 0 becomes 1 KiB, 99 MiB becomes 4096 KiB
        assert_eq!(clamp_cap(None), 512 * 1024);
        assert_eq!(clamp_cap(Some(0)), 1024);
        assert_eq!(clamp_cap(Some(99 * 1024 * 1024)), 4096 * 1024);
    }
}
