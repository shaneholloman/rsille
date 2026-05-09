//! Panel container widget.

use render::area::Area;
use unicode_width::UnicodeWidthChar;

use crate::event::Event;
use crate::layout::border_renderer;
use crate::layout::taffy_bridge::TaffyBridge;
use crate::layout::{Constraints, Direction};
use crate::style::{BorderStyle, Padding, Style};
use crate::widget::{EventCtx, IntoWidget, RenderCtx, Widget, WidgetKey};

/// Bordered container for grouping related content.
pub struct Panel<M = ()> {
    children: Vec<Box<dyn Widget<M>>>,
    title: Option<String>,
    border: Option<BorderStyle>,
    padding: Padding,
    gap: u16,
    style: Style,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Panel<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Panel")
            .field("children", &self.children.len())
            .field("title", &self.title)
            .field("border", &self.border)
            .field("padding", &self.padding)
            .field("gap", &self.gap)
            .field("style", &self.style)
            .finish()
    }
}

impl<M> Panel<M> {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            title: None,
            border: Some(BorderStyle::Single),
            padding: Padding::uniform(1),
            gap: 0,
            style: Style::default(),
            widget_key: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn borderless(mut self) -> Self {
        self.border = None;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn child(mut self, widget: impl IntoWidget<M>) -> Self {
        self.children.push(widget.into_widget());
        self
    }

    pub fn children<I>(mut self, widgets: I) -> Self
    where
        I: IntoIterator,
        I::Item: IntoWidget<M>,
    {
        self.children
            .extend(widgets.into_iter().map(IntoWidget::into_widget));
        self
    }
}

impl<M> Default for Panel<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Widget<M> for Panel<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        ctx.record_bounds(area);

        let surface_style = self.style.merge(ctx.theme().styles.surface_elevated);
        let border_style = ctx.theme().styles.border.merge(surface_style);
        let render_style = surface_style.to_render_style();
        let border_render_style = border_style.to_render_style();
        let _ = chunk.fill(0, 0, area.width(), area.height(), ' ', render_style);

        let inner = if let Some(border) = self.border {
            if area.width() < 2 || area.height() < 2 {
                return;
            }
            border_renderer::render_border(chunk, border, border_render_style);
            if let Some(title) = self.title.as_ref() {
                let available = area.width().saturating_sub(4) as usize;
                let display = truncate_to_width(title, available);
                if !display.is_empty() {
                    let _ = chunk.set_string(2, 0, &display, border_render_style);
                }
            }
            Area::new(
                (area.x() + 1, area.y() + 1).into(),
                (area.width() - 2, area.height() - 2).into(),
            )
        } else {
            area
        };

        let content = inner.shrink_saturating(
            self.padding.top,
            self.padding.bottom,
            self.padding.left,
            self.padding.right,
        );
        render_children_vertical(chunk, ctx, &self.children, content, self.gap);
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    fn constraints(&self) -> Constraints {
        container_constraints(
            &self.children,
            self.padding,
            self.border.is_some(),
            self.gap,
        )
    }

