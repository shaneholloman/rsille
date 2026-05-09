//! Label widget — pure text display

use crate::event::Event;
use crate::layout::Constraints;
use crate::style::{Color, Style};
use crate::widget::{EventCtx, RenderCtx, Widget};

/// Label widget for displaying text.
#[derive(Debug, Clone)]
pub struct Label<M = ()> {
    content: String,
    style: Style,
    widget_key: Option<String>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> Label<M> {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: Style::default(),
            widget_key: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
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

impl<M> Widget<M> for Label<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }
        let theme_style = ctx.theme().styles.text;
        let final_style = self.style.merge(theme_style);
        let render_style = final_style.to_render_style();

        for (row, line) in self.content.split('\n').enumerate() {
            if row >= area.height() as usize {
                break;
            }

            let line = line.strip_suffix('\r').unwrap_or(line);
            let _ = chunk.set_string(0, row as u16, line, render_style);
        }
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    fn constraints(&self) -> Constraints {
        let (width, height) = label_size(&self.content);
        Constraints {
            min_width: width,
            max_width: Some(width),
            min_height: height,
            max_height: Some(height),
            flex: None,
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Create a new label widget.
pub fn label<M>(content: impl Into<String>) -> Label<M> {
    Label::new(content)
}

fn label_size(content: &str) -> (u16, u16) {
    use unicode_width::UnicodeWidthStr;

    if content.is_empty() {
        return (0, 0);
    }

    let mut width = 0;
    let mut height = 0;
    for line in content.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        width = width.max(line.width() as u16);
        height += 1;
    }

    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::AnimationStore;
    use crate::style::Theme;
    use crate::widget::{RenderCtx, Widget, WidgetPath, WidgetStore};
    use render::area::Area;
    use render::buffer::Buffer;
    use render::chunk::Chunk;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[test]
    fn multiline_label_constraints_use_lines() {
        let label = label::<()>("alpha\nbeta\n");
        let constraints = label.constraints();

        assert_eq!(constraints.min_width, 5);
        assert_eq!(constraints.max_width, Some(5));
        assert_eq!(constraints.min_height, 3);
        assert_eq!(constraints.max_height, Some(3));
    }

    #[test]
    fn multiline_label_renders_each_line() {
        let label = label::<()>("ab\ncd");
        let mut buffer = Buffer::new((4, 2).into());
        let area = Area::new((0, 0).into(), (4, 2).into());
        let mut chunk = Chunk::new(&mut buffer, area).unwrap();
        let store = WidgetStore::new();
        let animation_store = AnimationStore::new();
        let theme = Theme::dark();
        let geometry = RefCell::new(HashMap::<WidgetPath, Area>::new());
        let ctx = RenderCtx::new(&store, &animation_store, &theme, None, &geometry);

        label.render(&mut chunk, &ctx);

        assert_eq!(cell_char(&buffer, 0, 0), Some('a'));
        assert_eq!(cell_char(&buffer, 1, 0), Some('b'));
        assert_eq!(cell_char(&buffer, 0, 1), Some('c'));
        assert_eq!(cell_char(&buffer, 1, 1), Some('d'));
    }

    fn cell_char(buffer: &Buffer, x: u16, y: u16) -> Option<char> {
        let index = (y * buffer.size().width + x) as usize;
        buffer.content()[index].content.c
    }
}
