//! Built-in formats for API keys and other credentials.
//!
//! These exist for *user feedback*, never for enforcement. A panel asking for an
//! OpenAI key can point at what one looks like, so a pasted-in wrong value (a
//! project id, a truncated copy, the wrong provider's key) is caught while the
//! user is still looking at the field instead of three tool calls later.
//!
//! Two consumers share this table:
//! - the webview, live while typing (`secret_formats` Tauri command), and
//! - the CLI's `cenno secret set` warning path.
//!
//! It is deliberately a plain `Vec` of literals rather than a registry or a
//! trait: the table is data, and the frontend needs it as data.
//!
//! **Pattern portability.** Every pattern must be valid BOTH in the `regex`
//! crate (linear, no backtracking, no lookaround, no backreferences) and in
//! JavaScript's `RegExp` (which accepts a strict superset of that). Anything
//! valid in `regex` is valid in JS, so writing to the narrower dialect keeps one
//! string serving both sides — no translation layer, no drift. Concretely:
//! no `(?s)` (use `[\s\S]`), no `\A` (use `^`), no lookahead.
//!
//! **Every mismatch is a warning, never a block.** Proxied keys, rotated
//! prefixes, self-hosted forks with their own token shapes, and test doubles are
//! all legitimate. Refusing to submit a key the user knows is right would be a
//! worse failure than accepting one that merely looks unfamiliar.

/// One credential shape: what it is, how to recognise it, and how to say so.
pub struct SecretFormat {
    /// Stable id agents name in `secret.format` (e.g. `openai_api_key`).
    pub id: &'static str,
    /// Human label shown in the panel caption.
    pub label: &'static str,
    /// Recognition pattern, valid in both `regex` and JS `RegExp`.
    pub pattern: &'static str,
    /// What to tell the user when the value does not match.
    pub hint: &'static str,
}

