//! Encrypted-at-rest storage for secrets the user types into a `kind: "secret"`
//! panel.
//!
//! # Threat model
//!
//! The value in this module's care is a credential the user pastes into cenno so
//! an agent can use it *without the agent ever reading it*. That single sentence
//! drives every design choice here:
//!
//! - The value never enters an MCP tool result — the agent receives a
//!   [`crate::protocol::SecretReceipt`] (name, format, a non-reversible hint).
//! - The value never enters the history DB, the CloudKit relay, `localStorage`
//!   drafts, or the TTS text path.
//! - At rest it is sops-encrypted with the user's age key. sops is invoked as a
//!   subprocess rather than linked as a library: it is the tool the user
//!   already trusts and can decrypt by hand (`sops -d ~/.cenno/secrets/x.sops.json`),
//!   and it keeps cenno free of a hard dependency on a crypto stack.
//!
//! # What this module does NOT defend against
//!
//! Anything running as the user can read the value while it is in flight
//! (webview memory, the child `sops` process's stdin) and can read the age key
//! at `~/.config/sops/age/keys.txt`. Full-disk encryption and not running
//! hostile processes as yourself remain the real boundary. sops-encryption buys
//! protection for the value *at rest* — backups, Time Machine snapshots, sync
//! folders, a shared laptop, a `grep` through your home directory — which is
//! exactly the thing that made storing it in plaintext `cenno.db` unacceptable.
//!
//! # Plaintext hygiene
//!
//! The value is handed to `sops` over **stdin**, never as an argv element (argv
//! is world-readable via `ps`). sops's stdout is piped straight into a `0600`
//! temp file in the destination directory, which is then atomically renamed into
//! place — so no plaintext file is ever created, and a crash mid-write can never
//! leave a half-written (or plaintext) secret behind.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::SecretsConfig;

/// A stored secret's non-sensitive identity, safe to show in a list or return to
/// an agent. Never contains the value.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct SecretMeta {
    pub name: String,
    pub format: Option<String>,
    pub label: Option<String>,
    /// When the secret was last written, RFC3339 (from sops metadata, so it is
    /// the encryption time — set on every store).
    pub updated_at: Option<String>,
}

/// Reject names that could escape the secrets directory or confuse a shell.
///
/// The name is a filename (`<name>.sops.json`) AND an environment-variable-ish
/// token in `cenno secret run`, so the charset is deliberately narrow: lowercase
/// alphanumerics, `_`, `-`, `.` — but a leading `.` or `-` is refused so no name
/// can be a hidden file or be mistaken for an option.
pub fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("secret name must not be empty".into());
    }
    if name.len() > 64 {
        return Err(format!(
            "secret name {name:?} is {} chars; the limit is 64",
            name.len()
        ));
    }
    let first = name.chars().next().expect("non-empty checked above");
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(format!(
            "secret name {name:?} must start with a lowercase letter or digit"
        ));
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-' || *c == '.'))
    {
        return Err(format!(
            "secret name {name:?} contains {bad:?}; use lowercase letters, digits, '_', '-' or '.'"
        ));
    }
    Ok(())
}

/// A non-reversible hint the user (and the agent) can use to tell two keys apart:
/// `sk-proj-…7a2f`. Deliberately lossy — enough to confirm "yes, that is the key
/// I pasted", never enough to use.
pub fn hint(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    // Too short to split without revealing most of it: show only the tail.
    if chars.len() < 16 {
        let tail: String = chars[chars.len().saturating_sub(3)..].iter().collect();
        return format!("…{tail}");
    }
    let head: String = chars[..6].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

/// Where secrets live: `~/.cenno/secrets` unless configured otherwise.
pub fn secrets_dir(cfg: &SecretsConfig) -> Result<PathBuf, String> {
    if let Some(dir) = cfg.dir.as_deref().filter(|d| !d.trim().is_empty()) {
        return Ok(PathBuf::from(shellexpand_tilde(dir)));
    }
    crate::config::cenno_dir()
        .map(|d| d.join("secrets"))
        .ok_or_else(|| "could not resolve the home directory for ~/.cenno".to_string())
}

/// Expand a leading `~` (and `~/`) against `$HOME`. `~user` is NOT supported —
/// it would require passwd lookups and is not worth the surface.
fn shellexpand_tilde(p: &str) -> String {
    if p == "~" || p.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{}", home.display(), &p[1..]);
        }
    }
    p.to_string()
}

