#![forbid(unsafe_code)]
//! Caller-driven frame loop + timeline clock (pure, no threads/timers).
//!
//! SOURCE EVIDENCE (upstream, read-only):
//! - `packages/core/src/renderer.ts` `setFrameCallback`/`removeFrameCallback`/
//!   `clearFrameCallbacks` (4182-4194), `requestLive`/`dropLive` (4196-4233),
//!   `start`/`internalStart` (4235-4264), `suspend` (4260), `resume` (4302),
//!   `stop`/`internalStop` (4374-4390), `destroy` (4392-4406).
//! - `startRenderLoop` (4641-4660): resets clock/fps counters; a pending
//!   feed-idle retry defers the first `loop()` while staying "running".
//! - `loop()` (4662-4822): backpressure gate runs BEFORE the `_frameId` bump
//!   (skipped frames never increment); animation requests drain then
//!   `dropLive()` each; frame callbacks run in registration order, each
//!   isolated by try/catch (a throwing callback cannot skip later ones).
//! - `collectStatSample` (4872-4877) + `maxStatSamples = 300` (817): push and
//!   shift past the cap; `getStats` (4884+) reports avg/min/max over a copy.
//! - `packages/core/src/animation/Timeline.ts`: `Timeline` clock fields
//!   (`currentTime`, `isPlaying`, `isComplete`, `duration`, `loop`, 285-292),
//!   `update(deltaTime)` (457-496: clamp + complete when `!loop`, wrap with
//!   overshoot when `loop`), `play` (402), `pause` (416), rewind
//!   (`currentTime = 0`, 449); `TimelineEngine.attach`/`detach` registers one
//!   frame callback via `setFrameCallback`/`removeFrameCallback` and balances
//!   `requestLive`/`dropLive`.
//!
//! BOUNDARY: caller owns cadence and calls [`FrameLoop::step_frame`]; no
//! threads, no timers, no I/O. Stats ring bounded at [`MAX_STAT_SAMPLES`].
//! `ponytail:` no per-property animation items/easing — add when porting
//! `TimelineAnimationItem`/`evaluateItem`.

use std::collections::VecDeque;

/// Max retained per-frame samples (mirrors `maxStatSamples = 300`).
pub const MAX_STAT_SAMPLES: usize = 300;

/// Lifecycle of the loop (mirrors `_isRunning`/`_controlState`/`_isDestroyed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameState {
    Idle,
    Running,
    Suspended,
    Stopped,
    Destroyed,
}

/// What `step_frame` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    Rendered,
    Skipped,
}

/// Errors for illegal transitions/calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    AlreadyRunning,
    Suspended,
    Stopped,
    Destroyed,
}

/// Aggregate over retained frame-time samples (mirrors `getStats` avg/min/max).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameStats {
    pub samples: usize,
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
}

/// Scalar animation clock (mirrors `Timeline` time/loop/complete fields).
#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pub duration_ms: f64,
    pub current_ms: f64,
    pub playing: bool,
    pub complete: bool,
    pub looping: bool,
}

impl Timeline {
    pub fn new(duration_ms: f64) -> Self {
        Self { duration_ms, current_ms: 0.0, playing: false, complete: false, looping: false }
    }

    /// Mirrors `play()` (Timeline.ts:402).
    pub fn play(&mut self) {
        if self.complete && !self.looping {
            self.current_ms = 0.0;
            self.complete = false;
        }
        self.playing = true;
    }

    /// Mirrors `pause()` (Timeline.ts:416).
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Mirrors rewind (`currentTime = 0`, Timeline.ts:449).
    pub fn rewind(&mut self) {
        self.current_ms = 0.0;
        self.complete = false;
    }

