//! Progress and loading indicator widgets.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::layout::Constraints;
use crate::style::Style;
use crate::widget::{RenderCtx, Widget};

/// Horizontal progress bar.
#[derive(Debug, Clone)]
pub struct ProgressBar<M = ()> {
    value: f64,
    label: Option<String>,
    width: u16,
    custom_style: Option<Style>,
    fill_style: Option<Style>,
    widget_key: Option<String>,
    marker: std::marker::PhantomData<fn() -> M>,
}

impl<M> ProgressBar<M> {
    pub fn new(value: f64) -> Self {
        Self {
            value,
            label: None,
            width: 24,
            custom_style: None,
            fill_style: None,
            widget_key: None,
            marker: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn width(mut self, width: u16) -> Self {
        self.width = width.max(3);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }

    pub fn fill_style(mut self, style: Style) -> Self {
        self.fill_style = Some(style);
        self
    }
}

impl<M> Widget<M> for ProgressBar<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let theme = ctx.theme();
        let track_style = self
            .custom_style
            .map(|style| style.merge(theme.styles.interactive))
            .unwrap_or(theme.styles.interactive)
            .to_render_style();
        let fill_style = self
            .fill_style
            .unwrap_or(theme.styles.selected)
            .to_render_style();
        let value = self.value.clamp(0.0, 1.0);
        let bar_width = area.width().saturating_sub(2).max(1);
        let filled = ((bar_width as f64) * value).round() as u16;

        let _ = chunk.set_char(0, 0, '[', track_style);
        for x in 0..bar_width {
            let style = if x < filled { fill_style } else { track_style };
            let ch = if x < filled { '#' } else { '-' };
            let _ = chunk.set_char(x + 1, 0, ch, style);
        }
        let _ = chunk.set_char(bar_width + 1, 0, ']', track_style);

        if let Some(label) = self.label.as_ref() {
            let text = truncate_to_width(label, area.width() as usize);
            let x = area.width().saturating_sub(text.width() as u16) / 2;
            let _ = chunk.set_string(x, 0, &text, fill_style);
        }
    }

    fn constraints(&self) -> Constraints {
        Constraints {
            min_width: self.width,
            max_width: None,
            min_height: 1,
            max_height: Some(1),
            flex: Some(1.0),
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Small frame-driven loading indicator.
#[derive(Debug, Clone)]
pub struct LoadingIndicator<M = ()> {
    label: Option<String>,
    frame: usize,
    custom_style: Option<Style>,
    widget_key: Option<String>,
    marker: std::marker::PhantomData<fn() -> M>,
}

impl<M> LoadingIndicator<M> {
    pub fn new() -> Self {
        Self {
            label: None,
            frame: 0,
            custom_style: None,
            widget_key: None,
            marker: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn frame(mut self, frame: usize) -> Self {
        self.frame = frame;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }
}

impl<M> Default for LoadingIndicator<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Widget<M> for LoadingIndicator<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let style = self
            .custom_style
            .map(|style| style.merge(ctx.theme().styles.interactive))
            .unwrap_or(ctx.theme().styles.interactive)
            .to_render_style();
        let frames = ['|', '/', '-', '\\'];
        let frame = self.frame;
        let spinner = frames[frame % frames.len()];
        let text = if let Some(label) = self.label.as_ref() {
            format!("{spinner} {label}")
        } else {
            spinner.to_string()
        };
        let display = truncate_to_width(&text, area.width() as usize);
        let _ = chunk.set_string(0, 0, &display, style);
    }

    fn constraints(&self) -> Constraints {
        let width = self
            .label
            .as_ref()
            .map(|label| label.width() as u16 + 2)
            .unwrap_or(1);
        Constraints {
            min_width: width,
            max_width: None,
            min_height: 1,
            max_height: Some(1),
            flex: None,
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}

pub fn progress_bar<M>(value: f64) -> ProgressBar<M> {
    ProgressBar::new(value)
}

pub fn loading_indicator<M>() -> LoadingIndicator<M> {
    LoadingIndicator::new()
}

#[cfg(test)]
mod tests {
    use crate::animation::AnimationStore;
    use crate::style::Theme;
    use crate::widget::{RenderCtx, Widget, WidgetStore};

    use super::*;

    #[test]
    fn progress_renders_current_value_directly() {
        let widget = ProgressBar::<()>::new(0.5);
        let mut buffer = render::buffer::Buffer::new((10, 1).into());
        let area = render::area::Area::new((0, 0).into(), (10, 1).into());
        let mut chunk = render::chunk::Chunk::new(&mut buffer, area).unwrap();
        let store = WidgetStore::default();
        let animation_store = AnimationStore::new();
        let theme = Theme::dark();
        let geometry = std::cell::RefCell::new(std::collections::HashMap::new());
        let ctx = RenderCtx::new(&store, &animation_store, &theme, None, &geometry);

        widget.render(&mut chunk, &ctx);

        let rendered: String = (0..10)
            .map(|x| {
                let index = x as usize;
                buffer.content()[index].content.c.unwrap()
            })
            .collect();
        assert_eq!(rendered, "[####----]");
    }
}
