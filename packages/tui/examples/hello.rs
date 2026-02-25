//! Hello World — minimal TUI example
//!
//! Run with: `cargo run -p tui --example hello`
//! Press Esc to quit.

use tui::prelude::*;

fn main() -> WidgetResult<()> {
    let state = ();

    App::new(state).run_inline(update, view)
}

fn update(_state: &mut (), _msg: ()) {}

fn view(_state: &()) -> impl Widget<()> {
    col::<()>()
        .child(label("Hello, TUI!"))
        .child(spacer())
        .child(label("Press Tab to navigate, Esc to quit."))
}
