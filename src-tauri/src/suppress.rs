//! Suppression state: "pause prompts for a while" + "don't show in fullscreen".
//!
//! `should_suppress()` gates the *display* path only (lib.rs notify closure
//! and replay) — prompts still register in the registry and the agent's
//! TimedOut contract is untouched. Fullscreen detection is a CGWindowList
//! bounds heuristic, called once per prompt arrival / replay attempt, never
//! polled.

use std::sync::Arc;

use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use parking_lot::Mutex;

struct Inner {
    /// Display is paused until this instant (None = not paused). Stored in
    /// UTC; comparisons against `Utc::now()` are tz-independent.
    pause_until: Option<DateTime<Utc>>,
    /// When true (the default), prompts are not shown while another app is
    /// fullscreen — they queue quietly until suppression lifts.
    hide_in_fullscreen: bool,
    /// Bumped on every pause/resume/lazy-expiry transition. Pause-expiry
    /// timers capture the generation at arm time and no-op at fire time if
    /// it moved — a re-pause or manual resume invalidates stale timers.
    generation: u64,
}

/// Result of a suppression check. `pause_cleared` tells the caller a
/// previously-set pause was found expired and lazily cleared — the caller
/// owns persistence and should clear the stored "pause_until" setting.
/// (Persistence stays out of this module: state here, storage at the edges.)
#[derive(Debug, Clone, Copy)]
pub struct SuppressCheck {
    pub suppress: bool,
    pub pause_cleared: bool,
}

/// Shared suppression state — cheap to clone, thread-safe.
#[derive(Clone)]
pub struct SuppressionState(Arc<Mutex<Inner>>);

impl SuppressionState {
    pub fn new(hide_in_fullscreen_default: bool) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            pause_until: None,
            hide_in_fullscreen: hide_in_fullscreen_default,
            generation: 0,
        })))
    }

    /// Should the prompt display be suppressed right now?
    ///
    /// True when paused (pause_until in the future) OR when the
    /// hide-in-fullscreen setting is on AND `fullscreen_check()` reports a
    /// fullscreen app. The check is injectable so tests never touch real
    /// window servers; production passes [`fullscreen_app_present`].
    ///
    /// An expired pause is lazily cleared here (see [`Self::check`] for the
    /// variant that reports the clear so the caller can persist it).
    /// Short-circuits: while paused, `fullscreen_check` is never invoked.
    pub fn should_suppress(&self, fullscreen_check: impl Fn() -> bool) -> bool {
        self.check(fullscreen_check).suppress
    }

    /// [`Self::should_suppress`] plus a `pause_cleared` flag for callers
    /// that persist the pause setting (lib.rs clears the DB row on expiry).
    pub fn check(&self, fullscreen_check: impl Fn() -> bool) -> SuppressCheck {
        let (paused, pause_cleared, hide_fs) = {
            let mut inner = self.0.lock();
            match inner.pause_until {
                Some(until) if until > Utc::now() => (true, false, false),
                Some(_) => {
                    // Pause deadline passed: clear it now so snapshot()/menus
                    // see the truth, and invalidate any armed expiry timer
                    // (it would otherwise resume+replay a pause that's gone).
                    inner.pause_until = None;
                    inner.generation += 1;
                    (false, true, inner.hide_in_fullscreen)
                }
                None => (false, false, inner.hide_in_fullscreen),
            }
        };
        // Lock dropped before the (comparatively pricey) fullscreen check.
        if paused {
            return SuppressCheck { suppress: true, pause_cleared: false };
        }
        SuppressCheck { suppress: hide_fs && fullscreen_check(), pause_cleared }
    }

    /// Current pause generation — capture before arming an expiry timer,
    /// compare at fire time; a mismatch means the timer is stale.
    pub fn pause_generation(&self) -> u64 {
        self.0.lock().generation
    }

    /// Pause for `minutes` from now. Returns the computed deadline so the
    /// caller can persist it (settings key "pause_until").
    pub fn pause_for(&self, minutes: i64) -> DateTime<Utc> {
        let until = Utc::now() + Duration::minutes(minutes);
        let mut inner = self.0.lock();
        inner.pause_until = Some(until);
        inner.generation += 1;
        until
    }

    /// Pause until "tomorrow" = the next local 05:00, converted to UTC.
    ///
    /// Why 05:00 and not midnight: late-night workers. Someone pausing cenno
    /// at 23:40 means "leave me alone for the rest of this working night" —
    /// a midnight boundary would un-pause 20 minutes later. 05:00 is past
    /// any plausible session end and before any plausible morning start.
    ///
    /// Returns the deadline for persistence.
    pub fn pause_until_tomorrow(&self) -> DateTime<Utc> {
        let until = next_5am_after(Local::now()).with_timezone(&Utc);
        let mut inner = self.0.lock();
        inner.pause_until = Some(until);
        inner.generation += 1;
        until
    }

    /// Restore a persisted pause deadline (app startup). Callers are
    /// responsible for ignoring already-expired values.
    pub fn restore_pause_until(&self, until: DateTime<Utc>) {
        let mut inner = self.0.lock();
        inner.pause_until = Some(until);
        inner.generation += 1;
    }

    /// Clear any pause immediately ("Resume now").
    pub fn resume(&self) {
        let mut inner = self.0.lock();
        inner.pause_until = None;
        inner.generation += 1;
    }

    pub fn set_hide_in_fullscreen(&self, hide: bool) {
        self.0.lock().hide_in_fullscreen = hide;
    }

    /// Read the current state: (pause_until, hide_in_fullscreen).
    pub fn snapshot(&self) -> (Option<DateTime<Utc>>, bool) {
        let inner = self.0.lock();
        (inner.pause_until, inner.hide_in_fullscreen)
    }
}

