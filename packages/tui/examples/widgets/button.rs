//! Button — interactive buttons with messages
//!
//! Run with: `cargo run -p tui --example button`
//! Use Tab to focus buttons, Enter/Space to activate. Esc to quit.

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Increment,
    Decrement,
    Reset,
}

fn main() -> WidgetResult<()> {
    let state = 0i32;

    App::new(state).run_inline(update, view)
}

fn update(state: &mut i32, msg: Msg) {
    match msg {
        Msg::Increment => *state += 1,
        Msg::Decrement => *state -= 1,
        Msg::Reset => *state = 0,
    }
}

fn view(state: &i32) -> impl Widget<Msg> {
    col::<Msg>()
        .child(label(format!("Count: {}", state)))
        .child(divider())
        .child(
            row::<Msg>()
                .gap(2)
                .child(button("+1").on_click(|| Msg::Increment))
                .child(button("-1").on_click(|| Msg::Decrement))
                .child(
                    button("Reset")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::Reset),
                ),
        )
}
