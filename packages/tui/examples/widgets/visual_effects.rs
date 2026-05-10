//! Visual post-processing effects.
//!
//! Run with: `cargo run -p tui --example visual_effects`
//! Use Tab to focus buttons. Enter/Space switches the active effect.

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Frame(FrameInfo),
    Next,
}

#[derive(Debug)]
struct State {
    mode: usize,
    progress: f64,
    mode_elapsed: f64,
}

const MODES: [&str; 4] = ["Fade", "Gradient", "Shatter", "Magic lamp"];

fn main() -> WidgetResult<()> {
    App::new(State {
        mode: 0,
        progress: 0.0,
        mode_elapsed: 0.0,
    })
    .on_frame(Msg::Frame)
    .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Frame(info) => {
            state.mode_elapsed += info.delta.as_secs_f64();
            state.progress = (state.mode_elapsed / effect_duration(state.mode)).clamp(0.0, 1.0);

            if state.mode_elapsed >= mode_duration(state.mode) {
                next_mode(state);
            }
        }
        Msg::Next => next_mode(state),
    }
}

fn next_mode(state: &mut State) {
    state.mode = (state.mode + 1) % MODES.len();
    state.progress = 0.0;
    state.mode_elapsed = 0.0;
}

fn effect_duration(mode: usize) -> f64 {
    match mode {
        0 => 4.0,
        1 => 5.0,
        2 => 2.4,
        _ => 4.6,
    }
}

fn mode_duration(mode: usize) -> f64 {
    match mode {
        0 => 5.0,
        1 => 6.0,
        2 => 2.4,
        _ => 5.6,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Visual Effects")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            row::<Msg>()
                .gap(2)
                .child(button("Next effect").on_click(|| Msg::Next).animated())
                .child(label(format!(
                    "{:<10} {:>3}%",
                    MODES[state.mode],
                    (state.progress * 100.0).round() as u8
                ))),
        )
        .child(divider())
        .child(
            split(effect_stage(state), inspector(state))
                .first_size(48)
                .min_first(36)
                .min_second(24)
                .resizable(false),
        )
}

fn effect_stage(state: &State) -> Visual<Msg> {
    let base = panel::<Msg>()
        .title("Effect Stage")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label("Release train").fg(Color::Cyan).bold())
        .child(label("canary/us-east"))
        .child(progress_bar::<Msg>(state.progress).animated())
        .child(
            row::<Msg>()
                .gap(2)
                .child(label("api").fg(Color::Green))
                .child(label("edge").fg(Color::Yellow))
                .child(label("jobs").fg(Color::Magenta)),
        );

    match state.mode {
        0 => visual(base)
            .progress(state.progress)
            .effect(VisualEffect::fade_in()),
        1 => visual(base).progress(1.0).effect(
            VisualEffect::gradient(
                Color::Rgb(56, 189, 248),
                Color::Rgb(244, 114, 182),
                GradientDirection::Diagonal,
            )
            .phase(state.progress),
        ),
        2 => visual(base)
            .progress(state.progress)
            .effect(VisualEffect::shatter().with_seed(42).with_spread(24.0, 8.0)),
        _ => visual(base)
            .progress(state.progress)
            .effect(VisualEffect::magic_lamp(VisualAnchor::Bottom).squeeze(0.04)),
    }
}

fn inspector(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Timing")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label(format!("Mode: {}", MODES[state.mode])))
        .child(label(format!(
            "Motion: {:.1}s",
            effect_duration(state.mode)
        )))
        .child(label(format!(
            "Hold: {:.1}s",
            mode_duration(state.mode) - effect_duration(state.mode)
        )))
        .child(divider())
        .child(label("Viewport: compact"))
}
