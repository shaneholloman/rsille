//! Data table widget with internal row navigation.

use std::sync::Arc;

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::event::{Event, KeyCode};
use crate::focus::FocusConfig;
use crate::layout::border_renderer;
use crate::layout::Constraints;
use crate::style::{BorderStyle, Style, ThemeManager};
use crate::widget::{EventCtx, EventPhase, RenderCtx, Widget};

/// Column alignment within a data table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Data table column definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableColumn {
    pub title: String,
    pub width: u16,
    pub align: TableAlign,
}

impl DataTableColumn {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        let width = title.width().max(8) as u16;
        Self {
            title,
            width,
            align: TableAlign::Left,
        }
    }

    pub fn width(mut self, width: u16) -> Self {
        self.width = width.max(3);
        self
    }

    pub fn align(mut self, align: TableAlign) -> Self {
        self.align = align;
        self
    }
}

/// A single row in the data table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTableRow {
    pub id: String,
    pub cells: Vec<String>,
    pub disabled: bool,
}

impl DataTableRow {
    pub fn new(id: impl Into<String>, cells: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            id: id.into(),
            cells: cells.into_iter().map(Into::into).collect(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Persistent data table state stored in the widget store.
#[derive(Debug, Clone, Default)]
pub struct DataTableState {
    pub active_row: Option<String>,
    pub scroll_offset: usize,
}

/// Focusable data table widget with internal row navigation.
pub struct DataTable<M = ()> {
    columns: Vec<DataTableColumn>,
    rows: Vec<DataTableRow>,
    height: u16,
    border: Option<BorderStyle>,
    disabled: bool,
    empty_message: String,
    custom_style: Option<Style>,
    custom_focus_style: Option<Style>,
    on_change: Option<Arc<dyn Fn(String) -> M + Send + Sync>>,
    on_submit: Option<Arc<dyn Fn(String) -> M + Send + Sync>>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for DataTable<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataTable")
            .field("columns", &self.columns)
            .field("rows", &self.rows)
            .field("height", &self.height)
            .field("border", &self.border)
            .field("disabled", &self.disabled)
            .field("empty_message", &self.empty_message)
            .field("on_change", &self.on_change.is_some())
            .field("on_submit", &self.on_submit.is_some())
            .finish()
    }
}

impl<M> DataTable<M> {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            rows: Vec::new(),
            height: 10,
            border: Some(BorderStyle::Single),
            disabled: false,
            empty_message: "No rows".to_owned(),
            custom_style: None,
            custom_focus_style: None,
            on_change: None,
            on_submit: None,
            widget_key: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn column(mut self, column: DataTableColumn) -> Self {
        self.columns.push(column);
        self
    }

    pub fn columns<I>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = DataTableColumn>,
    {
        self.columns.extend(columns);
        self
    }

    pub fn row(mut self, row: DataTableRow) -> Self {
        self.rows.push(row);
        self
    }

    pub fn rows<I>(mut self, rows: I) -> Self
    where
        I: IntoIterator<Item = DataTableRow>,
    {
        self.rows.extend(rows);
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(4);
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

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn empty_message(mut self, message: impl Into<String>) -> Self {
        self.empty_message = message.into();
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }

    pub fn focus_style(mut self, style: Style) -> Self {
        self.custom_focus_style = Some(style);
        self
    }

    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(String) -> M + Send + Sync + 'static,
    {
        self.on_change = Some(Arc::new(handler));
        self
    }

    pub fn on_submit<F>(mut self, handler: F) -> Self
    where
        F: Fn(String) -> M + Send + Sync + 'static,
    {
        self.on_submit = Some(Arc::new(handler));
        self
    }

    fn has_enabled_rows(&self) -> bool {
        self.rows.iter().any(|row| !row.disabled)
    }

    fn visible_rows(&self) -> usize {
        let border_padding = usize::from(self.border.is_some()) * 2;
        self.height
            .saturating_sub(border_padding as u16)
            .saturating_sub(2) as usize
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.rows.iter().position(|row| !row.disabled)
    }

    fn last_enabled_index(&self) -> Option<usize> {
        self.rows.iter().rposition(|row| !row.disabled)
    }

    fn index_for_id(&self, id: &str) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| row.id == id && !row.disabled)
    }

    fn active_index_from_state(&self, state: &DataTableState) -> Option<usize> {
        state
            .active_row
            .as_deref()
            .and_then(|id| self.index_for_id(id))
            .or_else(|| self.first_enabled_index())
    }

    fn next_enabled_index(&self, current: usize) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .skip(current.saturating_add(1))
            .find(|(_, row)| !row.disabled)
            .map(|(index, _)| index)
    }

