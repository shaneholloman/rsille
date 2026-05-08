//! Layered containers for overlays, popups, and anchored floating UI.

use render::area::Area;

use crate::event::Event;
use crate::focus::{FocusConfig, FocusScope};
use crate::layout::Constraints;
use crate::widget::{EventCtx, IntoWidget, RenderCtx, Widget, WidgetKey};

/// Cardinal anchor positions used for floating layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

/// Rectangle used to anchor a floating layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl AnchorRect {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn as_area(self, container: Area) -> Area {
        Area::new(
            (container.x() + self.x, container.y() + self.y).into(),
            (self.width, self.height).into(),
        )
    }
}

impl From<(u16, u16, u16, u16)> for AnchorRect {
    fn from(value: (u16, u16, u16, u16)) -> Self {
        Self::new(value.0, value.1, value.2, value.3)
    }
}

/// Placement strategy for an overlay child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPlacement {
    Fill,
    Floating {
        anchor: OverlayAnchor,
        popup_anchor: OverlayAnchor,
        offset_x: i16,
        offset_y: i16,
        width: Option<u16>,
        height: Option<u16>,
    },
    Anchored {
        rect: AnchorRect,
        anchor: OverlayAnchor,
        popup_anchor: OverlayAnchor,
        offset_x: i16,
        offset_y: i16,
        width: Option<u16>,
        height: Option<u16>,
    },
}

impl Default for OverlayPlacement {
    fn default() -> Self {
        Self::Fill
    }
}

/// Builder for a single floating layer.
pub struct OverlayLayer<M = ()> {
    widget: Box<dyn Widget<M>>,
    placement: OverlayPlacement,
    z_index: i16,
}

impl<M> std::fmt::Debug for OverlayLayer<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayLayer")
            .field("placement", &self.placement)
            .field("z_index", &self.z_index)
            .finish()
    }
}

impl<M> OverlayLayer<M> {
    pub fn new(widget: impl IntoWidget<M>) -> Self {
        Self {
            widget: widget.into_widget(),
            placement: OverlayPlacement::Floating {
                anchor: OverlayAnchor::Center,
                popup_anchor: OverlayAnchor::Center,
                offset_x: 0,
                offset_y: 0,
                width: None,
                height: None,
            },
            z_index: 0,
        }
    }

    pub fn placement(mut self, placement: OverlayPlacement) -> Self {
        self.placement = placement;
        self
    }

    pub fn floating(mut self, anchor: OverlayAnchor) -> Self {
        self.placement = OverlayPlacement::Floating {
            anchor,
            popup_anchor: anchor,
            offset_x: 0,
            offset_y: 0,
            width: None,
            height: None,
        };
        self
    }

    pub fn anchored(
        mut self,
        rect: impl Into<AnchorRect>,
        anchor: OverlayAnchor,
        popup_anchor: OverlayAnchor,
    ) -> Self {
        self.placement = OverlayPlacement::Anchored {
            rect: rect.into(),
            anchor,
            popup_anchor,
            offset_x: 0,
            offset_y: 0,
            width: None,
            height: None,
        };
        self
    }

    pub fn offset(mut self, offset_x: i16, offset_y: i16) -> Self {
        match &mut self.placement {
            OverlayPlacement::Fill => {}
            OverlayPlacement::Floating {
                offset_x: layer_x,
                offset_y: layer_y,
                ..
            }
            | OverlayPlacement::Anchored {
                offset_x: layer_x,
                offset_y: layer_y,
                ..
            } => {
                *layer_x = offset_x;
                *layer_y = offset_y;
            }
        }
        self
    }

    pub fn size(mut self, width: u16, height: u16) -> Self {
        match &mut self.placement {
            OverlayPlacement::Fill => {}
            OverlayPlacement::Floating {
                width: layer_width,
                height: layer_height,
                ..
            }
            | OverlayPlacement::Anchored {
                width: layer_width,
                height: layer_height,
                ..
            } => {
                *layer_width = Some(width);
                *layer_height = Some(height);
            }
        }
        self
    }

    pub fn z_index(mut self, z_index: i16) -> Self {
        self.z_index = z_index;
        self
    }
}

/// Stack renders all children into the same area in insertion order.
pub struct Stack<M = ()> {
    children: Vec<Box<dyn Widget<M>>>,
    widget_key: Option<String>,
    focus_scope: Option<FocusScope>,
}

impl<M> std::fmt::Debug for Stack<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stack")
            .field("children", &self.children.len())
            .finish()
    }
}

impl<M> Stack<M> {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            widget_key: None,
            focus_scope: None,
        }
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
}

impl<M> Default for Stack<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Widget<M> for Stack<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        ctx.record_bounds(area);

        for (index, child) in self.children.iter().enumerate() {
            ctx.render_child_at(
                chunk,
                WidgetKey::for_child(index, child.as_ref()),
                child.as_ref(),
                area,
            );
        }
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    fn constraints(&self) -> Constraints {
        let min_width = self
            .children
            .iter()
            .map(|child| child.constraints().min_width)
            .max()
            .unwrap_or(0);
        let min_height = self
            .children
            .iter()
            .map(|child| child.constraints().min_height)
            .max()
            .unwrap_or(0);

        Constraints {
            min_width,
            max_width: None,
            min_height,
            max_height: None,
            flex: Some(1.0),
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

/// Overlay renders a base widget and any number of popup layers above it.
pub struct Overlay<M = ()> {
    children: Vec<Box<dyn Widget<M>>>,
    placements: Vec<OverlayPlacement>,
    z_indices: Vec<i16>,
    widget_key: Option<String>,
    focus_scope: Option<FocusScope>,
}

impl<M> std::fmt::Debug for Overlay<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Overlay")
            .field("children", &self.children.len())
            .field("placements", &self.placements)
            .finish()
    }
}