    /// Mirrors `update(deltaTime)` (Timeline.ts:457-496).
    pub fn advance(&mut self, delta_ms: f64) {
        if !self.playing || self.complete {
            return;
        }
        self.current_ms += delta_ms;
        if self.looping {
            if self.current_ms >= self.duration_ms && self.duration_ms > 0.0 {
                self.current_ms %= self.duration_ms;
            }
        } else if self.current_ms >= self.duration_ms {
            self.current_ms = self.duration_ms;
            self.playing = false;
            self.complete = true;
        }
    }
}

/// Caller-driven frame loop (mirrors `CliRenderer` start/stop/suspend/resume +
/// `loop()` body ordering: backpressure gate, id bump, callbacks in order,
/// timeline advance, stats sample).
pub struct FrameLoop {
    state: FrameState,
    frame_id: u64,
    next_cb_id: u64,
    callbacks: Vec<(u64, Box<dyn FnMut(u64, f64)>)>,
    timelines: Vec<Timeline>,
    stats: VecDeque<f64>,
    backpressured: bool,
    live_requests: u32,
}

impl FrameLoop {
    pub fn new() -> Self {
        Self {
            state: FrameState::Idle,
            frame_id: 0,
            next_cb_id: 0,
            callbacks: Vec::new(),
            timelines: Vec::new(),
            stats: VecDeque::new(),
            backpressured: false,
            live_requests: 0,
        }
    }

    pub fn state(&self) -> FrameState {
        self.state
    }

    pub fn frame_id(&self) -> u64 {
        self.frame_id
    }

    /// Mirrors `start()`/`internalStart` (renderer.ts:4235-4264).
    pub fn start(&mut self) -> Result<(), FrameError> {
        match self.state {
            FrameState::Running => Err(FrameError::AlreadyRunning),
            FrameState::Destroyed => Err(FrameError::Destroyed),
            _ => {
                self.state = FrameState::Running;
                Ok(())
            }
        }
    }

    /// Mirrors `stop()`/`internalStop` (renderer.ts:4374-4390).
    pub fn stop(&mut self) {
        self.state = FrameState::Stopped;
    }

    /// Mirrors `suspend()` (renderer.ts:4260): running/idle both park.
    pub fn suspend(&mut self) {
        if self.state == FrameState::Destroyed {
            return;
        }
        self.state = FrameState::Suspended;
    }

    /// Mirrors `resume()`: parked loop returns to running.
    pub fn resume(&mut self) -> Result<(), FrameError> {
        match self.state {
            FrameState::Suspended => {
                self.state = FrameState::Running;
                Ok(())
            }
            FrameState::Destroyed => Err(FrameError::Destroyed),
            FrameState::Stopped => Err(FrameError::Stopped),
            _ => Ok(()),
        }
    }

    /// Mirrors `setFrameCallback` (renderer.ts:4182): returns callback id.
    pub fn add_callback(&mut self, cb: impl FnMut(u64, f64) + 'static) -> u64 {
        let id = self.next_cb_id;
        self.next_cb_id += 1;
        self.callbacks.push((id, Box::new(cb)));
        id
    }

    /// Mirrors `removeFrameCallback` (renderer.ts:4186). True if present.
    pub fn remove_callback(&mut self, id: u64) -> bool {
        let before = self.callbacks.len();
        self.callbacks.retain(|(cid, _)| *cid != id);
        self.callbacks.len() != before
    }

    /// Mirrors `clearFrameCallbacks` (renderer.ts:4190).
    pub fn clear_callbacks(&mut self) {
        self.callbacks.clear();
    }

    /// Mirrors `TimelineEngine.attach`: timeline advanced each step.
    /// Returns slot index for [`FrameLoop::detach_timeline`].
    pub fn attach_timeline(&mut self, tl: Timeline) -> usize {
        self.timelines.push(tl);
        self.timelines.len() - 1
    }

    /// Mirrors `TimelineEngine.detach`. True if slot held a timeline.
    pub fn detach_timeline(&mut self, slot: usize) -> bool {
        if slot < self.timelines.len() {
            self.timelines.remove(slot);
            true
        } else {
            false
        }
    }

