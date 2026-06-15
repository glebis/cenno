//! Cross-device prompt routing policy (the Mac-side resolver).
//!
//! The Mac owns the user's routing policy and resolves, for each prompt, the
//! set of eligible companion device classes and the mode each should use. The
//! result is stamped onto the CloudKit `Prompt` record as a flat `targets`
//! string (`"iphone:fallback,ipad:mirror"`) plus a `grace_s` value; companion
//! devices parse their own entry and self-filter (see the design spec).
//!
//! Authority model: the agent's `device_hint` is a *proposal*; the user's
//! policy is final. The hint can only NARROW to an already-eligible class — it
//! can never enable an `off` class nor escalate `fallback` → `mirror`.

use serde::{Deserialize, Serialize};

/// How a device class participates in routing. Per-class, set by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceMode {
    /// Never receives prompts. Hard gate — no agent hint can override.
    Off,
    /// Surfaces a prompt only after `grace_s` has elapsed and it's still pending.
    Fallback,
    /// Surfaces a prompt immediately (a live "second screen").
    Mirror,
}

impl DeviceMode {
    fn as_str(self) -> &'static str {
        match self {
            DeviceMode::Off => "off",
            DeviceMode::Fallback => "fallback",
            DeviceMode::Mirror => "mirror",
        }
    }
}

/// A companion device class the Mac can route to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceClass {
    Iphone,
    Ipad,
    Watch,
}

impl DeviceClass {
    fn as_str(self) -> &'static str {
        match self {
            DeviceClass::Iphone => "iphone",
            DeviceClass::Ipad => "ipad",
            DeviceClass::Watch => "watch",
        }
    }

    /// Parse an agent-supplied hint. Accepts `"phone"` as an alias for iphone.
    /// Unknown / `"any"` / `"mac"` → `None` (treated as "no hint").
    pub fn parse_hint(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "iphone" | "phone" => Some(DeviceClass::Iphone),
            "ipad" | "tablet" => Some(DeviceClass::Ipad),
            "watch" => Some(DeviceClass::Watch),
            _ => None,
        }
    }
}

fn default_iphone() -> DeviceMode {
    DeviceMode::Fallback
}
fn default_grace_s() -> u64 {
    20
}
fn default_allow_hint() -> bool {
    true
}

/// User's global routing policy (lives under `"routing"` in `~/.cenno/config.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RoutingConfig {
    #[serde(default = "default_iphone")]
    pub iphone: DeviceMode,
    pub ipad: DeviceMode,
    pub watch: DeviceMode,
    #[serde(default = "default_grace_s")]
    pub grace_s: u64,
    #[serde(default = "default_allow_hint")]
    pub allow_agent_hint: bool,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            iphone: default_iphone(),
            ipad: DeviceMode::Off,
            watch: DeviceMode::Off,
            grace_s: default_grace_s(),
            allow_agent_hint: default_allow_hint(),
        }
    }
}

/// Resolved routing decision for a single prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    /// Flat `"class:mode"` list, comma-joined, deterministically ordered.
    /// Empty when no companion device is eligible.
    pub targets: String,
    pub grace_s: u64,
}

impl RoutingConfig {
    fn mode_of(&self, class: DeviceClass) -> DeviceMode {
        match class {
            DeviceClass::Iphone => self.iphone,
            DeviceClass::Ipad => self.ipad,
            DeviceClass::Watch => self.watch,
        }
    }

