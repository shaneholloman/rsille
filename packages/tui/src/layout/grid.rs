//! Grid layout container

use render::area::Area;

use super::border_renderer::{render_background, render_border};
use super::grid_placement::GridPlacement;
use super::grid_track::GridTrack;
use super::taffy_bridge::TaffyBridge;
use crate::event::Event;
use crate::focus::{FocusConfig, FocusScope};
use crate::layout::Constraints;
use crate::style::{BorderStyle, Padding, Style};
use crate::widget::{EventCtx, IntoWidget, RenderCtx, Widget, WidgetKey};
use taffy::style::{AlignItems, JustifyItems};

/// Grid layout widget that arranges children in a 2D grid.
///
/// This is a pure layout container — it does not handle events or manage focus.
pub struct Grid<M = ()> {
    children: Vec<Box<dyn Widget<M>>>,
    placements: Vec<GridPlacement>,
    template_columns: Vec<GridTrack>,
    template_rows: Vec<GridTrack>,
    gap: u16,
    gap_row: Option<u16>,
    gap_column: Option<u16>,
    padding: Padding,
    border: Option<BorderStyle>,
    style: Style,
    align_items: Option<AlignItems>,
    justify_items: Option<JustifyItems>,
    focus_scope: Option<FocusScope>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Grid<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Grid")
            .field("children", &self.children.len())
            .field("template_columns", &self.template_columns)
            .field("template_rows", &self.template_rows)
            .field("gap", &self.gap)
            .field("padding", &self.padding)
            .field("border", &self.border)
            .field("style", &self.style)
            .finish()
    }
}

impl<M> Grid<M> {
    pub fn new() -> Self {
        Self {
            children: Vec::new(),
            placements: Vec::new(),
            template_columns: vec![GridTrack::Fr(1.0)],
            template_rows: vec![GridTrack::Auto],
            gap: 0,
            gap_row: None,
            gap_column: None,
            padding: Padding::ZERO,
            border: None,
            style: Style::default(),
            align_items: None,
            justify_items: None,
            focus_scope: None,
            widget_key: None,
        }
    }

    pub fn columns(mut self, template: &str) -> Self {
        self.template_columns = GridTrack::parse_template(template);
        self
    }

    pub fn rows(mut self, template: &str) -> Self {
        self.template_rows = GridTrack::parse_template(template);
        self
    }

    pub fn template_columns(mut self, tracks: Vec<GridTrack>) -> Self {
        self.template_columns = tracks;
        self
    }

    pub fn template_rows(mut self, tracks: Vec<GridTrack>) -> Self {
        self.template_rows = tracks;
        self
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn gap_row(mut self, gap: u16) -> Self {
        self.gap_row = Some(gap);
        self
    }

    pub fn gap_column(mut self, gap: u16) -> Self {
        self.gap_column = Some(gap);
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

    pub fn justify_items(mut self, justify: JustifyItems) -> Self {
        self.justify_items = Some(justify);
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

    pub fn len(&self) -> usize {
        self.children.len()
    }

    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }
}

impl<M> Grid<M> {
    pub fn child(mut self, widget: impl IntoWidget<M>) -> Self {
        self.children.push(widget.into_widget());
        self.placements.push(GridPlacement::default());
        self
    }

    pub fn child_at(mut self, widget: impl IntoWidget<M>, placement: GridPlacement) -> Self {
        self.children.push(widget.into_widget());
        self.placements.push(placement);
        self
    }

    pub fn child_area(self, widget: impl IntoWidget<M>, column: i16, row: i16) -> Self {
        self.child_at(widget, GridPlacement::new().area(column, row))
    }

    pub fn child_span(
        self,
        widget: impl IntoWidget<M>,
        column_start: i16,
        row_start: i16,
        column_span: u16,
        row_span: u16,
    ) -> Self {
        self.child_at(
            widget,
            GridPlacement::new().area_span(column_start, row_start, column_span, row_span),
        )
    }

    pub fn children<I>(mut self, widgets: I) -> Self
    where
        I: IntoIterator,
        I::Item: IntoWidget<M>,
    {
        for widget in widgets {
            self.children.push(widget.into_widget());
            self.placements.push(GridPlacement::default());
        }
        self
    }
}

impl<M> Widget<M> for Grid<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let render_style = self.style.to_render_style();

        let mut content_area = area;

        if self.style.bg_color.is_some() {
            render_background(chunk, render_style);
        }

        if let Some(ref border) = self.border {
            render_border(chunk, *border, render_style);
            content_area = Area::new(
                (area.x() + 1, area.y() + 1).into(),
                (
                    area.width().saturating_sub(2),
                    area.height().saturating_sub(2),
                )
                    .into(),
            );
        }

        let padded_area = Area::new(
            (
                content_area.x() + self.padding.left,
                content_area.y() + self.padding.top,
            )
                .into(),
            (
                content_area
                    .width()
                    .saturating_sub(self.padding.left + self.padding.right),
                content_area
                    .height()
                    .saturating_sub(self.padding.top + self.padding.bottom),
            )
                .into(),
        );

        if padded_area.width() == 0 || padded_area.height() == 0 {
            return;
        }

        let gap_row = self.gap_row.unwrap_or(self.gap);
        let gap_column = self.gap_column.unwrap_or(self.gap);

        let mut bridge = TaffyBridge::new();

        let items: Vec<(&dyn Widget<M>, &GridPlacement)> = self
            .children
            .iter()
            .zip(self.placements.iter())
            .map(|(w, p)| (w.as_ref() as &dyn Widget<M>, p))
            .collect();

        let child_areas = match bridge.compute_grid_layout_with_placement(
            &items,
            padded_area,
            &self.template_columns,
            &self.template_rows,
            gap_row,
            gap_column,
            self.align_items,
            self.justify_items,
        ) {
            Ok(areas) => areas,
            Err(_) => return,
        };

        for (index, (child, child_area)) in self.children.iter().zip(child_areas.iter()).enumerate()
        {
            if child_area.width() == 0 || child_area.height() == 0 {
                continue;
            }

            if !child_area.intersects(&padded_area) {
                continue;
            }

            if let Ok(mut child_chunk) = chunk.from_area(*child_area) {
                let child_ctx = ctx.child_ctx(WidgetKey::for_child(index, child.as_ref()));
                child.render(&mut child_chunk, &child_ctx);
            }
        }
    }

    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {
        // Containers don't handle events — the framework routes directly to focused widgets.
    }

    fn constraints(&self) -> Constraints {
        Constraints {
            min_width: 0,
            max_width: None,
            min_height: 0,
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

impl<M> Default for Grid<M> {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new grid layout.
pub fn grid<M>() -> Grid<M> {
    Grid::new()
}