// ---------------------------------------------------------------------------
// Fullscreen detection
// ---------------------------------------------------------------------------

/// Plain rect so the bounds-match heuristic stays pure and testable on every
/// platform (the CG types only exist on macOS).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Top slack for the fullscreen match, in points. On notched MacBooks a
/// fullscreen Space does NOT start at the display's y=0 — the window sits
/// below the camera-housing strip (observed live: display 1512x982, full-
/// screen TextEdit at y=33, height 949). 40pt comfortably covers notch
/// strips (32-38pt) and classic menu bars (24-25pt) without admitting
/// anything that looks like a normal window.
pub const FULLSCREEN_TOP_SLACK: f64 = 40.0;

/// The heuristic's core: a window counts as fullscreen iff, for some display,
/// it spans the display's full width, its bottom edge is flush with the
/// display's bottom, and its top edge is within [`FULLSCREEN_TOP_SLACK`] of
/// the display's top. Plain `bounds == display` is wrong on notched MacBooks
/// (see [`FULLSCREEN_TOP_SLACK`]); non-notched fullscreen (external displays)
/// is the slack=0 case of the same rule.
///
/// Normal windows fail the bottom-flush test as long as the Dock is visible
/// (and any window with a y-offset beyond the slack fails the top test).
/// Known false positive: a maximized window with the Dock auto-hidden has
/// fullscreen-identical bounds — indistinguishable by geometry alone, and
/// erring toward "quiet while something edge-to-edge is up" suits the
/// feature: suppressed prompts queue and replay, they're never lost.
/// Exact f64 compares are deliberate: all values are integral points from
/// the same CG coordinate space.
pub fn covers_any_display(window: Rect, displays: &[Rect]) -> bool {
    displays.iter().any(|d| {
        let top_offset = window.y - d.y;
        window.x == d.x
            && window.w == d.w
            && window.y + window.h == d.y + d.h
            && (0.0..=FULLSCREEN_TOP_SLACK).contains(&top_offset)
    })
}