/// Path of `<name>.sops.json` inside `dir`. Name must already be validated.
fn secret_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.sops.json"))
}

/// The sops binary to run. Configurable for Homebrew-on-Apple-Silicon vs
/// system paths, but resolved from `PATH` by default.
fn sops_bin(cfg: &SecretsConfig) -> String {
    cfg.sops_binary
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("sops")
        .to_string()
}

/// Resolve the age recipient to encrypt to.
///
/// Order of preference, most explicit first:
/// 1. `secrets.age_recipient` from `~/.cenno/config.json`.
/// 2. `SOPS_AGE_RECIPIENTS` from the environment.
/// 3. A `# public key: age1…` comment line in `~/.config/sops/age/keys.txt`
///    (where `age-keygen` writes it).
///
/// A `.sops.yaml` in `~/.cenno` is a fourth path that needs no recipient at all:
/// when this returns `None` and no `--age` flag is passed, sops finds the rules
/// file itself. Callers must therefore treat `None` as "let sops decide", not as
/// a failure.
fn age_recipient(cfg: &SecretsConfig) -> Option<String> {
    if let Some(r) = cfg.age_recipient.as_deref().map(str::trim) {
        if !r.is_empty() {
            return Some(r.to_string());
        }
    }
    if let Ok(env) = std::env::var("SOPS_AGE_RECIPIENTS") {
        let trimmed = env.trim();
        if !trimmed.is_empty() {
            // SOPS_AGE_RECIPIENTS is a comma-separated list; encrypting to the
            // first is the sane single-recipient interpretation.
            return Some(trimmed.split(',').next().unwrap_or(trimmed).trim().to_string());
        }
    }
    let keys = dirs::home_dir()?.join(".config/sops/age/keys.txt");
    let text = std::fs::read_to_string(keys).ok()?;
    text.lines()
        .find_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix('#')?.trim();
            let pubkey = rest
                .strip_prefix("public key:")
                .or_else(|| rest.strip_prefix("public key :"))?
                .trim();
            pubkey.starts_with("age1").then(|| pubkey.to_string())
        })
}

/// Store (or replace) a secret, encrypting it with sops.
///
/// `label` and `format` are written as `<key>_unencrypted` fields so they stay
/// readable in the ciphertext file — that is what makes `list` cheap and lets a
/// human see what a file holds without their age key.
pub fn store(
    cfg: &SecretsConfig,
    name: &str,
    value: &str,
    format: Option<&str>,
    label: Option<&str>,
) -> Result<SecretMeta, String> {
    validate_name(name)?;
    if value.is_empty() {
        return Err("refusing to store an empty secret".into());
    }
    if let Some(f) = format {
        crate::secret_formats::find(f)
            .ok_or_else(|| format!("unknown secret format {f:?}"))?;
    }

    let dir = secrets_dir(cfg)?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("creating {}: {e}", dir.display()))?;

    // `_unencrypted` suffix → sops leaves these fields in the clear. Metadata
    // only; the value is the one field that must be encrypted.
    let mut doc = serde_json::Map::new();
    doc.insert("value".into(), serde_json::Value::String(value.to_string()));
    doc.insert(
        "name_unencrypted".into(),
        serde_json::Value::String(name.to_string()),
    );
    if let Some(f) = format {
        doc.insert(
            "format_unencrypted".into(),
            serde_json::Value::String(f.to_string()),
        );
    }
    if let Some(l) = label {
        doc.insert(
            "label_unencrypted".into(),
            serde_json::Value::String(l.to_string()),
        );
    }
    let plaintext = serde_json::to_vec(&doc)
        .map_err(|e| format!("serializing secret payload: {e}"))?;
    drop(doc);

    let ciphertext = encrypt(cfg, &plaintext, &dir)?;
    let dest = secret_path(&dir, name);
    write_private_atomic(&dest, &ciphertext)?;

    // Read the metadata back from what actually landed on disk, so the returned
    // meta describes the file rather than the intent.
    read_meta(name, &dest)
}

