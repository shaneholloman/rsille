//! Theme system for global styling

use super::{Color, Style};
use crate::animation::AnimationTheme;

#[derive(Debug, Clone, Copy)]
struct Palette {
    primary: Color,
    secondary: Color,
    danger: Color,
    info: Color,
    success: Color,
    warning: Color,
    text: Color,
    text_muted: Color,
    background: Color,
    surface: Color,
    surface_elevated: Color,
    surface_modal: Color,
    surface_popup: Color,
    surface_tooltip: Color,
    border: Color,
    focus_ring: Color,
    focus_background: Color,
    overlay_backdrop: Color,
    scrollbar_track: Color,
    scrollbar_thumb: Color,
}

impl Palette {
    fn dark() -> Self {
        Self {
            primary: Color::Rgb(99, 102, 241),
            secondary: Color::Rgb(139, 92, 246),
            danger: Color::Rgb(239, 68, 68),
            info: Color::Rgb(56, 189, 248),
            success: Color::Rgb(34, 197, 94),
            warning: Color::Rgb(245, 158, 11),
            text: Color::Rgb(229, 229, 231),
            text_muted: Color::Rgb(161, 161, 170),
            background: Color::Rgb(24, 24, 27),
            surface: Color::Rgb(39, 39, 42),
            surface_elevated: Color::Rgb(49, 49, 56),
            surface_modal: Color::Rgb(54, 54, 61),
            surface_popup: Color::Rgb(44, 44, 50),
            surface_tooltip: Color::Rgb(17, 24, 39),
            border: Color::Rgb(63, 63, 70),
            focus_ring: Color::Rgb(129, 140, 248),
            focus_background: Color::Rgb(49, 46, 129),
            overlay_backdrop: Color::Rgb(9, 9, 11),
            scrollbar_track: Color::Rgb(63, 63, 70),
            scrollbar_thumb: Color::Rgb(129, 140, 248),
        }
    }

    fn light() -> Self {
        Self {
            primary: Color::Rgb(79, 70, 229),
            secondary: Color::Rgb(124, 58, 237),
            danger: Color::Rgb(220, 38, 38),
            info: Color::Rgb(2, 132, 199),
            success: Color::Rgb(22, 163, 74),
            warning: Color::Rgb(217, 119, 6),
            text: Color::Rgb(24, 24, 27),
            text_muted: Color::Rgb(113, 113, 122),
            background: Color::Rgb(250, 250, 250),
            surface: Color::Rgb(255, 255, 255),
            surface_elevated: Color::Rgb(244, 244, 245),
            surface_modal: Color::Rgb(255, 255, 255),
            surface_popup: Color::Rgb(248, 250, 252),
            surface_tooltip: Color::Rgb(39, 39, 42),
            border: Color::Rgb(212, 212, 216),
            focus_ring: Color::Rgb(67, 56, 202),
            focus_background: Color::Rgb(224, 231, 255),
            overlay_backdrop: Color::Rgb(228, 228, 231),
            scrollbar_track: Color::Rgb(212, 212, 216),
            scrollbar_thumb: Color::Rgb(79, 70, 229),
        }
    }
}

/// Semantic style roles for theming
///
/// This structure provides semantic styling that can be used by any widget,
/// including user-defined custom widgets. Instead of hardcoding styles for
/// specific components, we provide semantic roles based on the purpose and
/// context of UI elements.
#[derive(Debug, Clone)]
pub struct ThemeStyles {
    // === Action Styles ===
    /// Primary action style (e.g., primary buttons, key actions)
    pub primary_action: Style,
    /// Primary action hover state
    pub primary_action_hover: Style,
    /// Primary action focused state
    pub primary_action_focused: Style,
    /// Secondary action style (e.g., secondary buttons)
    pub secondary_action: Style,
    /// Secondary action hover state
    pub secondary_action_hover: Style,
    /// Secondary action focused state
    pub secondary_action_focused: Style,
    /// Destructive action style (e.g., dangerous buttons)
    pub destructive_action: Style,
    /// Destructive action hover state
    pub destructive_action_hover: Style,
    /// Destructive action focused state
    pub destructive_action_focused: Style,

    // === Interactive Element Styles ===
    /// Interactive element style (e.g., inputs, checkboxes, sliders)
    pub interactive: Style,
    /// Interactive element focused state
    pub interactive_focused: Style,
    /// Interactive element disabled state
    pub interactive_disabled: Style,

    // === Text Styles ===
    /// Regular text style (e.g., labels, paragraphs)
    pub text: Style,
    /// Muted/secondary text style
    pub text_muted: Style,
    /// Placeholder text style
    pub text_placeholder: Style,
    /// Heading text style
    pub text_heading: Style,

    // === Container Styles ===
    /// Surface style (e.g., container backgrounds)
    pub surface: Style,
    /// Elevated surface style (e.g., modals, popups, cards)
    pub surface_elevated: Style,
    /// Header surface style for table/calendar headings
    pub surface_header: Style,
    /// Modal/dialog surface style
    pub surface_modal: Style,
    /// Popup/popover surface style
    pub surface_popup: Style,
    /// Tooltip surface style
    pub surface_tooltip: Style,
    /// Backdrop style used behind overlay layers
    pub overlay_backdrop: Style,

