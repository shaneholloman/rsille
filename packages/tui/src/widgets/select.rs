//! Select widget with expandable option list.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::event::{Event, KeyCode};
use crate::focus::FocusConfig;
use crate::layout::border_renderer;
use crate::layout::Constraints;
use crate::style::{BorderStyle, Style, ThemeManager};
use crate::widget::{EventCtx, EventPhase, RenderCtx, Widget};

/// A single select option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl From<&str> for SelectOption {
    fn from(value: &str) -> Self {
        Self::new(value, value)
    }
}

impl From<String> for SelectOption {
    fn from(value: String) -> Self {
        Self::new(value.clone(), value)
    }
}

/// Persistent select state stored in the widget store.
#[derive(Debug, Clone, Default)]
pub struct SelectState {
    pub is_open: bool,
    pub active_option: Option<String>,
    pub selected_option: Option<String>,
    pub scroll_offset: usize,
}

/// Focusable select widget with an inline option menu.
pub struct Select<M = ()> {
    options: Vec<SelectOption>,
    height: u16,
    border: Option<BorderStyle>,
    placeholder: String,
    disabled: bool,
    custom_style: Option<Style>,
    custom_focus_style: Option<Style>,
    on_change: Option<Box<dyn Fn(String) -> M>>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Select<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Select")
            .field("options", &self.options)
            .field("height", &self.height)
            .field("border", &self.border)
            .field("placeholder", &self.placeholder)
            .field("disabled", &self.disabled)
            .field("on_change", &self.on_change.is_some())
            .finish()
    }
}

impl<M> Select<M> {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            height: 7,
            border: Some(BorderStyle::Single),
            placeholder: "Choose an option".to_owned(),
            disabled: false,
            custom_style: None,
            custom_focus_style: None,
            on_change: None,
            widget_key: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn option(mut self, option: impl Into<SelectOption>) -> Self {
        self.options.push(option.into());
        self
    }

