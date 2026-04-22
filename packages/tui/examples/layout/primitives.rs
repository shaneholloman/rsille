//! Layout primitives — scroll views, overlays, and split panes
//!
//! Run with: `cargo run -p tui --example primitives`
//! Press `p` to toggle the command palette and `Esc` to quit.
//! Use Tab to move focus, arrow keys to resize the split when the divider is focused,
//! and arrow keys / mouse wheel to scroll the viewports.

use tui::prelude::*;

#[derive(Debug, Default)]
struct State {
    palette_open: bool,
}

#[derive(Debug, Clone)]
enum Msg {
    TogglePalette,
}

fn main() -> WidgetResult<()> {
    App::new(State::default())
        .enable_mouse_capture()
        .on_key(KeyCode::Char('p'), || Msg::TogglePalette)
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::TogglePalette => state.palette_open = !state.palette_open,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let base = split(
        sidebar(),
        scroll_view(main_content())
            .focusable(true)
            .key("main-scroll")
            .border(BorderStyle::Single)
            .padding(Padding::uniform(1))
            .scrollbars(ScrollbarVisibility::Always),
    )
    .sidebar(28)
    .min_first(20)
    .min_second(28)
    .key("workspace");

    let anchored_help = OverlayLayer::new(help_popup())
        .anchored(
            (2, 1, 26, 5),
            OverlayAnchor::BottomLeft,
            OverlayAnchor::TopLeft,
        )
        .offset(0, 1)
        .size(34, 5)
        .z_index(5);

    let mut overlay_root = overlay(base).layer(anchored_help).key("layout-overlay");

    if state.palette_open {
        overlay_root = overlay_root
            .layer(
                OverlayLayer::new(command_palette())
                    .floating(OverlayAnchor::Center)
                    .size(44, 9)
                    .z_index(20),
            )
            .trap_focus();
    }

    overlay_root
}

fn sidebar() -> impl Widget<Msg> {
    scroll_view(
        col::<Msg>()
            .gap(1)
            .child(label("Explorer").bold())
            .child(divider())
            .children((0..24).map(|index| label(format!("src/features/{index:02}/component.rs")))),
    )
    .focusable(true)
    .key("sidebar-scroll")
    .border(BorderStyle::Single)
    .padding(Padding::uniform(1))
    .scrollbars(ScrollbarVisibility::Always)
}

fn main_content() -> impl Widget<Msg> {
    col::<Msg>()
        .gap(1)
        .child(label("Viewport primitives").bold())
        .child(label(
            "This pane is inside a generic scroll_view. Focus it and use the arrow keys or your mouse wheel.",
        ))
        .child(divider())
        .children((0..36).map(|index| {
            col::<Msg>()
                .border(BorderStyle::Single)
                .padding(Padding::uniform(1))
                .child(label(format!("Section {index:02}")).bold())
                .child(label(
                    "Shared scroll helpers now power scroll-to-offset and scroll-to-item style behavior.",
                ))
                .child(label(
                    "Overlay layers can render above layout flow, and split panes persist their divider position through widget state.",
                ))
        }))
}

fn help_popup() -> impl Widget<Msg> {
    col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .style(Style::default().bg(Color::Rgb(28, 32, 44)))
        .child(label("Anchored popup").bold())
        .child(label("This one is attached to the sidebar header region."))
        .child(label(
            "Use it for tooltips, select menus, or lightweight popovers.",
        ))
}

fn command_palette() -> impl Widget<Msg> {
    col::<Msg>()
        .border(BorderStyle::Double)
        .padding(Padding::uniform(1))
        .style(Style::default().bg(Color::Rgb(22, 28, 36)))
        .child(label("Command Palette").bold())
        .child(divider())
        .child(label("overlay + trap_focus gives you a modal-style layer."))
        .child(label("Press `p` again to close it."))
        .child(label(
            "The split under it stays mounted and keeps its pane size.",
        ))
}