    // === State Styles ===
    /// Selected/highlighted state
    pub selected: Style,
    /// Selected state while the widget is focused
    pub selected_focused: Style,
    /// Active list row when the widget is not focused
    pub list_active: Style,
    /// Active list row when the widget is focused
    pub list_active_focused: Style,
    /// Informational feedback state
    pub status_info: Style,
    /// Success feedback state
    pub status_success: Style,
    /// Warning feedback state
    pub status_warning: Style,
    /// Error/destructive feedback state
    pub status_error: Style,
    /// Hover state (generic)
    pub hover: Style,
    /// Disabled state (generic)
    pub disabled: Style,
    /// Default border style
    pub border: Style,
    /// Focused border style
    pub border_focused: Style,
    /// Text cursor style
    pub cursor: Style,

    // === Editing & Validation Styles ===
    /// Selection range highlight for editable text/content
    pub selection_range: Style,
    /// Informational validation or helper emphasis
    pub validation_info: Style,
    /// Success validation or helper emphasis
    pub validation_success: Style,
    /// Warning validation or helper emphasis
    pub validation_warning: Style,
    /// Error validation or helper emphasis
    pub validation_error: Style,

    // === Menu & Scroll Styles ===
    /// Hovered menu item style
    pub menu_item_hover: Style,
    /// Active menu item when the menu is not focused
    pub menu_item_active: Style,
    /// Active menu item when the menu is focused
    pub menu_item_active_focused: Style,
    /// Scrollbar track style
    pub scrollbar_track: Style,
    /// Scrollbar thumb style
    pub scrollbar_thumb: Style,
}

impl ThemeStyles {
    /// Create semantic styles for dark theme
    pub fn dark() -> Self {
        let palette = Palette::dark();
        Self::from_palette(palette, palette.text, palette.background)
    }

    /// Create semantic styles for light theme
    pub fn light() -> Self {
        let palette = Palette::light();
        Self::from_palette(palette, Color::White, Color::White)
    }

    fn from_palette(palette: Palette, action_fg: Color, status_fg: Color) -> Self {
        Self {
            // Action styles
            primary_action: Style::default().fg(action_fg).bg(palette.primary),
            primary_action_hover: Style::default().fg(action_fg).bg(palette.primary).bold(),
            primary_action_focused: Style::default().fg(action_fg).bg(palette.primary).bold(),
            secondary_action: Style::default().fg(action_fg).bg(palette.secondary),
            secondary_action_hover: Style::default().fg(action_fg).bg(palette.secondary).bold(),
            secondary_action_focused: Style::default().fg(action_fg).bg(palette.secondary).bold(),
            destructive_action: Style::default().fg(Color::White).bg(palette.danger),
            destructive_action_hover: Style::default().fg(Color::White).bg(palette.danger).bold(),
            destructive_action_focused: Style::default().fg(Color::White).bg(palette.danger).bold(),

            // Interactive element styles
            interactive: Style::default().fg(palette.text).bg(palette.surface),
            interactive_focused: Style::default()
                .fg(palette.text)
                .bg(palette.surface_elevated)
                .bold(),
            interactive_disabled: Style::default().fg(palette.text_muted).bg(palette.surface),

            // Text styles
            text: Style::default().fg(palette.text),
            text_muted: Style::default().fg(palette.text_muted),
            text_placeholder: Style::default().fg(palette.text_muted),
            text_heading: Style::default().fg(palette.text).bold(),

            // Container styles
            surface: Style::default().bg(palette.background).fg(palette.text),
            surface_elevated: Style::default()
                .bg(palette.surface_elevated)
                .fg(palette.text),
            surface_header: Style::default()
                .bg(palette.surface_elevated)
                .fg(palette.text)
                .bold(),
            surface_modal: Style::default().bg(palette.surface_modal).fg(palette.text),
            surface_popup: Style::default().bg(palette.surface_popup).fg(palette.text),
            surface_tooltip: Style::default().fg(action_fg).bg(palette.surface_tooltip),
            overlay_backdrop: Style::default().bg(palette.overlay_backdrop),

            // State styles
            selected: Style::default().fg(action_fg).bg(palette.primary),
            selected_focused: Style::default()
                .fg(palette.text)
                .bg(palette.focus_background)
                .bold(),
            list_active: Style::default()
                .fg(palette.text)
                .bg(palette.focus_background)
                .bold(),
            list_active_focused: Style::default().fg(palette.text).bg(palette.surface).bold(),
            status_info: Style::default().fg(status_fg).bg(palette.info).bold(),
            status_success: Style::default().fg(status_fg).bg(palette.success).bold(),
            status_warning: Style::default().fg(status_fg).bg(palette.warning).bold(),
            status_error: Style::default().fg(status_fg).bg(palette.danger).bold(),
            hover: Style::default().fg(action_fg).bg(palette.primary).bold(),
            disabled: Style::default().fg(palette.text_muted),
            border: Style::default().fg(palette.border),
            border_focused: Style::default().fg(palette.focus_ring),
            cursor: Style::default().fg(palette.background).bg(palette.text),
            selection_range: Style::default().bg(palette.focus_background),
            validation_info: Style::default().fg(palette.info).bold(),
            validation_success: Style::default().fg(palette.success).bold(),
            validation_warning: Style::default().fg(palette.warning).bold(),
            validation_error: Style::default().fg(palette.danger).bold(),
            menu_item_hover: Style::default()
                .fg(palette.text)
                .bg(palette.surface_elevated)
                .bold(),
            menu_item_active: Style::default()
                .fg(palette.text)
                .bg(palette.surface_elevated)
                .bold(),
            menu_item_active_focused: Style::default()
                .fg(palette.text)
                .bg(palette.focus_background)
                .bold(),
            scrollbar_track: Style::default()
                .fg(palette.scrollbar_track)
                .bg(palette.surface),
            scrollbar_thumb: Style::default()
                .fg(palette.scrollbar_thumb)
                .bg(palette.surface_elevated),
        }
    }
}

