//! Everything about time: frame timing, game-time scheduling and simple timers.
//!
//! There are two kinds of time in the engine:
//!
//! * **Game time** ([`Time`], reachable as `ctx.time`). It advances once per frame, can be
//!   slowed down or paused with [`Time::set_scale`], and drives the scheduler
//!   ([`Time::after`], [`Time::every`], [`Time::fired`]).
//! * **Real time** ([`Timer`], [`Cooldown`], [`Interval`]). These are standalone objects you
//!   keep in your own structs. They measure wall clock time and work without a context.

use std::time::{Duration, Instant};

/// Frame timing and a simple named-event scheduler. Available as `ctx.time`.
///
/// ```no_run
/// # use audio_code::prelude::*;
/// # fn f(ctx: &mut Context) {
/// // Fire an event called "spawn" every 2.5 seconds of game time.
/// ctx.time.every("spawn", 2.5);
/// // ...later, in update():
/// if ctx.time.fired("spawn") {
///     ctx.speech.speak("A new enemy appears");
/// }
/// # }
/// ```
#[derive(Debug)]
pub struct Time {
    start: Instant,
    last_frame: Option<Instant>,
    delta: f32,
    raw_delta: f32,
    elapsed: f64,
    frame: u64,
    scale: f32,
    max_delta: f32,
    fps: f32,
    schedule: Vec<Scheduled>,
    fired: Vec<String>,
}

#[derive(Debug)]
struct Scheduled {
    name: String,
    at: f64,
    repeat: Option<f64>,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub(crate) fn new() -> Self {
        Self {
            start: Instant::now(),
            last_frame: None,
            delta: 0.0,
            raw_delta: 0.0,
            elapsed: 0.0,
            frame: 0,
            scale: 1.0,
            max_delta: 0.25,
            fps: 0.0,
            schedule: Vec::new(),
            fired: Vec::new(),
        }
    }

    /// Advances the clock by one frame. Called by the engine.
    pub(crate) fn advance(&mut self, now: Instant) {
        let raw = match self.last_frame {
            Some(last) => now.duration_since(last).as_secs_f32(),
            None => 0.0,
        };
        self.last_frame = Some(now);
        // Clamp huge hitches (e.g. a breakpoint or a dragged window) so physics does not explode.
        self.raw_delta = raw.min(self.max_delta);
        self.delta = self.raw_delta * self.scale;
        self.elapsed += self.delta as f64;
        self.frame += 1;
        if raw > 0.0 {
            // Exponential moving average keeps the number readable.
            let current = 1.0 / raw;
            self.fps = if self.fps == 0.0 { current } else { self.fps * 0.9 + current * 0.1 };
        }

        self.fired.clear();
        let now = self.elapsed;
        let mut i = 0;
        while i < self.schedule.len() {
            if self.schedule[i].at <= now {
                self.fired.push(self.schedule[i].name.clone());
                match self.schedule[i].repeat {
                    Some(every) => self.schedule[i].at += every,
                    None => {
                        self.schedule.swap_remove(i);
                        continue;
                    }
                }
            }
            i += 1;
        }
    }

    /// Seconds of game time since the previous frame (affected by [`Time::set_scale`]).
    ///
    /// Multiply speeds by this to make movement frame rate independent:
    /// `position.x += speed * ctx.time.delta();`
    pub fn delta(&self) -> f32 {
        self.delta
    }

    /// Like [`Time::delta`] but ignores the time scale. Useful for menus while the game is paused.
    pub fn real_delta(&self) -> f32 {
        self.raw_delta
    }

