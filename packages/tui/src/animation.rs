//! Component-level animation primitives.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::widget::WidgetPath;

const VALUE_EPSILON: f64 = 0.000_001;

/// Animation timing and easing configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationSpec {
    pub duration: Duration,
    pub easing: Easing,
}

impl AnimationSpec {
    pub fn new(duration: Duration, easing: Easing) -> Self {
        Self { duration, easing }
    }
}

impl Default for AnimationSpec {
    fn default() -> Self {
        Self {
            duration: Duration::from_millis(180),
            easing: Easing::EaseOut,
        }
    }
}

/// Built-in easing curves for simple component animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

impl Easing {
    pub fn sample(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - ((-2.0 * t + 2.0).powi(2) / 2.0)
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AnimationKey {
    path: WidgetPath,
    channel: String,
}

impl AnimationKey {
    fn new(path: &WidgetPath, channel: &str) -> Self {
        Self {
            path: path.clone(),
            channel: channel.to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
struct ValueAnimation {
    displayed: f64,
    start: f64,
    target: f64,
    started_at: Instant,
    spec: AnimationSpec,
    active: bool,
}

impl ValueAnimation {
    fn new(target: f64, spec: AnimationSpec, now: Instant) -> Self {
        Self {
            displayed: target,
            start: target,
            target,
            started_at: now,
            spec,
            active: false,
        }
    }

    fn current_at(&self, now: Instant) -> f64 {
        if !self.active || nearly_equal(self.displayed, self.target) {
            return self.target;
        }

        let duration = self.spec.duration;
        if duration.is_zero() {
            return self.target;
        }

        let raw_progress =
            now.saturating_duration_since(self.started_at).as_secs_f64() / duration.as_secs_f64();
        if raw_progress >= 1.0 {
            return self.target;
        }

        let eased = self.spec.easing.sample(raw_progress);
        self.start + (self.target - self.start) * eased
    }

    fn update_to(&mut self, target: f64, spec: AnimationSpec, now: Instant) -> bool {
        let before = self.displayed;
        let current = self.current_at(now);
        self.displayed = current;

        if !nearly_equal(target, self.target) {
            self.start = current;
            self.target = target;
            self.started_at = now;
            self.spec = spec;
            self.active = !nearly_equal(current, target) && !spec.duration.is_zero();
            if spec.duration.is_zero() {
                self.displayed = target;
            }
            return true;
        }

        if self.active {
            if nearly_equal(self.displayed, self.target)
                || now.saturating_duration_since(self.started_at) >= self.spec.duration
            {
                self.displayed = self.target;
                self.active = false;
            }

            return true;
        }

        !nearly_equal(before, self.displayed)
    }
}

#[derive(Debug, Clone)]
struct PulseAnimation {
    value: f64,
    last_tick: Instant,
}

impl PulseAnimation {
    fn new(now: Instant) -> Self {
        Self {
            value: 0.0,
            last_tick: now,
        }
    }

    fn tick(&mut self, interval: Duration, now: Instant) {
        if interval.is_zero() {
            self.value += 1.0;
            self.last_tick = now;
            return;
        }

        let elapsed = now.saturating_duration_since(self.last_tick);
        let steps = elapsed.as_nanos() / interval.as_nanos();
        if steps > 0 {
            self.value += steps as f64;
            self.last_tick += interval.saturating_mul(steps.min(u32::MAX as u128) as u32);
        }
    }
}

/// Stores component animation state keyed by widget path and named channel.
#[derive(Debug, Default)]
pub struct AnimationStore {
    values: HashMap<AnimationKey, ValueAnimation>,
    pulses: HashMap<AnimationKey, PulseAnimation>,
}

impl AnimationStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self, path: &WidgetPath, channel: &str) -> Option<f64> {
        let key = AnimationKey::new(path, channel);
        self.values
            .get(&key)
            .map(|state| state.displayed)
            .or_else(|| self.pulses.get(&key).map(|state| state.value))
    }

    fn track_value(
        &mut self,
        path: &WidgetPath,
        channel: &str,
        target: f64,
        spec: AnimationSpec,
        now: Instant,
    ) -> bool {
        let key = AnimationKey::new(path, channel);
        self.values
            .entry(key)
            .or_insert_with(|| ValueAnimation::new(target, spec, now))
            .update_to(target, spec, now)
    }

    fn pulse(
        &mut self,
        path: &WidgetPath,
        channel: &str,
        interval: Duration,
        now: Instant,
    ) -> bool {
        let key = AnimationKey::new(path, channel);
        self.pulses
            .entry(key)
            .or_insert_with(|| PulseAnimation::new(now))
            .tick(interval, now);
        true
    }

    /// Remove animation channels whose widget path no longer exists.
    pub fn retain_active<F>(&mut self, mut is_active: F)
    where
        F: FnMut(&WidgetPath) -> bool,
    {
        self.values.retain(|key, _| is_active(&key.path));
        self.pulses.retain(|key, _| is_active(&key.path));
    }
}

/// Mutable context passed to a widget's animation hook.
pub struct AnimationCtx<'a> {
    store: &'a mut AnimationStore,
    path: WidgetPath,
    focused_path: Option<&'a WidgetPath>,
    now: Instant,
}

impl<'a> AnimationCtx<'a> {
    pub fn new(
        store: &'a mut AnimationStore,
        path: WidgetPath,
        focused_path: Option<&'a WidgetPath>,
        now: Instant,
    ) -> Self {
        Self {
            store,
            path,
            focused_path,
            now,
        }
    }

    pub fn path(&self) -> &WidgetPath {
        &self.path
    }

    pub fn now(&self) -> Instant {
        self.now
    }

    pub fn is_focused(&self) -> bool {
        self.focused_path
            .map(|path| path == &self.path)
            .unwrap_or(false)
    }

    pub fn track_value(&mut self, channel: &str, target: f64, spec: AnimationSpec) -> bool {
        self.store
            .track_value(&self.path, channel, target, spec, self.now)
    }

    pub fn pulse(&mut self, channel: &str, interval: Duration) -> bool {
        self.store.pulse(&self.path, channel, interval, self.now)
    }
}

fn nearly_equal(a: f64, b: f64) -> bool {
    (a - b).abs() <= VALUE_EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> WidgetPath {
        WidgetPath::root().child("progress")
    }

    #[test]
    fn easing_outputs_boundaries() {
        for easing in [
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
            Easing::EaseInOut,
        ] {
            assert_eq!(easing.sample(0.0), 0.0);
            assert_eq!(easing.sample(1.0), 1.0);
            let mid = easing.sample(0.5);
            assert!((0.0..=1.0).contains(&mid));
        }
    }

    #[test]
    fn retargets_from_current_displayed_value() {
        let mut store = AnimationStore::new();
        let path = path();
        let start = Instant::now();
        let spec = AnimationSpec::new(Duration::from_millis(100), Easing::Linear);

        assert!(!store.track_value(&path, "value", 0.0, spec, start));
        assert!(store.track_value(&path, "value", 1.0, spec, start));
        assert!(store.track_value(&path, "value", 0.5, spec, start + Duration::from_millis(50)));

        let displayed = store.value(&path, "value").unwrap();
        assert!((displayed - 0.5).abs() < 0.001);
    }

    #[test]
    fn completed_animation_stops_requesting_redraw() {
        let mut store = AnimationStore::new();
        let path = path();
        let start = Instant::now();
        let spec = AnimationSpec::new(Duration::from_millis(100), Easing::Linear);

        store.track_value(&path, "value", 0.0, spec, start);
        assert!(store.track_value(&path, "value", 1.0, spec, start));
        assert!(store.track_value(
            &path,
            "value",
            1.0,
            spec,
            start + Duration::from_millis(100)
        ));
        assert!(!store.track_value(
            &path,
            "value",
            1.0,
            spec,
            start + Duration::from_millis(120)
        ));
    }

    #[test]
    fn inactive_paths_are_pruned() {
        let mut store = AnimationStore::new();
        let path = path();
        let now = Instant::now();

        store.track_value(&path, "value", 1.0, AnimationSpec::default(), now);
        store.pulse(&path, "spinner", Duration::from_millis(80), now);
        store.retain_active(|_| false);

        assert!(store.value(&path, "value").is_none());
        assert!(store.value(&path, "spinner").is_none());
    }
}