/// A complete theme definition
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name
    pub name: String,
    /// Semantic style roles
    pub styles: ThemeStyles,
    /// Theme-level animation timing defaults.
    pub animations: AnimationTheme,
}

impl Theme {
    /// Create a new theme with the given name and semantic styles
    pub fn new(name: impl Into<String>, styles: ThemeStyles) -> Self {
        Self {
            name: name.into(),
            styles,
            animations: AnimationTheme::default(),
        }
    }

    /// Set theme-level animation defaults.
    pub fn with_animations(mut self, animations: AnimationTheme) -> Self {
        self.animations = animations;
        self
    }

    /// Create the built-in dark theme
    pub fn dark() -> Self {
        Self::new("dark", ThemeStyles::dark())
    }

    /// Create the built-in light theme
    pub fn light() -> Self {
        Self::new("light", ThemeStyles::light())
    }

    /// Create a theme builder for custom themes
    pub fn builder() -> ThemeBuilder {
        ThemeBuilder::new()
    }
}

/// Builder for creating custom themes
#[derive(Debug)]
pub struct ThemeBuilder {
    name: String,
    styles: Option<ThemeStyles>,
    animations: Option<AnimationTheme>,
}

impl ThemeBuilder {
    /// Create a new theme builder
    pub fn new() -> Self {
        Self {
            name: "custom".to_string(),
            styles: None,
            animations: None,
        }
    }

    /// Set the theme name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the style roles
    pub fn styles(mut self, styles: ThemeStyles) -> Self {
        self.styles = Some(styles);
        self
    }

    /// Set animation timing defaults.
    pub fn animations(mut self, animations: AnimationTheme) -> Self {
        self.animations = Some(animations);
        self
    }

    /// Build the theme, using dark theme defaults for unset fields
    pub fn build(self) -> Theme {
        let styles = self.styles.unwrap_or_else(ThemeStyles::dark);
        let animations = self.animations.unwrap_or_default();
        Theme::new(self.name, styles).with_animations(animations)
    }
}

impl Default for ThemeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_creation() {
        let theme = Theme::dark();
        assert_eq!(theme.name, "dark");
    }

    #[test]
    fn test_light_theme_creation() {
        let theme = Theme::light();
        assert_eq!(theme.name, "light");
    }

    #[test]
    fn test_custom_theme_builder() {
        let theme = Theme::builder()
            .name("custom")
            .styles(ThemeStyles::dark())
            .build();
        assert_eq!(theme.name, "custom");
    }

    #[test]
    fn test_theme_builder_accepts_animation_defaults() {
        let animations = AnimationTheme {
            fast: crate::animation::AnimationSpec::new(
                std::time::Duration::from_millis(50),
                crate::animation::Easing::Linear,
            ),
            ..AnimationTheme::default()
        };
        let theme = Theme::builder().animations(animations).build();

        assert_eq!(
            theme.animations.fast.duration,
            std::time::Duration::from_millis(50)
        );
    }

    #[test]
    fn test_extended_semantic_tokens_are_configured() {
        let styles = ThemeStyles::dark();

        assert!(!styles.surface_modal.is_empty());
        assert!(!styles.surface_popup.is_empty());
        assert!(!styles.surface_tooltip.is_empty());
        assert!(!styles.overlay_backdrop.is_empty());
        assert!(!styles.status_info.is_empty());
        assert!(!styles.status_success.is_empty());
        assert!(!styles.status_warning.is_empty());
        assert!(!styles.status_error.is_empty());
        assert!(!styles.validation_error.is_empty());
        assert!(!styles.menu_item_active_focused.is_empty());
        assert!(!styles.scrollbar_track.is_empty());
        assert!(!styles.scrollbar_thumb.is_empty());
    }
}