/// Is any app fullscreen right now? (macOS)
///
/// CGWindowList heuristic: walk on-screen windows (z-ordered, all apps), and
/// report true if any window at layer 0 (normal app windows; our own panel
/// floats at level 3 and can't false-positive) exactly covers a display.
/// Multi-display: a fullscreen app on ANY display suppresses — v1 keeps this
/// deliberately simple, and "don't interrupt while something is fullscreen
/// anywhere" is arguably the right focus semantics anyway.
///
/// Cost: one CGWindowListCopyWindowInfo + one bounds pass. Called once per
/// prompt arrival and once per replay attempt — never polled. Window bounds
/// and layers don't require the screen-recording permission (window *names*
/// would; we never read them).
#[cfg(target_os = "macos")]
pub fn fullscreen_app_present() -> bool {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::CGDisplay;
    use core_graphics::geometry::CGRect;
    use core_graphics::window as cgw;

    let displays: Vec<Rect> = match CGDisplay::active_displays() {
        Ok(ids) => ids
            .into_iter()
            .map(|id| {
                let b = CGDisplay::new(id).bounds();
                Rect { x: b.origin.x, y: b.origin.y, w: b.size.width, h: b.size.height }
            })
            .collect(),
        Err(_) => return false, // can't tell → don't suppress
    };

    let Some(windows) =
        cgw::copy_window_info(cgw::kCGWindowListOptionOnScreenOnly, cgw::kCGNullWindowID)
    else {
        return false;
    };

    let layer_key = unsafe { CFString::wrap_under_get_rule(cgw::kCGWindowLayer) };
    let bounds_key = unsafe { CFString::wrap_under_get_rule(cgw::kCGWindowBounds) };

    for item in windows.iter() {
        let dict = unsafe {
            CFDictionary::<CFString, CFType>::wrap_under_get_rule(*item as CFDictionaryRef)
        };
        let layer = dict
            .find(&layer_key)
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|n| n.to_i64());
        if layer != Some(0) {
            continue;
        }
        let Some(bounds) = dict
            .find(&bounds_key)
            .and_then(|v| v.downcast::<CFDictionary>())
            .and_then(|d| CGRect::from_dict_representation(&d))
        else {
            continue;
        };
        let rect = Rect {
            x: bounds.origin.x,
            y: bounds.origin.y,
            w: bounds.size.width,
            h: bounds.size.height,
        };
        if covers_any_display(rect, &displays) {
            return true;
        }
    }
    false
}

/// Non-macOS: no fullscreen detection — never suppress on that account.
#[cfg(not(target_os = "macos"))]
pub fn fullscreen_app_present() -> bool {
    false
}

