//! Presence enter and exit animations.
//!
//! Run with: `cargo run -p tui --example presence_animation`
//! Use Tab to focus buttons. Enter/Space activates the focused button.

use std::time::Duration;

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Tick,
    TogglePanel,
    ToggleToast,
}

#[derive(Debug)]
struct State {
    panel_open: bool,
    toast_open: bool,
    tick: usize,
}

fn main() -> WidgetResult<()> {
    App::new(State {
        panel_open: true,
        toast_open: false,
        tick: 0,
    })
    .on_tick(Duration::from_millis(1500), || Msg::Tick)
    .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Tick => {
            state.tick = state.tick.wrapping_add(1);
            state.panel_open = !state.panel_open;
            if state.tick % 2 == 0 {
                state.toast_open = !state.toast_open;
            }
        }
        Msg::TogglePanel => state.panel_open = !state.panel_open,
        Msg::ToggleToast => state.toast_open = !state.toast_open,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Presence Motion")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            row::<Msg>()
                .gap(2)
                .child(button("Panel").on_click(|| Msg::TogglePanel).animated())
                .child(button("Toast").on_click(|| Msg::ToggleToast).animated()),
        )
        .child(divider())
        .child(presence_area(state))
        .child(toast_slot(state))
}

fn presence_area(state: &State) -> Box<dyn Widget<Msg>> {
    if state.panel_open {
        return animate(
            panel::<Msg>()
                .title("Deployment Window")
                .padding(Padding::uniform(1))
                .gap(1)
                .child(label("Release candidate: rsille-tui"))
                .child(label(format!("Heartbeat: {}", state.tick)))
                .child(progress_bar::<Msg>((state.tick % 8) as f64 / 7.0).animated()),
        )
        .key("deployment-window")
        .layout(AnimationSpec::new(
            Duration::from_millis(260),
            Easing::EaseOut,
        ))
        .enter(Timeline::parallel(vec![
            Timeline::single(Transition::scale_from_center()),
            Timeline::single(Transition::expand()),
        ]))
        .exit(Timeline::sequence(vec![
            Timeline::single(Transition::collapse()),
            Timeline::single(Transition::fade_out()),
        ]))
        .into_widget();
    }

    animate(
        panel::<Msg>()
            .title("Deployment Window")
            .padding(Padding::uniform(1))
            .child(label("Window is parked.")),
    )
    .key("deployment-placeholder")
    .layout(AnimationSpec::fast())
    .into_widget()
}

fn toast_slot(state: &State) -> Box<dyn Widget<Msg>> {
    if state.toast_open {
        return animate(
            panel::<Msg>()
                .title("Toast")
                .padding(Padding::uniform(1))
                .child(label(format!("Background task {} completed", state.tick))),
        )
        .key("toast")
        .layout_transition(LayoutTransition::size(AnimationSpec::new(
            Duration::from_millis(180),
            Easing::EaseOut,
        )))
        .enter(Transition::expand())
        .exit(Transition::collapse())
        .into_widget();
    }

    spacer::<Msg>().height(1).into_widget()
}
