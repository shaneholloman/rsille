//! Theme system for global styling

use super::{Color, Style};

#[derive(Debug, Clone, Copy)]
struct Palette {
    primary: Color,
    secondary: Color,
    danger: Color,
    text: Color,
    text_muted: Color,
    background: Color,
    surface: Color,
    border: Color,
    focus_ring: Color,
    focus_background: Color,
}

impl Palette {
    fn dark() -> Self {
        Self {
            primary: Color::Rgb(99, 102, 241),
            secondary: Color::Rgb(139, 92, 246),
            danger: Color::Rgb(239, 68, 68),
            text: Color::Rgb(229, 229, 231),
            text_muted: Color::Rgb(161, 161, 170),
            background: Color::Rgb(24, 24, 27),
            surface: Color::Rgb(39, 39, 42),
            border: Color::Rgb(63, 63, 70),
            focus_ring: Color::Rgb(129, 140, 248),
            focus_background: Color::Rgb(49, 46, 129),
        }
    }

    fn light() -> Self {
        Self {
            primary: Color::Rgb(79, 70, 229),
            secondary: Color::Rgb(124, 58, 237),
            danger: Color::Rgb(220, 38, 38),
            text: Color::Rgb(24, 24, 27),
            text_muted: Color::Rgb(113, 113, 122),
            background: Color::Rgb(250, 250, 250),
            surface: Color::Rgb(255, 255, 255),
            border: Color::Rgb(212, 212, 216),
            focus_ring: Color::Rgb(67, 56, 202),
            focus_background: Color::Rgb(224, 231, 255),
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

    // === State Styles ===
    /// Selected/highlighted state
    pub selected: Style,
    /// Selected state while the widget is focused
    pub selected_focused: Style,
    /// Active list row when the widget is not focused
    pub list_active: Style,
    /// Active list row when the widget is focused
    pub list_active_focused: Style,
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
}

impl ThemeStyles {
    /// Create semantic styles for dark theme
    pub fn dark() -> Self {
        let palette = Palette::dark();
        Self::from_palette(palette, palette.text)
    }

    /// Create semantic styles for light theme
    pub fn light() -> Self {
        let palette = Palette::light();
        Self::from_palette(palette, Color::White)
    }

    fn from_palette(palette: Palette, action_fg: Color) -> Self {
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
            interactive_focused: Style::default().fg(palette.text).bg(palette.surface).bold(),
            interactive_disabled: Style::default().fg(palette.text_muted).bg(palette.surface),

            // Text styles
            text: Style::default().fg(palette.text),
            text_muted: Style::default().fg(palette.text_muted),
            text_placeholder: Style::default().fg(palette.text_muted),
            text_heading: Style::default().fg(palette.text).bold(),

            // Container styles
            surface: Style::default().bg(palette.background).fg(palette.text),
            surface_elevated: Style::default().bg(palette.surface).fg(palette.text),
            surface_header: Style::default().bg(palette.surface).fg(palette.text).bold(),

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
            hover: Style::default().fg(action_fg).bg(palette.primary).bold(),
            disabled: Style::default().fg(palette.text_muted),
            border: Style::default().fg(palette.border),
            border_focused: Style::default().fg(palette.focus_ring),
            cursor: Style::default().fg(palette.background).bg(palette.text),
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
}

impl Theme {
    /// Create a new theme with the given name and semantic styles
    pub fn new(name: impl Into<String>, styles: ThemeStyles) -> Self {
        Self {
            name: name.into(),
            styles,
        }
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
}

impl ThemeBuilder {
    /// Create a new theme builder
    pub fn new() -> Self {
        Self {
            name: "custom".to_string(),
            styles: None,
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

    /// Build the theme, using dark theme defaults for unset fields
    pub fn build(self) -> Theme {
        let styles = self.styles.unwrap_or_else(ThemeStyles::dark);
        Theme::new(self.name, styles)
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
}
