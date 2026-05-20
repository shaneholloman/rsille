//! Panel widget.
//!
//! Run with: `cargo run -p tui --example panel`

use tui::prelude::*;

fn main() -> WidgetResult<()> {
    App::new(()).run_inline(|_, _| {}, view)
}

fn view(_: &()) -> impl Widget<()> {
    row::<()>()
        .padding(Padding::uniform(1))
        .gap(2)
        .child(
            panel()
                .title("Default")
                .padding(Padding::uniform(1))
                .child(label("single border")),
        )
        .child(
            panel()
                .title("Rounded")
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .child(label("custom border")),
        )
        .child(
            panel()
                .borderless()
                .padding(Padding::uniform(1))
                .child(label("borderless")),
        )
}
