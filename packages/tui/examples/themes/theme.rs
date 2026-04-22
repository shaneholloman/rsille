//! Theme showcase with runtime switching.
//!
//! Run with: `cargo run -p tui --example theme`
//! Use Left/Right or 1/2/3 to switch themes. Tab focuses the interactive controls. Esc to quit.

use tui::prelude::*;
use tui::style::{ThemeColors, ThemeStyles};

const THEME_COUNT: usize = 3;

#[derive(Debug, Default)]
struct State {
    active_theme: usize,
    query: String,
    submitted: String,
}

#[derive(Debug, Clone)]
enum Msg {
    PrevTheme,
    NextTheme,
    SetTheme(usize),
    QueryChanged(String),
    QuerySubmitted(String),
    LoadSample,
    ClearQuery,
}

fn main() -> WidgetResult<()> {
    App::new(State::default())
        .with_theme_from(|state| theme_for(state.active_theme))
        .on_key(KeyCode::Left, || Msg::PrevTheme)
        .on_key(KeyCode::Right, || Msg::NextTheme)
        .on_key(KeyCode::Char('1'), || Msg::SetTheme(0))
        .on_key(KeyCode::Char('2'), || Msg::SetTheme(1))
        .on_key(KeyCode::Char('3'), || Msg::SetTheme(2))
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::PrevTheme => state.active_theme = cycle_theme(state.active_theme, -1),
        Msg::NextTheme => state.active_theme = cycle_theme(state.active_theme, 1),
        Msg::SetTheme(index) => state.active_theme = index.min(THEME_COUNT - 1),
        Msg::QueryChanged(value) => state.query = value,
        Msg::QuerySubmitted(value) => state.submitted = value,
        Msg::LoadSample => {
            state.query = format!("{} theme tokens", theme_title(state.active_theme));
        }
        Msg::ClearQuery => {
            state.query.clear();
            state.submitted.clear();
        }
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let theme = theme_for(state.active_theme);
    let card_style = Style::default()
        .fg(theme.colors.border)
        .bg(theme.colors.surface);

    col::<Msg>()
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .style(card_style)
                .gap(1)
                .child(label("Theme Showcase").style(theme.styles.text_heading))
                .child(label(format!(
                    "Current theme: {} ({})",
                    theme_title(state.active_theme),
                    theme.name
                )))
                .child(
                    label("Press Left/Right or 1/2/3 to switch themes. Tab moves into the input and action buttons.")
                        .style(theme.styles.text_muted),
                ),
        )
        .child(theme_picker_row(state.active_theme))
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .style(card_style)
                .gap(1)
                .child(label("Palette").style(theme.styles.text_heading))
                .child(
                    label(theme_description(state.active_theme)).style(theme.styles.text_muted),
                )
                .child(
                    row::<Msg>()
                        .gap(1)
                        .child(swatch(" primary ", theme.colors.primary))
                        .child(swatch(" secondary ", theme.colors.secondary))
                        .child(swatch(" success ", theme.colors.success))
                        .child(swatch(" warning ", theme.colors.warning))
                        .child(swatch(" danger ", theme.colors.danger)),
                )
                .child(
                    row::<Msg>()
                        .gap(1)
                        .child(swatch(" info ", theme.colors.info))
                        .child(swatch(" text ", theme.colors.text))
                        .child(swatch(" muted ", theme.colors.text_muted))
                        .child(swatch(" surface ", theme.colors.surface))
                        .child(swatch(" focus ", theme.colors.focus_ring)),
                ),
        )
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .style(card_style)
                .gap(1)
                .child(label("Interactive Preview").style(theme.styles.text_heading))
                .child(
                    text_input::<Msg>()
                        .key("theme-query")
                        .value(state.query.as_str())
                        .placeholder("Type and submit to see the active theme on inputs...")
                        .on_change(Msg::QueryChanged)
                        .on_submit(Msg::QuerySubmitted),
                )
                .child(
                    row::<Msg>()
                        .gap(1)
                        .child(button("Previous").variant(ButtonVariant::Ghost).on_click(|| Msg::PrevTheme))
                        .child(button("Next").on_click(|| Msg::NextTheme))
                        .child(
                            button("Load Sample")
                                .variant(ButtonVariant::Secondary)
                                .on_click(|| Msg::LoadSample),
                        )
                        .child(
                            button("Clear")
                                .variant(ButtonVariant::Destructive)
                                .on_click(|| Msg::ClearQuery),
                        ),
                )
                .child(label(format!(
                    "Submitted text: {}",
                    if state.submitted.is_empty() {
                        "nothing yet"
                    } else {
                        state.submitted.as_str()
                    }
                ))),
        )
}

