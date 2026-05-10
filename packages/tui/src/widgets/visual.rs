//! Visual post-processing effects for arbitrary widgets.
//!
//! This wrapper renders its child into an offscreen buffer, then maps the
//! resulting terminal cells back into the target chunk with optional color and
//! geometry transforms.

use std::time::{Duration, Instant};

use crossterm::style::{Color as CrosstermColor, Colors};
use render::area::Area;
use render::buffer::Buffer;
use render::style::Stylized;

use crate::animation::{
    AnimationSpec, Direction as AnimationDirection, Easing, InitialAnimation, MotionPolicy,
    Presence, Repeat, Timeline, Transition, TransitionEffect,
};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::offscreen::{for_each_blit_cell, render_to_offscreen, BlitOptions, BlitRegion};
use crate::style::{Color, EffectSlot, Style, Theme};
use crate::widget::{IntoWidget, RenderCtx, Widget, WidgetKey};

const LARGE_EFFECT_AREA_CELLS: u32 = 2_400;

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
    effect_preset: Option<String>,
    presence: Presence,
    enter_effect: Option<LifecycleVisualEffect>,
    exit_effect: Option<LifecycleVisualEffect>,
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
            .field("effect_preset", &self.effect_preset)
            .field("presence", &self.presence)
            .field("enter_effect", &self.enter_effect)
            .field("exit_effect", &self.exit_effect)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum LifecycleVisualEffect {
    Effect(VisualEffect),
    Theme(EffectSlot),
}

impl LifecycleVisualEffect {
    fn resolve(&self, theme: &Theme) -> VisualEffect {
        match self {
            Self::Effect(effect) => effect.clone(),
            Self::Theme(slot) => theme.effects.get(*slot),
        }
    }
}

/// Local configuration for a [`Visual`] wrapper.
///
/// Values set here override theme-level defaults for this single wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VisualConfig {
    cell_aspect: Option<f64>,
}

impl VisualConfig {
    /// Override the terminal cell aspect used by geometry effects.
    ///
    /// `cell_aspect` is applied as `logical_x = cell_x * cell_aspect`.
    /// The default `None` means "use `Theme::effects.cell_aspect`".
    pub fn cell_aspect(mut self, cell_aspect: f64) -> Self {
        self.cell_aspect = Some(sanitize_cell_aspect(cell_aspect));
        self
    }

