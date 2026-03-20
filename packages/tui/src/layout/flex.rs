//! Flex layout container

use render::area::Area;

use super::border_renderer::{render_background, render_border};
use super::taffy_bridge::TaffyBridge;
use crate::event::Event;
use crate::focus::{FocusConfig, FocusScope};
use crate::layout::Constraints;
use crate::style::{BorderStyle, Padding, Style, ThemeManager};
use crate::widget::{EventCtx, IntoWidget, RenderCtx, Widget, WidgetKey};
use taffy::style::{AlignItems, JustifyContent};

/// Layout direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Vertical,
    Horizontal,
}

/// Flex widget that arranges children using flexbox layout.
///
/// This is a pure layout container — it does not handle events or manage focus.
/// The framework handles focus chain building and event routing externally.
pub struct Flex<M = ()> {
    children: Vec<Box<dyn Widget<M>>>,
    direction: Direction,
    gap: u16,
    padding: Padding,
    border: Option<BorderStyle>,
    style: Style,
    align_items: Option<AlignItems>,
    justify_content: Option<JustifyContent>,
    focus_scope: Option<FocusScope>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Flex<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flex")
            .field("children", &self.children.len())
            .field("direction", &self.direction)
            .field("gap", &self.gap)
            .field("padding", &self.padding)
            .field("border", &self.border)
            .field("style", &self.style)
            .field("align_items", &self.align_items)
            .field("justify_content", &self.justify_content)
            .finish()
    }
}

impl<M> Flex<M> {
    fn with_direction(children: Vec<Box<dyn Widget<M>>>, direction: Direction) -> Self {
        Self {
            children,
            direction,
            gap: 0,
            padding: Padding::ZERO,
            border: None,
            style: Style::default(),
            align_items: None,
            justify_content: None,
            focus_scope: None,
            widget_key: None,
        }
    }

    pub fn vertical(children: Vec<Box<dyn Widget<M>>>) -> Self {
        Self::with_direction(children, Direction::Vertical)
    }

    pub fn horizontal(children: Vec<Box<dyn Widget<M>>>) -> Self {
        Self::with_direction(children, Direction::Horizontal)
    }

    pub fn new() -> Self {
        Self::vertical(Vec::new())
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn align_items(mut self, align: AlignItems) -> Self {
        self.align_items = Some(align);
        self
    }

    pub fn justify_content(mut self, justify: JustifyContent) -> Self {
        self.justify_content = Some(justify);
        self
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn focus_scope(mut self, scope: FocusScope) -> Self {
        self.focus_scope = Some(scope);
        self
    }

    pub fn trap_focus(self) -> Self {
        self.focus_scope(
            FocusScope::new()
                .trap_tab(true)
                .entry(crate::focus::ScopeEntry::LastFocused),
        )
    }

    pub fn when<F>(self, condition: bool, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        if condition {
            f(self)
        } else {
            self
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Widget<M>>) {
        self.children.push(child);
    }

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

impl<M: Send + Sync> Flex<M> {
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
            .extend(widgets.into_iter().map(|w| w.into_widget()));
        self
    }
}

impl<M: Send + Sync> Widget<M> for Flex<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let theme_style = ThemeManager::global().with_theme(|theme| theme.styles.surface);
        let final_style = self.style.merge(theme_style);
        let render_style = final_style.to_render_style();

        let border_area = if self.border.is_some() {
            if area.width() < 2 || area.height() < 2 {
                return;
            }
            Area::new(
                (area.x() + 1, area.y() + 1).into(),
                (area.width() - 2, area.height() - 2).into(),
            )
        } else {
            area
        };

        if final_style.bg_color.is_some() {
            render_background(chunk, render_style);
        }

        if let Some(border) = self.border {
            render_border(chunk, border, render_style);
        }

        let inner = border_area.shrink_saturating(
            self.padding.top,
            self.padding.bottom,
            self.padding.left,
            self.padding.right,
        );

        if inner.width() == 0 || inner.height() == 0 {
            return;
        }

        let mut bridge = TaffyBridge::new();
        let child_areas = match bridge.compute_layout(
            &self.children,
            inner,
            self.direction,
            self.gap,
            self.align_items,
            self.justify_content,
        ) {
            Ok(areas) => areas,
            Err(_) => return,
        };

        for (index, (child, child_area)) in self.children.iter().zip(child_areas).enumerate() {
            if child_area.width() == 0 || child_area.height() == 0 {
                continue;
            }

            if !child_area.intersects(&inner) {
                continue;
            }

            if let Ok(mut child_chunk) = chunk.from_area(child_area) {
                let child_ctx = ctx.child_ctx(WidgetKey::for_child(index, child.as_ref()));
                child.render(&mut child_chunk, &child_ctx);
            }
        }
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {
        // Containers don't handle events — the framework routes directly to focused widgets.
    }

    fn constraints(&self) -> Constraints {
        let border_size = if self.border.is_some() { 2 } else { 0 };

        if self.children.is_empty() {
            return Constraints::fixed(
                self.padding.horizontal_total() + border_size,
                self.padding.vertical_total() + border_size,
            );
        }

        match self.direction {
            Direction::Vertical => {
                let total_height = self
                    .children
                    .iter()
                    .map(|c| c.constraints().min_height)
                    .sum::<u16>()
                    + (self.children.len() as u16 - 1) * self.gap
                    + self.padding.vertical_total()
                    + border_size;

                let max_width = self
                    .children
                    .iter()
                    .map(|c| c.constraints().min_width)
                    .max()
                    .unwrap_or(0)
                    + self.padding.horizontal_total()
                    + border_size;

                Constraints {
                    min_width: max_width,
                    max_width: None,
                    min_height: total_height,
                    max_height: None,
                    flex: Some(1.0),
                }
            }
            Direction::Horizontal => {
                let total_width = self
                    .children
                    .iter()
                    .map(|c| c.constraints().min_width)
                    .sum::<u16>()
                    + (self.children.len() as u16 - 1) * self.gap
                    + self.padding.horizontal_total()
                    + border_size;

                let max_height = self
                    .children
                    .iter()
                    .map(|c| c.constraints().min_height)
                    .max()
                    .unwrap_or(0)
                    + self.padding.vertical_total()
                    + border_size;

                Constraints {
                    min_width: total_width,
                    max_width: None,
                    min_height: max_height,
                    max_height: Some(max_height),
                    flex: None,
                }
            }
        }
    }

    fn children(&self) -> &[Box<dyn Widget<M>>] {
        &self.children
    }

    fn focus_config(&self) -> FocusConfig {
        self.focus_scope
            .clone()
            .map(FocusConfig::Scope)
            .unwrap_or(FocusConfig::None)
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

impl<M> Default for Flex<M> {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new empty vertical flex layout.
pub fn col<M>() -> Flex<M> {
    Flex::new()
}

/// Create a new empty horizontal flex layout.
pub fn row<M>() -> Flex<M> {
    Flex::horizontal(Vec::new())
}