    fn prev_enabled_index(&self, current: usize) -> Option<usize> {
        self.rows
            .iter()
            .enumerate()
            .take(current)
            .rev()
            .find(|(_, row)| !row.disabled)
            .map(|(index, _)| index)
    }

    fn ensure_visible(scroll_offset: usize, active_index: usize, visible_rows: usize) -> usize {
        if visible_rows == 0 {
            return 0;
        }
        if active_index < scroll_offset {
            active_index
        } else if active_index >= scroll_offset + visible_rows {
            active_index + 1 - visible_rows
        } else {
            scroll_offset
        }
    }

    fn truncate_to_width(text: &str, max_width: usize) -> String {
        let mut out = String::new();
        let mut width = 0;

        for ch in text.chars() {
            let char_width = ch.width().unwrap_or(0);
            if width + char_width > max_width {
                break;
            }
            out.push(ch);
            width += char_width;
        }

        out
    }

    fn pad_cell(text: &str, width: usize, align: TableAlign) -> String {
        if width == 0 {
            return String::new();
        }

        let truncated = Self::truncate_to_width(text, width);
        let text_width = truncated.width();
        let remaining = width.saturating_sub(text_width);

        match align {
            TableAlign::Left => format!("{truncated}{}", " ".repeat(remaining)),
            TableAlign::Right => format!("{}{truncated}", " ".repeat(remaining)),
            TableAlign::Center => {
                let left = remaining / 2;
                let right = remaining.saturating_sub(left);
                format!("{}{}{}", " ".repeat(left), truncated, " ".repeat(right))
            }
        }
    }

