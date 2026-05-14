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
    overlay(
        col::<()>()
            .padding(Padding::uniform(1))
            .gap(1)
            .child(
                row::<()>()
                    .gap(1)
                    .child(
                        panel()
                            .title("fixed")
                            .padding(Padding::uniform(1))
                            .child(label("12 x natural")),
                    )
                    .child(
                        panel()
                            .title("fill")
                            .padding(Padding::uniform(1))
                            .child(label("fills remaining row space")),
                    ),
            )
            .child(
                panel()
                    .title("preferred text")
                    .padding(Padding::uniform(1))
                    .child(
                        label("A wrapping label measures height from the width proposed by its parent.")
                            .wrap(TextWrap::Word),
                    ),
            )
            .child(
                panel()
                    .title("scroll")
                    .padding(Padding::ZERO)
                    .child(
                        scroll_view(
                            label("row 1\nrow 2\nrow 3\nrow 4\nrow 5\nrow 6\nrow 7\nrow 8"),
                        )
                        .vertical(),
                    ),
            ),
    )
    .layer(
        OverlayLayer::new(
            panel()
                .title("overlay")
                .padding(Padding::uniform(1))
                .child(label("measured popup")),
        )
        .floating(OverlayAnchor::TopRight),
    )
}