    /// Seconds of game time since the game started.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Wall clock time since the engine started, ignoring the time scale.
    pub fn real_elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Number of frames since the game started.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Current frames per second (smoothed).
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Current time scale. 1.0 is normal speed.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Speeds up (> 1.0), slows down (< 1.0) or pauses (0.0) game time.
    /// Physics, the scheduler and [`Time::delta`] all follow the scale.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.0);
    }

    /// Pauses game time. Same as `set_scale(0.0)`.
    pub fn pause(&mut self) {
        self.scale = 0.0;
    }

    /// Resumes game time at normal speed.
    pub fn resume(&mut self) {
        self.scale = 1.0;
    }

    /// Whether game time is paused.
    pub fn is_paused(&self) -> bool {
        self.scale == 0.0
    }

    /// Schedules a one-shot event named `name` that fires after `seconds` of game time.
    /// Scheduling a name that is already pending replaces it.
    pub fn after(&mut self, name: impl Into<String>, seconds: f32) {
        let name = name.into();
        self.cancel(&name);
        self.schedule.push(Scheduled { at: self.elapsed + seconds as f64, name, repeat: None });
    }

    /// Schedules a repeating event named `name` that fires every `seconds` of game time.
    /// Scheduling a name that is already pending replaces it.
    pub fn every(&mut self, name: impl Into<String>, seconds: f32) {
        let name = name.into();
        let seconds = (seconds as f64).max(0.001);
        self.cancel(&name);
        self.schedule.push(Scheduled { at: self.elapsed + seconds, name, repeat: Some(seconds) });
    }

    /// Cancels a scheduled event. Does nothing if it does not exist.
    pub fn cancel(&mut self, name: &str) {
        self.schedule.retain(|s| s.name != name);
    }

    /// Whether an event with this name is waiting to fire.
    pub fn is_scheduled(&self, name: &str) -> bool {
        self.schedule.iter().any(|s| s.name == name)
    }

    /// Seconds until the named event fires, or `None` if it is not scheduled.
    pub fn remaining(&self, name: &str) -> Option<f32> {
        self.schedule.iter().find(|s| s.name == name).map(|s| (s.at - self.elapsed).max(0.0) as f32)
    }

    /// Returns `true` during the frame in which the named event fired.
    pub fn fired(&self, name: &str) -> bool {
        self.fired.iter().any(|n| n == name)
    }

    /// All events that fired this frame.
    pub fn fired_events(&self) -> impl Iterator<Item = &str> {
        self.fired.iter().map(String::as_str)
    }
}

/// A stopwatch measuring real time. Can be paused, resumed and restarted.
///
/// ```
/// use audio_code::time::Timer;
/// let mut timer = Timer::new();
/// // ...
/// if timer.elapsed_secs() > 3.0 {
///     timer.restart();
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Timer {
    started: Instant,
    offset: Duration,
    paused_at: Option<Instant>,
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

impl Timer {
    /// Creates a running timer starting at zero.
    pub fn new() -> Self {
        Self { started: Instant::now(), offset: Duration::ZERO, paused_at: None }
    }

    /// Time measured so far.
    pub fn elapsed(&self) -> Duration {
        let end = self.paused_at.unwrap_or_else(Instant::now);
        end.duration_since(self.started) + self.offset
    }

    /// Time measured so far in seconds.
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed().as_secs_f32()
    }

    /// Time measured so far in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }

    /// Whether at least `seconds` have passed.
    pub fn has_elapsed(&self, seconds: f32) -> bool {
        self.elapsed_secs() >= seconds
    }

    /// Resets to zero and keeps running (also unpauses).
    pub fn restart(&mut self) {
        *self = Self::new();
    }

    /// Sets the measured time to `seconds` without changing the paused state.
    pub fn set(&mut self, seconds: f32) {
        let now = Instant::now();
        self.started = now;
        self.offset = Duration::from_secs_f32(seconds.max(0.0));
        if self.paused_at.is_some() {
            self.paused_at = Some(now);
        }
    }

    /// Stops counting until [`Timer::resume`] is called.
    pub fn pause(&mut self) {
        if self.paused_at.is_none() {
            self.paused_at = Some(Instant::now());
        }
    }

    /// Continues counting after a pause.
    pub fn resume(&mut self) {
        if let Some(paused) = self.paused_at.take() {
            self.started += Instant::now().duration_since(paused);
        }
    }

    /// Whether the timer is paused.
    pub fn is_paused(&self) -> bool {
        self.paused_at.is_some()
    }
}