/// Encrypt `plaintext` with sops, returning the ciphertext bytes.
///
/// `dest_dir` is the child's working directory: sops discovers a `.sops.yaml`
/// relative to the file it operates on, and `/dev/stdin` has no directory, so
/// the cwd is what makes a rules file in `~/.cenno` (or `~/.cenno/secrets`)
/// discoverable.
///
/// When we resolve an explicit recipient, `--config /dev/null` suppresses that
/// discovery. Without it a `~/.sops.yaml` in an ancestor directory (a very
/// common setup — e.g. rules for `*.env` for other projects) is still loaded,
/// matches no rule for a `.sops.json` payload, and sops fails with
/// `error loading config: no matching creation rules found` *even though*
/// `--age` was passed. An explicit recipient is the more specific instruction,
/// so it wins. With no recipient the rules file is the only way to name a key,
/// so discovery stays on.
fn encrypt(cfg: &SecretsConfig, plaintext: &[u8], dest_dir: &Path) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new(sops_bin(cfg));
    cmd.args(["--encrypt", "--input-type", "json", "--output-type", "json"]);
    match age_recipient(cfg) {
        Some(recipient) => {
            cmd.arg("--config").arg("/dev/null");
            cmd.arg("--age").arg(recipient);
        }
        // No explicit recipient → let sops resolve it from .sops.yaml.
        None => {}
    }
    cmd.arg("/dev/stdin")
        .current_dir(dest_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        format!(
            "could not run {}: {e} — install sops (brew install sops) or set \
             secrets.sops_binary in ~/.cenno/config.json",
            sops_bin(cfg)
        )
    })?;

    // Write the plaintext to the child's stdin on this thread. The payload is
    // tiny (one credential), so a single write is enough; a broken pipe here
    // means sops already failed and its stderr carries the real reason.
    {
        let mut stdin = child.stdin.take().ok_or("sops stdin was not piped")?;
        stdin
            .write_all(plaintext)
            .map_err(|e| format!("writing secret to sops: {e}"))?;
    }

    let out = child
        .wait_with_output()
        .map_err(|e| format!("waiting for sops: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stderr = stderr.trim();
        // The one sops failure users actually hit is a rules file that does not
        // cover this path — say what to do instead of echoing sops verbatim.
        if stderr.contains("no matching creation rules") {
            return Err(format!(
                "sops could not encrypt the secret: {stderr} — a .sops.yaml in \
                 this directory or a parent has no rule for <name>.sops.json. \
                 Set secrets.age_recipient in ~/.cenno/config.json (or \
                 SOPS_AGE_RECIPIENTS) so cenno can name the key itself."
            ));
        }
        return Err(format!("sops could not encrypt the secret: {stderr}"));
    }
    if out.stdout.is_empty() {
        return Err("sops produced no output".into());
    }
    Ok(out.stdout)
}

/// Write `bytes` to `dest` via a `0600` temp file in the same directory plus an
/// atomic rename, so `dest` is either the old file or the complete new one.
fn write_private_atomic(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let dir = dest.parent().ok_or("secret path has no parent directory")?;
    // Unique-ish temp name: pid + nanoseconds. `create_new` makes a collision a
    // hard error rather than a silent overwrite of someone else's file.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".tmp-{}-{nanos}", std::process::id()));

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| format!("creating {}: {e}", tmp.display()))?;
    let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("writing {}: {e}", tmp.display()));
    }
    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("moving secret into {}: {e}", dest.display()));
    }
    Ok(())
}