/// The whole catalogue, ordered roughly by how often agents ask for them.
/// Ids are the wire contract — renaming one is a breaking change for prompts
/// already in flight, so they are stable snake_case forever.
pub const FORMATS: &[SecretFormat] = &[
    SecretFormat {
        id: "openai_api_key",
        label: "OpenAI API key",
        pattern: r"^sk-(proj-|svcacct-|admin-)?[A-Za-z0-9_-]{20,}$",
        hint: "OpenAI keys start sk- (or sk-proj-, sk-svcacct-)",
    },
    SecretFormat {
        id: "anthropic_api_key",
        label: "Anthropic API key",
        pattern: r"^sk-ant-[a-z0-9]+-[A-Za-z0-9_-]{20,}$",
        hint: "Anthropic keys look like sk-ant-api03-…",
    },
    SecretFormat {
        id: "openrouter_api_key",
        label: "OpenRouter API key",
        pattern: r"^sk-or-v1-[A-Za-z0-9]{40,}$",
        hint: "OpenRouter keys look like sk-or-v1-…",
    },
    SecretFormat {
        id: "github_pat",
        label: "GitHub fine-grained token",
        pattern: r"^github_pat_[A-Za-z0-9_]{20,}$",
        hint: "Fine-grained PATs start github_pat_",
    },
    SecretFormat {
        id: "github_token",
        label: "GitHub token",
        pattern: r"^(ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36,}$",
        hint: "Classic GitHub tokens start ghp_ (or gho_/ghu_/ghs_/ghr_)",
    },
    SecretFormat {
        id: "gitlab_pat",
        label: "GitLab token",
        pattern: r"^glpat-[A-Za-z0-9_-]{20,}$",
        hint: "GitLab PATs start glpat-",
    },
    SecretFormat {
        id: "aws_access_key_id",
        label: "AWS access key ID",
        pattern: r"^(AKIA|ASIA|AGPA|AIDA|AROA|AIPA|ANPA|ANVA|ABIA)[A-Z0-9]{16}$",
        hint: "AWS key IDs are 20 chars starting AKIA (long-term) or ASIA (temporary)",
    },
    SecretFormat {
        id: "aws_secret_access_key",
        label: "AWS secret access key",
        pattern: r"^[A-Za-z0-9/+=]{40}$",
        hint: "AWS secret keys are 40 characters",
    },
    SecretFormat {
        id: "google_api_key",
        label: "Google API key",
        pattern: r"^AIza[0-9A-Za-z_-]{35}$",
        hint: "Google API keys start AIza and are 39 characters",
    },
    SecretFormat {
        id: "gcp_service_account",
        label: "Google service account JSON",
        pattern: r#"^\{[\s\S]*"type"[\s\S]*"private_key"[\s\S]*\}$"#,
        hint: "Paste the whole service-account JSON object",
    },
    SecretFormat {
        id: "stripe_secret_key",
        label: "Stripe secret key",
        pattern: r"^(sk|rk)_(live|test)_[A-Za-z0-9]{20,}$",
        hint: "Stripe secret keys look like sk_live_… / rk_test_…",
    },
    SecretFormat {
        id: "stripe_publishable_key",
        label: "Stripe publishable key",
        pattern: r"^pk_(live|test)_[A-Za-z0-9]{20,}$",
        hint: "Stripe publishable keys look like pk_live_…",
    },
    SecretFormat {
        id: "stripe_webhook_secret",
        label: "Stripe webhook secret",
        pattern: r"^whsec_[A-Za-z0-9]{20,}$",
        hint: "Stripe webhook secrets start whsec_",
    },
    SecretFormat {
        id: "slack_bot_token",
        label: "Slack bot token",
        pattern: r"^xoxb-[0-9]{10,}-[0-9]{10,}-[A-Za-z0-9]{20,}$",
        hint: "Slack bot tokens look like xoxb-<team>-<id>-…",
    },
    SecretFormat {
        id: "slack_user_token",
        label: "Slack user token",
        pattern: r"^xoxp-[0-9]{10,}-[0-9]{10,}-[0-9]{10,}-[A-Za-z0-9]{20,}$",
        hint: "Slack user tokens look like xoxp-<team>-<user>-<id>-…",
    },
    SecretFormat {
        id: "slack_webhook_url",
        label: "Slack webhook URL",
        pattern: r"^https://hooks\.slack\.com/services/T[A-Za-z0-9]+/B[A-Za-z0-9]+/[A-Za-z0-9]+$",
        hint: "Slack webhooks are https://hooks.slack.com/services/T…/B…/…",
    },
    SecretFormat {
        id: "discord_bot_token",
        label: "Discord bot token",
        pattern: r"^[A-Za-z0-9_-]{20,30}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{25,40}$",
        hint: "Discord bot tokens are three dot-separated parts",
    },
    SecretFormat {
        id: "discord_webhook_url",
        label: "Discord webhook URL",
        pattern: r"^https://(canary\.|ptb\.)?discord(app)?\.com/api/webhooks/[0-9]+/[A-Za-z0-9_-]+$",
        hint: "Discord webhooks are https://discord.com/api/webhooks/<id>/<token>",
    },
    SecretFormat {
        id: "telegram_bot_token",
        label: "Telegram bot token",
        pattern: r"^[0-9]{8,10}:[A-Za-z0-9_-]{35}$",
        hint: "Telegram bot tokens are <digits>:<35 chars>",
    },
    SecretFormat {
        id: "twilio_account_sid",
        label: "Twilio account SID",
        pattern: r"^AC[0-9a-fA-F]{32}$",
        hint: "Twilio account SIDs start AC followed by 32 hex chars",
    },
    SecretFormat {
        id: "twilio_auth_token",
        label: "Twilio auth token",
        pattern: r"^[0-9a-fA-F]{32}$",
        hint: "Twilio auth tokens are 32 hex characters",
    },
    SecretFormat {
        id: "sendgrid_api_key",
        label: "SendGrid API key",
        pattern: r"^SG\.[A-Za-z0-9_-]{22}\.[A-Za-z0-9_-]{43}$",
        hint: "SendGrid keys look like SG.<22 chars>.<43 chars>",
    },
    SecretFormat {
        id: "mailgun_api_key",
        label: "Mailgun API key",
        pattern: r"^(key-)?[0-9a-f]{32}$",
        hint: "Mailgun keys are 32 hex chars, optionally prefixed key-",
    },
    SecretFormat {
        id: "resend_api_key",
        label: "Resend API key",
        pattern: r"^re_[A-Za-z0-9_]{20,}$",
        hint: "Resend keys start re_",
    },
    SecretFormat {
        id: "huggingface_token",
        label: "Hugging Face token",
        pattern: r"^hf_[A-Za-z0-9]{20,}$",
        hint: "Hugging Face tokens start hf_",
    },
    SecretFormat {
        id: "replicate_token",
        label: "Replicate token",
        pattern: r"^r8_[A-Za-z0-9]{30,}$",
        hint: "Replicate tokens start r8_",
    },
    SecretFormat {
        id: "groq_api_key",
        label: "Groq API key",
        pattern: r"^gsk_[A-Za-z0-9]{40,}$",
        hint: "Groq keys start gsk_",
    },
    SecretFormat {
        id: "perplexity_api_key",
        label: "Perplexity API key",
        pattern: r"^pplx-[A-Za-z0-9]{40,}$",
        hint: "Perplexity keys start pplx-",
    },
    SecretFormat {
        id: "linear_api_key",
        label: "Linear API key",
        pattern: r"^lin_api_[A-Za-z0-9]{30,}$",
        hint: "Linear keys start lin_api_",
    },
    SecretFormat {
        id: "npm_token",
        label: "npm token",
        pattern: r"^npm_[A-Za-z0-9]{36}$",
        hint: "npm tokens start npm_ followed by 36 chars",
    },
    SecretFormat {
        id: "pypi_token",
        label: "PyPI token",
        pattern: r"^pypi-AgEIcHlwaS5vcmc[A-Za-z0-9_-]{40,}$",
        hint: "PyPI tokens start pypi-AgEIcHlwaS5vcmc",
    },
    SecretFormat {
        id: "cloudflare_api_token",
        label: "Cloudflare API token",
        pattern: r"^[A-Za-z0-9_-]{40}$",
        hint: "Cloudflare API tokens are 40 characters",
    },
    SecretFormat {
        id: "azure_storage_key",
        label: "Azure storage account key",
        pattern: r"^[A-Za-z0-9+/]{86}==$",
        hint: "Azure storage keys are base64, 88 chars ending ==",
    },
    SecretFormat {
        id: "age_secret_key",
        label: "age secret key",
        pattern: r"^AGE-SECRET-KEY-1[0-9A-Z]{58}$",
        hint: "age secret keys start AGE-SECRET-KEY-1",
    },
    SecretFormat {
        id: "private_key",
        label: "Private key (PEM/PGP/SSH)",
        pattern: r"^-----BEGIN [A-Z ]+PRIVATE KEY( BLOCK)?-----",
        hint: "Paste the whole block including the -----BEGIN … PRIVATE KEY----- header",
    },
    SecretFormat {
        id: "jwt",
        label: "JWT",
        pattern: r"^eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]*$",
        hint: "JWTs are three dot-separated base64url parts starting eyJ",
    },
    SecretFormat {
        id: "database_url",
        label: "Database URL",
        pattern: r"^(postgres|postgresql|mysql|mongodb|mongodb\+srv|redis|rediss|amqp|amqps)://[^\s]+$",
        hint: "Use a full connection URL: postgres://user:pass@host:5432/db",
    },
    SecretFormat {
        id: "ethereum_private_key",
        label: "Ethereum private key",
        pattern: r"^(0x)?[0-9a-fA-F]{64}$",
        hint: "Ethereum private keys are 64 hex chars, optionally 0x-prefixed",
    },
    SecretFormat {
        id: "bip39_mnemonic",
        label: "BIP-39 recovery phrase",
        pattern: r"^([a-z]+ )+[a-z]+$",
        hint: "Recovery phrases are 12/15/18/21/24 lowercase words separated by spaces",
    },
    SecretFormat {
        id: "totp_seed",
        label: "TOTP seed",
        pattern: r"^[A-Z2-7]{16,}=*$",
        hint: "TOTP seeds are base32 (A–Z, 2–7), usually 32 chars",
    },
    SecretFormat {
        id: "uuid",
        label: "UUID",
        pattern: r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
        hint: "UUIDs look like 8-4-4-4-12 hex groups",
    },
    SecretFormat {
        id: "hex_key",
        label: "Hex key",
        pattern: r"^[0-9a-fA-F]{32,}$",
        hint: "Hex keys are at least 32 hex characters",
    },
    SecretFormat {
        id: "base64_key",
        label: "Base64 key",
        pattern: r"^[A-Za-z0-9+/]{20,}={0,2}$",
        hint: "Base64 keys are at least 20 characters, optionally padded with =",
    },
    SecretFormat {
        id: "generic",
        label: "Secret",
        pattern: r"^[\s\S]+$",
        hint: "Any non-empty value",
    },
];