/// Limits how often something can happen, e.g. firing a weapon.
///
/// ```
/// use audio_code::time::Cooldown;
/// let mut gun = Cooldown::new(0.5);
/// if gun.try_use() {
///     // fire! The next try_use() returns false for half a second.
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Cooldown {
    length: f32,
    timer: Timer,
    used: bool,
}

impl Cooldown {
    /// Creates a cooldown of `seconds` that is ready immediately.
    pub fn new(seconds: f32) -> Self {
        Self { length: seconds, timer: Timer::new(), used: false }
    }

    /// Whether the cooldown has run out.
    pub fn ready(&self) -> bool {
        !self.used || self.timer.has_elapsed(self.length)
    }

    /// If ready, starts the cooldown and returns `true`. Otherwise returns `false`.
    pub fn try_use(&mut self) -> bool {
        if self.ready() {
            self.trigger();
            true
        } else {
            false
        }
    }

    /// Starts the cooldown regardless of whether it was ready.
    pub fn trigger(&mut self) {
        self.used = true;
        self.timer.restart();
    }

    /// Makes the cooldown ready immediately.
    pub fn reset(&mut self) {
        self.used = false;
    }

    /// Seconds until ready (0.0 if ready).
    pub fn remaining(&self) -> f32 {
        if self.ready() { 0.0 } else { self.length - self.timer.elapsed_secs() }
    }

    /// Changes the cooldown length.
    pub fn set_length(&mut self, seconds: f32) {
        self.length = seconds;
    }
}

/// Fires at a fixed real-time rate. Great for footsteps, radar beeps or clocks.
///
/// ```
/// use audio_code::time::Interval;
/// let mut steps = Interval::new(0.4);
/// // in update():
/// if steps.tick() {
///     // play a footstep
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Interval {
    every: f32,
    timer: Timer,
}

impl Interval {
    /// Creates an interval that fires every `seconds`. The first tick happens after `seconds`.
    pub fn new(seconds: f32) -> Self {
        Self { every: seconds.max(0.001), timer: Timer::new() }
    }

    /// Returns `true` once each time the interval elapses. Call it every frame.
    /// If several intervals were missed only one tick is reported.
    pub fn tick(&mut self) -> bool {
        let elapsed = self.timer.elapsed_secs();
        if elapsed >= self.every {
            // Keep the remainder so the rhythm stays steady.
            self.timer.set((elapsed - self.every).min(self.every));
            true
        } else {
            false
        }
    }

    /// Restarts counting from zero.
    pub fn reset(&mut self) {
        self.timer.restart();
    }

    /// Changes how often the interval fires.
    pub fn set_interval(&mut self, seconds: f32) {
        self.every = seconds.max(0.001);
    }

    /// Current interval length in seconds.
    pub fn interval(&self) -> f32 {
        self.every
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_fires_once_and_repeats() {
        let mut t = Time::new();
        let base = Instant::now();
        t.advance(base);
        t.after("once", 0.1);
        t.every("rep", 0.05);
        t.advance(base + Duration::from_millis(60));
        assert!(!t.fired("once"));
        assert!(t.fired("rep"));
        t.advance(base + Duration::from_millis(120));
        assert!(t.fired("once"));
        assert!(t.fired("rep"));
        assert!(!t.is_scheduled("once"));
        assert!(t.is_scheduled("rep"));
    }

    #[test]
    fn scale_pauses_game_time() {
        let mut t = Time::new();
        let base = Instant::now();
        t.advance(base);
        t.pause();
        t.advance(base + Duration::from_millis(100));
        assert_eq!(t.delta(), 0.0);
        assert!(t.real_delta() > 0.0);
    }

    #[test]
    fn cooldown_blocks() {
        let mut c = Cooldown::new(10.0);
        assert!(c.try_use());
        assert!(!c.try_use());
        c.reset();
        assert!(c.ready());
    }
}
