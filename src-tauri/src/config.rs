//! config.rs — external configuration from `~/.cenno`.
//!
//! Two optional, hot-droppable files (both absent → built-in defaults):
//!   - `~/.cenno/config.json` — panel geometry/position, prompt defaults, and
//!     declarative custom widget templates.
//!   - `~/.cenno/tokens.json` — W3C DTCG design tokens; the webview flattens
//!     them to `--cenno-*` CSS variables and overrides the built-in theme.
//!
//! Loading is lenient: a missing file is fine; a malformed one logs a warning
//! and falls back to defaults rather than crashing a tray app the user can't
//! see errors from.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// `~/.cenno` — the config directory. None if the home dir can't be resolved.
pub fn cenno_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".cenno"))
}

fn config_path() -> Option<PathBuf> {
    cenno_dir().map(|d| d.join("config.json"))
}

fn tokens_path() -> Option<PathBuf> {
    cenno_dir().map(|d| d.join("tokens.json"))
}

/// Screen anchor for the default panel position.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

/// Default panel position: either explicit logical-point coordinates or a
/// screen-corner anchor with a margin (defaults to 16pt).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PanelPosition {
    Coords { x: f64, y: f64 },
    Anchored {
        anchor: Anchor,
        #[serde(default = "default_margin")]
        margin: f64,
    },
}
fn default_margin() -> f64 {
    16.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PanelConfig {
    /// Fixed panel width in logical points (built-in: 420).
    pub width: Option<f64>,
    /// Minimum content-driven height (built-in: 240).
    pub min_height: Option<f64>,
    /// Maximum content-driven height (built-in: 560).
    pub max_height: Option<f64>,
    /// Where a fresh panel appears when there's no remembered position.
    pub position: Option<PanelPosition>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DefaultsConfig {
    /// Default `timeout_s` for prompts that don't set one (built-in: 120).
    pub timeout_s: Option<u64>,
    /// Default flow theme when a prompt omits `flow` (built-in: none → question).
    pub flow: Option<String>,
}

/// Voice-out ("sound-out") settings. Opt-in: absent or `enabled:false` means
/// no prompt is ever spoken and no audio backend is touched.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TtsConfig {
    /// Master switch for speaking prompts aloud (built-in: false).
    pub enabled: Option<bool>,
    /// Minimum urgency that gets read aloud: "low" | "normal" | "high"
    /// (built-in: "high" — only High-urgency prompts speak until lowered).
    /// Reuses AskRequest.urgency rather than a parallel priority field.
    pub min_urgency: Option<String>,
    /// On-device voice identifier. For the `system` engine: an
    /// AVSpeechSynthesisVoice id (absent → auto-pick best installed). For the
    /// `supertonic` engine: a voice-style name like "F3".
    pub voice: Option<String>,
    /// TTS engine: "system" (AVSpeechSynthesizer, default) or "supertonic"
    /// (on-device neural). Supertonic falls back to system if assets are
    /// missing or synthesis fails.
    pub engine: Option<String>,
    /// Custom path to a Supertonic model directory (containing `onnx/` +
    /// `voice_styles/`). Absent → the default `~/.cenno/models/supertonic-3`
    /// cache. An invalid path falls back to AVSpeech, never crashes.
    pub model_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub panel: PanelConfig,
    pub defaults: DefaultsConfig,
    /// Voice-out settings (sound-out). Defaults to disabled.
    pub tts: TtsConfig,
    /// Cross-device prompt routing policy (which companion devices receive
    /// prompts and how). See `crate::routing`.
    pub routing: crate::routing::RoutingConfig,
    /// Declarative custom widget templates, keyed by name. Each value is an
    /// A2UI component-tree template the desugar layer expands (validated at the
    /// boundary like any agent payload — no code execution).
    pub widgets: std::collections::HashMap<String, serde_json::Value>,
}

impl Config {
    /// Load `~/.cenno/config.json`. Missing → defaults; malformed → defaults
    /// with a logged warning (returned in `.1` for the caller to surface).
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return Self::default(), // absent (or unreadable) → defaults
        };
        match serde_json::from_str::<Config>(&raw) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("cenno: ignoring malformed {}: {e}", path.display());
                Self::default()
            }
        }
    }

    /// Write the config back to `~/.cenno/config.json`, creating `~/.cenno`
    /// if needed. Pretty-printed so a human can keep hand-editing it. Used by
    /// the settings window to persist Voice/TTS and defaults choices.
    pub fn save(&self) -> Result<(), String> {
        let path = config_path().ok_or_else(|| "could not resolve ~/.cenno".to_string())?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| format!("serializing config: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("writing {}: {e}", path.display()))
    }
}

/// Resolved, clamped panel geometry — built-in defaults overridden by config.
#[derive(Debug, Clone, Copy)]
pub struct PanelGeometry {
    pub width: f64,
    pub min_height: f64,
    pub max_height: f64,
}

impl PanelGeometry {
    pub const DEFAULT: PanelGeometry = PanelGeometry {
        width: 420.0,
        min_height: 240.0,
        max_height: 560.0,
    };

