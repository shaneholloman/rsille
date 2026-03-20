//! Button widget — focusable interactive button

use std::sync::Arc;

use crate::event::{Event, KeyCode};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::style::{Color, Style, ThemeManager};
use crate::widget::{EventCtx, EventPhase, RenderCtx, Widget};

/// Button style variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Link,
    Destructive,
}

/// Interactive button widget.
///
/// Focusable. Activated by Enter or Space when focused.
pub struct Button<M = ()> {
    label: String,
    variant: ButtonVariant,
    disabled: bool,
    on_click: Option<Arc<dyn Fn() -> M + Send + Sync>>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Button<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Button")
            .field("label", &self.label)
            .field("variant", &self.variant)
            .field("disabled", &self.disabled)
            .field("on_click", &self.on_click.is_some())
            .finish()
    }
}

impl<M> Button<M> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::default(),
            disabled: false,
            on_click: None,
            widget_key: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn() -> M + Send + Sync + 'static,
    {
        self.on_click = Some(Arc::new(handler));
        self
    }

    fn compute_style(&self, is_focused: bool) -> Style {
        ThemeManager::global().with_theme(|theme| {
            if self.disabled {
                return theme.styles.disabled;
            }
            match self.variant {
                ButtonVariant::Primary => {
                    if is_focused {
                        theme.styles.primary_action_focused
                    } else {
                        theme.styles.primary_action
                    }
                }
                ButtonVariant::Secondary => {
                    if is_focused {
                        theme.styles.secondary_action_focused
                    } else {
                        theme.styles.secondary_action
                    }
                }
                ButtonVariant::Ghost => {
                    if is_focused {
                        theme.styles.interactive_focused
                    } else {
                        Style::default().fg(theme.colors.text)
                    }
                }
                ButtonVariant::Link => {
                    let base = if is_focused {
                        theme.styles.interactive_focused
                    } else {
                        theme.styles.text
                    };
                    base.underlined()
                }
                ButtonVariant::Destructive => {
                    if is_focused {
                        Style::default()
                            .fg(Color::White)
                            .bg(theme.colors.danger)
                            .bold()
                    } else {
                        Style::default().fg(Color::White).bg(theme.colors.danger)
                    }
                }
            }
        })
    }
}

impl<M: Send + Sync> Widget<M> for Button<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let is_focused = ctx.is_focused();
        let style = self.compute_style(is_focused);
        let render_style = style.to_render_style();

        let width = area.width();
        let height = area.height();

        // Background fill for solid variants
        if matches!(
            self.variant,
            ButtonVariant::Primary | ButtonVariant::Secondary | ButtonVariant::Destructive
        ) {
            let _ = chunk.fill(0, 0, width, height, ' ', render_style);
        }

        // Border for Ghost variant
        if matches!(self.variant, ButtonVariant::Ghost) {
            let border_style = ThemeManager::global().with_theme(|theme| {
                if is_focused {
                    Style::default()
                        .fg(theme.colors.focus_ring)
                        .to_render_style()
                } else {
                    Style::default().fg(theme.colors.border).to_render_style()
                }
            });
            use crate::layout::border_renderer;
            use crate::style::BorderStyle;
            border_renderer::render_border(chunk, BorderStyle::Single, border_style);
        }

        // Center label text
        use unicode_width::UnicodeWidthStr;
        let text_width = self.label.width() as u16;
        let padding_offset = if matches!(self.variant, ButtonVariant::Ghost) {
            1
        } else {
            0
        };
        let available_width = width.saturating_sub(padding_offset * 2);
        let text_x = if available_width > text_width {
            padding_offset + (available_width - text_width) / 2
        } else {
            padding_offset
        };
        let text_y = height / 2;

        let _ = chunk.set_string(text_x, text_y, &self.label, render_style);
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        if ctx.phase() != EventPhase::Target {
            return;
        }
        if self.disabled {
            return;
        }
        if let Event::Key(key_event) = event {
            if matches!(key_event.code, KeyCode::Enter | KeyCode::Char(' ')) {
                if let Some(ref handler) = self.on_click {
                    ctx.emit(handler());
                }
            }
        }
    }

    fn constraints(&self) -> Constraints {
        use unicode_width::UnicodeWidthStr;
        let label_width = self.label.width() as u16;
        let (total_width, height) = match self.variant {
            ButtonVariant::Ghost => (label_width + 6, 3),
            _ => (label_width + 4, 1),
        };
        Constraints {
            min_width: total_width,
            max_width: Some(total_width),
            min_height: height,
            max_height: Some(height),
            flex: None,
        }
    }

    fn focus_config(&self) -> FocusConfig {
        if self.disabled {
            FocusConfig::None
        } else {
            FocusConfig::Leaf
        }
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

/// Create a new button widget.
pub fn button<M>(label: impl Into<String>) -> Button<M> {
    Button::new(label)
}
