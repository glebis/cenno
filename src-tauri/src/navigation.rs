//! Rust-owned external navigation boundaries.
//!
//! The webview cannot choose a Personal Contracts origin. An owner must pin
//! one exact origin with the CLI ceremony, after which the webview may provide
//! only an optional bounded authorization identifier to focus. Ordinary
//! Markdown links and fixed About destinations use separate commands and do
//! not read or modify this trust state.

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri_plugin_opener::OpenerExt as _;
use url::{Host, Url};

const TRUST_DIRECTORY: &str = "trust";
const TRUST_FILE: &str = "personal-contracts-review-origin.json";
const TRUST_SCHEMA_VERSION: u8 = 1;
const MAX_TRUST_BYTES: u64 = 4 * 1024;
const MAX_FOCUS_ID_BYTES: usize = 160;
const MAX_EXTERNAL_URL_BYTES: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PersonalContractsOriginTrust {
    schema_version: u8,
    origin: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedExternalLink {
    CennoRepository,
    AuthorGithub,
    AuthorLinkedin,
}

impl FixedExternalLink {
    fn url(self) -> &'static str {
        match self {
            Self::CennoRepository => "https://github.com/glebis/cenno",
            Self::AuthorGithub => "https://github.com/glebis",
            Self::AuthorLinkedin => "https://www.linkedin.com/in/glebkalinin/",
        }
    }
}

fn trust_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "home directory is unavailable".to_owned())?;
    Ok(home.join(".cenno").join(TRUST_DIRECTORY).join(TRUST_FILE))
}

fn expected_owner() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot mutate process state.
    unsafe { libc::geteuid() }
}

fn validate_owner_only_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "Personal Contracts navigation trust is unavailable".to_owned())?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Personal Contracts navigation trust is unavailable".to_owned());
    }
    if metadata.uid() != expected_owner() || metadata.permissions().mode() & 0o077 != 0 {
        return Err("Personal Contracts navigation trust is unavailable".to_owned());
    }
    Ok(())
}

fn ensure_owner_only_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => validate_owner_only_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|_| "could not create Personal Contracts navigation trust".to_owned())?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                .map_err(|_| "could not secure Personal Contracts navigation trust".to_owned())?;
            validate_owner_only_directory(path)
        }
        Err(_) => Err("Personal Contracts navigation trust is unavailable".to_owned()),
    }
}

fn validate_origin(input: &str, allow_loopback_http: bool) -> Result<String, String> {
    if input.is_empty() || input.len() > 2048 || input.trim() != input {
        return Err("origin must be one canonical URL origin".to_owned());
    }
    let parsed = Url::parse(input).map_err(|_| "origin must be a valid URL".to_owned())?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
    {
        return Err(
            "origin must not contain credentials, a query, fragment, or non-root path".to_owned(),
        );
    }
    let host = parsed
        .host()
        .ok_or_else(|| "origin must contain a host".to_owned())?;
    let loopback = match host {
        Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    };
    if parsed.scheme() != "https"
        && !(allow_loopback_http && parsed.scheme() == "http" && loopback)
    {
        return Err("origin must use HTTPS".to_owned());
    }
    let canonical = parsed.origin().ascii_serialization();
    if input != canonical {
        return Err(format!("origin must use canonical form {canonical}"));
    }
    Ok(canonical)
}

