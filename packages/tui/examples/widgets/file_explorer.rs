//! FileExplorer widget with expandable and lazy directories.
//!
//! Run with: `cargo run -p tui --example file_explorer`

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Focused(String),
    Opened(String),
    Load(String),
    Selected(Vec<String>),
}

#[derive(Debug, Default)]
struct State {
    focused: String,
    opened: String,
    load_request: String,
    selected: Vec<String>,
}

fn main() -> WidgetResult<()> {
    App::new(State::default()).run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Focused(id) => state.focused = id,
        Msg::Opened(id) => state.opened = id,
        Msg::Load(id) => state.load_request = id,
        Msg::Selected(ids) => state.selected = ids,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    row::<Msg>()
        .padding(Padding::uniform(1))
        .gap(2)
        .child(
            file_explorer::<Msg>()
                .key("workspace-files")
                .height(18)
                .multi_select(true)
                .items([
                    FileExplorerItem::directory("packages", "packages")
                        .child(
                            FileExplorerItem::directory("packages/tui", "tui")
                                .child(
                                    FileExplorerItem::directory("packages/tui/src", "src")
                                        .child(FileExplorerItem::file(
                                            "packages/tui/src/lib.rs",
                                            "lib.rs",
                                        ))
                                        .child(FileExplorerItem::file(
                                            "packages/tui/src/app.rs",
                                            "app.rs",
                                        ))
                                        .child(FileExplorerItem::lazy_directory(
                                            "packages/tui/src/widgets",
                                            "widgets",
                                        )),
                                )
                                .child(
                                    FileExplorerItem::directory(
                                        "packages/tui/examples",
                                        "examples",
                                    )
                                    .child(FileExplorerItem::file(
                                        "packages/tui/examples/widgets/data_table.rs",
                                        "data_table.rs",
                                    ))
                                    .child(
                                        FileExplorerItem::file(
                                            "packages/tui/examples/widgets/file_explorer.rs",
                                            "file_explorer.rs",
                                        ),
                                    ),
                                ),
                        )
                        .child(FileExplorerItem::directory("packages/render", "render")),
                    FileExplorerItem::directory("docs", "docs")
                        .child(FileExplorerItem::file(
                            "docs/tui-gap-data-components.md",
                            "tui-gap-data-components.md",
                        ))
                        .child(FileExplorerItem::file(
                            "docs/tui-gap-async-model.md",
                            "tui-gap-async-model.md",
                        )),
                    FileExplorerItem::file("Cargo.toml", "Cargo.toml"),
                ])
                .on_change(Msg::Focused)
                .on_open(Msg::Opened)
                .on_load_children(Msg::Load)
                .on_selection_change(Msg::Selected),
        )
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(1)
                .child(label("FileExplorer Example").bold())
                .child(label(format!(
                    "Focused: {}",
                    if state.focused.is_empty() {
                        "none"
                    } else {
                        &state.focused
                    }
                )))
                .child(label(format!(
                    "Opened: {}",
                    if state.opened.is_empty() {
                        "none"
                    } else {
                        &state.opened
                    }
                )))
                .child(label(format!(
                    "Lazy load: {}",
                    if state.load_request.is_empty() {
                        "none"
                    } else {
                        &state.load_request
                    }
                )))
                .child(label(format!("Selected: {}", state.selected.len()))),
        )
}
