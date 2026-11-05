//! Periodic timers on a wall-clock grid.
//!
//! A timer on the grid of `every` ticks when the wall clock reads a whole
//! multiple of `every` since the Unix epoch: a 1 s timer at every second, a
//! 2 s timer at every even second, a 60 s timer at the top of every minute.
//! Timers whose periods divide each other share ticks, so they fire in one
//! wakeup, and so do the timers of processes on machines whose clocks agree.
//! On a battery-powered machine a few wakeups cost far less than as many
//! spread out.
//!
//! Nothing is registered and there is no shared state beyond the reference
//! below: each function only computes where its period's ticks fall. Timers
//! line up when their periods divide each other, so pick periods such as
//! 250 ms, 1 s, 2 s, 10 s and 60 s.
//!
//! The wall clock is read once, when the grid is first used; it moves with
//! the monotonic clock after that, so a step of the wall clock doesn't move
//! it, and can't make a timer fire twice or skip.
//!
//! ```
//! use std::time::{Duration, Instant};
//!
//! let now = Instant::now();
//! let tick = timer_grid::next(Duration::from_secs(2), now);
//! assert!(tick > now && tick <= now + Duration::from_secs(2));
//! ```

use std::{
    sync::LazyLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// An instant, and the wall clock at it, in ns since the Unix epoch.
static REFERENCE: LazyLock<(Instant, u128)> = LazyLock::new(|| {
    let wall = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (Instant::now(), wall.as_nanos())
});

/// The wall clock at `t`, in ns since the Unix epoch.
fn wall_ns(t: Instant) -> u128 {
    let (at, wall) = *REFERENCE;
    match t.checked_duration_since(at) {
        Some(d) => wall + d.as_nanos(),
        None => wall.saturating_sub(at.duration_since(t).as_nanos()),
    }
}

/// The first tick of the grid of `every` after `after`. A tick is not after
/// itself: given one, this is the next.
pub fn next(every: Duration, after: Instant) -> Instant {
    let every_ns = every.as_nanos().max(1);
    after + Duration::from_nanos((every_ns - wall_ns(after) % every_ns) as u64)
}

#[cfg(feature = "tokio")]
mod tokio_timers {
    use super::next;
    use std::time::Duration;
    use tokio::time::{self, Instant, Interval, MissedTickBehavior};

    /// An interval on the grid of `every`, first ticking at the first tick
    /// after `start`. Ticks missed, while it wasn't polled or was slow, are
    /// skipped, so it stays on the grid.
    ///
    /// For an idle timeout, which acts when nothing happened between two
    /// ticks, start it a period out, `interval_at(now + every, every)`, so
    /// its first tick is more than a period away.
    pub fn interval_at(start: Instant, every: Duration) -> Interval {
        let mut i = time::interval_at(next(every, start.into_std()).into(), every);
        i.set_missed_tick_behavior(MissedTickBehavior::Skip);
        i
    }

    /// An interval on the grid of `every`, first ticking at its next tick.
    pub fn interval(every: Duration) -> Interval {
        interval_at(Instant::now(), every)
    }

    /// Sleep until the next tick of the grid of `every`.
    pub fn sleep(every: Duration) -> time::Sleep {
        time::sleep_until(next(every, Instant::now().into_std()).into())
    }
}

#[cfg(feature = "tokio")]
pub use tokio_timers::{interval, interval_at, sleep};

#[cfg(test)]
mod test {
    use super::*;

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn ticks_fall_on_whole_multiples_of_the_period_in_wall_time() {
        let now = Instant::now();
        let t = next(SECOND, now);
        assert!(t > now && t <= now + SECOND);
        assert_eq!(wall_ns(t) % SECOND.as_nanos(), 0);
        assert_eq!(next(SECOND, t), t + SECOND, "a tick is not after itself");
        let odd = Duration::from_millis(1300);
        assert_eq!(wall_ns(next(odd, now)) % odd.as_nanos(), 0);
    }

    #[test]
    fn a_grid_whose_period_divides_anothers_ticks_with_it() {
        let now = Instant::now();
        let even = next(2 * SECOND, now);
        let quarter = Duration::from_millis(250);
        let mut t = next(quarter, now);
        while t < even {
            t = next(quarter, t);
        }
        assert_eq!(t, even);
        let minute = next(60 * SECOND, now);
        assert_eq!(next(2 * SECOND, minute - Duration::from_millis(1)), minute);
    }

    #[test]
    fn an_instant_before_the_grid_was_first_used_has_ticks_too() {
        let before = REFERENCE.0 - Duration::from_millis(300);
        let t = next(SECOND, before);
        assert!(t > before && t <= before + SECOND);
        assert_eq!(wall_ns(t) % SECOND.as_nanos(), 0);
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn an_interval_first_ticks_on_the_grid_after_its_start() {
        let every = Duration::from_millis(20);
        let start = tokio::time::Instant::now() + Duration::from_millis(50);
        let mut i = interval_at(start, every);
        let first = i.tick().await;
        assert!(first > start);
        assert_eq!(wall_ns(first.into_std()) % every.as_nanos(), 0);
        let second = i.tick().await;
        assert_eq!(second, first + every);
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn sleep_wakes_on_the_next_tick() {
        let every = Duration::from_millis(200);
        sleep(every).await;
        // Woken at a tick, or a little after on a busy machine, not before
        // one, which would leave it most of a period past the last.
        let late = wall_ns(Instant::now()) % every.as_nanos();
        assert!(late < every.as_nanos() / 2, "{late} ns past the tick");
    }
}