    /// Apply a `PanelConfig` over the defaults, keeping values sane (positive,
    /// min ≤ max, width within a reasonable band).
    pub fn from_config(panel: &PanelConfig) -> Self {
        let d = Self::DEFAULT;
        let width = panel.width.filter(|w| w.is_finite()).unwrap_or(d.width).clamp(240.0, 1200.0);
        let min_height = panel.min_height.filter(|h| h.is_finite()).unwrap_or(d.min_height).clamp(120.0, 2000.0);
        let max_height = panel
            .max_height
            .filter(|h| h.is_finite())
            .unwrap_or(d.max_height)
            .clamp(min_height, 2000.0);
        Self { width, min_height, max_height }
    }
}

/// Read `~/.cenno/tokens.json` as a raw JSON value for the webview to convert.
/// None if absent or malformed (built-in tokens then stand alone).
pub fn user_tokens() -> Option<serde_json::Value> {
    let path = tokens_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("cenno: ignoring malformed {}: {e}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_is_all_defaults() {
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert!(cfg.panel.width.is_none());
        assert!(cfg.defaults.timeout_s.is_none());
        assert!(cfg.widgets.is_empty());
        let geo = PanelGeometry::from_config(&cfg.panel);
        assert_eq!(geo.width, 420.0);
        assert_eq!(geo.min_height, 240.0);
        assert_eq!(geo.max_height, 560.0);
    }

    #[test]
    fn panel_overrides_apply_and_clamp() {
        let cfg: Config =
            serde_json::from_str(r#"{"panel":{"width":520,"min_height":300,"max_height":700}}"#)
                .unwrap();
        let geo = PanelGeometry::from_config(&cfg.panel);
        assert_eq!(geo.width, 520.0);
        assert_eq!(geo.min_height, 300.0);
        assert_eq!(geo.max_height, 700.0);
    }

    #[test]
    fn absurd_geometry_is_clamped_not_trusted() {
        let cfg: Config =
            serde_json::from_str(r#"{"panel":{"width":5,"min_height":9000,"max_height":1}}"#)
                .unwrap();
        let geo = PanelGeometry::from_config(&cfg.panel);
        assert_eq!(geo.width, 240.0); // floored
        assert_eq!(geo.min_height, 2000.0); // ceiled
        assert!(geo.max_height >= geo.min_height); // never inverted
    }

    #[test]
    fn position_parses_coords_and_anchor() {
        let coords: PanelConfig = serde_json::from_str(r#"{"position":{"x":100,"y":80}}"#).unwrap();
        assert!(matches!(coords.position, Some(PanelPosition::Coords { x, y }) if x == 100.0 && y == 80.0));

        let anchored: PanelConfig =
            serde_json::from_str(r#"{"position":{"anchor":"top-right","margin":24}}"#).unwrap();
        assert!(matches!(
            anchored.position,
            Some(PanelPosition::Anchored { anchor: Anchor::TopRight, margin }) if margin == 24.0
        ));

        // margin defaults to 16 when omitted
        let default_margin: PanelConfig =
            serde_json::from_str(r#"{"position":{"anchor":"center"}}"#).unwrap();
        assert!(matches!(
            default_margin.position,
            Some(PanelPosition::Anchored { anchor: Anchor::Center, margin }) if margin == 16.0
        ));
    }

    #[test]
    fn tts_defaults_to_disabled_and_parses_overrides() {
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert!(cfg.tts.enabled.is_none()); // absent → treated as off
        assert!(cfg.tts.min_urgency.is_none());

        let cfg: Config =
            serde_json::from_str(r#"{"tts":{"enabled":true,"min_urgency":"normal","voice":"com.apple.voice.premium.en-US.Zoe"}}"#).unwrap();
        assert_eq!(cfg.tts.enabled, Some(true));
        assert_eq!(cfg.tts.min_urgency.as_deref(), Some("normal"));
        assert_eq!(cfg.tts.voice.as_deref(), Some("com.apple.voice.premium.en-US.Zoe"));
    }

    #[test]
    fn tts_full_settings_round_trip_under_deny_unknown_fields() {
        // The settings UI writes engine + voice + model_path; all must parse
        // under deny_unknown_fields (audit BLOCKER: a field the struct doesn't
        // model would make Config::load fall back to defaults).
        let src = r#"{"tts":{"enabled":true,"min_urgency":"low","engine":"supertonic","voice":"F3","model_path":"/Users/x/models/supertonic-3"}}"#;
        let cfg: Config = serde_json::from_str(src).expect("must parse");
        assert_eq!(cfg.tts.engine.as_deref(), Some("supertonic"));
        assert_eq!(cfg.tts.voice.as_deref(), Some("F3"));
        assert_eq!(cfg.tts.model_path.as_deref(), Some("/Users/x/models/supertonic-3"));
        // Re-serialize → re-deserialize must round-trip (no field drops the file).
        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: Config = serde_json::from_str(&json).expect("re-deserialize must parse");
        assert_eq!(cfg2.tts.model_path.as_deref(), Some("/Users/x/models/supertonic-3"));
    }

    #[test]
    fn malformed_config_does_not_panic() {
        // deny_unknown_fields rejects typos; from_str errs and load() would
        // fall back. Here we assert the parse simply errors (no panic).
        assert!(serde_json::from_str::<Config>(r#"{"panel":{"widht":420}}"#).is_err());
    }
}
