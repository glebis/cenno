//! Bounded loader that turns ONE agent-supplied audio reference — a local file
//! or a `data:audio/…;base64,…` URI — into a `data:` URI the panel's player
//! decodes.
//!
//! Same shape and same order of checks as `image.rs`, and for the same reason:
//! prompts are untrusted input. A path is not evidence of a file, an extension
//! is not evidence of a format, and a `data:` URI is not evidence of audio.
//!
//! 1. `~` expansion, then an absolute-path requirement — nothing resolves
//!    against an unexpected working directory;
//! 2. extension allowlist — before reading, so a `.txt` costs no I/O;
//! 3. size cap from metadata — before reading, so a huge file never lands in
//!    memory;
//! 4. read, bounded structurally (`.take(cap + 1)`);
//! 5. magic-byte sniff — the bytes must actually be the format claimed.
//!
//! Data URIs skip 1–4 but pay the same size cap (after decode) and the same
//! magic-byte sniff.
//!
//! There is no network access anywhere here.

use base64::Engine;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::protocol::AudioSource;

/// Hard cap on decoded audio: 5 MiB — roughly 20 minutes of speech at 128 kbps.
pub const MAX_AUDIO_BYTES: usize = 5 * 1024 * 1024;

/// Every extension we accept, for error messages.
const ALLOWED_EXTENSIONS: &str = "mp3, wav, m4a or ogg";

/// Why an audio reference was refused.
///
/// `Oversize` is the one recoverable variant: the MCP layer turns it into a
/// panel notice instead of a tool error, so the user still gets the question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    NotFound,
    NotAFile,
    NotAbsolute,
    BadExtension { ext: Option<String> },
    Oversize { bytes: usize, max: usize },
    MagicMismatch,
    BadDataUri,
    ReadFailed(String),
}

impl AudioError {
    /// The `InvalidParams`-facing message: a full, actionable sentence.
    pub fn message(&self, subject: &str) -> String {
        match self {
            AudioError::NotFound => format!(
                "file not found: {subject} — the assistant sent a path that does not exist"
            ),
            AudioError::NotAFile => format!(
                "not a regular file: {subject} — cenno loads single audio files"
            ),
            AudioError::NotAbsolute => format!(
                "not an absolute path: {subject} — cenno loads local audio by absolute path \
                 only (e.g. /Users/you/clip.mp3, or ~/clip.mp3)"
            ),
            AudioError::BadExtension { ext: Some(e) } => format!(
                "not audio (extension \".{e}\"): {subject} — cenno only loads {ALLOWED_EXTENSIONS}"
            ),
            AudioError::BadExtension { ext: None } => format!(
                "not audio (no extension): {subject} — cenno only loads {ALLOWED_EXTENSIONS}"
            ),
            AudioError::Oversize { bytes, max } => format!(
                "audio too large ({} , max {}): {subject}",
                human_size(*bytes),
                human_size(*max)
            ),
            AudioError::MagicMismatch => format!(
                "contents are not a valid audio file: {subject} — the extension (or data: URI \
                 MIME type) says one format but the first bytes disagree; the file is \
                 mislabeled or corrupt"
            ),
            AudioError::BadDataUri => format!(
                "malformed audio data: URI: {subject} — expected \
                 data:audio/<mpeg|mp3|wav|x-wav|mp4|m4a|ogg>;base64,…"
            ),
            AudioError::ReadFailed(e) => format!("could not read: {subject} — {e}"),
        }
    }

    pub fn is_oversize(&self) -> bool {
        matches!(self, AudioError::Oversize { .. })
    }

    pub fn oversize_bytes(&self) -> Option<usize> {
        match self {
            AudioError::Oversize { bytes, .. } => Some(*bytes),
            _ => None,
        }
    }
}

/// A validated audio payload ready for the webview.
#[derive(Debug, Clone)]
pub struct LoadedAudio {
    /// Always a `data:` URI with a sniffed MIME type — the frontend never
    /// resolves a path itself.
    pub data_uri: String,
    pub mime: &'static str,
    /// Decoded byte length.
    pub bytes: usize,
    /// `Some` when the source was a file (used by "Show in Finder").
    pub source_path: Option<PathBuf>,
}

/// One decimal, and the smaller of KB/MB so the number stays readable.
fn human_size(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b < MB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{:.1} MB", b / MB)
    }
}

fn mime_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "mp3" => Some("audio/mpeg"),
        "wav" => Some("audio/wav"),
        "m4a" => Some("audio/mp4"),
        "ogg" => Some("audio/ogg"),
        _ => None,
    }
}

