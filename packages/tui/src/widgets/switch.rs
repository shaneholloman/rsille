//! Switch / toggle widget.

use unicode_width::UnicodeWidthStr;

use crate::animation::{AnimationConfig, AnimationCtx, AnimationSlot, AnimationSpec};
use crate::event::{Event, KeyCode};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::style::Style;
use crate::widget::{EventCtx, EventPhase, RenderCtx, Widget};

/// Focusable boolean switch.
pub struct Switch<M = ()> {
    label: String,
    checked: bool,
    disabled: bool,
    custom_style: Option<Style>,
    custom_focus_style: Option<Style>,
    animation: Option<AnimationConfig>,
    on_change: Option<Box<dyn Fn(bool) -> M>>,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Switch<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Switch")
            .field("label", &self.label)
            .field("checked", &self.checked)
            .field("disabled", &self.disabled)
            .field("on_change", &self.on_change.is_some())
            .finish()
    }
}

impl<M> Switch<M> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            checked: false,
            disabled: false,
            custom_style: None,
            custom_focus_style: None,
            animation: None,
            on_change: None,
            widget_key: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
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

    pub fn animated(mut self) -> Self {
        self.animation = Some(AnimationConfig::Theme(AnimationSlot::Normal));
        self
    }

    pub fn animation(mut self, spec: AnimationSpec) -> Self {
        self.animation = Some(AnimationConfig::Custom(spec));
        self
    }

    pub fn on_change<F>(mut self, handler: F) -> Self
    where
        F: Fn(bool) -> M + 'static,
    {
        self.on_change = Some(Box::new(handler));
        self
    }
}

impl<M> Widget<M> for Switch<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let theme = ctx.theme();
        let target_value = if self.checked { 1.0 } else { 0.0 };
        let animated_value = self
            .animation
            .and_then(|_| ctx.animation_value("checked"))
            .unwrap_or(target_value)
            .clamp(0.0, 1.0);
        let visual_checked = if self.animation.is_some() {
            animated_value >= 0.5
        } else {
            self.checked
        };
        let base_style = if self.disabled {
            theme.styles.interactive_disabled
        } else if ctx.is_focused() {
            theme.styles.interactive_focused
        } else if visual_checked {
            theme.styles.selected
        } else {
            theme.styles.interactive
        };
        let style = if ctx.is_focused() {
            self.custom_focus_style
                .map(|s| s.merge(base_style))
                .or_else(|| self.custom_style.map(|s| s.merge(base_style)))
                .unwrap_or(base_style)
        } else {
            self.custom_style
                .map(|s| s.merge(base_style))
                .unwrap_or(base_style)
        }
        .to_render_style();

        let track = if self.animation.is_none() {
            if self.checked {
                "[ ON]"
            } else {
                "[OFF]"
            }
        } else if animated_value >= 0.85 {
            "[ ON]"
        } else if animated_value <= 0.15 {
            "[OFF]"
        } else {
            "[ --]"
        };
        let text = if self.label.is_empty() {
            track.to_owned()
        } else {
            format!("{track} {}", self.label)
        };
        let display = truncate_to_width(&text, area.width() as usize);
        let _ = chunk.set_string(0, 0, &display, style);
    }

    fn animate(&self, ctx: &mut AnimationCtx) -> bool {
        let Some(animation) = self.animation else {
            return false;
        };

        let target = if self.checked { 1.0 } else { 0.0 };
        let spec = animation.resolve(ctx.animation_theme());
        ctx.track_value("checked", target, spec)
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        if ctx.phase() != EventPhase::Target || self.disabled {
            return;
        }

        let Event::Key(key_event) = event else {
            return;
        };

        if matches!(key_event.code, KeyCode::Enter | KeyCode::Char(' ')) {
            ctx.set_handled();
            if let Some(handler) = self.on_change.as_ref() {
                ctx.emit(handler(!self.checked));
            }
        }
    }

    fn constraints(&self) -> Constraints {
        let label_width = if self.label.is_empty() {
            0
        } else {
            self.label.width() as u16 + 1
        };
        Constraints {
            min_width: 5 + label_width,
            max_width: None,
            min_height: 1,
            max_height: Some(1),
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

fn truncate_to_width(text: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width {
            break;
        }
        out.push(ch);
        width += ch_width;
    }
    out
}

/// Create a switch widget.
pub fn switch<M>(label: impl Into<String>) -> Switch<M> {
    Switch::new(label)
}

/// Alias for [`switch`].
pub fn toggle<M>(label: impl Into<String>) -> Switch<M> {
    Switch::new(label)
}