    /// Return the local cell aspect override, if one was configured.
    pub fn cell_aspect_override(self) -> Option<f64> {
        self.cell_aspect
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedVisualConfig {
    cell_aspect: f64,
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

    fn resolve_config(&self, theme: &Theme) -> ResolvedVisualConfig {
        ResolvedVisualConfig {
            cell_aspect: self
                .config
                .cell_aspect_override()
                .unwrap_or(theme.effects.cell_aspect),
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

#[derive(Debug, Clone, PartialEq)]
struct ResolvedEffectGroup {
    progress: f64,
    effects: Vec<VisualEffect>,
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

        let child_ctx = ctx.child_ctx(WidgetKey::for_child(0, self.child.as_ref()));
        let Some(offscreen) = render_to_offscreen(area, |offscreen_chunk| {
            self.child.render(offscreen_chunk, &child_ctx);
        }) else {
            return;
        };
        let seed = self
            .seed
            .unwrap_or_else(|| stable_seed(&self.channel, self.widget_key.as_deref()));
        let resolved_config = self.resolve_config(ctx.theme());
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
        );
        let effect_groups = self.resolve_effect_groups(ctx);
        if effect_groups.iter().all(|group| group.effects.is_empty()) {
            blit_with_effects(chunk, &offscreen, &[], &visual_ctx);
            return;
        }
        blit_with_effect_groups(chunk, &offscreen, &effect_groups, &visual_ctx);
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

/// A terminal-cell visual effect.
#[derive(Debug, Clone, PartialEq)]
pub enum VisualEffect {
    Fade {
        from: f64,
        to: f64,
    },
    Gradient {
        start: Color,
        end: Color,
        direction: GradientDirection,
        target: GradientTarget,
        phase: f64,
    },
    Shatter {
        seed: u64,
        spread_x: f64,
        spread_y: f64,
        fade: bool,
    },
    MagicLamp {
        anchor: VisualAnchor,
        squeeze: f64,
    },
    Wipe {
        direction: WipeDirection,
        mode: WipeMode,
        softness: f64,
    },
    Dissolve {
        seed: u64,
        mode: DissolveMode,
    },
    Wave {
        axis: WaveAxis,
        amplitude: f64,
        wavelength: f64,
        phase: f64,
    },
    Glitch {
        seed: u64,
        intensity: f64,
    },
    Sequence(Vec<VisualEffect>),
    Parallel(Vec<VisualEffect>),
    Stagger {
        delay: f64,
        mode: StaggerMode,
        effect: Box<VisualEffect>,
    },
}

impl VisualEffect {
    pub fn fade_in() -> Self {
        Self::Fade { from: 0.0, to: 1.0 }
    }

    pub fn fade_out() -> Self {
        Self::Fade { from: 1.0, to: 0.0 }
    }

    pub fn gradient(start: Color, end: Color, direction: GradientDirection) -> Self {
        Self::Gradient {
            start,
            end,
            direction,
            target: GradientTarget::Foreground,
            phase: 0.0,
        }
    }

    pub fn background_gradient(start: Color, end: Color, direction: GradientDirection) -> Self {
        Self::Gradient {
            start,
            end,
            direction,
            target: GradientTarget::Background,
            phase: 0.0,
        }
    }

    pub fn shatter() -> Self {
        Self::Shatter {
            seed: 0x5EED,
            spread_x: 18.0,
            spread_y: 7.0,
            fade: true,
        }
    }

    pub fn magic_lamp(anchor: VisualAnchor) -> Self {
        Self::MagicLamp {
            anchor,
            squeeze: 0.08,
        }
    }

    pub fn wipe(direction: WipeDirection) -> Self {
        Self::Wipe {
            direction,
            mode: WipeMode::Hide,
            softness: 0.0,
        }
    }

    pub fn reveal(direction: WipeDirection) -> Self {
        Self::Wipe {
            direction,
            mode: WipeMode::Reveal,
            softness: 0.0,
        }
    }

    pub fn dissolve() -> Self {
        Self::Dissolve {
            seed: 0xD155_01F3,
            mode: DissolveMode::In,
        }
    }

    pub fn dissolve_out() -> Self {
        Self::Dissolve {
            seed: 0xD155_01F3,
            mode: DissolveMode::Out,
        }
    }

    pub fn wave(axis: WaveAxis) -> Self {
        Self::Wave {
            axis,
            amplitude: 2.0,
            wavelength: 6.0,
            phase: 0.0,
        }
    }

    pub fn glitch() -> Self {
        Self::Glitch {
            seed: 0x6_117C_4,
            intensity: 0.42,
        }
    }

    /// Run child effects one after another with equal duration slices.
    ///
    /// Completed children are sampled at `1.0`; the active child receives its
    /// local progress; pending children are skipped.
    pub fn sequence(effects: Vec<VisualEffect>) -> Self {
        Self::Sequence(effects)
    }

    /// Run child effects over the same progress range.
    ///
    /// Children are applied in list order. Later children see earlier geometry
    /// and color changes; visibility remains hidden once any child hides a cell.
    pub fn parallel(effects: Vec<VisualEffect>) -> Self {
        Self::Parallel(effects)
    }

    /// Delay a child effect by local row.
    pub fn stagger_rows(delay: f64, effect: VisualEffect) -> Self {
        Self::stagger(delay, StaggerMode::Rows, effect)
    }

    /// Delay a child effect by local column.
    pub fn stagger_cols(delay: f64, effect: VisualEffect) -> Self {
        Self::stagger(delay, StaggerMode::Cols, effect)
    }

    /// Delay a child effect by source character index, row-major.
    pub fn stagger_chars(delay: f64, effect: VisualEffect) -> Self {
        Self::stagger(delay, StaggerMode::Chars, effect)
    }

    fn stagger(delay: f64, mode: StaggerMode, effect: VisualEffect) -> Self {
        Self::Stagger {
            delay: sanitize_delay(delay),
            mode,
            effect: Box::new(effect),
        }
    }

    /// Return the reduced-motion form of this effect.
    ///
    /// Spatial motion is replaced with fades, while color-only effects remain
    /// unchanged. Composition is preserved with zero stagger delay.
    pub fn reduced(&self) -> Self {
        match self {
            Self::Fade { .. } | Self::Gradient { .. } => self.clone(),
            Self::Shatter { .. } => Self::fade_out(),
            Self::MagicLamp { .. } => Self::fade_in(),
            Self::Wipe { mode, .. } => fade_for_visibility_mode(*mode),
            Self::Dissolve { mode, .. } => match mode {
                DissolveMode::In => Self::fade_in(),
                DissolveMode::Out => Self::fade_out(),
            },
            Self::Wave { .. } | Self::Glitch { .. } => Self::fade_in(),
            Self::Sequence(effects) => Self::Sequence(
                effects
                    .iter()
                    .map(VisualEffect::reduced)
                    .collect::<Vec<_>>(),
            ),
            Self::Parallel(effects) => Self::Parallel(
                effects
                    .iter()
                    .map(VisualEffect::reduced)
                    .collect::<Vec<_>>(),
            ),
            Self::Stagger { effect, mode, .. } => Self::Stagger {
                delay: 0.0,
                mode: *mode,
                effect: Box::new(effect.reduced()),
            },
        }
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        match &mut self {
            Self::Shatter { seed: current, .. }
            | Self::Dissolve { seed: current, .. }
            | Self::Glitch { seed: current, .. } => {
                *current = seed;
            }
            _ => {}
        }
        self
    }

    pub fn with_spread(mut self, x: f64, y: f64) -> Self {
        if let Self::Shatter {
            spread_x, spread_y, ..
        } = &mut self
        {
            *spread_x = x.max(0.0);
            *spread_y = y.max(0.0);
        }
        self
    }

    pub fn target(mut self, target: GradientTarget) -> Self {
        if let Self::Gradient {
            target: current, ..
        } = &mut self
        {
            *current = target;
        }
        self
    }

    pub fn phase(mut self, phase: f64) -> Self {
        match &mut self {
            Self::Gradient { phase: current, .. } | Self::Wave { phase: current, .. } => {
                *current = finite_or(phase, 0.0);
            }
            _ => {}
        }
        self
    }

    pub fn squeeze(mut self, squeeze: f64) -> Self {
        if let Self::MagicLamp {
            squeeze: current, ..
        } = &mut self
        {
            *current = squeeze.clamp(0.0, 1.0);
        }
        self
    }

    pub fn softness(mut self, softness: f64) -> Self {
        if let Self::Wipe {
            softness: current, ..
        } = &mut self
        {
            *current = sanitize_softness(softness);
        }
        self
    }

    pub fn amplitude(mut self, amplitude: f64) -> Self {
        if let Self::Wave {
            amplitude: current, ..
        } = &mut self
        {
            *current = finite_or(amplitude, 0.0).max(0.0);
        }
        self
    }

    pub fn wavelength(mut self, wavelength: f64) -> Self {
        if let Self::Wave {
            wavelength: current,
            ..
        } = &mut self
        {
            *current = finite_or(wavelength, 6.0).max(f64::EPSILON);
        }
        self
    }

    pub fn intensity(mut self, intensity: f64) -> Self {
        if let Self::Glitch {
            intensity: current, ..
        } = &mut self
        {
            *current = finite_or(intensity, 0.0).clamp(0.0, 1.0);
        }
        self
    }
}

/// Spatial axis used by staggered visual effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaggerMode {
    Rows,
    Cols,
    Chars,
}

/// Direction used by [`VisualEffect::Gradient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientDirection {
    Horizontal,
    Vertical,
    Diagonal,
}

/// Which color channel a gradient should override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientTarget {
    Foreground,
    Background,
}

/// Direction used by [`VisualEffect::Wipe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
    CenterOut,
    EdgesIn,
    DiagonalDown,
    DiagonalUp,
}

/// Visibility mode for wipe-style effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WipeMode {
    Reveal,
    Hide,
}

