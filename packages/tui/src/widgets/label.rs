//! Label widget — pure text display

use crate::event::Event;
use crate::layout::Constraints;
use crate::style::{Color, Style, ThemeManager};
use crate::widget::{EventCtx, RenderCtx, Widget};

/// Label widget for displaying text.
#[derive(Debug, Clone)]
pub struct Label<M = ()> {
    content: String,
    style: Style,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> Label<M> {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: Style::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.style = self.style.fg(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.style = self.style.bg(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.style = self.style.bold();
        self
    }

    pub fn italic(mut self) -> Self {
        self.style = self.style.italic();
        self
    }

    pub fn underline(mut self) -> Self {
        self.style = self.style.underlined();
        self
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

impl<M: Send + Sync> Widget<M> for Label<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, _ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }
        let theme_style = ThemeManager::global().with_theme(|theme| theme.styles.text);
        let final_style = self.style.merge(theme_style);
        let _ = chunk.set_string(0, 0, &self.content, final_style.to_render_style());
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    fn constraints(&self) -> Constraints {
        use unicode_width::UnicodeWidthStr;
        let width = self.content.width() as u16;
        let height = if self.content.is_empty() { 0 } else { 1 };
        Constraints {
            min_width: width,
            max_width: Some(width),
            min_height: height,
            max_height: Some(height),
            flex: None,
        }
    }
}

/// Create a new label widget.
pub fn label<M>(content: impl Into<String>) -> Label<M> {
    Label::new(content)
}
