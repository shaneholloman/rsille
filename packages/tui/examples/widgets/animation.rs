//! Component animations.
//!
//! Run with: `cargo run -p tui --example animation`
//! Use Tab to move focus. Enter/Space toggles switches and buttons.

use std::time::Duration;

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Tick,
    DeployChanged(bool),
    ReviewChanged(bool),
    AlertsChanged(bool),
    Bump,
    Reset,
}

#[derive(Debug)]
struct State {
    target: f64,
    step: usize,
    deploy: bool,
    review: bool,
    alerts: bool,
}

const TARGETS: [f64; 6] = [0.08, 0.76, 0.22, 1.0, 0.42, 0.9];

fn main() -> WidgetResult<()> {
    App::new(State {
        target: TARGETS[0],
        step: 0,
        deploy: false,
        review: true,
        alerts: false,
    })
    .on_tick(Duration::from_millis(900), || Msg::Tick)
    .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Tick => advance(state),
        Msg::DeployChanged(value) => state.deploy = value,
        Msg::ReviewChanged(value) => state.review = value,
        Msg::AlertsChanged(value) => state.alerts = value,
        Msg::Bump => advance(state),
        Msg::Reset => {
            state.target = TARGETS[0];
            state.step = 0;
            state.deploy = false;
            state.review = true;
            state.alerts = false;
        }
    }
}

fn advance(state: &mut State) {
    state.step = (state.step + 1) % TARGETS.len();
    state.target = TARGETS[state.step];

    if state.step % 2 == 0 {
        state.alerts = !state.alerts;
    }
    if state.target >= 0.9 {
        state.deploy = true;
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let percent = (state.target * 100.0).round() as u8;

    panel::<Msg>()
        .title("Animation Lab")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label(format!("Target: {percent}%")))
        .child(spinner_row())
        .child(divider())
        .child(progress_section(state.target))
        .child(divider())
        .child(switch_section(state))
        .child(divider())
        .child(action_row())
}

fn spinner_row() -> impl Widget<Msg> {
    row::<Msg>()
        .gap(3)
        .child(
            loading_indicator::<Msg>()
                .key("sync-spinner")
                .label("Sync")
                .animated(),
        )
        .child(
            loading_indicator::<Msg>()
                .key("build-spinner")
                .label("Build")
                .animated(),
        )
        .child(
            loading_indicator::<Msg>()
                .key("deploy-spinner")
                .label("Deploy")
                .animated(),
        )
}

fn progress_section(target: f64) -> impl Widget<Msg> {
    col::<Msg>()
        .gap(1)
        .child(easing_row(
            "Linear",
            target,
            AnimationSpec::new(Duration::from_millis(520), Easing::Linear),
            "linear-progress",
        ))
        .child(easing_row(
            "Ease out",
            target,
            AnimationSpec::new(Duration::from_millis(520), Easing::EaseOut),
            "ease-out-progress",
        ))
        .child(easing_row(
            "Ease in/out",
            target,
            AnimationSpec::new(Duration::from_millis(720), Easing::EaseInOut),
            "ease-in-out-progress",
        ))
}

fn easing_row(label_text: &str, value: f64, spec: AnimationSpec, key: &str) -> impl Widget<Msg> {
    row::<Msg>()
        .gap(2)
        .child(label(format!("{label_text:<11}")))
        .child(
            progress_bar::<Msg>(value)
                .key(key)
                .width(34)
                .animation(spec),
        )
}

fn switch_section(state: &State) -> impl Widget<Msg> {
    col::<Msg>()
        .gap(1)
        .child(
            row::<Msg>()
                .gap(2)
                .child(
                    switch("Deploy")
                        .key("deploy-switch")
                        .checked(state.deploy)
                        .on_change(Msg::DeployChanged)
                        .animated(),
                )
                .child(
                    switch("Review")
                        .key("review-switch")
                        .checked(state.review)
                        .on_change(Msg::ReviewChanged)
                        .animated(),
                ),
        )
        .child(
            switch("Alerts")
                .key("alerts-switch")
                .checked(state.alerts)
                .on_change(Msg::AlertsChanged)
                .animation(AnimationSpec::new(
                    Duration::from_millis(320),
                    Easing::EaseInOut,
                )),
        )
}

fn action_row() -> impl Widget<Msg> {
    row::<Msg>()
        .gap(2)
        .child(button("Step").on_click(|| Msg::Bump).animated())
        .child(
            button("Commit")
                .variant(ButtonVariant::Primary)
                .on_click(|| Msg::Bump)
                .animated(),
        )
        .child(
            button("Reset")
                .variant(ButtonVariant::Secondary)
                .on_click(|| Msg::Reset)
                .animated(),
        )
}