    fn compute_column_widths(&self, total_width: u16) -> Vec<u16> {
        if self.columns.is_empty() || total_width == 0 {
            return Vec::new();
        }

        let separator_count = self.columns.len().saturating_sub(1) as u16;
        if total_width <= separator_count {
            return vec![1; self.columns.len()];
        }

        let available = total_width - separator_count;
        let mut widths: Vec<u16> = self
            .columns
            .iter()
            .map(|column| column.width.max(3))
            .collect();
        let mut used = widths.iter().sum::<u16>();

        if used < available {
            if let Some(last) = widths.last_mut() {
                *last += available - used;
            }
            return widths;
        }

        while used > available {
            let mut changed = false;
            for width in widths.iter_mut().rev() {
                if *width > 3 && used > available {
                    *width -= 1;
                    used -= 1;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        widths
    }

    fn draw_row(
        chunk: &mut render::chunk::Chunk,
        x: u16,
        y: u16,
        widths: &[u16],
        columns: &[DataTableColumn],
        cells: &[String],
        style: render::style::Style,
    ) {
        let mut cursor_x = x;

        for (index, width) in widths.iter().enumerate() {
            if *width == 0 {
                continue;
            }

            let column = &columns[index];
            let cell_text = cells.get(index).map(String::as_str).unwrap_or("");
            let rendered = Self::pad_cell(cell_text, *width as usize, column.align);
            let _ = chunk.set_string(cursor_x, y, &rendered, style);
            cursor_x = cursor_x.saturating_add(*width);

            if index + 1 < widths.len() {
                let _ = chunk.set_char(cursor_x, y, '|', style);
                cursor_x = cursor_x.saturating_add(1);
            }
        }
    }
}

impl<M> Default for DataTable<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Send + Sync + 'static> Widget<M> for DataTable<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let is_focused = ctx.is_focused();
        let base_style = ThemeManager::global().with_theme(|theme| {
            if self.disabled {
                theme.styles.interactive_disabled
            } else {
                theme.styles.surface_elevated
            }
        });
        let row_style = self
            .custom_style
            .as_ref()
            .map(|style| style.merge(base_style))
            .unwrap_or(base_style)
            .to_render_style();
        let active_style = if is_focused {
            self.custom_focus_style.unwrap_or_else(|| {
                ThemeManager::global().with_theme(|theme| {
                    Style::default()
                        .fg(theme.colors.text)
                        .bg(theme.colors.focus_background)
                        .bold()
                })
            })
        } else {
            ThemeManager::global().with_theme(|theme| theme.styles.selected)
        }
        .to_render_style();
        let header_style = ThemeManager::global().with_theme(|theme| {
            Style::default()
                .fg(theme.colors.text)
                .bg(theme.colors.surface)
                .bold()
                .to_render_style()
        });
        let muted_style =
            ThemeManager::global().with_theme(|theme| theme.styles.text_muted.to_render_style());
        let disabled_style = ThemeManager::global()
            .with_theme(|theme| theme.styles.interactive_disabled.to_render_style());
        let border_style = ThemeManager::global().with_theme(|theme| {
            if is_focused {
                Style::default()
                    .fg(theme.colors.focus_ring)
                    .to_render_style()
            } else {
                Style::default().fg(theme.colors.border).to_render_style()
            }
        });

        let _ = chunk.fill(0, 0, area.width(), area.height(), ' ', row_style);

        let (content_x, content_y, content_width, content_height) =
            if let Some(border) = self.border {
                if area.width() < 2 || area.height() < 2 {
                    return;
                }
                border_renderer::render_border(chunk, border, border_style);
                (1u16, 1u16, area.width() - 2, area.height() - 2)
            } else {
                (0u16, 0u16, area.width(), area.height())
            };

        if content_width == 0 || content_height == 0 {
            return;
        }

        if self.columns.is_empty() {
            let message =
                Self::truncate_to_width("Add columns to render a table", content_width as usize);
            let _ = chunk.set_string(content_x, content_y, &message, muted_style);
            return;
        }

        let widths = self.compute_column_widths(content_width);
        if widths.is_empty() {
            return;
        }

        let header_cells: Vec<String> = self
            .columns
            .iter()
            .map(|column| column.title.clone())
            .collect();
        Self::draw_row(
            chunk,
            content_x,
            content_y,
            &widths,
            &self.columns,
            &header_cells,
            header_style,
        );

        if content_height <= 1 {
            return;
        }

        let separator_y = content_y + 1;
        let _ = chunk.fill(content_x, separator_y, content_width, 1, '-', muted_style);
        let mut separator_x = content_x;
        for (index, width) in widths.iter().enumerate() {
            separator_x = separator_x.saturating_add(*width);
            if index + 1 < widths.len() {
                let _ = chunk.set_char(separator_x, separator_y, '+', muted_style);
                separator_x = separator_x.saturating_add(1);
            }
        }

        if content_height <= 2 {
            return;
        }

        if self.rows.is_empty() {
            let message = Self::truncate_to_width(&self.empty_message, content_width as usize);
            let _ = chunk.set_string(content_x, content_y + 2, &message, muted_style);
            return;
        }

        let state = ctx.state_or_default::<DataTableState>();
        let active_index = self.active_index_from_state(state);
        let visible_rows = content_height.saturating_sub(2) as usize;
        let mut scroll_offset = state.scroll_offset.min(self.rows.len().saturating_sub(1));
        if let Some(active_index) = active_index {
            scroll_offset = Self::ensure_visible(scroll_offset, active_index, visible_rows);
        }

        for row in 0..visible_rows {
            let row_index = scroll_offset + row;
            if row_index >= self.rows.len() {
                break;
            }

            let row_item = &self.rows[row_index];
            let is_active = active_index == Some(row_index);
            let style = if row_item.disabled {
                disabled_style
            } else if is_active {
                active_style
            } else {
                row_style
            };
            let y = content_y + 2 + row as u16;
            let _ = chunk.fill(content_x, y, content_width, 1, ' ', style);
            Self::draw_row(
                chunk,
                content_x,
                y,
                &widths,
                &self.columns,
                &row_item.cells,
                style,
            );
        }
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        if ctx.phase() != EventPhase::Target || self.disabled || !self.has_enabled_rows() {
            return;
        }

        let Event::Key(key_event) = event else {
            return;
        };

        let visible_rows = self.visible_rows().max(1);
        let mut moved = false;
        let mut emit_submit = None;

        let next_active_id = {
            let state = ctx.state_mut::<DataTableState>();
            let Some(mut active_index) = self.active_index_from_state(state) else {
                return;
            };

            match key_event.code {
                KeyCode::Up => {
                    if let Some(index) = self.prev_enabled_index(active_index) {
                        active_index = index;
                        moved = true;
                    }
                }
                KeyCode::Down => {
                    if let Some(index) = self.next_enabled_index(active_index) {
                        active_index = index;
                        moved = true;
                    }
                }
                KeyCode::Home => {
                    if let Some(index) = self.first_enabled_index() {
                        active_index = index;
                        moved = true;
                    }
                }
                KeyCode::End => {
                    if let Some(index) = self.last_enabled_index() {
                        active_index = index;
                        moved = true;
                    }
                }
                KeyCode::PageUp => {
                    for _ in 0..visible_rows {
                        if let Some(index) = self.prev_enabled_index(active_index) {
                            active_index = index;
                            moved = true;
                        } else {
                            break;
                        }
                    }
                }
                KeyCode::PageDown => {
                    for _ in 0..visible_rows {
                        if let Some(index) = self.next_enabled_index(active_index) {
                            active_index = index;
                            moved = true;
                        } else {
                            break;
                        }
                    }
                }
                KeyCode::Enter => {
                    let active_id = self.rows[active_index].id.clone();
                    if let Some(ref handler) = self.on_submit {
                        emit_submit = Some(handler(active_id));
                    }
                    ctx.set_handled();
                    if let Some(message) = emit_submit {
                        ctx.emit(message);
                    }
                    return;
                }
                _ => return,
            }

            state.active_row = Some(self.rows[active_index].id.clone());
            state.scroll_offset =
                Self::ensure_visible(state.scroll_offset, active_index, visible_rows);
            state.active_row.clone()
        };

        if moved {
            ctx.set_handled();
            if let (Some(active_id), Some(handler)) = (next_active_id, self.on_change.as_ref())
            {
                ctx.emit(handler(active_id));
            }
        }
    }

    fn constraints(&self) -> Constraints {
        let separator_width = self.columns.len().saturating_sub(1) as u16;
        let content_width = self
            .columns
            .iter()
            .map(|column| column.width.max(column.title.width() as u16))
            .sum::<u16>()
            + separator_width;
        let border_size = if self.border.is_some() { 2 } else { 0 };

        Constraints {
            min_width: content_width.max(16) + border_size,
            max_width: None,
            min_height: self.height,
            max_height: Some(self.height),
            flex: Some(1.0),
        }
    }

    fn focus_config(&self) -> FocusConfig {
        if self.disabled || !self.has_enabled_rows() {
            FocusConfig::None
        } else {
            FocusConfig::Composite
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Create a new data table widget.
pub fn data_table<M>() -> DataTable<M> {
    DataTable::new()
}