    fn children(&self) -> &[Box<dyn Widget<M>>] {
        &self.children
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

pub(crate) fn render_children_vertical<M>(
    chunk: &mut render::chunk::Chunk,
    ctx: &RenderCtx,
    children: &[Box<dyn Widget<M>>],
    content: Area,
    gap: u16,
) {
    if content.width() == 0 || content.height() == 0 {
        return;
    }

    let mut bridge = TaffyBridge::new();
    let child_areas =
        match bridge.compute_layout(children, content, Direction::Vertical, gap, None, None) {
            Ok(areas) => areas,
            Err(_) => {
                render_children_vertical_min_size(chunk, ctx, children, content, gap);
                return;
            }
        };

    for (index, (child, child_area)) in children.iter().zip(child_areas).enumerate() {
        let child_area = Area::new(
            (content.x(), child_area.y()).into(),
            (content.width(), child_area.height()).into(),
        );

        if child_area.width() == 0 || child_area.height() == 0 {
            continue;
        }

        if !child_area.intersects(&content) {
            continue;
        }

        ctx.render_child_at(
            chunk,
            WidgetKey::for_child(index, child.as_ref()),
            child.as_ref(),
            child_area,
        );
    }
}

fn render_children_vertical_min_size<M>(
    chunk: &mut render::chunk::Chunk,
    ctx: &RenderCtx,
    children: &[Box<dyn Widget<M>>],
    content: Area,
    gap: u16,
) {
    let mut y = content.y();
    let bottom = content.y().saturating_add(content.height());
    for (index, child) in children.iter().enumerate() {
        if y >= bottom {
            break;
        }
        let remaining = bottom - y;
        let height = child.constraints().min_height.min(remaining);
        if height == 0 {
            continue;
        }
        let child_area = Area::new((content.x(), y).into(), (content.width(), height).into());
        ctx.render_child_at(
            chunk,
            WidgetKey::for_child(index, child.as_ref()),
            child.as_ref(),
            child_area,
        );
        y = y.saturating_add(height).saturating_add(gap);
    }
}

pub(crate) fn container_constraints<M>(
    children: &[Box<dyn Widget<M>>],
    padding: Padding,
    has_border: bool,
    gap: u16,
) -> Constraints {
    let border_size = if has_border { 2 } else { 0 };
    let child_width = children
        .iter()
        .map(|child| child.constraints().min_width)
        .max()
        .unwrap_or(0);
    let child_height = children
        .iter()
        .map(|child| child.constraints().min_height)
        .sum::<u16>()
        .saturating_add(gap.saturating_mul(children.len().saturating_sub(1) as u16));

    Constraints {
        min_width: child_width
            .saturating_add(padding.horizontal_total())
            .saturating_add(border_size),
        max_width: None,
        min_height: child_height
            .saturating_add(padding.vertical_total())
            .saturating_add(border_size),
        max_height: None,
        flex: Some(1.0),
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

pub fn panel<M>() -> Panel<M> {
    Panel::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::AnimationStore;
    use crate::event::Event;
    use crate::style::Theme;
    use crate::widget::{RenderCtx, Widget, WidgetPath, WidgetStore};
    use render::buffer::Buffer;
    use render::chunk::Chunk;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[test]
    fn panel_gives_flex_child_remaining_vertical_space() {
        let recorded = Rc::new(RefCell::new(None));
        let child = RecordingWidget {
            recorded: Rc::clone(&recorded),
            constraints: Constraints {
                min_width: 4,
                max_width: None,
                min_height: 2,
                max_height: None,
                flex: Some(1.0),
            },
        };
        let panel = panel::<()>()
            .borderless()
            .padding(Padding::ZERO)
            .gap(1)
            .child(crate::widgets::label("head"))
            .child(child);

        let mut buffer = Buffer::new((20, 8).into());
        let area = Area::new((0, 0).into(), (20, 8).into());
        let mut chunk = Chunk::new(&mut buffer, area).unwrap();
        let store = WidgetStore::new();
        let animation_store = AnimationStore::new();
        let theme = Theme::dark();
        let geometry = RefCell::new(HashMap::<WidgetPath, Area>::new());
        let ctx = RenderCtx::new(&store, &animation_store, &theme, None, &geometry);

        panel.render(&mut chunk, &ctx);

        assert_eq!(
            *recorded.borrow(),
            Some(Area::new((0, 2).into(), (20, 6).into()))
        );
    }

    #[test]
    fn panel_still_passes_full_content_width_to_fixed_children() {
        let recorded = Rc::new(RefCell::new(None));
        let child = RecordingWidget {
            recorded: Rc::clone(&recorded),
            constraints: Constraints::fixed(4, 2),
        };
        let panel = panel::<()>()
            .borderless()
            .padding(Padding::ZERO)
            .child(child);

        let mut buffer = Buffer::new((20, 8).into());
        let area = Area::new((0, 0).into(), (20, 8).into());
        let mut chunk = Chunk::new(&mut buffer, area).unwrap();
        let store = WidgetStore::new();
        let animation_store = AnimationStore::new();
        let theme = Theme::dark();
        let geometry = RefCell::new(HashMap::<WidgetPath, Area>::new());
        let ctx = RenderCtx::new(&store, &animation_store, &theme, None, &geometry);

        panel.render(&mut chunk, &ctx);

        assert_eq!(
            *recorded.borrow(),
            Some(Area::new((0, 0).into(), (20, 2).into()))
        );
    }

    struct RecordingWidget {
        recorded: Rc<RefCell<Option<Area>>>,
        constraints: Constraints,
    }

    impl Widget<()> for RecordingWidget {
        fn render(&self, chunk: &mut render::chunk::Chunk, _ctx: &RenderCtx) {
            *self.recorded.borrow_mut() = Some(chunk.area());
        }

        fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<()>) {}

        fn constraints(&self) -> Constraints {
            self.constraints
        }
    }
}
