//! Visual post-processing effects for arbitrary widgets.
//!
//! This wrapper renders its child into an offscreen buffer, then delegates
//! terminal-cell post-processing to the internal visual engine.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::animation::{
    AnimationSpec, Direction as AnimationDirection, Easing, InitialAnimation, Presence, Repeat,
    Timeline, Transition, TransitionEffect,
};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::offscreen::with_reused_offscreen;
use crate::style::{Color, EffectSlot, Theme};
use crate::visual_engine::{
    blit_with_effect_groups, blit_with_effects, lifecycle_progress, stable_seed,
    ResolvedEffectGroup, ResolvedVisualConfig,
};
pub use crate::visual_engine::{
    BlurMode, CellEffect, CellSample, CustomCellEffect, DissolveMode, GradientDirection,
    GradientTarget, LargeAreaPolicy, StaggerMode, TerminalVisualCapabilities, TypewriterMode,
    VisualAnchor, VisualConfig, VisualCtx, VisualDegradation, VisualEffect, VisualEffectCost,
    VisualPerformanceConfig, VisualProfile, WaveAxis, WipeDirection, WipeMode,
};
use crate::widget::{IntoWidget, RenderCtx, Widget, WidgetKey};

type ProfileHook = Arc<dyn Fn(VisualProfile) + Send + Sync + 'static>;

/// Wrap a widget with terminal-cell visual effects.
pub fn visual<M>(child: impl IntoWidget<M>) -> Visual<M> {
    Visual::new(child)
}

/// Generic visual post-processing wrapper.
pub struct Visual<M = ()> {
    child: Box<dyn Widget<M>>,
    effects: Vec<VisualEffect>,
    animation: Option<AnimationSpec>,
    progress: Option<f64>,
    channel: String,
    widget_key: Option<String>,
    seed: Option<u64>,
    config: VisualConfig,
    profile_hook: Option<ProfileHook>,
    effect_preset: Option<String>,
    presence: Presence,
    pub(crate) enter_effect: Option<LifecycleVisualEffect>,
    pub(crate) exit_effect: Option<LifecycleVisualEffect>,
}

impl<M> std::fmt::Debug for Visual<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Visual")
            .field("effects", &self.effects)
            .field("animation", &self.animation)
            .field("progress", &self.progress)
            .field("channel", &self.channel)
            .field("seed", &self.seed)
            .field("config", &self.config)
            .field("profile_hook", &self.profile_hook.is_some())
            .field("effect_preset", &self.effect_preset)
            .field("presence", &self.presence)
            .field("enter_effect", &self.enter_effect)
            .field("exit_effect", &self.exit_effect)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LifecycleVisualEffect {
    Effect(VisualEffect),
    Theme(EffectSlot),
}

impl LifecycleVisualEffect {
    pub(crate) fn resolve(&self, theme: &Theme) -> VisualEffect {
        match self {
            Self::Effect(effect) => effect.clone(),
            Self::Theme(slot) => theme.effects.get(*slot),
        }
    }
}

