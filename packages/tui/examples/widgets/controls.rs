//! Assorted control widgets.
//!
//! Run with: `cargo run -p tui --example controls`
//! Use Tab to move focus. Enter/Space toggles controls, arrows move composite widgets.

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    RememberChanged(bool),
    AutoSaveChanged(bool),
    DensityChanged(String),
    NotesChanged(String),
    TabChanged(String),
    MenuSelected(String),
    ToggleAdvanced(bool),
    OpenDialog,
    CloseDialog,
    Frame(u64),
}

#[derive(Debug)]
struct State {
    remember: bool,
    autosave: bool,
    density: String,
    notes: String,
    tab: String,
    menu_action: String,
    advanced_open: bool,
    show_dialog: bool,
    frame: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            remember: true,
            autosave: false,
            density: "comfortable".to_owned(),
            notes: "Draft release notes here.".to_owned(),
            tab: "overview".to_owned(),
            menu_action: "none".to_owned(),
            advanced_open: true,
            show_dialog: false,
            frame: 0,
        }
    }
}

fn main() -> WidgetResult<()> {
    App::new(State::default())
        .on_frame(|info| Msg::Frame(info.frame))
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::RememberChanged(value) => state.remember = value,
        Msg::AutoSaveChanged(value) => state.autosave = value,
        Msg::DensityChanged(value) => state.density = value,
        Msg::NotesChanged(value) => state.notes = value,
        Msg::TabChanged(value) => state.tab = value,
        Msg::MenuSelected(value) => state.menu_action = value,
        Msg::ToggleAdvanced(value) => state.advanced_open = value,
        Msg::OpenDialog => state.show_dialog = true,
        Msg::CloseDialog => state.show_dialog = false,
        Msg::Frame(frame) => state.frame = frame,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let base = col::<Msg>()
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label("Controls").bold())
        .child(divider())
        .child(
            row::<Msg>()
                .gap(2)
                .child(settings_panel(state))
                .child(status_panel(state)),
        );

    let ui = overlay(base);
    if state.show_dialog {
        ui.layer(
            OverlayLayer::new(confirm_dialog())
                .floating(OverlayAnchor::Center)
                .size(42, 9)
                .z_index(10),
        )
        .trap_focus()
    } else {
        ui
    }
}

fn settings_panel(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Settings")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            checkbox("Remember this workspace")
                .checked(state.remember)
                .on_change(Msg::RememberChanged),
        )
        .child(
            switch("Auto save")
                .checked(state.autosave)
                .on_change(Msg::AutoSaveChanged),
        )
        .child(
            radio_group::<Msg>()
                .key("density")
                .selected(state.density.clone())
                .options([
                    RadioOption::new("compact", "Compact"),
                    RadioOption::new("comfortable", "Comfortable"),
                    RadioOption::new("spacious", "Spacious"),
                ])
                .on_change(Msg::DensityChanged),
        )
        .child(
            textarea::<Msg>()
                .key("notes")
                .height(6)
                .value(state.notes.clone())
                .placeholder("Notes")
                .on_change(Msg::NotesChanged),
        )
        .child(
            collapsible("Advanced")
                .expanded(state.advanced_open)
                .padding(Padding::new(1, 0, 0, 2))
                .gap(1)
                .on_toggle(Msg::ToggleAdvanced)
                .child(label(
                    "Nested content stays out of focus order while collapsed.",
                ))
                .child(button("Open dialog").on_click(|| Msg::OpenDialog)),
        )
}

fn status_panel(state: &State) -> impl Widget<Msg> {
    let progress = ((state.frame % 120) as f64) / 119.0;

    panel::<Msg>()
        .title("Navigation")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            tabs::<Msg>()
                .key("tabs")
                .selected(state.tab.clone())
                .tabs([
                    TabItem::new("overview", "Overview"),
                    TabItem::new("activity", "Activity"),
                    TabItem::new("billing", "Billing").disabled(true),
                ])
                .on_change(Msg::TabChanged),
        )
        .child(label(format!("Active tab: {}", state.tab)))
        .child(
            menu::<Msg>()
                .key("actions")
                .height(7)
                .items([
                    MenuItem::new("refresh", "Refresh"),
                    MenuItem::new("archive", "Archive"),
                    MenuItem::new("delete", "Delete").disabled(true),
                    MenuItem::new("export", "Export"),
                ])
                .on_select(Msg::MenuSelected),
        )
        .child(label(format!("Last action: {}", state.menu_action)))
        .child(
            progress_bar::<Msg>(progress)
                .label(format!("{:>3}%", (progress * 100.0).round() as u8))
                .width(28),
        )
        .child(
            loading_indicator::<Msg>()
                .frame(state.frame as usize)
                .label("Syncing"),
        )
}

fn confirm_dialog() -> impl Widget<Msg> {
    dialog::<Msg>()
        .title("Dialog")
        .gap(1)
        .child(label("This surface traps focus through the overlay.").bold())
        .child(label("Use the buttons below to close it."))
        .child(
            row::<Msg>()
                .gap(2)
                .child(button("Cancel").on_click(|| Msg::CloseDialog))
                .child(
                    button("Apply")
                        .variant(ButtonVariant::Primary)
                        .on_click(|| Msg::CloseDialog),
                ),
        )
}