    /// Mirrors `requestLive` (renderer.ts:4196): caller-scoped demand count.
    pub fn request_live(&mut self) {
        self.live_requests += 1;
    }

    /// Mirrors `dropLive` (renderer.ts:4213): saturates at zero.
    pub fn drop_live(&mut self) {
        self.live_requests = self.live_requests.saturating_sub(1);
    }

    pub fn live_requests(&self) -> u32 {
        self.live_requests
    }

    pub fn set_backpressured(&mut self, bp: bool) {
        self.backpressured = bp;
    }

    /// One caller-driven frame. Mirrors `loop()` ordering: suspended/stopped/
    /// destroyed are hard failures; backpressure short-circuits BEFORE the id
    /// bump (renderer.ts:4672); otherwise bump, run callbacks in registration
    /// order, advance timelines, record one bounded stats sample.
    pub fn step_frame(&mut self, delta_ms: f64) -> Result<StepOutcome, FrameError> {
        match self.state {
            FrameState::Suspended => return Err(FrameError::Suspended),
            FrameState::Stopped => return Err(FrameError::Stopped),
            FrameState::Destroyed => return Err(FrameError::Destroyed),
            _ => {}
        }
        if self.backpressured {
            return Ok(StepOutcome::Skipped);
        }
        self.frame_id += 1;
        let id = self.frame_id;
        for (_, cb) in self.callbacks.iter_mut() {
            cb(id, delta_ms);
        }
        for tl in self.timelines.iter_mut() {
            tl.advance(delta_ms);
        }
        self.stats.push_back(delta_ms);
        while self.stats.len() > MAX_STAT_SAMPLES {
            self.stats.pop_front();
        }
        Ok(StepOutcome::Rendered)
    }

    /// Mirrors `getStats` avg/min/max (renderer.ts:4884+).
    pub fn stats(&self) -> FrameStats {
        let n = self.stats.len();
        if n == 0 {
            return FrameStats { samples: 0, avg_ms: 0.0, min_ms: 0.0, max_ms: 0.0 };
        }
        let mut sum = 0.0;
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for &s in self.stats.iter() {
            sum += s;
            if s < min {
                min = s;
            }
            if s > max {
                max = s;
            }
        }
        FrameStats { samples: n, avg_ms: sum / n as f64, min_ms: min, max_ms: max }
    }

    pub fn stats_len(&self) -> usize {
        self.stats.len()
    }
}

