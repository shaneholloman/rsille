//! Animated widget wrapper.
//!
//! Run with: `cargo run -p tui --example animated`

use std::time::Duration;

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Toggle,
}

fn main() -> WidgetResult<()> {
    App::new(true).run_inline(|open, _| *open = !*open, view)
}

fn view(open: &bool) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Animated")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(button("Toggle").on_click(|| Msg::Toggle))
        .child(divider())
        .child(
            animate(
                panel::<Msg>()
                    .title(if *open { "Open" } else { "Closed" })
                    .padding(Padding::uniform(1))
                    .child(label("The wrapper applies layout animation.")),
            )
            .key("animated-panel")
            .layout(AnimationSpec::new(
                Duration::from_millis(240),
                Easing::EaseOut,
            )),
        )
}
