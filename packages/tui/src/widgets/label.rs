//! Label widget — pure text display

use std::borrow::Cow;

use crate::event::Event;
use crate::layout::{Constraints, HorizontalAlign, VerticalAlign};
use crate::style::{Color, Style};
use crate::widget::{EventCtx, RenderCtx, Widget};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Label widget for displaying text.
#[derive(Debug, Clone)]
pub struct Label<M = ()> {
    content: String,
    style: Style,
    constraints: Option<Constraints>,
    horizontal_align: HorizontalAlign,
    vertical_align: VerticalAlign,
    tab_width: Option<u16>,
    widget_key: Option<String>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> Label<M> {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: Style::default(),
            constraints: None,
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Top,
            tab_width: None,
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

    /// Override the label's layout constraints.
    pub fn constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = Some(constraints);
        self
    }

    /// Give the label an exact width in terminal cells.
    pub fn width(mut self, width: u16) -> Self {
        let mut constraints = self.effective_constraints();
        constraints.min_width = width;
        constraints.max_width = Some(width);
        self.constraints = Some(constraints);
        self
    }

    /// Give the label an exact height in terminal rows.
    pub fn height(mut self, height: u16) -> Self {
        let mut constraints = self.effective_constraints();
        constraints.min_height = height;
        constraints.max_height = Some(height);
        self.constraints = Some(constraints);
        self
    }

    /// Give the label an exact width and height.
    pub fn fixed(mut self, width: u16, height: u16) -> Self {
        self.constraints = Some(Constraints::fixed(width, height));
        self
    }

    /// Set horizontal placement for each rendered line inside the allocated area.
    pub fn align(mut self, align: HorizontalAlign) -> Self {
        self.horizontal_align = align;
        self
    }

    /// Set vertical placement for the text block inside the allocated area.
    pub fn valign(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }

    /// Expand tab characters to terminal-cell tab stops while measuring and rendering.
    pub fn tab_width(mut self, width: u16) -> Self {
        self.tab_width = Some(width.max(1));
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

    fn effective_constraints(&self) -> Constraints {
        self.constraints.unwrap_or_else(|| {
            let (width, height) = label_size(&self.content, self.tab_width);
            Constraints {
                min_width: width,
                max_width: Some(width),
                min_height: height,
                max_height: Some(height),
                flex: None,
            }
        })
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
        let (_, content_height) = label_size(&self.content, self.tab_width);
        let y_offset = self.vertical_align.offset(area.height(), content_height);
        let visible_rows = area.height().saturating_sub(y_offset) as usize;

        for (row, line) in self.content.split('\n').enumerate() {
            if row >= visible_rows {
                break;
            }

            let y = y_offset + row as u16;

            let line = line.strip_suffix('\r').unwrap_or(line);
            let line = expand_tabs(line, self.tab_width);
            let line_width = display_width(line.as_ref());
            let x = self.horizontal_align.offset(area.width(), line_width);
            let _ = chunk.set_string(x, y, line.as_ref(), render_style);
        }
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    fn constraints(&self) -> Constraints {
        self.effective_constraints()
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Create a new label widget.
pub fn label<M>(content: impl Into<String>) -> Label<M> {
    Label::new(content)
}

fn label_size(content: &str, tab_width: Option<u16>) -> (u16, u16) {
    if content.is_empty() {
        return (0, 0);
    }

    let mut width: u16 = 0;
    let mut height: u16 = 0;
    for line in content.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let line = expand_tabs(line, tab_width);
        width = width.max(display_width(line.as_ref()));
        height = height.saturating_add(1);
    }

    (width, height)
}

fn display_width(content: &str) -> u16 {
    content.width().min(u16::MAX as usize) as u16
}

fn expand_tabs(content: &str, tab_width: Option<u16>) -> Cow<'_, str> {
    let Some(tab_width) = tab_width else {
        return Cow::Borrowed(content);
    };

    if !content.contains('\t') {
        return Cow::Borrowed(content);
    }

    let tab_width = tab_width as usize;
    let mut expanded = String::with_capacity(content.len());
    let mut current_width = 0;
    for ch in content.chars() {
        if ch == '\t' {
            let spaces = tab_width - (current_width % tab_width);
            expanded.extend(std::iter::repeat(' ').take(spaces));
            current_width += spaces;
        } else {
            expanded.push(ch);
            current_width += ch.width().unwrap_or(0);
        }
    }

    Cow::Owned(expanded)
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
        let constraints = Widget::constraints(&label);

        assert_eq!(constraints.min_width, 5);
        assert_eq!(constraints.max_width, Some(5));
        assert_eq!(constraints.min_height, 3);
        assert_eq!(constraints.max_height, Some(3));
    }

    #[test]
    fn label_width_and_height_override_intrinsic_constraints() {
        let label = label::<()>("alpha").width(12).height(4);
        let constraints = Widget::constraints(&label);

        assert_eq!(constraints.min_width, 12);
        assert_eq!(constraints.max_width, Some(12));
        assert_eq!(constraints.min_height, 4);
        assert_eq!(constraints.max_height, Some(4));
    }

    #[test]
    fn label_tab_width_expands_tabs_for_constraints() {
        let label = label::<()>("a\tb").tab_width(4);
        let constraints = Widget::constraints(&label);

        assert_eq!(constraints.min_width, 5);
        assert_eq!(constraints.max_width, Some(5));
    }

    #[test]
    fn multiline_label_renders_each_line() {
        let label = label::<()>("ab\ncd");
        let buffer = render_widget(&label, 4, 2);

        assert_eq!(cell_char(&buffer, 0, 0), Some('a'));
        assert_eq!(cell_char(&buffer, 1, 0), Some('b'));
        assert_eq!(cell_char(&buffer, 0, 1), Some('c'));
        assert_eq!(cell_char(&buffer, 1, 1), Some('d'));
    }

    #[test]
    fn label_can_center_multiline_content_in_allocated_area() {
        let label = label::<()>("ab\ncd")
            .fixed(6, 4)
            .align(HorizontalAlign::Center)
            .valign(VerticalAlign::Middle);
        let buffer = render_widget(&label, 6, 4);

        assert_eq!(cell_char(&buffer, 2, 1), Some('a'));
        assert_eq!(cell_char(&buffer, 3, 1), Some('b'));
        assert_eq!(cell_char(&buffer, 2, 2), Some('c'));
        assert_eq!(cell_char(&buffer, 3, 2), Some('d'));
    }

    #[test]
    fn label_can_align_content_to_bottom_right() {
        let label = label::<()>("xy")
            .fixed(5, 3)
            .align(HorizontalAlign::Right)
            .valign(VerticalAlign::Bottom);
        let buffer = render_widget(&label, 5, 3);

        assert_eq!(cell_char(&buffer, 3, 2), Some('x'));
        assert_eq!(cell_char(&buffer, 4, 2), Some('y'));
    }

    #[test]
    fn label_alignment_uses_unicode_display_width() {
        let label = label::<()>("你").width(4).align(HorizontalAlign::Center);
        let buffer = render_widget(&label, 4, 1);

        assert_eq!(cell_char(&buffer, 1, 0), Some('你'));
    }

    fn render_widget(widget: &Label<()>, width: u16, height: u16) -> Buffer {
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

    fn cell_char(buffer: &Buffer, x: u16, y: u16) -> Option<char> {
        let index = (y * buffer.size().width + x) as usize;
        buffer.content()[index].content.c
    }
}
