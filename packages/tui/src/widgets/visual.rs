//! Visual post-processing effects for arbitrary widgets.
//!
//! This wrapper renders its child into an offscreen buffer, then maps the
//! resulting terminal cells back into the target chunk with optional color and
//! geometry transforms.

use std::time::Duration;

use crossterm::style::{Color as CrosstermColor, Colors};
use render::area::Area;
use render::buffer::Buffer;
use render::style::Stylized;

use crate::animation::{
    AnimationSpec, Direction as AnimationDirection, Repeat, Timeline, Transition, TransitionEffect,
};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::style::{Color, Style};
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
}

impl<M> std::fmt::Debug for Visual<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Visual")
            .field("effects", &self.effects)
            .field("animation", &self.animation)
            .field("progress", &self.progress)
            .field("channel", &self.channel)
            .finish()
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

    fn render_child_to_buffer(&self, area: Area, ctx: &RenderCtx) -> Option<Buffer> {
        let mut offscreen = Buffer::new(area.real_size());
        let mut offscreen_chunk = render::chunk::Chunk::new(&mut offscreen, area).ok()?;
        let child_ctx = ctx.child_ctx(WidgetKey::for_child(0, self.child.as_ref()));
        self.child.render(&mut offscreen_chunk, &child_ctx);
        Some(offscreen)
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

        let Some(offscreen) = self.render_child_to_buffer(area, ctx) else {
            return;
        };
        let progress = self.resolve_progress(ctx);
        blit_with_effects(chunk, area, &offscreen, &self.effects, progress);
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

fn blit_with_effects(
    chunk: &mut render::chunk::Chunk,
    area: Area,
    source: &Buffer,
    effects: &[VisualEffect],
    progress: f64,
) {
    let source_width = source.size().width as usize;

    for local_y in 0..area.height() {
        let source_y = area.y().saturating_add(local_y);
        for local_x in 0..area.width() {
            let source_x = area.x().saturating_add(local_x);
            let index = source_y as usize * source_width + source_x as usize;
            let Some(cell) = source.content().get(index) else {
                continue;
            };
            if cell.is_occupied {
                continue;
            }

            let Some(ch) = cell.content.c else {
                continue;
            };
            if ch == ' ' && !cell.content.has_color() && !cell.content.has_attr() {
                continue;
            }

            let mut sample = CellSample {
                x: local_x as f64,
                y: local_y as f64,
                content: cell.content,
                visible: true,
            };

            for effect in effects {
                effect.apply(&mut sample, area, progress);
                if !sample.visible {
                    break;
                }
            }

            if !sample.visible {
                continue;
            }

            let dest_x = sample.x.round() as i32;
            let dest_y = sample.y.round() as i32;
            if dest_x < 0
                || dest_y < 0
                || dest_x >= area.width() as i32
                || dest_y >= area.height() as i32
            {
                continue;
            }

            let _ = chunk.set_forced(dest_x as u16, dest_y as u16, sample.content);
        }
    }
}

#[derive(Clone, Copy)]
struct CellSample {
    x: f64,
    y: f64,
    content: Stylized,
    visible: bool,
}

impl VisualEffect {
    fn apply(&self, sample: &mut CellSample, area: Area, progress: f64) {
        let progress = progress.clamp(0.0, 1.0);
        match *self {
            Self::Fade { from, to } => {
                let alpha = lerp(from, to, progress).clamp(0.0, 1.0);
                let threshold = noise(sample.x as u16, sample.y as u16, 0xFAD3);
                sample.visible = threshold <= alpha;
            }
            Self::Gradient {
                start,
                end,
                direction,
                target,
                phase,
            } => {
                let shifted = gradient_position(sample.x, sample.y, area, direction) + phase;
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
                let jitter = hash_pair(sample.x as u16, sample.y as u16, seed);
                let angle = jitter.0 * std::f64::consts::TAU;
                let force = 0.35 + jitter.1 * 0.95;
                let lift = 0.3 + noise(sample.y as u16, sample.x as u16, seed ^ 0xBEEF) * 0.8;
                sample.x += angle.cos() * spread_x * force * t;
                sample.y += (angle.sin() * spread_y * force - spread_y * lift) * t;

                if fade {
                    let threshold = noise(sample.x as u16, sample.y as u16, seed ^ 0xFADE);
                    sample.visible = threshold > progress * 0.78;
                }
            }
            Self::MagicLamp { anchor, squeeze } => {
                let (anchor_x, anchor_y) = anchor.resolve(area.width(), area.height());
                let open = ease_out_cubic(progress);
                let pull = 1.0 - open;
                let scale = open + pull * squeeze;
                let row = if area.height() <= 1 {
                    0.5
                } else {
                    sample.y / area.height().saturating_sub(1) as f64
                };
                let bow =
                    ((row - 0.5) * std::f64::consts::PI).sin() * pull * area.width() as f64 * 0.16;

                sample.x = anchor_x + (sample.x - anchor_x) * scale + bow;
                sample.y = anchor_y + (sample.y - anchor_y) * (open * open + pull * squeeze);
            }
        }
    }
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
    use crate::style::Theme;
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
