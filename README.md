# timer-grid

Periodic timers on a wall-clock grid, so that timers due together fire in one
wakeup, within a process and across machines.

A timer on the grid of `every` ticks when the wall clock reads a whole multiple
of `every` since the Unix epoch: a 1 s timer at every second, a 2 s timer at
every even second, a 60 s timer at the top of every minute. Timers whose
periods divide each other share ticks. A process with a heartbeat per
connection, a load sample every 2 s and a save every minute wakes once a
second instead of once per timer, each on its own phase. Machines whose clocks
agree (NTP keeps ordinary ones within milliseconds to tens of ms) tick together
too, so a heartbeat one sends on its tick reaches the others as they wake for
theirs.

On a battery-powered machine, each wakeup brings a CPU out of a deep idle
state; a few wakeups cost far less than as many spread out. This is the timer
coalescing operating systems do with timer slack, done at the application
level where the periods are known.

```rust
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Ticks at every even second of the wall clock.
    let mut every_2s = timer_grid::interval(Duration::from_secs(2));
    loop {
        every_2s.tick().await;
        // ...
    }
}
```

- `next(every, after)`: the first tick of the grid of `every` after `after`,
  for a `std::time::Instant`. Needs no runtime.
- `interval(every)`, `interval_at(start, every)`: a tokio `Interval` on the
  grid, first ticking at the first tick after now or `start`. Missed ticks are
  skipped, so it stays on the grid. For an idle timeout, start it a period out
  (`interval_at(now + every, every)`) so its first tick is more than a period
  away.
- `sleep(every)`: sleep until the next tick.

The tokio functions are behind the default `tokio` feature.

Nothing is registered and there is no shared state beyond one reference
reading: each call only computes where its period's ticks fall. Timers line up
when their periods divide each other, so pick periods such as 250 ms, 1 s,
2 s, 10 s and 60 s. The wall clock is read once, when the grid is first used,
and moves with the monotonic clock after that, so a step of the wall clock
can't make a timer fire twice or skip.

License: MIT