impl Default for FrameLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn t01_start_running() {
        let mut l = FrameLoop::new();
        assert_eq!(l.state(), FrameState::Idle);
        assert!(l.start().is_ok());
        assert_eq!(l.state(), FrameState::Running);
    }

    #[test]
    fn t02_double_start_error() {
        let mut l = FrameLoop::new();
        assert!(l.start().is_ok());
        assert_eq!(l.start(), Err(FrameError::AlreadyRunning));
        assert_eq!(l.state(), FrameState::Running);
    }

    #[test]
    fn t03_suspend_resume_preserves_frame_id() {
        let mut l = FrameLoop::new();
        l.start().unwrap();
        l.step_frame(16.0).unwrap();
        l.step_frame(16.0).unwrap();
        assert_eq!(l.frame_id(), 2);
        l.suspend();
        assert_eq!(l.state(), FrameState::Suspended);
        assert_eq!(l.step_frame(16.0), Err(FrameError::Suspended));
        assert_eq!(l.frame_id(), 2);
        l.resume().unwrap();
        assert_eq!(l.state(), FrameState::Running);
        l.step_frame(16.0).unwrap();
        assert_eq!(l.frame_id(), 3);
    }

    #[test]
    fn t04_stop_stopped() {
        let mut l = FrameLoop::new();
        l.start().unwrap();
        l.step_frame(16.0).unwrap();
        l.stop();
        assert_eq!(l.state(), FrameState::Stopped);
        assert_eq!(l.step_frame(16.0), Err(FrameError::Stopped));
        // start after stop re-enters running (mirrors internalStart gate).
        assert!(l.start().is_ok());
        assert_eq!(l.state(), FrameState::Running);
    }

    #[test]
    fn t05_step_increments_callback_order() {
        let mut l = FrameLoop::new();
        let log: Rc<RefCell<Vec<(u64, u64)>>> = Rc::new(RefCell::new(Vec::new()));
        for tag in [1u64, 2, 3] {
            let log = Rc::clone(&log);
            l.add_callback(move |fid, _| log.borrow_mut().push((tag, fid)));
        }
        assert_eq!(l.step_frame(16.0), Ok(StepOutcome::Rendered));
        assert_eq!(l.step_frame(16.0), Ok(StepOutcome::Rendered));
        assert_eq!(l.frame_id(), 2);
        let log = log.borrow();
        assert_eq!(
            *log,
            vec![(1, 1), (2, 1), (3, 1), (1, 2), (2, 2), (3, 2)]
        );
    }

    #[test]
    fn t06_callback_removal() {
        let mut l = FrameLoop::new();
        let hits: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let h = Rc::clone(&hits);
        let id = l.add_callback(move |_, _| *h.borrow_mut() += 1);
        l.add_callback(|_, _| {});
        assert!(l.remove_callback(id));
        assert!(!l.remove_callback(id));
        l.step_frame(16.0).unwrap();
        assert_eq!(*hits.borrow(), 0);
        assert_eq!(l.frame_id(), 1);
    }

    #[test]
    fn t07_timeline_attach_detach() {
        let mut l = FrameLoop::new();
        let mut tl = Timeline::new(100.0);
        tl.play();
        let slot = l.attach_timeline(tl);
        l.step_frame(30.0).unwrap();
        l.step_frame(30.0).unwrap();
        assert!((l.timelines[slot].current_ms - 60.0).abs() < 1e-9);
        assert!(l.detach_timeline(slot));
        assert!(!l.detach_timeline(slot));
        l.step_frame(30.0).unwrap();
        assert!(l.timelines.is_empty());
        // completion clamp path (!loop): reaches duration, stops, flags complete.
        let mut t2 = Timeline::new(50.0);
        t2.play();
        t2.advance(60.0);
        assert!(t2.complete && !t2.playing);
        assert!((t2.current_ms - 50.0).abs() < 1e-9);
    }

    #[test]
    fn t08_stats_ring_cap() {
        let mut l = FrameLoop::new();
        for _ in 0..(MAX_STAT_SAMPLES + 50) {
            l.step_frame(16.0).unwrap();
        }
        assert_eq!(l.stats_len(), MAX_STAT_SAMPLES);
        let s = l.stats();
        assert_eq!(s.samples, MAX_STAT_SAMPLES);
        assert!((s.avg_ms - 16.0).abs() < 1e-9);
        assert!((s.min_ms - 16.0).abs() < 1e-9);
        assert!((s.max_ms - 16.0).abs() < 1e-9);
    }

    #[test]
    fn t09_backpressure_skip() {
        let mut l = FrameLoop::new();
        l.start().unwrap();
        let hits: Rc<RefCell<u32>> = Rc::new(RefCell::new(0));
        let h = Rc::clone(&hits);
        l.add_callback(move |_, _| *h.borrow_mut() += 1);
        l.set_backpressured(true);
        assert_eq!(l.step_frame(16.0), Ok(StepOutcome::Skipped));
        assert_eq!(l.frame_id(), 0);
        assert_eq!(*hits.borrow(), 0);
        assert_eq!(l.stats_len(), 0);
        l.set_backpressured(false);
        assert_eq!(l.step_frame(16.0), Ok(StepOutcome::Rendered));
        assert_eq!(l.frame_id(), 1);
        assert_eq!(*hits.borrow(), 1);
    }
}