impl<M> Overlay<M> {
    pub fn new(base: impl IntoWidget<M>) -> Self {
        Self {
            children: vec![base.into_widget()],
            placements: vec![OverlayPlacement::Fill],
            z_indices: vec![i16::MIN],
            widget_key: None,
            focus_scope: None,
        }
    }

    pub fn layer(mut self, layer: OverlayLayer<M>) -> Self {
        self.children.push(layer.widget);
        self.placements.push(layer.placement);
        self.z_indices.push(layer.z_index);
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

    fn anchor_point(area: Area, anchor: OverlayAnchor) -> (i32, i32) {
        let x0 = i32::from(area.x());
        let y0 = i32::from(area.y());
        let x1 = x0 + i32::from(area.width());
        let y1 = y0 + i32::from(area.height());
        let cx = x0 + i32::from(area.width()) / 2;
        let cy = y0 + i32::from(area.height()) / 2;

        match anchor {
            OverlayAnchor::TopLeft => (x0, y0),
            OverlayAnchor::Top => (cx, y0),
            OverlayAnchor::TopRight => (x1, y0),
            OverlayAnchor::Left => (x0, cy),
            OverlayAnchor::Center => (cx, cy),
            OverlayAnchor::Right => (x1, cy),
            OverlayAnchor::BottomLeft => (x0, y1),
            OverlayAnchor::Bottom => (cx, y1),
            OverlayAnchor::BottomRight => (x1, y1),
        }
    }

    fn anchored_area(
        anchor_area: Area,
        anchor: OverlayAnchor,
        popup_anchor: OverlayAnchor,
        width: u16,
        height: u16,
        offset_x: i16,
        offset_y: i16,
    ) -> Area {
        let (anchor_x, anchor_y) = Self::anchor_point(anchor_area, anchor);
        let popup_x = match popup_anchor {
            OverlayAnchor::TopLeft | OverlayAnchor::Left | OverlayAnchor::BottomLeft => anchor_x,
            OverlayAnchor::Top | OverlayAnchor::Center | OverlayAnchor::Bottom => {
                anchor_x - i32::from(width) / 2
            }
            OverlayAnchor::TopRight | OverlayAnchor::Right | OverlayAnchor::BottomRight => {
                anchor_x - i32::from(width)
            }
        } + i32::from(offset_x);
        let popup_y = match popup_anchor {
            OverlayAnchor::TopLeft | OverlayAnchor::Top | OverlayAnchor::TopRight => anchor_y,
            OverlayAnchor::Left | OverlayAnchor::Center | OverlayAnchor::Right => {
                anchor_y - i32::from(height) / 2
            }
            OverlayAnchor::BottomLeft | OverlayAnchor::Bottom | OverlayAnchor::BottomRight => {
                anchor_y - i32::from(height)
            }
        } + i32::from(offset_y);

        Area::new(
            (popup_x.max(0) as u16, popup_y.max(0) as u16).into(),
            (width, height).into(),
        )
    }

    fn placement_area(&self, container: Area, index: usize) -> Area {
        let child = &self.children[index];
        let constraints = child.constraints();
        let placement = self.placements[index];

        match placement {
            OverlayPlacement::Fill => container,
            OverlayPlacement::Floating {
                anchor,
                popup_anchor,
                offset_x,
                offset_y,
                width,
                height,
            } => {
                let width = width
                    .unwrap_or(constraints.min_width)
                    .min(container.width());
                let height = height
                    .unwrap_or(constraints.min_height)
                    .min(container.height());
                Self::anchored_area(
                    container,
                    anchor,
                    popup_anchor,
                    width,
                    height,
                    offset_x,
                    offset_y,
                )
            }
            OverlayPlacement::Anchored {
                rect,
                anchor,
                popup_anchor,
                offset_x,
                offset_y,
                width,
                height,
            } => {
                let width = width
                    .unwrap_or(constraints.min_width)
                    .min(container.width());
                let height = height
                    .unwrap_or(constraints.min_height)
                    .min(container.height());
                Self::anchored_area(
                    rect.as_area(container),
                    anchor,
                    popup_anchor,
                    width,
                    height,
                    offset_x,
                    offset_y,
                )
            }
        }
    }
}

impl<M> Widget<M> for Overlay<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 || self.children.is_empty() {
            return;
        }

        ctx.record_bounds(area);

        ctx.render_child_at(
            chunk,
            WidgetKey::for_child(0, self.children[0].as_ref()),
            self.children[0].as_ref(),
            area,
        );

        let mut layer_indices: Vec<usize> = (1..self.children.len()).collect();
        layer_indices.sort_by_key(|index| self.z_indices[*index]);

        for index in layer_indices {
            let layer_area = self.placement_area(area, index);
            if layer_area.width() == 0 || layer_area.height() == 0 {
                continue;
            }
            if layer_area.intersects(&area) {
                ctx.render_child_at(
                    chunk,
                    WidgetKey::for_child(index, self.children[index].as_ref()),
                    self.children[index].as_ref(),
                    layer_area,
                );
            }
        }
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    fn constraints(&self) -> Constraints {
        self.children
            .first()
            .map(|child| child.constraints())
            .unwrap_or_else(Constraints::content)
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

/// Create a full-area stack container.
pub fn stack<M>() -> Stack<M> {
    Stack::new()
}

/// Create an overlay container with a base widget.
pub fn overlay<M>(base: impl IntoWidget<M>) -> Overlay<M> {
    Overlay::new(base)
}
