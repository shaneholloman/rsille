//! Tree widget with expandable nodes and keyboard navigation.
//!
//! Run with: `cargo run -p tui --example tree`
//! Use Left/Right to collapse or expand, Up/Down to move, and Enter to open leaf nodes.

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Focused(String),
    Opened(String),
}

#[derive(Debug, Default)]
struct State {
    active: String,
    opened: String,
}

fn main() -> WidgetResult<()> {
    App::new(State::default()).run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Focused(id) => state.active = id,
        Msg::Opened(id) => state.opened = id,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    row::<Msg>()
        .padding(Padding::uniform(1))
        .gap(2)
        .child(
            tree::<Msg>()
                .key("workspace")
                .height(12)
                .items([
                    TreeItem::new("packages", "packages")
                        .child(
                            TreeItem::new("packages/tui", "tui")
                                .child(TreeItem::new("packages/tui/src", "src"))
                                .child(TreeItem::new("packages/tui/examples", "examples")),
                        )
                        .child(TreeItem::new("packages/render", "render"))
                        .child(TreeItem::new("packages/canvas", "canvas")),
                    TreeItem::new("docs", "docs")
                        .child(TreeItem::new("docs/roadmap.md", "roadmap.md"))
                        .child(TreeItem::new("docs/theme.md", "theme.md")),
                    TreeItem::new("scripts", "scripts")
                        .child(TreeItem::new("scripts/release.sh", "release.sh")),
                ])
                .on_change(Msg::Focused)
                .on_submit(Msg::Opened),
        )
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(1)
                .child(label("Tree Example").bold())
                .child(label("Right expands, Left collapses."))
                .child(label("Enter opens the current leaf node."))
                .child(divider().text("Focus"))
                .child(label(format!(
                    "Active node: {}",
                    if state.active.is_empty() {
                        "none"
                    } else {
                        &state.active
                    }
                )))
                .child(label(format!(
                    "Opened leaf: {}",
                    if state.opened.is_empty() {
                        "none"
                    } else {
                        &state.opened
                    }
                )))
                .child(
                    button("Open current leaf")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::Opened("triggered-by-button".to_owned())),
                ),
        )
}