/// Visibility mode for dissolve effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DissolveMode {
    In,
    Out,
}

/// Axis used by [`VisualEffect::Wave`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveAxis {
    Rows,
    Cols,
}

/// Anchor point for spatial effects such as magic-lamp minimization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VisualAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// Normalized coordinates where (0, 0) is top-left and (1, 1) is bottom-right.
    Relative(f64, f64),
}

impl VisualAnchor {
    fn resolve(self, width: u16, height: u16) -> (f64, f64) {
        let w = width.saturating_sub(1) as f64;
        let h = height.saturating_sub(1) as f64;
        let (x, y) = match self {
            Self::TopLeft => (0.0, 0.0),
            Self::Top => (0.5, 0.0),
            Self::TopRight => (1.0, 0.0),
            Self::Left => (0.0, 0.5),
            Self::Center => (0.5, 0.5),
            Self::Right => (1.0, 0.5),
            Self::BottomLeft => (0.0, 1.0),
            Self::Bottom => (0.5, 1.0),
            Self::BottomRight => (1.0, 1.0),
            Self::Relative(x, y) => (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)),
        };
        (x * w, y * h)
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

/// Runtime context shared by visual effects while sampling a widget.
#[derive(Debug, Clone, Copy)]
pub struct VisualCtx<'a> {
    /// Effect progress in the inclusive range `0.0..=1.0`.
    pub progress: f64,
    /// Logical area occupied by the wrapped widget.
    pub area: Area,
    /// Runtime render instant for this frame.
    pub now: Instant,
    /// Runtime render frame number for this frame.
    pub frame: u64,
    /// Terminal cell width divided by cell height. Defaults to `1.0` until
    /// terminal-specific configuration is introduced.
    pub cell_aspect: f64,
    /// Global motion behavior active for this render pass.
    pub motion_policy: MotionPolicy,
    /// Theme active for this render pass.
    pub theme: &'a Theme,
    /// Optional named entry point for future theme effect presets.
    pub effect_preset: Option<&'a str>,
    /// Stable seed available to deterministic effects.
    pub seed: u64,
}

impl<'a> VisualCtx<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        progress: f64,
        area: Area,
        now: Instant,
        frame: u64,
        cell_aspect: f64,
        motion_policy: MotionPolicy,
        theme: &'a Theme,
        effect_preset: Option<&'a str>,
        seed: u64,
    ) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            area,
            now,
            frame,
            cell_aspect: cell_aspect.max(f64::EPSILON),
            motion_policy,
            theme,
            effect_preset,
            seed,
        }
    }

    /// Convert a cell-space x coordinate into aspect-corrected logical space.
    pub fn logical_x(self, cell_x: f64) -> f64 {
        cell_x * self.cell_aspect
    }

    /// Convert a cell-space point into aspect-corrected logical space.
    pub fn logical_point(self, cell_x: f64, cell_y: f64) -> (f64, f64) {
        (self.logical_x(cell_x), cell_y)
    }

    /// Scale a horizontal cell offset by the active cell aspect.
    pub fn aspect_adjusted_x_offset(self, cell_dx: f64) -> f64 {
        cell_dx * self.cell_aspect
    }

    /// Measure the logical distance between two local cell-space points.
    pub fn logical_distance(self, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
        let (ax, ay) = self.logical_point(ax, ay);
        let (bx, by) = self.logical_point(bx, by);
        ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
    }

    /// Whether area-sensitive effects should prefer cheaper reduced behavior.
    pub fn is_large_area(self) -> bool {
        self.area.width() as u32 * self.area.height() as u32 > LARGE_EFFECT_AREA_CELLS
    }

    fn with_progress(self, progress: f64) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            ..self
        }
    }
}

/// A single sampled terminal cell in a visual effect pipeline.
///
/// Coordinates are local to [`VisualCtx::area`]. `source_*` identifies the
/// child-rendered cell being sampled; `dest_*` is the current destination after
/// geometry effects have mapped it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellSample {
    pub source_x: f64,
    pub source_y: f64,
    pub dest_x: f64,
    pub dest_y: f64,
    pub content: Stylized,
    pub visible: bool,
}

impl CellSample {
    pub fn new(source_x: u16, source_y: u16, content: Stylized) -> Self {
        let source_x = source_x as f64;
        let source_y = source_y as f64;
        Self {
            source_x,
            source_y,
            dest_x: source_x,
            dest_y: source_y,
            content,
            visible: true,
        }
    }
}

fn blit_with_effects(
    chunk: &mut render::chunk::Chunk,
    source: &Buffer,
    effects: &[VisualEffect],
    ctx: &VisualCtx<'_>,
) {
    let region = BlitRegion::new(
        ctx.area.x() as usize,
        ctx.area.y() as usize,
        0,
        0,
        ctx.area.width() as usize,
        ctx.area.height() as usize,
    );

    for_each_blit_cell(
        source,
        region,
        Some(ctx.area.size()),
        BlitOptions::default().skip_blank(),
        |cell| {
            let mut sample = CellSample::new(cell.dest_x, cell.dest_y, cell.content);

            for effect in effects {
                effect.apply(&mut sample, ctx);
                if !sample.visible {
                    break;
                }
            }

            let dest_x = sample.dest_x.round() as i32;
            let dest_y = sample.dest_y.round() as i32;
            if sample.visible
                && dest_x >= 0
                && dest_y >= 0
                && dest_x < ctx.area.width() as i32
                && dest_y < ctx.area.height() as i32
            {
                let _ = chunk.set_forced(dest_x as u16, dest_y as u16, sample.content);
            }
        },
    );
}

