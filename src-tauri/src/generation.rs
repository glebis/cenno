//! generation.rs — a lock-free supersession gate.
//!
//! A `Generation` hands out tokens; a token is valid until anyone begins a
//! newer one or invalidates the gate. Used where "the latest request wins and
//! everything older must stand down" — cancelling a Supertonic utterance whose
//! audio is still being synthesized, and cancelling the deferred panel-show
//! fallback once the webview has signalled readiness. Same idea as the pause
//! generation counter in suppress.rs, extracted because two more subsystems
//! now need it.

use std::sync::atomic::{AtomicU64, Ordering};

pub struct Generation(AtomicU64);

impl Generation {
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    /// Start a new generation, superseding all outstanding tokens, and return
    /// the token for this one. Tokens start at 1, so a 0 captured before any
    /// begin() can never read as live.
    pub fn begin(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Supersede all outstanding tokens without starting a new claimant.
    pub fn invalidate(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    /// Whether `token` is still the latest generation.
    pub fn is_current(&self, token: u64) -> bool {
        self.0.load(Ordering::SeqCst) == token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_token_is_current() {
        let gen = Generation::new();
        let t = gen.begin();
        assert!(gen.is_current(t));
    }

    #[test]
    fn invalidate_supersedes_outstanding_token() {
        let gen = Generation::new();
        let t = gen.begin();
        gen.invalidate();
        assert!(!gen.is_current(t));
    }

    #[test]
    fn newer_begin_supersedes_older_token() {
        let gen = Generation::new();
        let old = gen.begin();
        let new = gen.begin();
        assert!(!gen.is_current(old));
        assert!(gen.is_current(new));
    }

    #[test]
    fn checking_does_not_consume_the_token() {
        let gen = Generation::new();
        let t = gen.begin();
        assert!(gen.is_current(t));
        assert!(gen.is_current(t)); // still current on a second check
    }

    #[test]
    fn token_from_before_any_begin_is_never_current() {
        // A subsystem that captured 0 before the first begin() must not treat
        // it as live once someone has begun.
        let gen = Generation::new();
        let implicit = 0u64;
        gen.begin();
        assert!(!gen.is_current(implicit));
    }
}
