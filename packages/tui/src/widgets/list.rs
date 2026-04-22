//! List widget with roving internal selection.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::event::{Event, KeyCode};
use crate::focus::FocusConfig;
use crate::layout::border_renderer;
use crate::layout::Constraints;
use crate::style::{BorderStyle, Style, ThemeManager};
use crate::widget::{EventCtx, EventPhase, RenderCtx, Widget};

/// A single list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub id: String,
    pub label: String,
    pub disabled: bool,
}

impl ListItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl From<&str> for ListItem {
    fn from(value: &str) -> Self {
        Self::new(value, value)
    }
}

impl From<String> for ListItem {
    fn from(value: String) -> Self {
        Self::new(value.clone(), value)
    }
}

/// Persistent list state stored in the widget store.
#[derive(Debug, Clone, Default)]
pub struct ListState {
    pub active_item: Option<String>,
    pub scroll_offset: usize,
}

/// Focusable list widget with internal arrow-key navigation.
pub struct List<M = ()> {
    items: Vec<ListItem>,
    height: u16,
    border: Option<BorderStyle>,
    disabled: bool,
    custom_style: Option<Style>,
    custom_focus_style: Option<Style>,
    on_change: Option<Box<dyn Fn(String) -> M>>,
    on_submit: Option<Box<dyn Fn(String) -> M>>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for List<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("List")
            .field("items", &self.items)
            .field("height", &self.height)
            .field("border", &self.border)
            .field("disabled", &self.disabled)
            .field("on_change", &self.on_change.is_some())
            .field("on_submit", &self.on_submit.is_some())
            .finish()
    }
}

impl<M> List<M> {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            height: 6,
            border: Some(BorderStyle::Single),
            disabled: false,
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

    pub fn item(mut self, item: impl Into<ListItem>) -> Self {
        self.items.push(item.into());
        self
    }

    pub fn items<I>(mut self, items: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<ListItem>,
    {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(1);
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
        F: Fn(String) -> M + 'static,
    {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn on_submit<F>(mut self, handler: F) -> Self
    where
        F: Fn(String) -> M + 'static,
    {
        self.on_submit = Some(Box::new(handler));
        self
    }

    fn has_enabled_items(&self) -> bool {
        self.items.iter().any(|item| !item.disabled)
    }

    fn visible_rows(&self) -> usize {
        let border_padding = usize::from(self.border.is_some()) * 2;
        self.height.saturating_sub(border_padding as u16) as usize
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.items.iter().position(|item| !item.disabled)
    }

    fn last_enabled_index(&self) -> Option<usize> {
        self.items.iter().rposition(|item| !item.disabled)
    }

    fn index_for_id(&self, id: &str) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.id == id && !item.disabled)
    }

    fn active_index_from_state(&self, state: &ListState) -> Option<usize> {
        state
            .active_item
            .as_deref()
            .and_then(|id| self.index_for_id(id))
            .or_else(|| self.first_enabled_index())
    }

    fn next_enabled_index(&self, current: usize) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .skip(current.saturating_add(1))
            .find(|(_, item)| !item.disabled)
            .map(|(index, _)| index)
    }

    fn prev_enabled_index(&self, current: usize) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .take(current)
            .rev()
            .find(|(_, item)| !item.disabled)
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
}

impl<M> Default for List<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> Widget<M> for List<M> {
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
                theme.styles.surface
            }
        });
        let active_style = if is_focused {
            self.custom_focus_style.unwrap_or_else(|| {
                ThemeManager::global().with_theme(|theme| theme.styles.interactive_focused)
            })
        } else {
            ThemeManager::global().with_theme(|theme| {
                Style::default()
                    .fg(theme.colors.text)
                    .bg(theme.colors.focus_background)
                    .bold()
            })
        };
        let row_style = self
            .custom_style
            .as_ref()
            .map(|style| style.merge(base_style))
            .unwrap_or(base_style)
            .to_render_style();
        let active_row_style = active_style.to_render_style();
        let disabled_row_style = ThemeManager::global()
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

        let state = ctx.state_or_default::<ListState>();
        let Some(active_index) = self.active_index_from_state(state) else {
            return;
        };

        let visible_rows = content_height as usize;
        let mut scroll_offset = state.scroll_offset.min(self.items.len().saturating_sub(1));
        scroll_offset = Self::ensure_visible(scroll_offset, active_index, visible_rows);

        for row in 0..visible_rows {
            let item_index = scroll_offset + row;
            if item_index >= self.items.len() {
                break;
            }

            let item = &self.items[item_index];
            let is_active = item_index == active_index;
            let style = if item.disabled {
                disabled_row_style
            } else if is_active {
                active_row_style
            } else {
                row_style
            };

            let y = content_y + row as u16;
            let _ = chunk.fill(content_x, y, content_width, 1, ' ', style);

            let prefix = if is_active { "> " } else { "  " };
            let available_width = content_width.saturating_sub(prefix.width() as u16) as usize;
            let text = Self::truncate_to_width(&item.label, available_width);
            let line = format!("{prefix}{text}");
            let _ = chunk.set_string(content_x, y, &line, style);
        }
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        if ctx.phase() != EventPhase::Target || self.disabled || !self.has_enabled_items() {
            return;
        }

        let Event::Key(key_event) = event else {
            return;
        };

        let visible_rows = self.visible_rows().max(1);
        let mut emit_submit = None;
        let mut moved = false;

        let next_active_id = {
            let state = ctx.state_mut::<ListState>();
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
                    let active_id = self.items[active_index].id.clone();
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

            state.active_item = Some(self.items[active_index].id.clone());
            state.scroll_offset =
                Self::ensure_visible(state.scroll_offset, active_index, visible_rows);
            state.active_item.clone()
        };

        if moved {
            ctx.set_handled();
            if let (Some(active_id), Some(handler)) = (next_active_id, self.on_change.as_ref()) {
                ctx.emit(handler(active_id));
            }
        }
    }

    fn constraints(&self) -> Constraints {
        let widest_label = self
            .items
            .iter()
            .map(|item| item.label.width() as u16)
            .max()
            .unwrap_or(10);

        Constraints {
            min_width: widest_label + 4,
            max_width: None,
            min_height: self.height,
            max_height: Some(self.height),
            flex: Some(1.0),
        }
    }

    fn focus_config(&self) -> FocusConfig {
        if self.disabled || !self.has_enabled_items() {
            FocusConfig::None
        } else {
            FocusConfig::Composite
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Create a new list widget.
pub fn list<M>() -> List<M> {
    List::new()
}