fn atomic_write_trust(path: &Path, trust: &PersonalContractsOriginTrust) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "invalid Personal Contracts navigation trust path".to_owned())?;
    ensure_owner_only_directory(directory)?;
    let payload = serde_json::to_vec_pretty(trust)
        .map_err(|_| "could not encode Personal Contracts navigation trust".to_owned())?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = directory.join(format!(".{TRUST_FILE}.{}.{}.tmp", std::process::id(), unique));
    let write_result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)
            .map_err(|_| "could not stage Personal Contracts navigation trust".to_owned())?;
        file.write_all(&payload)
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|_| "could not persist Personal Contracts navigation trust".to_owned())?;
        fs::rename(&temporary, path)
            .map_err(|_| "could not activate Personal Contracts navigation trust".to_owned())?;
        File::open(directory)
            .and_then(|directory_file| directory_file.sync_all())
            .map_err(|_| "could not persist Personal Contracts navigation trust directory".to_owned())?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn read_trust(path: &Path) -> Result<PersonalContractsOriginTrust, String> {
    let directory = path
        .parent()
        .ok_or_else(|| "Personal Contracts navigation trust is unavailable".to_owned())?;
    validate_owner_only_directory(directory)?;
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| {
            "Personal Contracts review origin is not configured; run the explicit trust ceremony"
                .to_owned()
        })?;
    let metadata = file
        .metadata()
        .map_err(|_| "Personal Contracts navigation trust is unavailable".to_owned())?;
    if !metadata.is_file()
        || metadata.uid() != expected_owner()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.len() > MAX_TRUST_BYTES
    {
        return Err("Personal Contracts navigation trust is unavailable".to_owned());
    }
    let mut payload = Vec::with_capacity(metadata.len() as usize);
    file.read_to_end(&mut payload)
        .map_err(|_| "Personal Contracts navigation trust is unavailable".to_owned())?;
    let trust: PersonalContractsOriginTrust = serde_json::from_slice(&payload)
        .map_err(|_| "Personal Contracts navigation trust is unavailable".to_owned())?;
    if trust.schema_version != TRUST_SCHEMA_VERSION {
        return Err("Personal Contracts navigation trust is unavailable".to_owned());
    }
    let canonical = validate_origin(&trust.origin, cfg!(debug_assertions))?;
    if canonical != trust.origin {
        return Err("Personal Contracts navigation trust is unavailable".to_owned());
    }
    Ok(trust)
}

/// Owner-operated CLI ceremony. Repeating the origin in two separately named
/// arguments makes accidental clipboard substitution visible at invocation.
pub fn provision_personal_contracts_origin(
    origin: &str,
    confirm_exact_origin: &str,
) -> Result<PathBuf, String> {
    if origin != confirm_exact_origin {
        return Err("origin and --confirm-exact-origin must match exactly".to_owned());
    }
    let canonical = validate_origin(origin, cfg!(debug_assertions))?;
    let path = trust_path()?;
    atomic_write_trust(
        &path,
        &PersonalContractsOriginTrust {
            schema_version: TRUST_SCHEMA_VERSION,
            origin: canonical,
        },
    )?;
    Ok(path)
}

fn validate_focus_id(focus_id: Option<&str>) -> Result<Option<&str>, String> {
    let Some(value) = focus_id else { return Ok(None) };
    if value.is_empty()
        || value.len() > MAX_FOCUS_ID_BYTES
        || !value.is_ascii()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err("focusId must be a bounded authorization identifier".to_owned());
    }
    Ok(Some(value))
}

fn personal_contracts_review_url(
    trust: &PersonalContractsOriginTrust,
    focus_id: Option<&str>,
) -> Result<String, String> {
    let focus_id = validate_focus_id(focus_id)?;
    let mut url = Url::parse(&format!("{}/contracts", trust.origin))
        .map_err(|_| "Personal Contracts navigation trust is unavailable".to_owned())?;
    if let Some(focus_id) = focus_id {
        url.query_pairs_mut().append_pair("focus", focus_id);
    }
    Ok(url.into())
}

fn sanitized_external_url(input: &str) -> Result<String, String> {
    if input.is_empty() || input.len() > MAX_EXTERNAL_URL_BYTES || input.trim() != input {
        return Err("external link is invalid".to_owned());
    }
    let mut url = Url::parse(input).map_err(|_| "external link is invalid".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host().is_none() {
        return Err("external link must use HTTP or HTTPS".to_owned());
    }
    url.set_username("")
        .map_err(|_| "external link is invalid".to_owned())?;
    url.set_password(None)
        .map_err(|_| "external link is invalid".to_owned())?;
    Ok(url.into())
}

#[tauri::command]
pub fn open_personal_contracts_review(
    app: tauri::AppHandle,
    focus_id: Option<String>,
) -> Result<(), String> {
    let trust = read_trust(&trust_path()?)?;
    let url = personal_contracts_review_url(&trust, focus_id.as_deref())?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "could not open Personal Contracts review queue".to_owned())
}

