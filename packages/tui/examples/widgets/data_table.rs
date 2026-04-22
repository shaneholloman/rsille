//! DataTable widget with internal row navigation.
//!
//! Run with: `cargo run -p tui --example data_table`
//! Use Up/Down/Home/End/PageUp/PageDown to move between rows. Press Enter to submit.

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Highlighted(String),
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
        Msg::Highlighted(id) => state.active = id,
        Msg::Opened(id) => state.opened = id,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    row::<Msg>()
        .padding(Padding::uniform(1))
        .gap(2)
        .child(
            data_table::<Msg>()
                .key("deployments")
                .height(10)
                .columns([
                    DataTableColumn::new("Service").width(18),
                    DataTableColumn::new("Stage").width(10),
                    DataTableColumn::new("Latency")
                        .width(10)
                        .align(TableAlign::Right),
                    DataTableColumn::new("Owner").width(12),
                ])
                .rows([
                    DataTableRow::new("api-prod", ["api-gateway", "prod", "34 ms", "platform"]),
                    DataTableRow::new("worker-prod", ["job-worker", "prod", "51 ms", "ops"]),
                    DataTableRow::new("search-staging", ["search", "staging", "89 ms", "search"]),
                    DataTableRow::new("billing-prod", ["billing", "prod", "41 ms", "finance"]),
                    DataTableRow::new("auth-canary", ["auth", "canary", "27 ms", "identity"]),
                    DataTableRow::new("email-queue", ["mailer", "queue", "73 ms", "growth"]),
                ])
                .on_change(Msg::Highlighted)
                .on_submit(Msg::Opened),
        )
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(1)
                .child(label("DataTable Example").bold())
                .child(label("Tab can move focus away from the table."))
                .child(label("Enter commits the current row."))
                .child(divider().text("Selection"))
                .child(label(format!(
                    "Active row: {}",
                    if state.active.is_empty() {
                        "none"
                    } else {
                        &state.active
                    }
                )))
                .child(label(format!(
                    "Submitted row: {}",
                    if state.opened.is_empty() {
                        "none"
                    } else {
                        &state.opened
                    }
                )))
                .child(
                    button("Inspect selection")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::Opened("manual-inspect".to_owned())),
                ),
        )
}