impl<M> Visual<M> {
    pub fn new(child: impl IntoWidget<M>) -> Self {
        Self {
            child: child.into_widget(),
            effects: Vec::new(),
            animation: None,
            progress: None,
            channel: "visual".to_owned(),
            widget_key: None,
            seed: None,
            config: VisualConfig::default(),
            profile_hook: None,
            effect_preset: None,
            presence: Presence::default(),
            enter_effect: None,
            exit_effect: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn effect(mut self, effect: VisualEffect) -> Self {
        self.effects.push(effect);
        self
    }

    /// Add a user-defined cell effect to the built-in effect pipeline.
    pub fn custom_effect(mut self, effect: impl CellEffect) -> Self {
        self.effects.push(VisualEffect::custom(effect));
        self
    }

    pub fn effects<I>(mut self, effects: I) -> Self
    where
        I: IntoIterator<Item = VisualEffect>,
    {
        self.effects.extend(effects);
        self
    }

    /// Override visual defaults for this wrapper.
    ///
    /// Local config wins over theme defaults. For example:
    ///
    /// ```no_run
    /// # use tui::prelude::*;
    /// let widget = visual(label::<()>("demo"))
    ///     .config(VisualConfig::default().cell_aspect(0.5));
    /// ```
    pub fn config(mut self, config: VisualConfig) -> Self {
        self.config = config;
        self
    }

    /// Convenience for overriding only the terminal cell aspect.
    pub fn cell_aspect(mut self, cell_aspect: f64) -> Self {
        self.config = self.config.cell_aspect(cell_aspect);
        self
    }

    /// Override the performance strategy for this wrapper.
    pub fn performance(mut self, performance: VisualPerformanceConfig) -> Self {
        self.config = self.config.performance(performance);
        self
    }

    /// Install an optional profiling hook for this wrapper.
    ///
    /// Profiling is entirely opt-in; when no hook is set the render path avoids
    /// timing and callback work.
    pub fn profile(mut self, hook: impl Fn(VisualProfile) + Send + Sync + 'static) -> Self {
        self.profile_hook = Some(Arc::new(hook));
        self
    }

    pub fn fade_in(self) -> Self {
        self.effect(VisualEffect::fade_in())
    }

    pub fn fade_out(self) -> Self {
        self.effect(VisualEffect::fade_out())
    }

    pub fn gradient(self, start: Color, end: Color, direction: GradientDirection) -> Self {
        self.effect(VisualEffect::gradient(start, end, direction))
    }

    pub fn shatter(self) -> Self {
        self.effect(VisualEffect::shatter())
    }

    pub fn magic_lamp(self, anchor: VisualAnchor) -> Self {
        self.effect(VisualEffect::magic_lamp(anchor))
    }

    pub fn wipe(self, direction: WipeDirection) -> Self {
        self.effect(VisualEffect::wipe(direction))
    }

    pub fn reveal(self, direction: WipeDirection) -> Self {
        self.effect(VisualEffect::reveal(direction))
    }

    pub fn dissolve(self) -> Self {
        self.effect(VisualEffect::dissolve())
    }

    pub fn wave(self, axis: WaveAxis) -> Self {
        self.effect(VisualEffect::wave(axis))
    }

    pub fn glitch(self) -> Self {
        self.effect(VisualEffect::glitch())
    }

    pub fn scanline(self) -> Self {
        self.effect(VisualEffect::scanline())
    }

    pub fn typewriter(self) -> Self {
        self.effect(VisualEffect::typewriter())
    }

    pub fn blur_like(self) -> Self {
        self.effect(VisualEffect::blur_like())
    }

    pub fn highlight_sweep(self) -> Self {
        self.effect(VisualEffect::highlight_sweep())
    }

    pub fn sparkle(self) -> Self {
        self.effect(VisualEffect::sparkle())
    }

    /// Drive effect progress with a one-shot animation from 0.0 to 1.0.
    pub fn animation(mut self, spec: AnimationSpec) -> Self {
        self.animation = Some(spec);
        self.progress = None;
        self
    }

    /// Drive effect progress manually.
    pub fn progress(mut self, progress: f64) -> Self {
        self.progress = Some(progress.clamp(0.0, 1.0));
        self.animation = None;
        self
    }

    /// Use a stable animation channel. Changing the channel starts a fresh
    /// animation without changing widget identity.
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = channel.into();
        self
    }

    /// Override the stable seed used by visual sampling.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Select a named effect preset for future theme-level effect resolution.
    pub fn effect_preset(mut self, preset: impl Into<String>) -> Self {
        self.effect_preset = Some(preset.into());
        self
    }

    /// Play a visual effect automatically when this widget first appears.
    pub fn enter(mut self, effect: VisualEffect) -> Self {
        self.enter_effect = Some(LifecycleVisualEffect::Effect(effect));
        self.presence = self.presence.enter(visual_enter_timeline());
        self.presence.initial = InitialAnimation::Play;
        self
    }

    /// Play a visual effect automatically after this widget leaves the tree.
    pub fn exit(mut self, effect: VisualEffect) -> Self {
        self.exit_effect = Some(LifecycleVisualEffect::Effect(effect));
        self.presence = self.presence.exit(visual_exit_timeline());
        self
    }

    /// Resolve the enter effect from the active theme during rendering.
    pub fn enter_theme(mut self, slot: EffectSlot) -> Self {
        self.enter_effect = Some(LifecycleVisualEffect::Theme(slot));
        self.presence = self.presence.enter(visual_enter_timeline());
        self.presence.initial = InitialAnimation::Play;
        self
    }

    /// Resolve the exit effect from the active theme during rendering.
    pub fn exit_theme(mut self, slot: EffectSlot) -> Self {
        self.exit_effect = Some(LifecycleVisualEffect::Theme(slot));
        self.presence = self.presence.exit(visual_exit_timeline());
        self
    }

    fn resolve_progress(&self, ctx: &RenderCtx) -> f64 {
        if let Some(progress) = self.progress {
            return progress;
        }

        let Some(spec) = self.animation else {
            return 1.0;
        };

        let transition = Transition::new(TransitionEffect::BorderEmphasis, spec);
        let frames = ctx.track_timeline(&self.channel, Timeline::single(transition), false);
        frames.last().map(|frame| frame.progress).unwrap_or(1.0)
    }

    pub(crate) fn resolve_config(&self, theme: &Theme) -> ResolvedVisualConfig {
        ResolvedVisualConfig {
            cell_aspect: self
                .config
                .cell_aspect_override()
                .unwrap_or(theme.effects.cell_aspect),
            performance: self.config.performance_config(),
        }
    }

    fn has_visual_work(&self) -> bool {
        !self.effects.is_empty()
            || self.animation.is_some()
            || self.progress.is_some()
            || self.enter_effect.is_some()
            || self.exit_effect.is_some()
    }

    fn lifecycle_frames(
        &self,
        ctx: &RenderCtx,
        channel: &str,
    ) -> Vec<crate::animation::TimelineFrame> {
        let timeline = match channel {
            "exit" => self.presence.exit.clone(),
            _ => self.presence.enter.clone(),
        };
        let Some(timeline) = timeline else {
            return Vec::new();
        };
        ctx.track_timeline(channel, timeline, false)
    }

    fn resolve_effect_groups(&self, ctx: &RenderCtx) -> Vec<ResolvedEffectGroup> {
        if ctx.is_exit_render() {
            let Some(effect) = self
                .exit_effect
                .as_ref()
                .map(|effect| effect.resolve(ctx.theme()))
            else {
                return Vec::new();
            };
            return vec![ResolvedEffectGroup {
                progress: lifecycle_progress(&self.lifecycle_frames(ctx, "exit"), 1.0),
                effects: vec![effect],
            }];
        }

        let mut groups = Vec::new();
        if !self.effects.is_empty() || self.animation.is_some() || self.progress.is_some() {
            groups.push(ResolvedEffectGroup {
                progress: self.resolve_progress(ctx),
                effects: self.effects.clone(),
            });
        }

        if let Some(effect) = self
            .enter_effect
            .as_ref()
            .map(|effect| effect.resolve(ctx.theme()))
        {
            groups.push(ResolvedEffectGroup {
                progress: lifecycle_progress(&self.lifecycle_frames(ctx, "enter"), 1.0),
                effects: vec![effect],
            });
        }

        groups
    }
}

impl<M> Widget<M> for Visual<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        ctx.record_bounds(area);

