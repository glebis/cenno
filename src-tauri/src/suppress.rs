//! Suppression state: "pause prompts for a while" + "don't show in fullscreen".
//!
//! Task 3 STUB — this module only holds the state and the date math the tray
//! menu drives. The actual gating (`should_suppress()`), persistence-on-load
//! glue, fullscreen detection, and pending-prompt replay land in Task 4.

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
}

/// Shared suppression state — cheap to clone, thread-safe.
#[derive(Clone)]
pub struct SuppressionState(Arc<Mutex<Inner>>);

impl SuppressionState {
    pub fn new(hide_in_fullscreen_default: bool) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            pause_until: None,
            hide_in_fullscreen: hide_in_fullscreen_default,
        })))
    }

    /// Pause for `minutes` from now. Returns the computed deadline so the
    /// caller can persist it (settings key "pause_until").
    pub fn pause_for(&self, minutes: i64) -> DateTime<Utc> {
        let until = Utc::now() + Duration::minutes(minutes);
        self.0.lock().pause_until = Some(until);
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
        self.0.lock().pause_until = Some(until);
        until
    }

    /// Restore a persisted pause deadline (app startup). Callers are
    /// responsible for ignoring already-expired values.
    pub fn restore_pause_until(&self, until: DateTime<Utc>) {
        self.0.lock().pause_until = Some(until);
    }

    /// Clear any pause immediately ("Resume now").
    pub fn resume(&self) {
        self.0.lock().pause_until = None;
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
}