/// Does `bytes` carry the signature that `mime` promises?
///
/// Deliberately tolerant on MP3: an ID3v2 tag, or any MPEG frame sync
/// (`0xFF` + top three bits set). Files exported by encoders disagree on the
/// exact frame header, and rejecting a real MP3 is worse than accepting a
/// mislabeled one that the decoder will reject a moment later.
fn magic_bytes_match(mime: &str, bytes: &[u8]) -> bool {
    match mime {
        "audio/mpeg" => {
            bytes.starts_with(b"ID3")
                || (bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0)
        }
        "audio/wav" => bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "audio/mp4" => {
            bytes.len() >= 12
                && &bytes[4..8] == b"ftyp"
                && matches!(
                    &bytes[8..12],
                    b"M4A " | b"M4B " | b"mp42" | b"mp41" | b"isom" | b"iso2"
                )
        }
        "audio/ogg" => bytes.starts_with(b"OggS"),
        _ => false,
    }
}

/// Expand a leading `~` to `$HOME`.
fn expand_tilde(path: &str) -> Result<String, AudioError> {
    if path == "~" || path.starts_with("~/") {
        let home = std::env::var("HOME").map_err(|_| AudioError::NotAbsolute)?;
        Ok(format!("{home}{}", &path[1..]))
    } else {
        Ok(path.to_string())
    }
}

/// Read one local audio file and return its validated `LoadedAudio`.
///
/// Deliberately takes no `AppHandle` so tests run without a Tauri runtime.
pub fn load_audio_file(path: &str) -> Result<LoadedAudio, AudioError> {
    let path = expand_tilde(path)?;
    if !Path::new(&path).is_absolute() {
        return Err(AudioError::NotAbsolute);
    }

    let meta = std::fs::metadata(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AudioError::NotFound,
        _ => AudioError::ReadFailed(e.to_string()),
    })?;
    if !meta.is_file() {
        return Err(AudioError::NotAFile);
    }

    let ext = Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    let mime = match ext.as_deref() {
        Some(e) => mime_for_extension(e).ok_or_else(|| AudioError::BadExtension {
            ext: Some(e.to_string()),
        })?,
        None => return Err(AudioError::BadExtension { ext: None }),
    };

    let size = meta.len();
    if size > MAX_AUDIO_BYTES as u64 {
        return Err(AudioError::Oversize {
            bytes: size as usize,
            max: MAX_AUDIO_BYTES,
        });
    }

    let mut bytes = Vec::with_capacity(size as usize);
    std::fs::File::open(&path)
        .map_err(|e| AudioError::ReadFailed(e.to_string()))?
        .take(MAX_AUDIO_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| AudioError::ReadFailed(e.to_string()))?;
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err(AudioError::Oversize {
            bytes: bytes.len(),
            max: MAX_AUDIO_BYTES,
        });
    }

    if !magic_bytes_match(mime, &bytes) {
        return Err(AudioError::MagicMismatch);
    }

    Ok(LoadedAudio {
        data_uri: encode_data_uri(mime, &bytes),
        mime,
        bytes: bytes.len(),
        source_path: Some(PathBuf::from(&path)),
    })
}