/// Decrypt and return a secret's value.
///
/// This is the ONLY function that yields a plaintext value, and its callers are
/// restricted to the CLI (`cenno secret get` / `run`) — the Tauri command layer
/// never calls it, so no webview or agent-facing path can read a secret back.
pub fn get(cfg: &SecretsConfig, name: &str) -> Result<String, String> {
    validate_name(name)?;
    let dir = secrets_dir(cfg)?;
    let path = secret_path(&dir, name);
    if !path.exists() {
        return Err(format!(
            "no secret named {name:?} (looked for {})",
            path.display()
        ));
    }
    let doc = decrypt_doc(cfg, &path)?;
    match doc.get("value").and_then(|v| v.as_str()) {
        Some(v) if !v.is_empty() => Ok(v.to_string()),
        _ => Err(format!("{} has no \"value\" field", path.display())),
    }
}

/// Decrypt a file into a JSON object.
fn decrypt_doc(cfg: &SecretsConfig, path: &Path) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let mut child = Command::new(sops_bin(cfg))
        .args(["--decrypt", "--input-type", "json", "--output-type", "json"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "could not run {}: {e} — install sops (brew install sops)",
                sops_bin(cfg)
            )
        })?;
    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .ok_or("sops stdout was not piped")?
        .read_to_end(&mut stdout)
        .map_err(|e| format!("reading sops output: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("waiting for sops: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "sops could not decrypt {}: {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let parsed: serde_json::Value = serde_json::from_slice(&stdout)
        .map_err(|e| format!("decrypted {} is not valid JSON: {e}", path.display()))?;
    parsed
        .as_object()
        .cloned()
        .ok_or_else(|| format!("decrypted {} is not a JSON object", path.display()))
}

/// Read a secret file's plaintext metadata WITHOUT decrypting it.
fn read_meta(name: &str, path: &Path) -> Result<SecretMeta, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("reading {}: {e}", path.display()))?;
    let doc: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("{} is not valid JSON: {e}", path.display()))?;
    let updated_at = doc
        .get("sops")
        .and_then(|s| s.get("lastmodified"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let field = |key: &str| {
        doc.get(format!("{key}_unencrypted"))
            .and_then(|v| v.as_str())
            .map(str::to_string)
    };
    Ok(SecretMeta {
        name: field("name").unwrap_or_else(|| name.to_string()),
        format: field("format"),
        label: field("label"),
        updated_at,
    })
}

/// Every stored secret, alphabetically. Metadata comes from the plaintext
/// `_unencrypted` fields, so listing never needs the age key.
pub fn list(cfg: &SecretsConfig) -> Result<Vec<SecretMeta>, String> {
    let dir = secrets_dir(cfg)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("reading {}: {e}", dir.display())),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(name) = fname.strip_suffix(".sops.json") else { continue };
        // A file whose metadata is unreadable is skipped rather than failing the
        // whole listing: one corrupt secret must not hide the others.
        if let Ok(meta) = read_meta(name, &path) {
            out.push(meta);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Delete a secret. Returns whether a file was actually removed.
pub fn delete(cfg: &SecretsConfig, name: &str) -> Result<bool, String> {
    validate_name(name)?;
    let dir = secrets_dir(cfg)?;
    let path = secret_path(&dir, name);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("removing {}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(dir: &Path) -> SecretsConfig {
        SecretsConfig {
            dir: Some(dir.display().to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn names_reject_traversal_and_flags() {
        for good in ["openai", "openai_api_key", "a", "a-b.c_1", "k9"] {
            assert!(validate_name(good).is_ok(), "{good} should be valid");
        }
        for bad in [
            "", "OpenAI", "a/b", "../x", "-leading", ".hidden", "a b", "a$b", &"x".repeat(65),
        ] {
            assert!(validate_name(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn hint_reveals_ends_only() {
        let h = hint("sk-proj-abcdefghijklmnop1234");
        assert!(h.starts_with("sk-pro"), "{h}");
        assert!(h.ends_with("1234"), "{h}");
        assert!(!h.contains("abcdefgh"), "{h} must not leak the middle");
        assert_eq!(hint("short"), "…ort");
        assert_eq!(hint(""), "");
    }

    #[test]
    fn path_is_name_dot_sops_json() {
        let dir = Path::new("/tmp/x");
        assert_eq!(secret_path(dir, "k").display().to_string(), "/tmp/x/k.sops.json");
    }

    #[test]
    fn list_of_missing_dir_is_empty_not_an_error() {
        let tmp = std::env::temp_dir().join(format!("cenno-secrets-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(list(&cfg(&tmp)).unwrap(), Vec::new());
    }

    #[test]
    fn store_refuses_empty_and_unknown_format() {
        let tmp = std::env::temp_dir().join(format!("cenno-secrets-refuse-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let c = cfg(&tmp);
        assert!(store(&c, "k", "", None, None).is_err());
        assert!(store(&c, "k", "v", Some("no_such_format"), None).is_err());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// End-to-end against the real sops binary + the user's age key. Skipped
    /// (not failed) when either is unavailable, so `cargo test` stays green on
    /// a machine without the toolchain.
    #[test]
    fn roundtrip_through_sops_when_available() {
        let tmp = std::env::temp_dir().join(format!("cenno-secrets-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let c = cfg(&tmp);
        let probe = Command::new("sops").arg("--version").output();
        let have_sops = probe.map(|o| o.status.success()).unwrap_or(false);
        if !have_sops {
            eprintln!("skipping: sops not installed");
            return;
        }
        if age_recipient(&c).is_none() && !tmp.join(".sops.yaml").exists() {
            eprintln!("skipping: no age recipient configured");
            return;
        }

        let value = "sk-proj-abcdefghijklmnopqrstuvwx1234";
        let meta = store(&c, "openai_api_key", value, Some("openai_api_key"), Some("OpenAI")).unwrap();
        assert_eq!(meta.name, "openai_api_key");
        assert_eq!(meta.format.as_deref(), Some("openai_api_key"));
        assert!(meta.updated_at.is_some(), "sops writes lastmodified");

        // At rest the value must be absent in plaintext.
        let on_disk = std::fs::read_to_string(secret_path(&tmp, "openai_api_key")).unwrap();
        assert!(
            !on_disk.contains(value),
            "the plaintext value leaked into the ciphertext file"
        );
        assert!(on_disk.contains("ENC["), "file should be sops-encrypted");

        // …and file mode must be 0600.
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(secret_path(&tmp, "openai_api_key"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "secret file must be owner-only");

        assert_eq!(get(&c, "openai_api_key").unwrap(), value);
        let listed = list(&c).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "openai_api_key");
        assert!(delete(&c, "openai_api_key").unwrap());
        assert!(!delete(&c, "openai_api_key").unwrap());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// Regression: a `.sops.yaml` that matches nothing must not break storing.
    ///
    /// sops searches for its config from the operand's directory upward, so an
    /// unrelated `~/.sops.yaml` (rules for `*.env`, say) is found from the
    /// secrets directory too. It then matches no rule for a `.sops.json` payload
    /// and fails with `error loading config: no matching creation rules found`
    /// **even with `--age` passed** — which is why `encrypt` passes
    /// `--config /dev/null` whenever it names a recipient itself.
    #[test]
    fn unrelated_sops_yaml_does_not_block_an_explicit_recipient() {
        let tmp = std::env::temp_dir().join(format!(
            "cenno-secrets-yaml-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let c = cfg(&tmp);

        let have_sops = Command::new("sops")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !have_sops {
            eprintln!("skipping: sops not installed");
            return;
        }
        let Some(recipient) = age_recipient(&c) else {
            eprintln!("skipping: no age recipient configured");
            return;
        };

        // A rules file that deliberately does not cover <name>.sops.json.
        std::fs::write(
            tmp.join(".sops.yaml"),
            format!("creation_rules:\n  - path_regex: \\.env.*$\n    age: {recipient}\n"),
        )
        .unwrap();

        store(&c, "openai", "sk-proj-regression1234567890", None, None)
            .expect("an unrelated .sops.yaml must not block an explicit --age");
        assert_eq!(get(&c, "openai").unwrap(), "sk-proj-regression1234567890");
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