/// Pure date math for "until tomorrow": the next 05:00 STRICTLY after `now`,
/// in `now`'s own timezone. Generic over `TimeZone` so tests can pin a
/// `FixedOffset` instead of depending on the machine's local zone.
///
/// At exactly 05:00:00 the result is tomorrow's 05:00 — a user pausing at
/// the boundary wants a real pause, not a zero-length one.
///
/// DST edge: if 05:00 doesn't exist (spring-forward) or is ambiguous on the
/// target day, take the earliest valid mapping, falling back to +1h past the
/// gap. (For real-world DST rules 05:00 is never affected — transitions
/// happen at 01:00–04:00 — but the math must not panic.)
pub fn next_5am_after<Tz: TimeZone>(now: DateTime<Tz>) -> DateTime<Tz> {
    let tz = now.timezone();
    let mut day = now.date_naive();
    loop {
        let candidate = tz
            .with_ymd_and_hms(day.year(), day.month(), day.day(), 5, 0, 0)
            .earliest()
            .or_else(|| {
                // 05:00 swallowed by a DST gap: use 06:00 that day instead.
                tz.with_ymd_and_hms(day.year(), day.month(), day.day(), 6, 0, 0)
                    .earliest()
            });
        if let Some(c) = candidate {
            if c > now {
                return c;
            }
        }
        day = day.succ_opt().expect("date overflow computing next 5am");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    /// CET (UTC+1), independent of the host machine's timezone.
    fn tz() -> FixedOffset {
        FixedOffset::east_opt(3600).unwrap()
    }

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<FixedOffset> {
        tz().with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    #[test]
    fn next_5am_before_5_is_same_day() {
        // 04:59:59 → today 05:00.
        assert_eq!(next_5am_after(at(2026, 6, 10, 4, 59, 59)), at(2026, 6, 10, 5, 0, 0));
        // Midnight → today 05:00 (the late-night-worker case this exists for).
        assert_eq!(next_5am_after(at(2026, 6, 10, 0, 0, 0)), at(2026, 6, 10, 5, 0, 0));
    }

    #[test]
    fn next_5am_at_exactly_5_is_tomorrow() {
        assert_eq!(next_5am_after(at(2026, 6, 10, 5, 0, 0)), at(2026, 6, 11, 5, 0, 0));
    }

    #[test]
    fn next_5am_after_5_is_tomorrow() {
        assert_eq!(next_5am_after(at(2026, 6, 10, 5, 0, 1)), at(2026, 6, 11, 5, 0, 0));
        // Evening → tomorrow 05:00.
        assert_eq!(next_5am_after(at(2026, 6, 10, 23, 30, 0)), at(2026, 6, 11, 5, 0, 0));
    }

    #[test]
    fn next_5am_crosses_month_and_year() {
        assert_eq!(next_5am_after(at(2026, 6, 30, 18, 0, 0)), at(2026, 7, 1, 5, 0, 0));
        assert_eq!(next_5am_after(at(2026, 12, 31, 23, 59, 59)), at(2027, 1, 1, 5, 0, 0));
    }

    #[test]
    fn pause_for_sets_deadline_minutes_ahead() {
        let s = SuppressionState::new(true);
        let before = Utc::now();
        let until = s.pause_for(15);
        let after = Utc::now();

        // Deadline is now+15min within the call window.
        assert!(until >= before + Duration::minutes(15));
        assert!(until <= after + Duration::minutes(15));
        // And it's what snapshot() reports.
        assert_eq!(s.snapshot().0, Some(until));
    }

    #[test]
    fn pause_until_tomorrow_lands_on_a_local_5am_in_the_future() {
        let s = SuppressionState::new(true);
        let until = s.pause_until_tomorrow();
        assert_eq!(s.snapshot().0, Some(until));

        let local = until.with_timezone(&Local);
        assert_eq!((local.format("%H:%M:%S")).to_string(), "05:00:00");
        assert!(until > Utc::now());
        // Never more than ~24h + DST slack away.
        assert!(until <= Utc::now() + Duration::hours(25));
    }

    #[test]
    fn resume_clears_pause() {
        let s = SuppressionState::new(true);
        s.pause_for(60);
        assert!(s.snapshot().0.is_some());
        s.resume();
        assert_eq!(s.snapshot().0, None);
    }

    #[test]
    fn hide_in_fullscreen_default_and_toggle() {
        let s = SuppressionState::new(true);
        assert!(s.snapshot().1);
        s.set_hide_in_fullscreen(false);
        assert!(!s.snapshot().1);

        let s2 = SuppressionState::new(false);
        assert!(!s2.snapshot().1);
    }

    #[test]
    fn restore_pause_until_round_trips() {
        let s = SuppressionState::new(true);
        let dt = Utc::now() + Duration::hours(2);
        s.restore_pause_until(dt);
        assert_eq!(s.snapshot().0, Some(dt));
    }

    // --- should_suppress / check ---

    #[test]
    fn suppresses_while_paused_without_consulting_fullscreen() {
        let s = SuppressionState::new(true);
        s.pause_for(15);
        // While paused, the fullscreen check must not even run (it's the
        // expensive half) — a panicking closure proves the short-circuit.
        assert!(s.should_suppress(|| panic!("fullscreen check ran while paused")));
    }

    #[test]
    fn expired_pause_does_not_suppress_and_is_cleared_with_flag() {
        let s = SuppressionState::new(false);
        s.restore_pause_until(Utc::now() - Duration::seconds(1));

        let check = s.check(|| false);
        assert!(!check.suppress);
        assert!(check.pause_cleared, "caller must learn the pause expired (to persist the clear)");
        assert_eq!(s.snapshot().0, None, "expired pause lazily cleared");

        // Second check: nothing left to clear.
        let again = s.check(|| false);
        assert!(!again.suppress);
        assert!(!again.pause_cleared);
    }

    #[test]
    fn fullscreen_suppresses_only_when_setting_is_on() {
        let on = SuppressionState::new(true);
        assert!(on.should_suppress(|| true));
        assert!(!on.should_suppress(|| false));

        let off = SuppressionState::new(false);
        assert!(!off.should_suppress(|| true));
    }

    #[test]
    fn fullscreen_check_skipped_when_setting_is_off() {
        let s = SuppressionState::new(false);
        assert!(!s.should_suppress(|| panic!("fullscreen check ran with setting off")));
    }

    #[test]
    fn generation_bumps_on_every_pause_transition() {
        let s = SuppressionState::new(true);
        let g0 = s.pause_generation();
        s.pause_for(15);
        let g1 = s.pause_generation();
        assert!(g1 > g0);
        s.resume();
        let g2 = s.pause_generation();
        assert!(g2 > g1);
        s.pause_until_tomorrow();
        assert!(s.pause_generation() > g2);

        // Lazy expiry clear bumps too — stale expiry timers must not fire
        // a second resume+replay after check() already handled the expiry.
        s.restore_pause_until(Utc::now() - Duration::seconds(1));
        let g3 = s.pause_generation();
        let _ = s.check(|| false);
        assert!(s.pause_generation() > g3);
    }

    // --- bounds heuristic ---

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn window_exactly_matching_display_is_fullscreen() {
        // Non-notched displays (externals): fullscreen = exact bounds match.
        let displays = [r(0.0, 0.0, 1728.0, 1117.0)];
        assert!(covers_any_display(r(0.0, 0.0, 1728.0, 1117.0), &displays));
    }

    #[test]
    fn notched_fullscreen_window_is_fullscreen() {
        // Live-observed geometry on a notched MacBook: display 1512x982,
        // fullscreen TextEdit window at (0, 33, 1512, 949) — below the
        // camera-housing strip, bottom flush with the display.
        let displays = [r(0.0, 0.0, 1512.0, 982.0)];
        assert!(covers_any_display(r(0.0, 33.0, 1512.0, 949.0), &displays));
        // The fullscreen toolbar accessory window (same top, short) is not.
        assert!(!covers_any_display(r(0.0, 33.0, 1512.0, 68.0), &displays));
    }

    #[test]
    fn off_by_one_window_is_not_fullscreen() {
        let displays = [r(0.0, 0.0, 1728.0, 1117.0)];
        // Maximized below the menu bar with the Dock visible: bottom not flush.
        assert!(!covers_any_display(r(0.0, 25.0, 1728.0, 1024.0), &displays));
        // One point short in width / height.
        assert!(!covers_any_display(r(0.0, 0.0, 1727.0, 1117.0), &displays));
        assert!(!covers_any_display(r(0.0, 0.0, 1728.0, 1116.0), &displays));
        // Top offset beyond the notch/menu-bar slack.
        assert!(!covers_any_display(r(0.0, 41.0, 1728.0, 1076.0), &displays));
        // Sticking out above the display (negative offset).
        assert!(!covers_any_display(r(0.0, -10.0, 1728.0, 1127.0), &displays));
    }

    #[test]
    fn window_matching_second_display_is_fullscreen() {
        let displays = [r(0.0, 0.0, 1728.0, 1117.0), r(1728.0, 0.0, 2560.0, 1440.0)];
        assert!(covers_any_display(r(1728.0, 0.0, 2560.0, 1440.0), &displays));
        // …but a window matching neither is not.
        assert!(!covers_any_display(r(0.0, 0.0, 2560.0, 1440.0), &displays));
    }
}