/// Decode a `data:audio/…;base64,…` URI and validate the payload.
pub fn load_audio_uri(uri: &str) -> Result<LoadedAudio, AudioError> {
    let trimmed = uri.trim();
    // The scheme is case-insensitive per RFC 2397; compare on the lowercased
    // head rather than requiring a literal lowercase `data:`.
    if trimmed.len() < 5 || !trimmed[..5].eq_ignore_ascii_case("data:") {
        return Err(AudioError::BadDataUri);
    }
    let rest = &trimmed[5..];
    let (meta, payload) = rest.split_once(',').ok_or(AudioError::BadDataUri)?;
    let mut parts = meta.split(';');
    let declared = parts.next().unwrap_or_default().to_ascii_lowercase();
    let base64 = parts.any(|p| p.eq_ignore_ascii_case("base64"));
    if !base64 {
        return Err(AudioError::BadDataUri);
    }
    let declared_mime = match declared.as_str() {
        "audio/mpeg" | "audio/mp3" => "audio/mpeg",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "audio/wav",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "audio/mp4",
        "audio/ogg" => "audio/ogg",
        _ => return Err(AudioError::BadDataUri),
    };
    // Cheap pre-check so an obviously huge payload is refused before we hand a
    // multi-megabyte string to the decoder. base64 is 4 chars per 3 bytes.
    if payload.len() / 4 * 3 > MAX_AUDIO_BYTES + 3 {
        return Err(AudioError::Oversize {
            bytes: payload.len() / 4 * 3,
            max: MAX_AUDIO_BYTES,
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|_| AudioError::BadDataUri)?;
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err(AudioError::Oversize {
            bytes: bytes.len(),
            max: MAX_AUDIO_BYTES,
        });
    }
    // The declared MIME and the bytes must agree — same rule as a file's
    // extension and its magic bytes.
    if !magic_bytes_match(declared_mime, &bytes) {
        return Err(AudioError::MagicMismatch);
    }
    Ok(LoadedAudio {
        data_uri: encode_data_uri(declared_mime, &bytes),
        mime: declared_mime,
        bytes: bytes.len(),
        source_path: None,
    })
}

fn encode_data_uri(mime: &str, bytes: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:{mime};base64,{encoded}")
}

/// Does this reference look like a data URI rather than a path?
pub fn is_data_uri(value: &str) -> bool {
    let lower = value.trim_start().to_ascii_lowercase();
    lower.starts_with("data:")
}

/// Load whichever kind of reference the agent supplied: a `data:` URI is
/// decoded, anything else is read from disk.
pub fn load_audio(src: &AudioSource) -> Result<LoadedAudio, AudioError> {
    if src.is_uri() {
        load_audio_uri(src.as_str())
    } else {
        load_audio_file(src.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const WAV: &[u8] = b"RIFF\x24\x00\x00\x00WAVEfmt ";
    const MP3: &[u8] = b"ID3\x03\x00\x00\x00";
    const OGG: &[u8] = b"OggS\x00\x02\x00\x00";
    const M4A: &[u8] = b"\x00\x00\x00\x18ftypM4A \x00\x00\x00\x00";

    fn test_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("cenno-audio-tests-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_tmp(name: &str, bytes: &[u8]) -> PathBuf {
        let p = test_dir().join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn accepts_each_supported_file_format_and_sniffs_its_mime() {
        for (name, bytes, mime) in [
            ("clip.mp3", MP3, "audio/mpeg"),
            ("clip.wav", WAV, "audio/wav"),
            ("clip.m4a", M4A, "audio/mp4"),
            ("clip.ogg", OGG, "audio/ogg"),
        ] {
            let p = write_tmp(name, bytes);
            let loaded = load_audio_file(p.to_str().unwrap()).unwrap();
            assert_eq!(loaded.mime, mime);
            assert_eq!(loaded.bytes, bytes.len());
            assert_eq!(loaded.source_path.as_deref(), Some(p.as_path()));
            let b64_payload = loaded
                .data_uri
                .strip_prefix(&format!("data:{mime};base64,"))
                .unwrap_or_else(|| panic!("bad prefix: {}", loaded.data_uri));
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(b64_payload)
                .expect("valid base64");
            assert_eq!(decoded, bytes, "transport must be byte-faithful");
        }
    }

    #[test]
    fn accepts_each_supported_data_uri_format() {
        for (declared, bytes, mime) in [
            ("audio/mpeg", MP3, "audio/mpeg"),
            ("audio/mp3", MP3, "audio/mpeg"),
            ("audio/wav", WAV, "audio/wav"),
            ("audio/x-wav", WAV, "audio/wav"),
            ("audio/mp4", M4A, "audio/mp4"),
            ("audio/m4a", M4A, "audio/mp4"),
            ("audio/ogg", OGG, "audio/ogg"),
        ] {
            let uri = format!("data:{declared};base64,{}", b64(bytes));
            let loaded = load_audio_uri(&uri).unwrap();
            assert_eq!(loaded.mime, mime, "{declared}");
            assert!(loaded.source_path.is_none());
        }
    }

    #[test]
    fn missing_file_names_the_reason() {
        let err = load_audio_file("/definitely/not/here.mp3").unwrap_err();
        assert_eq!(err, AudioError::NotFound);
        assert!(err.message("/definitely/not/here.mp3").contains("file not found"));
    }

    #[test]
    fn rejects_a_relative_path() {
        assert_eq!(
            load_audio_file("relative/clip.mp3").unwrap_err(),
            AudioError::NotAbsolute
        );
    }

    #[test]
    fn rejects_a_non_audio_extension_before_reading() {
        let p = write_tmp("notes.txt", b"hello");
        let err = load_audio_file(p.to_str().unwrap()).unwrap_err();
        assert_eq!(
            err,
            AudioError::BadExtension {
                ext: Some("txt".into())
            }
        );
        assert!(err.message(p.to_str().unwrap()).contains(".txt"));
    }

    #[test]
    fn rejects_a_directory_even_when_named_like_audio() {
        let dir = test_dir().join("looks-like.mp3");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(
            load_audio_file(dir.to_str().unwrap()).unwrap_err(),
            AudioError::NotAFile
        );
    }

    #[test]
    fn rejects_an_oversized_file_naming_both_sizes() {
        let mut big = WAV.to_vec();
        big.resize(MAX_AUDIO_BYTES + 10, 0);
        let p = write_tmp("big.wav", &big);
        let err = load_audio_file(p.to_str().unwrap()).unwrap_err();
        assert!(err.is_oversize(), "{err:?}");
        assert_eq!(err.oversize_bytes(), Some(MAX_AUDIO_BYTES + 10));
        let msg = err.message(p.to_str().unwrap());
        assert!(msg.contains("5.0 MB"), "{msg}");
    }

    #[test]
    fn rejects_bytes_that_disagree_with_the_extension() {
        let p = write_tmp("liar.mp3", b"this is a text file, not a frame sync");
        assert_eq!(
            load_audio_file(p.to_str().unwrap()).unwrap_err(),
            AudioError::MagicMismatch
        );
    }

    #[test]
    fn rejects_a_data_uri_whose_declared_mime_lies() {
        let uri = format!("data:audio/wav;base64,{}", b64(MP3));
        assert_eq!(load_audio_uri(&uri).unwrap_err(), AudioError::MagicMismatch);
    }

    #[test]
    fn rejects_malformed_data_uris() {
        for bad in [
            "data:audio/mpeg,abc",                       // not base64
            "data:text/plain;base64,aGVsbG8=",           // wrong MIME family
            "data:audio/mpeg;base64",                    // no comma
            "notadatauri",                               // no scheme
            "data:audio/mpeg;base64,!!!!",               // invalid base64
        ] {
            assert_eq!(
                load_audio_uri(bad).unwrap_err(),
                AudioError::BadDataUri,
                "{bad}"
            );
        }
    }

    #[test]
    fn rejects_an_oversized_data_uri_without_decoding_it() {
        // A 6 MiB payload of base64-encoded zeros.
        let payload = "A".repeat(MAX_AUDIO_BYTES / 3 * 4 + 8);
        let uri = format!("data:audio/wav;base64,{payload}");
        let err = load_audio_uri(&uri).unwrap_err();
        assert!(err.is_oversize(), "{err:?}");
    }

    #[test]
    fn magic_sniff_handles_short_and_boundary_inputs_without_panicking() {
        assert!(!magic_bytes_match("audio/mpeg", b""));
        assert!(!magic_bytes_match("audio/mpeg", b"\xff"));
        assert!(magic_bytes_match("audio/mpeg", b"\xff\xfb"));
        assert!(!magic_bytes_match("audio/wav", b"RIFF"));
        assert!(!magic_bytes_match("audio/wav", b"RIFFabcdWAV"));
        assert!(magic_bytes_match("audio/wav", b"RIFFabcdWAVE"));
        assert!(!magic_bytes_match("audio/mp4", b"\x00\x00\x00\x18ftyp"));
        assert!(!magic_bytes_match("audio/mp4", b"\x00\x00\x00\x18ftypXXXX"));
        assert!(magic_bytes_match("audio/mp4", b"\x00\x00\x00\x18ftypisom"));
        assert!(!magic_bytes_match("audio/ogg", b"Ogg"));
        assert!(magic_bytes_match("audio/ogg", b"OggS"));
        assert!(!magic_bytes_match("audio/unknown", b"OggS"));
    }

    #[test]
    fn data_uri_detection_is_case_insensitive_about_the_scheme() {
        let src = AudioSource(format!("DATA:audio/ogg;base64,{}", b64(OGG)));
        let loaded = load_audio(&src).unwrap();
        assert_eq!(loaded.mime, "audio/ogg");
    }

    #[test]
    fn file_source_routes_to_the_file_loader() {
        let p = write_tmp("routed.wav", WAV);
        let loaded = load_audio(&AudioSource(p.to_str().unwrap().into())).unwrap();
        assert_eq!(loaded.mime, "audio/wav");
    }

    /// The wire shape agents actually send: one plain JSON string, either a
    /// data URI or a path. Both must survive a round trip through AskRequest.
    #[test]
    fn ask_request_audio_accepts_a_bare_string_either_way() {
        let req: crate::protocol::AskRequest =
            serde_json::from_str(r#"{"title":"t","audio":"data:audio/wav;base64,QUJD"}"#).unwrap();
        let src = req.audio.expect("audio present");
        assert!(src.is_uri(), "a data: string is a URI, got {src:?}");

        let req: crate::protocol::AskRequest =
            serde_json::from_str(r#"{"title":"t","audio":"/tmp/clip.mp3"}"#).unwrap();
        let src = req.audio.expect("audio present");
        assert!(!src.is_uri(), "a path is not a URI, got {src:?}");
        assert_eq!(src.as_str(), "/tmp/clip.mp3");
    }
}