    /// Resolve eligible targets, honoring the agent hint within policy limits.
    pub fn resolve(&self, agent_hint: Option<DeviceClass>) -> Routed {
        // Deterministic class order.
        let order = [DeviceClass::Iphone, DeviceClass::Ipad, DeviceClass::Watch];
        let eligible: Vec<DeviceClass> = order
            .into_iter()
            .filter(|c| self.mode_of(*c) != DeviceMode::Off)
            .collect();

        // The hint may narrow to a single eligible class. It can never enable an
        // off class (filter below drops it) nor change a class's mode.
        let selected: Vec<DeviceClass> = match agent_hint {
            Some(hint) if self.allow_agent_hint && eligible.contains(&hint) => vec![hint],
            _ => eligible,
        };

        let targets = selected
            .iter()
            .map(|c| format!("{}:{}", c.as_str(), self.mode_of(*c).as_str()))
            .collect::<Vec<_>>()
            .join(",");

        Routed {
            targets,
            grace_s: self.grace_s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_iphone_fallback_others_off() {
        let cfg = RoutingConfig::default();
        let r = cfg.resolve(None);
        assert_eq!(r.targets, "iphone:fallback");
        assert_eq!(r.grace_s, 20);
    }

    #[test]
    fn no_eligible_devices_yields_empty_targets() {
        let cfg = RoutingConfig {
            iphone: DeviceMode::Off,
            ..RoutingConfig::default()
        };
        assert_eq!(cfg.resolve(None).targets, "");
    }

    #[test]
    fn mixed_modes_encode_per_class() {
        let cfg = RoutingConfig {
            iphone: DeviceMode::Fallback,
            ipad: DeviceMode::Mirror,
            ..RoutingConfig::default()
        };
        assert_eq!(cfg.resolve(None).targets, "iphone:fallback,ipad:mirror");
    }

    #[test]
    fn hint_narrows_to_one_eligible_class() {
        let cfg = RoutingConfig {
            iphone: DeviceMode::Fallback,
            ipad: DeviceMode::Mirror,
            ..RoutingConfig::default()
        };
        let r = cfg.resolve(Some(DeviceClass::Ipad));
        assert_eq!(r.targets, "ipad:mirror");
    }

    #[test]
    fn hint_to_off_class_is_ignored() {
        // iphone allowed (fallback), ipad off. Agent asks for ipad → ignored,
        // falls back to all eligible (iphone).
        let cfg = RoutingConfig {
            iphone: DeviceMode::Fallback,
            ipad: DeviceMode::Off,
            ..RoutingConfig::default()
        };
        assert_eq!(cfg.resolve(Some(DeviceClass::Ipad)).targets, "iphone:fallback");
    }

    #[test]
    fn hint_cannot_escalate_fallback_to_mirror() {
        let cfg = RoutingConfig {
            iphone: DeviceMode::Fallback,
            ..RoutingConfig::default()
        };
        // Even hinted, iphone keeps its fallback mode.
        assert_eq!(cfg.resolve(Some(DeviceClass::Iphone)).targets, "iphone:fallback");
    }

    #[test]
    fn hint_disabled_globally_ignores_hint() {
        let cfg = RoutingConfig {
            iphone: DeviceMode::Fallback,
            ipad: DeviceMode::Mirror,
            allow_agent_hint: false,
            ..RoutingConfig::default()
        };
        // Hint ignored → all eligible.
        assert_eq!(
            cfg.resolve(Some(DeviceClass::Ipad)).targets,
            "iphone:fallback,ipad:mirror"
        );
    }

    #[test]
    fn parse_hint_accepts_phone_alias() {
        assert_eq!(DeviceClass::parse_hint("phone"), Some(DeviceClass::Iphone));
        assert_eq!(DeviceClass::parse_hint("iPhone"), Some(DeviceClass::Iphone));
        assert_eq!(DeviceClass::parse_hint("ipad"), Some(DeviceClass::Ipad));
        assert_eq!(DeviceClass::parse_hint("any"), None);
        assert_eq!(DeviceClass::parse_hint("mac"), None);
    }

    #[test]
    fn config_deserializes_partial_with_defaults() {
        let cfg: RoutingConfig = serde_json::from_str(r#"{"ipad":"mirror"}"#).unwrap();
        assert_eq!(cfg.iphone, DeviceMode::Fallback); // default kept
        assert_eq!(cfg.ipad, DeviceMode::Mirror);
        assert_eq!(cfg.grace_s, 20);
        assert!(cfg.allow_agent_hint);
    }
}