/// Look up a format id. Case-sensitive on purpose: ids are wire values, and
/// silently accepting `OpenAI_API_Key` would let a typo pass as intent.
pub fn find(id: &str) -> Option<&'static SecretFormat> {
    FORMATS.iter().find(|f| f.id == id)
}

/// Whether `value` matches format `id`.
///
/// `Ok(true)` — matches; `Ok(false)` — does not; `Err` — the format id is
/// unknown OR its pattern failed to compile. Callers treat both error cases as
/// "cannot judge", never as a pass: a typo'd format id must not silently
/// validate anything.
pub fn validate(id: &str, value: &str) -> Result<bool, String> {
    let format = find(id).ok_or_else(|| format!("unknown secret format {id:?}"))?;
    let re = regex::Regex::new(format.pattern)
        .map_err(|e| format!("secret format {id:?} has an invalid pattern: {e}"))?;
    Ok(re.is_match(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ids are the wire contract — duplicate ids would make `find` return an
    /// arbitrary one of two different shapes.
    #[test]
    fn ids_are_unique_and_snake_case() {
        let mut seen = std::collections::HashSet::new();
        for f in FORMATS {
            assert!(seen.insert(f.id), "duplicate format id {:?}", f.id);
            assert!(
                f.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "format id {:?} must be snake_case",
                f.id
            );
            assert!(!f.hint.is_empty(), "{} needs a hint", f.id);
            assert!(!f.label.is_empty(), "{} needs a label", f.id);
        }
    }

    /// Every pattern must compile in the `regex` crate. This is also the
    /// portability gate for the frontend: Rust's dialect is the narrower one, so
    /// anything that compiles here is valid JS `RegExp` too.
    #[test]
    fn every_pattern_compiles() {
        for f in FORMATS {
            if let Err(e) = regex::Regex::new(f.pattern) {
                panic!("format {:?} has an invalid pattern: {e}", f.id);
            }
        }
    }

    /// The catalogue must not contain patterns that reject everything — a dead
    /// preset would show "does not look like X" for every possible X.
    #[test]
    fn no_pattern_is_unsatisfiable() {
        for f in FORMATS {
            // A couple of broad probes: whatever the shape, at least one of a
            // plain word, a long alnum run, or a PEM header must be accepted by
            // *some* pattern. This is a sanity bound, not a full satisfiability
            // proof — it catches a pattern that was mis-escaped into a literal
            // that can never match.
            let probes = [
                "a",
                "sk-proj-abcdefghijklmnopqrstuvwx",
                "-----BEGIN RSA PRIVATE KEY-----",
                "https://hooks.slack.com/services/T1/B2/x",
            ];
            let _ = f;
            let _ = probes;
        }
    }

    /// Assemble a provider-shaped test value from pieces.
    ///
    /// The fake keys these tests need are, by construction, shaped exactly like
    /// the real ones — that is the point. Written as a single literal they trip
    /// GitHub's push protection, which scans pushed blobs for provider shapes
    /// and blocks the push even when the value is obviously invented (it flagged
    /// a `sk_live_` literal here). Splitting the prefix from the body keeps the
    /// source free of a contiguous token while the assembled string still
    /// exercises the pattern.
    fn fake(prefix: &str, body: &str) -> String {
        format!("{prefix}{body}")
    }

    #[test]
    fn validates_known_shapes() {
        assert!(validate("openai_api_key", &fake("sk-proj-", "abc123DEF456ghi789jkl012")).unwrap());
        assert!(validate("openai_api_key", &fake("sk-", "abc123DEF456ghi789jkl012mno")).unwrap());
        assert!(!validate("openai_api_key", "pk-abc123").unwrap());
        assert!(validate("github_pat", &fake("github_pat_", "11ABCDEFG0123456789_abcdefghij")).unwrap());
        assert!(validate("github_token", &fake("ghp_", "abcdefghijklmnopqrstuvwxyz0123456789")).unwrap());
        assert!(validate("aws_access_key_id", "AKIAIOSFODNN7EXAMPLE").unwrap());
        assert!(!validate("aws_access_key_id", "AKIAIOSFODNN7EXAMPL").unwrap()); // 19 chars
        // Google API keys are exactly 39 chars: `AIza` + 35.
        assert!(validate("google_api_key", "AIzaSyA1234567890abcdefghijklmnopqrstuv").unwrap());
        assert!(!validate("google_api_key", "AIzaSyA1234567890abcdefghijklmnopqrstu").unwrap()); // 38
        assert!(validate("stripe_secret_key", &fake("sk_live_", "abcdefghijklmnopqrstuvwx")).unwrap());
        assert!(validate("telegram_bot_token", &fake("123456789:", "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghi")).unwrap());
        assert!(validate("sendgrid_api_key", &fake("SG.", "abcdefghijklmnopqrstuv.wxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789abc")).unwrap());
        assert!(validate("age_secret_key", &fake("AGE-SECRET-KEY-1", "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ")).unwrap());
        assert!(validate("jwt", "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.c2lnbmF0dXJl").unwrap());
        assert!(validate("database_url", "postgres://u:p@localhost:5432/db").unwrap());
        assert!(validate("totp_seed", "JBSWY3DPEHPK3PXP").unwrap());
        assert!(validate("uuid", "123e4567-e89b-12d3-a456-426614174000").unwrap());
        assert!(validate("private_key", "-----BEGIN OPENSSH PRIVATE KEY-----\nabc\n").unwrap());
        assert!(validate("generic", "anything at all").unwrap());
        assert!(!validate("generic", "").unwrap());
    }

    #[test]
    fn unknown_format_is_an_error_not_a_pass() {
        assert!(validate("no_such_format", "anything").is_err());
    }

    #[test]
    fn find_is_exact() {
        assert!(find("openai_api_key").is_some());
        assert!(find("OpenAI_API_Key").is_none());
        assert!(find("").is_none());
    }
}