fn theme_picker_row(active_theme: usize) -> Flex<Msg> {
    row::<Msg>()
        .gap(1)
        .child(theme_button("1 Dark", 0, active_theme))
        .child(theme_button("2 Light", 1, active_theme))
        .child(theme_button("3 Sunset", 2, active_theme))
}

fn theme_button(label_text: &'static str, index: usize, active_theme: usize) -> Button<Msg> {
    let variant = if index == active_theme {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    };

    button(label_text)
        .variant(variant)
        .on_click(move || Msg::SetTheme(index))
}

fn swatch(label_text: &'static str, color: Color) -> Label<Msg> {
    label(label_text).style(Style::default().fg(contrast_color(color)).bg(color).bold())
}

fn cycle_theme(active_theme: usize, delta: isize) -> usize {
    match delta {
        -1 => {
            if active_theme == 0 {
                THEME_COUNT - 1
            } else {
                active_theme - 1
            }
        }
        1 => (active_theme + 1) % THEME_COUNT,
        _ => active_theme,
    }
}

fn theme_title(index: usize) -> &'static str {
    match index.min(THEME_COUNT - 1) {
        0 => "Dark",
        1 => "Light",
        _ => "Sunset",
    }
}

fn theme_description(index: usize) -> &'static str {
    match index.min(THEME_COUNT - 1) {
        0 => "Built-in dark palette for dense terminal workflows.",
        1 => "Built-in light palette for bright terminal sessions.",
        _ => "Custom warm palette showing how to build a branded theme.",
    }
}

fn theme_for(index: usize) -> Theme {
    match index.min(THEME_COUNT - 1) {
        0 => Theme::dark(),
        1 => Theme::light(),
        _ => sunset_theme(),
    }
}

fn sunset_theme() -> Theme {
    let colors = ThemeColors {
        primary: Color::Rgb(255, 140, 92),
        secondary: Color::Rgb(255, 196, 107),
        success: Color::Rgb(104, 211, 145),
        danger: Color::Rgb(255, 107, 129),
        warning: Color::Rgb(255, 184, 77),
        info: Color::Rgb(122, 211, 255),
        text: Color::Rgb(255, 243, 230),
        text_muted: Color::Rgb(223, 198, 176),
        background: Color::Rgb(34, 20, 35),
        surface: Color::Rgb(59, 36, 58),
        border: Color::Rgb(137, 93, 123),
        focus_ring: Color::Rgb(255, 214, 102),
        focus_background: Color::Rgb(92, 53, 84),
    };

    let mut styles = ThemeStyles::dark(&colors);
    styles.primary_action = Style::default()
        .fg(Color::Rgb(34, 20, 35))
        .bg(colors.primary)
        .bold();
    styles.primary_action_focused = Style::default()
        .fg(Color::Rgb(34, 20, 35))
        .bg(colors.focus_ring)
        .bold();
    styles.secondary_action = Style::default()
        .fg(Color::Rgb(34, 20, 35))
        .bg(colors.secondary);
    styles.secondary_action_focused = Style::default()
        .fg(Color::Rgb(34, 20, 35))
        .bg(colors.secondary)
        .bold();
    styles.interactive = Style::default().fg(colors.text).bg(colors.surface);
    styles.interactive_focused = Style::default()
        .fg(colors.text)
        .bg(colors.focus_background)
        .bold();
    styles.surface = Style::default().fg(colors.text).bg(colors.background);
    styles.surface_elevated = Style::default().fg(colors.text).bg(colors.surface);
    styles.selected = Style::default()
        .fg(Color::Rgb(34, 20, 35))
        .bg(colors.primary)
        .bold();
    styles.hover = Style::default()
        .fg(Color::Rgb(34, 20, 35))
        .bg(colors.focus_ring)
        .bold();

    Theme::builder()
        .name("sunset")
        .colors(colors)
        .styles(styles)
        .build()
}

fn contrast_color(color: Color) -> Color {
    match color {
        Color::Black
        | Color::Red
        | Color::Green
        | Color::Blue
        | Color::Magenta
        | Color::Rgb(0..=127, _, _)
        | Color::Indexed(0..=7) => Color::White,
        Color::Rgb(r, g, b) => {
            let luminance = (r as u32 * 299) + (g as u32 * 587) + (b as u32 * 114);
            if luminance / 1000 > 150 {
                Color::Black
            } else {
                Color::White
            }
        }
        _ => Color::Black,
    }
}