fn blit_with_effect_groups(
    chunk: &mut render::chunk::Chunk,
    source: &Buffer,
    groups: &[ResolvedEffectGroup],
    ctx: &VisualCtx<'_>,
) {
    let effective_groups: Vec<(VisualCtx<'_>, Vec<VisualEffect>)> = groups
        .iter()
        .map(|group| {
            let group_ctx =
                ctx.with_progress(effective_progress(group.progress, ctx.motion_policy));
            let effects = effective_effects(&group.effects, &group_ctx);
            (group_ctx, effects)
        })
        .collect();
    let region = BlitRegion::new(
        ctx.area.x() as usize,
        ctx.area.y() as usize,
        0,
        0,
        ctx.area.width() as usize,
        ctx.area.height() as usize,
    );

    for_each_blit_cell(
        source,
        region,
        Some(ctx.area.size()),
        BlitOptions::default().skip_blank(),
        |cell| {
            let mut sample = CellSample::new(cell.dest_x, cell.dest_y, cell.content);

            for (group_ctx, effects) in &effective_groups {
                for effect in effects {
                    effect.apply(&mut sample, group_ctx);
                    if !sample.visible {
                        break;
                    }
                }

                if !sample.visible {
                    break;
                }
            }

            let dest_x = sample.dest_x.round() as i32;
            let dest_y = sample.dest_y.round() as i32;
            if sample.visible
                && dest_x >= 0
                && dest_y >= 0
                && dest_x < ctx.area.width() as i32
                && dest_y < ctx.area.height() as i32
            {
                let _ = chunk.set_forced(dest_x as u16, dest_y as u16, sample.content);
            }
        },
    );
}

fn lifecycle_progress(frames: &[crate::animation::TimelineFrame], fallback: f64) -> f64 {
    frames
        .last()
        .map(|frame| frame.progress)
        .unwrap_or(fallback)
        .clamp(0.0, 1.0)
}

fn effective_effects(effects: &[VisualEffect], ctx: &VisualCtx<'_>) -> Vec<VisualEffect> {
    if ctx.motion_policy.reduced_motion || ctx.is_large_area() {
        effects.iter().map(VisualEffect::reduced).collect()
    } else {
        effects.to_vec()
    }
}

fn effective_progress(progress: f64, motion_policy: MotionPolicy) -> f64 {
    if motion_policy.enabled {
        progress.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

impl VisualEffect {
    fn apply(&self, sample: &mut CellSample, ctx: &VisualCtx<'_>) {
        let progress = ctx.progress;
        match self {
            Self::Fade { from, to } => {
                let alpha = lerp(*from, *to, progress).clamp(0.0, 1.0);
                let threshold = noise(sample.dest_x as u16, sample.dest_y as u16, 0xFAD3);
                sample.visible = threshold <= alpha;
            }
            Self::Gradient {
                start,
                end,
                direction,
                target,
                phase,
            } => {
                let shifted =
                    gradient_position(sample.dest_x, sample.dest_y, ctx.area, *direction) + *phase;
                let t = if (0.0..=1.0).contains(&shifted) {
                    shifted
                } else {
                    shifted.rem_euclid(1.0)
                };
                let color = start.interpolate(*end, t);
                apply_color(&mut sample.content, color, *target);
            }
            Self::Shatter {
                seed,
                spread_x,
                spread_y,
                fade,
            } => {
                let t = ease_out_cubic(progress);
                let jitter = hash_pair(sample.source_x as u16, sample.source_y as u16, *seed);
                let angle = jitter.0 * std::f64::consts::TAU;
                let force = 0.35 + jitter.1 * 0.95;
                let lift = 0.3
                    + noise(
                        sample.source_y as u16,
                        sample.source_x as u16,
                        *seed ^ 0xBEEF,
                    ) * 0.8;
                sample.dest_x += ctx.aspect_adjusted_x_offset(angle.cos() * *spread_x * force * t);
                sample.dest_y += (angle.sin() * *spread_y * force - *spread_y * lift) * t;

                if *fade {
                    let threshold =
                        noise(sample.dest_x as u16, sample.dest_y as u16, *seed ^ 0xFADE);
                    sample.visible = threshold > progress;
                }
            }
            Self::MagicLamp { anchor, squeeze } => {
                let (anchor_x, anchor_y) = anchor.resolve(ctx.area.width(), ctx.area.height());
                let open = ease_out_cubic(progress);
                let pull = 1.0 - open;
                let scale = open + pull * *squeeze;
                let row = if ctx.area.height() <= 1 {
                    0.5
                } else {
                    sample.dest_y / ctx.area.height().saturating_sub(1) as f64
                };
                let bow = ctx.aspect_adjusted_x_offset(
                    ((row - 0.5) * std::f64::consts::PI).sin()
                        * pull
                        * ctx.area.width() as f64
                        * 0.16,
                );

                sample.dest_x = anchor_x
                    + ctx.aspect_adjusted_x_offset((sample.dest_x - anchor_x) * scale)
                    + bow;
                sample.dest_y =
                    anchor_y + (sample.dest_y - anchor_y) * (open * open + pull * *squeeze);
            }
            Self::Wipe {
                direction,
                mode,
                softness,
            } => {
                let position = wipe_position(sample.source_x, sample.source_y, ctx, *direction);
                sample.visible = wipe_visible(
                    position,
                    progress,
                    *mode,
                    *softness,
                    ctx.seed ^ 0xA11C_E5,
                    sample.source_x as u16,
                    sample.source_y as u16,
                );
            }
            Self::Dissolve { seed, mode } => {
                let threshold = noise(
                    sample.source_x as u16,
                    sample.source_y as u16,
                    effect_seed(ctx, *seed),
                );
                sample.visible = match mode {
                    DissolveMode::In => threshold <= progress,
                    DissolveMode::Out => threshold > progress,
                };
            }
            Self::Wave {
                axis,
                amplitude,
                wavelength,
                phase,
            } => {
                let envelope = (progress * std::f64::consts::PI).sin().max(0.0);
                if envelope > 0.0 && *amplitude > 0.0 {
                    let wave = match axis {
                        WaveAxis::Rows => {
                            (sample.source_y / wavelength.max(f64::EPSILON) + *phase + progress)
                                * std::f64::consts::TAU
                        }
                        WaveAxis::Cols => {
                            (ctx.logical_x(sample.source_x) / wavelength.max(f64::EPSILON)
                                + *phase
                                + progress)
                                * std::f64::consts::TAU
                        }
                    };
                    let offset = wave.sin() * *amplitude * envelope;
                    match axis {
                        WaveAxis::Rows => {
                            sample.dest_x += ctx.aspect_adjusted_x_offset(offset);
                        }
                        WaveAxis::Cols => {
                            sample.dest_y += offset;
                        }
                    }
                }
            }
            Self::Glitch { seed, intensity } => {
                apply_glitch(sample, ctx, effect_seed(ctx, *seed), *intensity);
            }
            Self::Sequence(effects) => {
                apply_sequence(effects, sample, ctx);
            }
            Self::Parallel(effects) => {
                for effect in effects {
                    effect.apply(sample, ctx);
                }
            }
            Self::Stagger {
                delay,
                mode,
                effect,
            } => {
                let local_progress = staggered_progress(sample, ctx, *delay, *mode);
                effect.apply(sample, &ctx.with_progress(local_progress));
            }
        }
    }
}

fn apply_sequence(effects: &[VisualEffect], sample: &mut CellSample, ctx: &VisualCtx<'_>) {
    let len = effects.len();
    if len == 0 {
        return;
    }

    let scaled = ctx.progress.clamp(0.0, 1.0) * len as f64;
    let active = scaled.floor().min((len - 1) as f64) as usize;
    let local = if active == len - 1 && ctx.progress >= 1.0 {
        1.0
    } else {
        scaled - active as f64
    };

    for (index, effect) in effects.iter().enumerate() {
        if index < active {
            effect.apply(sample, &ctx.with_progress(1.0));
        } else if index == active {
            effect.apply(sample, &ctx.with_progress(local));
        } else {
            break;
        }

        if !sample.visible {
            break;
        }
    }
}

fn wipe_position(x: f64, y: f64, ctx: &VisualCtx<'_>, direction: WipeDirection) -> f64 {
    let max_x = ctx.area.width().saturating_sub(1) as f64;
    let max_y = ctx.area.height().saturating_sub(1) as f64;
    let logical_x = ctx.logical_x(x);
    let logical_max_x = ctx.logical_x(max_x);

    let divide = |value: f64, max: f64| {
        if max <= 0.0 {
            0.0
        } else {
            (value / max).clamp(0.0, 1.0)
        }
    };

    match direction {
        WipeDirection::LeftToRight => divide(logical_x, logical_max_x),
        WipeDirection::RightToLeft => divide(logical_max_x - logical_x, logical_max_x),
        WipeDirection::TopToBottom => divide(y, max_y),
        WipeDirection::BottomToTop => divide(max_y - y, max_y),
        WipeDirection::CenterOut => {
            let center_x = logical_max_x * 0.5;
            let center_y = max_y * 0.5;
            let max_distance = [
                (0.0, 0.0),
                (logical_max_x, 0.0),
                (0.0, max_y),
                (logical_max_x, max_y),
            ]
            .into_iter()
            .map(|(corner_x, corner_y)| {
                ((corner_x - center_x).powi(2) + (corner_y - center_y).powi(2)).sqrt()
            })
            .fold(0.0, f64::max);
            let distance = ((logical_x - center_x).powi(2) + (y - center_y).powi(2)).sqrt();
            divide(distance, max_distance)
        }
        WipeDirection::EdgesIn => {
            let distance_to_edge = logical_x
                .min((logical_max_x - logical_x).max(0.0))
                .min(y.min((max_y - y).max(0.0)));
            let max_distance = (logical_max_x * 0.5).min(max_y * 0.5);
            divide(distance_to_edge, max_distance)
        }
        WipeDirection::DiagonalDown => divide(logical_x + y, logical_max_x + max_y),
        WipeDirection::DiagonalUp => divide(logical_x + (max_y - y), logical_max_x + max_y),
    }
}

fn wipe_visible(
    position: f64,
    progress: f64,
    mode: WipeMode,
    softness: f64,
    seed: u64,
    x: u16,
    y: u16,
) -> bool {
    let position = position.clamp(0.0, 1.0);
    let progress = progress.clamp(0.0, 1.0);
    if progress <= 0.0 {
        return mode == WipeMode::Hide;
    }
    if progress >= 1.0 {
        return mode == WipeMode::Reveal;
    }

    if softness <= 0.0 {
        return match mode {
            WipeMode::Reveal => position <= progress,
            WipeMode::Hide => position > progress,
        };
    }

    let feather = softness.max(f64::EPSILON);
    if position <= progress - feather {
        return mode == WipeMode::Reveal;
    }
    if position >= progress + feather {
        return mode == WipeMode::Hide;
    }

    let reveal_coverage = ((progress + feather - position) / (feather * 2.0)).clamp(0.0, 1.0);
    let coverage = match mode {
        WipeMode::Reveal => reveal_coverage,
        WipeMode::Hide => 1.0 - reveal_coverage,
    };
    noise(x, y, seed) <= coverage
}

fn apply_glitch(sample: &mut CellSample, ctx: &VisualCtx<'_>, seed: u64, intensity: f64) {
    let envelope = (ctx.progress * std::f64::consts::PI).sin().max(0.0);
    let active = finite_or(intensity, 0.0).clamp(0.0, 1.0) * envelope;
    if active <= 0.0 {
        return;
    }

    let x = sample.source_x as u16;
    let y = sample.source_y as u16;
    let gate = noise(x, y, seed ^ 0x61_17C);
    if gate > active {
        return;
    }

    let horizontal = (noise(y, x, seed ^ 0x0FF5) * 3.0).floor() - 1.0;
    let vertical = if noise(x, y, seed ^ 0x5CA1) < active * 0.45 {
        if noise(y, x, seed ^ 0x51DE) < 0.5 {
            -1.0
        } else {
            1.0
        }
    } else {
        0.0
    };
    sample.dest_x += ctx.aspect_adjusted_x_offset(horizontal);
    sample.dest_y += vertical;

    if noise(x, y, seed ^ 0xC0DE) < active * 0.55 {
        let charset = ['#', '%', '&', '+', '*', '!', '?', '/', '\\'];
        let index = (noise(y, x, seed ^ 0xC4A7) * charset.len() as f64) as usize;
        sample.content.c = Some(charset[index.min(charset.len() - 1)]);
    }

    let color = if noise(x, y, seed ^ 0xC010) < 0.5 {
        Color::Rgb(255, 80, 120)
    } else {
        Color::Rgb(80, 220, 255)
    };
    apply_color(&mut sample.content, color, GradientTarget::Foreground);
}

fn staggered_progress(
    sample: &CellSample,
    ctx: &VisualCtx<'_>,
    delay: f64,
    mode: StaggerMode,
) -> f64 {
    if delay <= 0.0 {
        return ctx.progress;
    }

    let (index, max_index) = match mode {
        StaggerMode::Rows => (
            sample.source_y.round().max(0.0),
            ctx.area.height().saturating_sub(1) as f64,
        ),
        StaggerMode::Cols => (
            sample.source_x.round().max(0.0),
            ctx.area.width().saturating_sub(1) as f64,
        ),
        StaggerMode::Chars => {
            let width = ctx.area.width().max(1) as f64;
            (
                sample.source_y.round().max(0.0) * width + sample.source_x.round().max(0.0),
                (ctx.area.width() as u32 * ctx.area.height() as u32).saturating_sub(1) as f64,
            )
        }
    };

    let max_offset = (delay * max_index).min(0.95);
    let offset = (delay * index).min(max_offset);
    let duration = (1.0 - max_offset).max(f64::EPSILON);
    ((ctx.progress - offset) / duration).clamp(0.0, 1.0)
}

fn sanitize_cell_aspect(cell_aspect: f64) -> f64 {
    if cell_aspect.is_finite() {
        cell_aspect.max(f64::EPSILON)
    } else {
        1.0
    }
}

fn sanitize_delay(delay: f64) -> f64 {
    if delay.is_finite() {
        delay.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn sanitize_softness(softness: f64) -> f64 {
    finite_or(softness, 0.0).clamp(0.0, 1.0)
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn fade_for_visibility_mode(mode: WipeMode) -> VisualEffect {
    match mode {
        WipeMode::Reveal => VisualEffect::fade_in(),
        WipeMode::Hide => VisualEffect::fade_out(),
    }
}

fn effect_seed(ctx: &VisualCtx<'_>, seed: u64) -> u64 {
    ctx.seed ^ seed
}

fn stable_seed(channel: &str, widget_key: Option<&str>) -> u64 {
    let mut seed = 0xCBF2_9CE4_8422_2325;
    for byte in channel
        .bytes()
        .chain([0])
        .chain(widget_key.unwrap_or("").bytes())
    {
        seed ^= byte as u64;
        seed = seed.wrapping_mul(0x0000_0100_0000_01B3);
    }
    seed
}

fn gradient_position(x: f64, y: f64, area: Area, direction: GradientDirection) -> f64 {
    let width = area.width().saturating_sub(1) as f64;
    let height = area.height().saturating_sub(1) as f64;
    match direction {
        GradientDirection::Horizontal => {
            if width <= 0.0 {
                1.0
            } else {
                x / width
            }
        }
        GradientDirection::Vertical => {
            if height <= 0.0 {
                1.0
            } else {
                y / height
            }
        }
        GradientDirection::Diagonal => {
            if width + height <= 0.0 {
                1.0
            } else {
                (x + y) / (width + height)
            }
        }
    }
    .clamp(0.0, 1.0)
}

fn apply_color(content: &mut Stylized, color: Color, target: GradientTarget) {
    let color = to_crossterm_color(color);
    let mut colors = content.style.colors.unwrap_or(Colors {
        foreground: None,
        background: None,
    });

    match target {
        GradientTarget::Foreground => colors.foreground = Some(color),
        GradientTarget::Background => colors.background = Some(color),
    }
    content.style.colors = Some(colors);
}

fn to_crossterm_color(color: Color) -> CrosstermColor {
    let render_style = Style::default().fg(color).to_render_style();
    render_style
        .colors
        .and_then(|colors| colors.foreground)
        .unwrap_or(CrosstermColor::Reset)
}

fn lerp(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn hash_pair(x: u16, y: u16, seed: u64) -> (f64, f64) {
    (noise(x, y, seed), noise(y, x, seed.rotate_left(17)))
}

fn noise(x: u16, y: u16, seed: u64) -> f64 {
    let mut value = seed
        ^ ((x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
        ^ ((y as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9));
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 31;
    (value as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use render::chunk::Chunk;

    use super::*;
    use crate::animation::AnimationStore;
    use crate::style::{Theme, ThemeEffects};
    use crate::widget::{WidgetPath, WidgetStore};
    use crate::widgets::label;

    #[test]
    fn visual_wrapper_renders_child_content() {
        let widget = visual(label::<()>("hello")).progress(1.0).fade_in();
        let buffer = render_widget(&widget, 8, 1);

        assert_eq!(cell_char(&buffer, 0, 0), Some('h'));
        assert_eq!(cell_char(&buffer, 4, 0), Some('o'));
    }

    #[test]
    fn fade_can_hide_cells_at_zero_progress() {
        let widget = visual(label::<()>("hello")).progress(0.0).fade_in();
        let buffer = render_widget(&widget, 8, 1);

        assert_eq!(cell_char(&buffer, 0, 0), Some(' '));
        assert_eq!(cell_char(&buffer, 4, 0), Some(' '));
    }

    #[test]
    fn gradient_overrides_cell_foreground() {
        let widget = visual(label::<()>("ab")).progress(1.0).gradient(
            Color::Rgb(0, 0, 0),
            Color::Rgb(10, 0, 0),
            GradientDirection::Horizontal,
        );
        let buffer = render_widget(&widget, 2, 1);
        let first = cell(&buffer, 0, 0).unwrap();
        let second = cell(&buffer, 1, 0).unwrap();

        assert_ne!(first.content.style.colors, second.content.style.colors);
    }

    #[test]
    fn geometry_effect_updates_destination_not_source() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (5, 3).into());
        let ctx = VisualCtx::new(
            0.0,
            area,
            std::time::Instant::now(),
            7,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let mut sample = CellSample::new(0, 0, Stylized::plain('x'));

        VisualEffect::magic_lamp(VisualAnchor::Bottom)
            .squeeze(0.0)
            .apply(&mut sample, &ctx);

        assert_eq!((sample.source_x, sample.source_y), (0.0, 0.0));
        assert_ne!((sample.dest_x, sample.dest_y), (0.0, 0.0));
    }

    #[test]
    fn visual_config_resolves_theme_default_and_local_override() {
        let theme = Theme::dark().with_effects(ThemeEffects::default().cell_aspect(0.25));

        let themed = visual(label::<()>("x"));
        assert_eq!(themed.resolve_config(&theme).cell_aspect, 0.25);

        let local = visual(label::<()>("x")).config(VisualConfig::default().cell_aspect(0.5));
        assert_eq!(local.resolve_config(&theme).cell_aspect, 0.5);
    }

    #[test]
    fn visual_enter_exit_declare_presence() {
        let widget = visual(label::<()>("x"))
            .enter(VisualEffect::fade_in())
            .exit(VisualEffect::fade_out());
        let presence = widget.presence().unwrap();

        assert!(presence.enter.is_some());
        assert!(presence.exit.is_some());
        assert_eq!(presence.initial, InitialAnimation::Play);
    }

    #[test]
    fn visual_theme_slots_declare_presence_and_resolve_late() {
        let widget = visual(label::<()>("x"))
            .enter_theme(EffectSlot::ToastEnter)
            .exit_theme(EffectSlot::ToastExit);

        assert!(matches!(
            widget.enter_effect,
            Some(LifecycleVisualEffect::Theme(EffectSlot::ToastEnter))
        ));
        assert!(matches!(
            widget.exit_effect,
            Some(LifecycleVisualEffect::Theme(EffectSlot::ToastExit))
        ));
        assert!(widget.presence().unwrap().enter.is_some());
        assert_eq!(
            widget
                .enter_effect
                .as_ref()
                .unwrap()
                .resolve(&Theme::dark()),
            ThemeEffects::default().toast_enter
        );
    }

    #[test]
    fn shatter_uses_cell_aspect_for_horizontal_offset() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (10, 4).into());
        let square_ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let narrow_ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            0.5,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let effect = VisualEffect::Shatter {
            seed: 0x1234,
            spread_x: 8.0,
            spread_y: 0.0,
            fade: false,
        };
        let mut square = CellSample::new(3, 1, Stylized::plain('x'));
        let mut narrow = CellSample::new(3, 1, Stylized::plain('x'));

        effect.apply(&mut square, &square_ctx);
        effect.apply(&mut narrow, &narrow_ctx);

        assert_ne!(square.dest_x, narrow.dest_x);
        assert_eq!(square.dest_y, narrow.dest_y);
        assert!((narrow.dest_x - narrow.source_x).abs() < (square.dest_x - square.source_x).abs());
    }

    #[test]
    fn magic_lamp_uses_cell_aspect_for_horizontal_bow() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (5, 3).into());
        let square_ctx = VisualCtx::new(
            0.0,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let narrow_ctx = VisualCtx::new(
            0.0,
            area,
            std::time::Instant::now(),
            1,
            0.5,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let effect = VisualEffect::magic_lamp(VisualAnchor::Bottom).squeeze(0.0);
        let mut square = CellSample::new(0, 0, Stylized::plain('x'));
        let mut narrow = CellSample::new(0, 0, Stylized::plain('x'));

        effect.apply(&mut square, &square_ctx);
        effect.apply(&mut narrow, &narrow_ctx);

        assert_ne!(square.dest_x, narrow.dest_x);
        assert_eq!(square.dest_y, narrow.dest_y);
    }

    #[test]
    fn disabled_motion_samples_final_visual_progress() {
        assert_eq!(effective_progress(0.25, MotionPolicy::disabled()), 1.0);
        assert_eq!(effective_progress(0.25, MotionPolicy::default()), 0.25);
    }

    #[test]
    fn reduced_motion_replaces_spatial_effects_with_fades() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (8, 2).into());
        let ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::reduced_motion(),
            &theme,
            None,
            123,
        );

        let effects = effective_effects(&[VisualEffect::shatter()], &ctx);
        assert!(matches!(
            effects[0],
            VisualEffect::Fade { from: 1.0, to: 0.0 }
        ));

        let mut sample = CellSample::new(3, 1, Stylized::plain('x'));
        effects[0].apply(&mut sample, &ctx);

        assert_eq!((sample.dest_x, sample.dest_y), (3.0, 1.0));
    }

    #[test]
    fn large_areas_use_reduced_effects() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (120, 30).into());
        let ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );

        let effects = effective_effects(&[VisualEffect::magic_lamp(VisualAnchor::Bottom)], &ctx);
        assert!(matches!(
            effects[0],
            VisualEffect::Fade { from: 0.0, to: 1.0 }
        ));
    }

    #[test]
    fn sequence_applies_completed_then_active_effects() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (2, 1).into());
        let early_ctx = VisualCtx::new(
            0.25,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let late_ctx = early_ctx.with_progress(0.75);
        let effect = VisualEffect::sequence(vec![
            VisualEffect::gradient(
                Color::Rgb(255, 0, 0),
                Color::Rgb(255, 0, 0),
                GradientDirection::Horizontal,
            ),
            VisualEffect::gradient(
                Color::Rgb(0, 0, 255),
                Color::Rgb(0, 0, 255),
                GradientDirection::Horizontal,
            ),
        ]);
        let mut early = CellSample::new(0, 0, Stylized::plain('x'));
        let mut late = CellSample::new(0, 0, Stylized::plain('x'));

        effect.apply(&mut early, &early_ctx);
        effect.apply(&mut late, &late_ctx);

        assert_ne!(early.content.style.colors, late.content.style.colors);
    }

    #[test]
    fn parallel_applies_later_effects_last() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (2, 1).into());
        let ctx = VisualCtx::new(
            1.0,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let effect = VisualEffect::parallel(vec![
            VisualEffect::gradient(
                Color::Rgb(255, 0, 0),
                Color::Rgb(255, 0, 0),
                GradientDirection::Horizontal,
            ),
            VisualEffect::gradient(
                Color::Rgb(0, 0, 255),
                Color::Rgb(0, 0, 255),
                GradientDirection::Horizontal,
            ),
        ]);
        let mut parallel = CellSample::new(0, 0, Stylized::plain('x'));
        let mut blue = CellSample::new(0, 0, Stylized::plain('x'));

        effect.apply(&mut parallel, &ctx);
        VisualEffect::gradient(
            Color::Rgb(0, 0, 255),
            Color::Rgb(0, 0, 255),
            GradientDirection::Horizontal,
        )
        .apply(&mut blue, &ctx);

        assert_eq!(parallel.content.style.colors, blue.content.style.colors);
    }

    #[test]
    fn stagger_rows_delays_later_rows() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (4, 3).into());
        let ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let top = CellSample::new(0, 0, Stylized::plain('x'));
        let bottom = CellSample::new(0, 2, Stylized::plain('x'));

        assert!(
            staggered_progress(&top, &ctx, 0.2, StaggerMode::Rows)
                > staggered_progress(&bottom, &ctx, 0.2, StaggerMode::Rows)
        );
    }

    #[test]
    fn center_wipe_uses_aspect_corrected_distance() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (7, 3).into());
        let square_ctx = VisualCtx::new(
            0.88,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let narrow_ctx = VisualCtx::new(
            0.88,
            area,
            std::time::Instant::now(),
            1,
            0.5,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let effect = VisualEffect::reveal(WipeDirection::CenterOut);
        let mut square = CellSample::new(6, 1, Stylized::plain('x'));
        let mut narrow = CellSample::new(6, 1, Stylized::plain('x'));

        effect.apply(&mut square, &square_ctx);
        effect.apply(&mut narrow, &narrow_ctx);

        assert!(!square.visible);
        assert!(narrow.visible);
    }

    #[test]
    fn dissolve_is_stable_for_same_seed() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (8, 2).into());
        let ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            9001,
        );
        let effect = VisualEffect::dissolve().with_seed(77);
        let mut first = CellSample::new(4, 1, Stylized::plain('x'));
        let mut second = CellSample::new(4, 1, Stylized::plain('x'));

        effect.apply(&mut first, &ctx);
        effect.apply(&mut second, &ctx);

        assert_eq!(first.visible, second.visible);
    }

    #[test]
    fn wave_uses_cell_aspect_for_row_offsets() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (12, 4).into());
        let square_ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let narrow_ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            0.5,
            MotionPolicy::default(),
            &theme,
            None,
            123,
        );
        let effect = VisualEffect::wave(WaveAxis::Rows)
            .amplitude(4.0)
            .wavelength(5.0);
        let mut square = CellSample::new(4, 1, Stylized::plain('x'));
        let mut narrow = CellSample::new(4, 1, Stylized::plain('x'));

        effect.apply(&mut square, &square_ctx);
        effect.apply(&mut narrow, &narrow_ctx);

        assert_ne!(square.dest_x, narrow.dest_x);
        assert_eq!(square.dest_y, narrow.dest_y);
        assert!((narrow.dest_x - narrow.source_x).abs() < (square.dest_x - square.source_x).abs());
    }

    #[test]
    fn glitch_is_stable_for_same_seed() {
        let theme = Theme::dark();
        let area = Area::new((0, 0).into(), (8, 2).into());
        let ctx = VisualCtx::new(
            0.5,
            area,
            std::time::Instant::now(),
            1,
            1.0,
            MotionPolicy::default(),
            &theme,
            None,
            42,
        );
        let effect = VisualEffect::glitch().with_seed(123).intensity(1.0);
        let mut first = CellSample::new(3, 1, Stylized::plain('x'));
        let mut second = CellSample::new(3, 1, Stylized::plain('x'));

        effect.apply(&mut first, &ctx);
        effect.apply(&mut second, &ctx);

        assert_eq!(first, second);
    }

    #[test]
    fn reduced_motion_replaces_new_spatial_effects() {
        assert!(matches!(
            VisualEffect::reveal(WipeDirection::LeftToRight).reduced(),
            VisualEffect::Fade { from: 0.0, to: 1.0 }
        ));
        assert!(matches!(
            VisualEffect::dissolve_out().reduced(),
            VisualEffect::Fade { from: 1.0, to: 0.0 }
        ));
        assert!(matches!(
            VisualEffect::wave(WaveAxis::Rows).reduced(),
            VisualEffect::Fade { from: 0.0, to: 1.0 }
        ));
        assert!(matches!(
            VisualEffect::glitch().reduced(),
            VisualEffect::Fade { from: 0.0, to: 1.0 }
        ));
    }

    fn render_widget(widget: &impl Widget<()>, width: u16, height: u16) -> Buffer {
        let mut buffer = Buffer::new((width, height).into());
        let area = Area::new((0, 0).into(), (width, height).into());
        let mut chunk = Chunk::new(&mut buffer, area).unwrap();
        let store = WidgetStore::new();
        let animation_store = AnimationStore::new();
        let theme = Theme::dark();
        let geometry = RefCell::new(HashMap::<WidgetPath, Area>::new());
        let ctx = RenderCtx::new(&store, &animation_store, &theme, None, &geometry);
        widget.render(&mut chunk, &ctx);
        drop(chunk);
        buffer
    }

    fn cell(buffer: &Buffer, x: u16, y: u16) -> Option<&render::buffer::Cell> {
        let index = (y * buffer.size().width + x) as usize;
        buffer.content().get(index)
    }

    fn cell_char(buffer: &Buffer, x: u16, y: u16) -> Option<char> {
        cell(buffer, x, y)?.content.c
    }
}
