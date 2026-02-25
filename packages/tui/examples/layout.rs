//! Layout — col/row flex layout with padding and borders
//!
//! Run with: `cargo run -p tui --example layout`
//! Press Esc to quit.

use tui::prelude::*;

fn main() -> WidgetResult<()> {
    let state = ();

    App::new(state).run_inline(update, view)
}

fn update(_state: &mut (), _msg: ()) {}

fn view(_state: &()) -> impl Widget<()> {
    col::<()>()
        .padding(Padding::uniform(2))
        .gap(1)
        .child(
            col::<()>()
                .border(BorderStyle::Single)
                .padding(Padding::uniform(1))
                .child(label("Vertical layout"))
                .child(divider())
                .child(label("Item 1"))
                .child(label("Item 2"))
                .child(label("Item 3")),
        )
        .child(
            row::<()>()
                .border(BorderStyle::Single)
                .padding(Padding::uniform(1))
                .gap(2)
                .child(label("A"))
                .child(label("B"))
                .child(label("C")),
        )
}
