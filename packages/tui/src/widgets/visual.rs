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
    AnimationSpec, Direction as AnimationDirection, MotionPolicy, Repeat, Timeline, Transition,
    TransitionEffect,
};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::offscreen::{for_each_blit_cell, render_to_offscreen, BlitOptions, BlitRegion};
use crate::style::{Color, Style, Theme};
use crate::widget::{IntoWidget, RenderCtx, Widget, WidgetKey};

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
            .finish()
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
}

impl<M> Widget<M> for Visual<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        ctx.record_bounds(area);

        if self.effects.is_empty() && self.animation.is_none() && self.progress.is_none() {
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
        let progress = self.resolve_progress(ctx);
        let seed = self
            .seed
            .unwrap_or_else(|| stable_seed(&self.channel, self.widget_key.as_deref()));
        let resolved_config = self.resolve_config(ctx.theme());
        let visual_ctx = VisualCtx::new(
            progress,
            area,
            ctx.now(),
            ctx.frame(),
            resolved_config.cell_aspect,
            ctx.motion_policy(),
            ctx.theme(),
            self.effect_preset.as_deref(),
            seed,
        );
        blit_with_effects(chunk, &offscreen, &self.effects, &visual_ctx);
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

    pub fn with_seed(mut self, seed: u64) -> Self {
        if let Self::Shatter { seed: current, .. } = &mut self {
            *current = seed;
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
        if let Self::Gradient { phase: current, .. } = &mut self {
            *current = phase;
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

impl VisualEffect {
    fn apply(&self, sample: &mut CellSample, ctx: &VisualCtx<'_>) {
        let progress = ctx.progress;
        match *self {
            Self::Fade { from, to } => {
                let alpha = lerp(from, to, progress).clamp(0.0, 1.0);
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
                    gradient_position(sample.dest_x, sample.dest_y, ctx.area, direction) + phase;
                let t = if (0.0..=1.0).contains(&shifted) {
                    shifted
                } else {
                    shifted.rem_euclid(1.0)
                };
                let color = start.interpolate(end, t);
                apply_color(&mut sample.content, color, target);
            }
            Self::Shatter {
                seed,
                spread_x,
                spread_y,
                fade,
            } => {
                let t = ease_out_cubic(progress);
                let jitter = hash_pair(sample.source_x as u16, sample.source_y as u16, seed);
                let angle = jitter.0 * std::f64::consts::TAU;
                let force = 0.35 + jitter.1 * 0.95;
                let lift = 0.3
                    + noise(
                        sample.source_y as u16,
                        sample.source_x as u16,
                        seed ^ 0xBEEF,
                    ) * 0.8;
                sample.dest_x += ctx.aspect_adjusted_x_offset(angle.cos() * spread_x * force * t);
                sample.dest_y += (angle.sin() * spread_y * force - spread_y * lift) * t;

                if fade {
                    let threshold =
                        noise(sample.dest_x as u16, sample.dest_y as u16, seed ^ 0xFADE);
                    sample.visible = threshold > progress * 0.78;
                }
            }
            Self::MagicLamp { anchor, squeeze } => {
                let (anchor_x, anchor_y) = anchor.resolve(ctx.area.width(), ctx.area.height());
                let open = ease_out_cubic(progress);
                let pull = 1.0 - open;
                let scale = open + pull * squeeze;
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
                    anchor_y + (sample.dest_y - anchor_y) * (open * open + pull * squeeze);
            }
        }
    }
}

fn sanitize_cell_aspect(cell_aspect: f64) -> f64 {
    if cell_aspect.is_finite() {
        cell_aspect.max(f64::EPSILON)
    } else {
        1.0
    }
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