#[tauri::command]
pub fn open_fixed_external_link(
    app: tauri::AppHandle,
    destination: FixedExternalLink,
) -> Result<(), String> {
    app.opener()
        .open_url(destination.url(), None::<&str>)
        .map_err(|_| "could not open external link".to_owned())
}

#[tauri::command]
pub fn open_markdown_external_link(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let sanitized = sanitized_external_url(&url)?;
    app.opener()
        .open_url(sanitized, None::<&str>)
        .map_err(|_| "could not open external link".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn trust(origin: &str) -> PersonalContractsOriginTrust {
        PersonalContractsOriginTrust {
            schema_version: TRUST_SCHEMA_VERSION,
            origin: origin.to_owned(),
        }
    }

    #[test]
    fn production_origin_requires_canonical_https_root_without_credentials() {
        assert_eq!(
            validate_origin("https://contracts.example.test", false).unwrap(),
            "https://contracts.example.test"
        );
        for invalid in [
            "http://contracts.example.test",
            "https://user@contracts.example.test",
            "https://contracts.example.test/path",
            "https://contracts.example.test?query=yes",
            "https://contracts.example.test#fragment",
            "https://CONTRACTS.example.test",
            "https://contracts.example.test/",
        ] {
            assert!(validate_origin(invalid, false).is_err(), "accepted {invalid}");
        }
    }

    #[test]
    fn loopback_http_is_only_allowed_by_debug_or_test_policy() {
        assert_eq!(
            validate_origin("http://127.0.0.1:3000", true).unwrap(),
            "http://127.0.0.1:3000"
        );
        assert!(validate_origin("http://localhost:3000", false).is_err());
        assert!(validate_origin("http://example.test:3000", true).is_err());
    }

    #[test]
    fn review_path_and_focus_are_constructed_locally_and_encoded() {
        assert_eq!(
            personal_contracts_review_url(&trust("https://contracts.example.test"), None).unwrap(),
            "https://contracts.example.test/contracts"
        );
        assert_eq!(
            personal_contracts_review_url(
                &trust("https://contracts.example.test"),
                Some("authorization:scan_01")
            )
            .unwrap(),
            "https://contracts.example.test/contracts?focus=authorization%3Ascan_01"
        );
        for invalid in ["", "../admin", "x?next=https://evil.test", "☃"] {
            assert!(personal_contracts_review_url(
                &trust("https://contracts.example.test"),
                Some(invalid)
            )
            .is_err());
        }
    }

    #[test]
    fn markdown_links_allow_only_http_s_and_strip_credentials() {
        assert_eq!(
            sanitized_external_url("https://user:secret@example.test/path?q=1#part").unwrap(),
            "https://example.test/path?q=1#part"
        );
        assert!(sanitized_external_url("file:///etc/passwd").is_err());
        assert!(sanitized_external_url("mailto:person@example.test").is_err());
        assert!(sanitized_external_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn trust_file_is_owner_only_and_round_trips() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join("trust");
        let path = directory.join(TRUST_FILE);
        atomic_write_trust(&path, &trust("https://contracts.example.test")).unwrap();
        assert_eq!(
            fs::symlink_metadata(&directory)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::symlink_metadata(&path)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(read_trust(&path).unwrap(), trust("https://contracts.example.test"));
    }

    #[test]
    fn trust_reader_rejects_symlinks_and_permissive_files() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let directory = temp.path().join("trust");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let target = temp.path().join("target.json");
        fs::write(
            &target,
            serde_json::to_vec(&trust("https://contracts.example.test")).unwrap(),
        )
        .unwrap();
        let linked = directory.join(TRUST_FILE);
        symlink(&target, &linked).unwrap();
        assert!(read_trust(&linked).is_err());

        fs::remove_file(&linked).unwrap();
        fs::rename(&target, &linked).unwrap();
        fs::set_permissions(&linked, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(read_trust(&linked).is_err());
    }

    #[test]
    fn trust_writer_rejects_a_symlinked_directory() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let real = temp.path().join("real");
        let linked = temp.path().join("trust");
        fs::create_dir(&real).unwrap();
        symlink(&real, &linked).unwrap();
        assert!(atomic_write_trust(
            &linked.join(TRUST_FILE),
            &trust("https://contracts.example.test")
        )
        .is_err());
    }
}