    pub fn options<I>(mut self, options: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<SelectOption>,
    {
        self.options.extend(options.into_iter().map(Into::into));
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

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
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

    fn has_enabled_options(&self) -> bool {
        self.options.iter().any(|option| !option.disabled)
    }

    fn visible_rows(&self) -> usize {
        let border_padding = usize::from(self.border.is_some()) * 2;
        self.height
            .saturating_sub(border_padding as u16)
            .saturating_sub(2) as usize
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.options.iter().position(|option| !option.disabled)
    }

    fn last_enabled_index(&self) -> Option<usize> {
        self.options.iter().rposition(|option| !option.disabled)
    }

    fn index_for_value(&self, value: &str) -> Option<usize> {
        self.options
            .iter()
            .position(|option| option.value == value && !option.disabled)
    }

    fn selected_label<'a>(&'a self, state: &'a SelectState) -> Option<&'a str> {
        state
            .selected_option
            .as_deref()
            .and_then(|value| self.options.iter().find(|option| option.value == value))
            .map(|option| option.label.as_str())
    }

    fn active_index_from_state(&self, state: &SelectState) -> Option<usize> {
        state
            .active_option
            .as_deref()
            .and_then(|value| self.index_for_value(value))
            .or_else(|| {
                state
                    .selected_option
                    .as_deref()
                    .and_then(|value| self.index_for_value(value))
            })
            .or_else(|| self.first_enabled_index())
    }

    fn next_enabled_index(&self, current: usize) -> Option<usize> {
        self.options
            .iter()
            .enumerate()
            .skip(current.saturating_add(1))
            .find(|(_, option)| !option.disabled)
            .map(|(index, _)| index)
    }

    fn prev_enabled_index(&self, current: usize) -> Option<usize> {
        self.options
            .iter()
            .enumerate()
            .take(current)
            .rev()
            .find(|(_, option)| !option.disabled)
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

impl<M> Default for Select<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> Widget<M> for Select<M> {
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
        let field_style = self
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
        let disabled_style = ThemeManager::global()
            .with_theme(|theme| theme.styles.interactive_disabled.to_render_style());
        let muted_style =
            ThemeManager::global().with_theme(|theme| theme.styles.text_muted.to_render_style());
        let border_style = ThemeManager::global().with_theme(|theme| {
            if is_focused {
                Style::default()
                    .fg(theme.colors.focus_ring)
                    .to_render_style()
            } else {
                Style::default().fg(theme.colors.border).to_render_style()
            }
        });

        let _ = chunk.fill(0, 0, area.width(), area.height(), ' ', field_style);

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

        let state = ctx.state_or_default::<SelectState>();
        let indicator = if state.is_open { "^" } else { "v" };
        let indicator_width = indicator.width() as u16;
        let display_width = content_width.saturating_sub(indicator_width + 1) as usize;
        let display_text = self
            .selected_label(state)
            .map(|label| Self::truncate_to_width(label, display_width))
            .unwrap_or_else(|| Self::truncate_to_width(&self.placeholder, display_width));
        let display_style = if self.selected_label(state).is_some() {
            field_style
        } else {
            muted_style
        };
        let _ = chunk.set_string(content_x, content_y, &display_text, display_style);
        if content_width > indicator_width {
            let indicator_x = content_x + content_width - indicator_width;
            let _ = chunk.set_string(indicator_x, content_y, indicator, display_style);
        }

        if !state.is_open || content_height <= 2 {
            return;
        }

        let separator_y = content_y + 1;
        let _ = chunk.fill(content_x, separator_y, content_width, 1, '-', muted_style);

        if self.options.is_empty() {
            let message = Self::truncate_to_width("No options", content_width as usize);
            let _ = chunk.set_string(content_x, content_y + 2, &message, muted_style);
            return;
        }

        let active_index = self.active_index_from_state(state);
        let visible_rows = content_height.saturating_sub(2) as usize;
        let mut scroll_offset = state
            .scroll_offset
            .min(self.options.len().saturating_sub(1));
        if let Some(active_index) = active_index {
            scroll_offset = Self::ensure_visible(scroll_offset, active_index, visible_rows);
        }

        for row in 0..visible_rows {
            let option_index = scroll_offset + row;
            if option_index >= self.options.len() {
                break;
            }

            let option = &self.options[option_index];
            let is_active = active_index == Some(option_index);
            let is_selected = state.selected_option.as_deref() == Some(option.value.as_str());
            let style = if option.disabled {
                disabled_style
            } else if is_active {
                active_style
            } else {
                field_style
            };

            let y = content_y + 2 + row as u16;
            let _ = chunk.fill(content_x, y, content_width, 1, ' ', style);

            let prefix = if is_selected { "* " } else { "  " };
            let available = content_width.saturating_sub(prefix.width() as u16) as usize;
            let text = Self::truncate_to_width(&option.label, available);
            let line = format!("{prefix}{text}");
            let _ = chunk.set_string(content_x, y, &line, style);
        }
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        if ctx.phase() != EventPhase::Target || self.disabled || !self.has_enabled_options() {
            return;
        }

        let Event::Key(key_event) = event else {
            return;
        };

        let visible_rows = self.visible_rows().max(1);
        let mut emit_change = None;
        let mut did_handle = true;

        {
            let state = ctx.state_mut::<SelectState>();

            if state.active_option.is_none() {
                if let Some(index) = self.active_index_from_state(state) {
                    state.active_option = Some(self.options[index].value.clone());
                }
            }

            let Some(mut active_index) = self.active_index_from_state(state) else {
                return;
            };

            match key_event.code {
                KeyCode::Down => {
                    if !state.is_open {
                        state.is_open = true;
                    } else if let Some(index) = self.next_enabled_index(active_index) {
                        active_index = index;
                    }
                }
                KeyCode::Up => {
                    if !state.is_open {
                        state.is_open = true;
                    } else if let Some(index) = self.prev_enabled_index(active_index) {
                        active_index = index;
                    }
                }
                KeyCode::Home if state.is_open => {
                    if let Some(index) = self.first_enabled_index() {
                        active_index = index;
                    }
                }
                KeyCode::End if state.is_open => {
                    if let Some(index) = self.last_enabled_index() {
                        active_index = index;
                    }
                }
                KeyCode::PageUp if state.is_open => {
                    for _ in 0..visible_rows {
                        if let Some(index) = self.prev_enabled_index(active_index) {
                            active_index = index;
                        } else {
                            break;
                        }
                    }
                }
                KeyCode::PageDown if state.is_open => {
                    for _ in 0..visible_rows {
                        if let Some(index) = self.next_enabled_index(active_index) {
                            active_index = index;
                        } else {
                            break;
                        }
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if !state.is_open {
                        state.is_open = true;
                    } else {
                        let value = self.options[active_index].value.clone();
                        state.selected_option = Some(value.clone());
                        state.is_open = false;
                        if let Some(ref handler) = self.on_change {
                            emit_change = Some(handler(value));
                        }
                    }
                }
                KeyCode::Esc if state.is_open => {
                    state.is_open = false;
                }
                _ => {
                    did_handle = false;
                }
            }

            state.active_option = Some(self.options[active_index].value.clone());
            state.scroll_offset =
                Self::ensure_visible(state.scroll_offset, active_index, visible_rows);
        }

        if did_handle {
            ctx.set_handled();
        }
        if let Some(message) = emit_change {
            ctx.emit(message);
        }
    }

    fn constraints(&self) -> Constraints {
        let widest_label = self
            .options
            .iter()
            .map(|option| option.label.width() as u16)
            .max()
            .unwrap_or(16);
        let border_size = if self.border.is_some() { 2 } else { 0 };

        Constraints {
            min_width: widest_label.max(self.placeholder.width() as u16) + 6 + border_size,
            max_width: None,
            min_height: self.height,
            max_height: Some(self.height),
            flex: None,
        }
    }

    fn focus_config(&self) -> FocusConfig {
        if self.disabled || !self.has_enabled_options() {
            FocusConfig::None
        } else {
            FocusConfig::Composite
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Create a new select widget.
pub fn select<M>() -> Select<M> {
    Select::new()
}
