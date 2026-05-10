//! Fuzzy command palette with overlay-friendly behavior.
//!
//! Run with: `cargo run -p tui --example command_palette`

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Highlighted(String),
    Run(String),
    Close,
}

#[derive(Debug, Default)]
struct State {
    highlighted: String,
    executed: String,
}

fn main() -> WidgetResult<()> {
    App::new(State::default()).run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Highlighted(id) => state.highlighted = id,
        Msg::Run(id) => state.executed = id,
        Msg::Close => state.executed = "palette-closed".to_owned(),
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    row::<Msg>()
        .padding(Padding::uniform(1))
        .gap(2)
        .child(
            visual(
                command_palette::<Msg>()
                    .key("commands")
                    .height(12)
                    .title("Workspace Commands")
                    .prompt(">")
                    .items([
                        CommandItem::new("open-file", "Open file")
                            .keywords(["file", "finder", "jump"]),
                        CommandItem::new("new-terminal", "New terminal")
                            .keywords(["shell", "console", "spawn"]),
                        CommandItem::new("toggle-preview", "Toggle preview panel")
                            .keywords(["preview", "panel", "layout"]),
                        CommandItem::new("deploy-preview", "Deploy preview")
                            .keywords(["vercel", "preview", "ship"]),
                        CommandItem::new("sync-schema", "Sync schema")
                            .keywords(["database", "migrate", "db"]),
                        CommandItem::new("open-settings", "Open settings")
                            .keywords(["preferences", "config"]),
                    ])
                    .on_change(Msg::Highlighted)
                    .on_submit(Msg::Run)
                    .on_close(|| Msg::Close),
            )
            .key("commands-visual")
            .enter_theme(EffectSlot::ModalEnter)
            .exit_theme(EffectSlot::ModalExit),
        )
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(1)
                .child(label("Command Palette Example").bold())
                .child(label("Type to filter with fuzzy matching."))
                .child(label("Enter runs the current command, Esc emits close."))
                .child(divider().text("State"))
                .child(label(format!(
                    "Highlighted: {}",
                    if state.highlighted.is_empty() {
                        "none"
                    } else {
                        &state.highlighted
                    }
                )))
                .child(label(format!(
                    "Executed: {}",
                    if state.executed.is_empty() {
                        "none"
                    } else {
                        &state.executed
                    }
                ))),
        )
}