        if !self.has_visual_work() {
            let child_ctx = ctx.child_ctx(WidgetKey::for_child(0, self.child.as_ref()));
            self.child.render(chunk, &child_ctx);
            return;
        }

        let seed = self
            .seed
            .unwrap_or_else(|| stable_seed(&self.channel, self.widget_key.as_deref()));
        let child_ctx = ctx.child_ctx(WidgetKey::for_child(0, self.child.as_ref()));
        let resolved_config = self.resolve_config(ctx.theme());
        let profile_enabled = self.profile_hook.is_some();
        let mut offscreen_render_time = Duration::ZERO;

        let Some(report) = with_reused_offscreen(
            area,
            Some(seed),
            |offscreen_chunk| {
                let started = profile_enabled.then(Instant::now);
                self.child.render(offscreen_chunk, &child_ctx);
                if let Some(started) = started {
                    offscreen_render_time = started.elapsed();
                }
            },
            |offscreen| {
                let visual_ctx = VisualCtx::new(
                    1.0,
                    area,
                    ctx.now(),
                    ctx.frame(),
                    resolved_config.cell_aspect,
                    ctx.motion_policy(),
                    ctx.theme(),
                    self.effect_preset.as_deref(),
                    seed,
                )
                .with_performance(resolved_config.performance);
                let visual_ctx = visual_ctx.with_capabilities(ctx.visual_capabilities());
                let effect_groups = self.resolve_effect_groups(ctx);
                if effect_groups.iter().all(|group| group.effects.is_empty()) {
                    return blit_with_effects(chunk, offscreen, &[], &visual_ctx, profile_enabled);
                }
                blit_with_effect_groups(
                    chunk,
                    offscreen,
                    &effect_groups,
                    &visual_ctx,
                    profile_enabled,
                )
            },
        ) else {
            return;
        };

        if let Some(hook) = &self.profile_hook {
            hook(report.into_profile(offscreen_render_time));
        }
    }

    fn constraints(&self) -> Constraints {
        self.child.constraints()
    }

    fn focus_config(&self) -> FocusConfig {
        FocusConfig::None
    }

    fn children(&self) -> &[Box<dyn Widget<M>>] {
        std::slice::from_ref(&self.child)
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }

    fn presence(&self) -> Option<&Presence> {
        if self.presence.enter.is_some() || self.presence.exit.is_some() {
            Some(&self.presence)
        } else {
            None
        }
    }
}

/// Convenience for a looping visual animation.
pub fn looping_visual_spec(duration: Duration) -> AnimationSpec {
    AnimationSpec::new(duration, crate::animation::Easing::Linear)
        .repeat(Repeat::Forever)
        .direction(AnimationDirection::Normal)
}

fn visual_enter_timeline() -> Timeline {
    Timeline::single(Transition::new(
        TransitionEffect::Fade,
        AnimationSpec::new(Duration::from_millis(180), Easing::EaseOut),
    ))
}

fn visual_exit_timeline() -> Timeline {
    Timeline::single(Transition::new(
        TransitionEffect::Fade,
        AnimationSpec::new(Duration::from_millis(140), Easing::EaseIn),
    ))
}
