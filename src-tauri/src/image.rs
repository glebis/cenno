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
use std::io::Read;

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
/// `max_bytes` is clamped here, so no caller can re-open the DoS the clamp
/// exists to close.
pub fn load_image_bytes(path: &str, max_bytes: u64) -> Result<(String, Vec<u8>), String> {
    let max_bytes = max_bytes.clamp(MIN_CAP_BYTES, MAX_CAP_BYTES);
    let path = expand_tilde(path)?;

    if !std::path::Path::new(&path).is_absolute() {
        return Err(format!(
            "not an absolute path: {path} — cenno loads local images by absolute path only \
             (e.g. /Users/you/shot.png, or ~/shot.png)"
        ));
    }

    // 2. Existence + kind. `metadata` reads no content, so reporting the true
    //    first error (missing vs. wrong type) costs nothing on the read surface.
    let meta = std::fs::metadata(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            format!("file not found: {path} — the assistant sent a path that does not exist")
        }
        std::io::ErrorKind::PermissionDenied => format!(
            "not readable: {path} — cenno has no permission to read it; check the file's \
             permissions"
        ),
        _ => format!("could not stat: {path} — {e}"),
    })?;
    if !meta.is_file() {
        return Err(format!(
            "not a regular file: {path} — cenno loads single image files by absolute path"
        ));
    }

    // 3. Extension allowlist — before any read.
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let mime = match ext.as_deref() {
        Some(e) => mime_for_extension(e).ok_or_else(|| {
            format!(
                "not an image (extension \".{e}\"): {path} — cenno only loads \
                 {ALLOWED_EXTENSIONS}"
            )
        })?,
        None => {
            return Err(format!(
                "not an image (no extension): {path} — cenno only loads {ALLOWED_EXTENSIONS}"
            ))
        }
    };

    // 4. Size — still before any read.
    let size = meta.len();
    if size > max_bytes {
        return Err(format!(
            "too large ({} , max {}): {path} — raise max_bytes (up to {}) or downscale the image",
            human_size(size),
            human_size(max_bytes),
            human_size(MAX_CAP_BYTES)
        ));
    }

    // 5. Read, bounded by `max_bytes` structurally — a file that grows between
    //    the stat above and this read cannot push past the cap.
    let mut bytes = Vec::with_capacity(size as usize);
    std::fs::File::open(&path)
        .map_err(|e| format!("could not read: {path} — {e}"))?
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("could not read: {path} — {e}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "too large ({} read, max {}): {path} — the file grew while it was being read; \
             raise max_bytes or downscale the image",
            human_size(bytes.len() as u64),
            human_size(max_bytes)
        ));
    }

    // 6. The bytes must actually be the format the extension claims.
    if !magic_bytes_match(mime, &bytes) {
        return Err(format!(
            "contents are not a valid {}: {path} — the extension says {} but the first bytes \
             disagree; the file is mislabeled or corrupt",
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
/// `async` keeps the file read + base64 encode off the thread servicing the
/// panel webview, which is mid-slide-in when this fires.
#[tauri::command(async)]
pub fn load_image(path: String, max_bytes: Option<u64>) -> Result<String, String> {
    data_uri(&path, max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Per-test unique directory: two concurrent `cargo test` runs (or a stale
    /// file from a previous run) cannot interfere.
    fn test_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("cenno-image-tests-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_tmp(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let p = test_dir().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0];

    #[test]
    fn accepts_a_real_png_and_returns_a_data_uri() {
        let p = write_tmp("ok.png", PNG);
        let uri = data_uri(p.to_str().unwrap(), None).unwrap();
        let b64 = uri
            .strip_prefix("data:image/png;base64,")
            .expect("prefix");
        // The whole point of the command is faithful byte transport — decode
        // and compare, do not just trust the prefix.
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("valid base64");
        assert_eq!(decoded, PNG);
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
    fn rejects_a_directory_even_when_named_like_an_image() {
        // Named `.png` so it passes the extension gate and actually exercises
        // the `is_file()` guard that keeps `read` away from non-regular files.
        let dir = test_dir().join("looks-like.png");
        std::fs::create_dir_all(&dir).unwrap();
        let err = data_uri(dir.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not a regular file"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_fifo_named_like_an_image() {
        let dir = test_dir();
        let fifo = dir.join("pipe.png");
        let ok = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            eprintln!("mkfifo unavailable — skipping");
            return;
        }
        let err = data_uri(fifo.to_str().unwrap(), None).unwrap_err();
        assert!(err.contains("not a regular file"), "{err}");
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
    fn clamp_is_enforced_inside_the_read_not_only_in_data_uri() {
        // `load_image_bytes` is the shared entry point; a caller passing a huge
        // cap directly must still be bounded.
        let mut big = PNG.to_vec();
        big.resize(MAX_CAP_BYTES as usize + 1, 0);
        let p = write_tmp("huge.png", &big);
        let err = load_image_bytes(p.to_str().unwrap(), u64::MAX).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn magic_sniff_handles_short_and_boundary_inputs_without_panicking() {
        // A refactor to direct indexing must not introduce an OOB panic here.
        assert!(!magic_bytes_match("image/png", b""));
        assert!(!magic_bytes_match("image/webp", b""));
        assert!(!magic_bytes_match("image/webp", b"RIFF"));
        assert!(!magic_bytes_match("image/webp", b"RIFFabcdWEB")); // 11 bytes
        assert!(magic_bytes_match("image/webp", b"RIFFabcdWEBP")); // 12 bytes
        assert!(!magic_bytes_match("image/gif", b"GIF"));
        assert!(magic_bytes_match("image/gif", b"GIF89a"));
        assert!(!magic_bytes_match("image/jpeg", b"\xff\xd8"));
        assert!(magic_bytes_match("image/jpeg", b"\xff\xd8\xff\xe0"));
    }

    #[test]
    fn expands_a_tilde_path() {
        let home = std::env::var("HOME").expect("HOME");
        if home.is_empty() || !std::path::Path::new(&home).is_absolute() {
            eprintln!("HOME unusable — skipping");
            return;
        }
        // Write a real PNG somewhere under $HOME and address it as ~/…
        let rel = format!("cenno-image-test-{}.png", std::process::id());
        let abs = std::path::Path::new(&home).join(&rel);
        std::fs::write(&abs, PNG).unwrap();
        let tilde = format!("~/{rel}");
        let uri = data_uri(&tilde, None);
        let _ = std::fs::remove_file(&abs);
        let uri = uri.unwrap_or_else(|e| panic!("~ expansion failed: {e}"));
        assert!(uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn clamps_the_cap_to_the_documented_range() {
        // 0 becomes 1 KiB, 99 MiB becomes 4096 KiB
        assert_eq!(clamp_cap(None), 512 * 1024);
        assert_eq!(clamp_cap(Some(0)), 1024);
        assert_eq!(clamp_cap(Some(99 * 1024 * 1024)), 4096 * 1024);
    }
}
