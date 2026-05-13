//! Visual post-processing effects regression stage.
//!
//! Run with: `cargo run -p tui --example visual_effects`
//! Use Tab to focus buttons. Enter/Space switches the active effect.
//!
//! Regression coverage:
//! - compact stage: small text areas should not drift or overflow.
//! - wide stage: large areas exercise the automatic reduced-effect path.
//! - policy preview: normal, reduced, and disabled variants are shown without
//!   requiring a manual global configuration change.

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

#[derive(Debug, Clone, Copy)]
struct Mode {
    name: &'static str,
    risk: &'static str,
}

const MODES: [Mode; 13] = [
    Mode {
        name: "Fade",
        risk: "opacity mask",
    },
    Mode {
        name: "Gradient",
        risk: "color sweep",
    },
    Mode {
        name: "Shatter",
        risk: "cell aspect",
    },
    Mode {
        name: "Magic lamp",
        risk: "anchor mapping",
    },
    Mode {
        name: "Wipe",
        risk: "stagger reveal",
    },
    Mode {
        name: "Dissolve",
        risk: "stable noise",
    },
    Mode {
        name: "Wave",
        risk: "row offset",
    },
    Mode {
        name: "Glitch",
        risk: "seeded jitter",
    },
    Mode {
        name: "Scanline",
        risk: "crt overlay",
    },
    Mode {
        name: "Typewriter",
        risk: "row-major reveal",
    },
    Mode {
        name: "Blur-like",
        risk: "cell degradation",
    },
    Mode {
        name: "Highlight",
        risk: "focus sweep",
    },
    Mode {
        name: "Sparkle",
        risk: "density gate",
    },
];

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
        1 | 6 | 7 => 4.8,
        8 | 11 | 12 => 4.8,
        2 => 2.4,
        3 => 4.2,
        _ => 3.4,
    }
}

fn mode_duration(mode: usize) -> f64 {
    effect_duration(mode) + 1.0
}

fn view(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Visual Effects")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            row::<Msg>()
                .gap(2)
                .child(button("Next effect").on_click(|| Msg::Next))
                .child(label(format!(
                    "{:<11} {:>3}%  {}",
                    MODES[state.mode].name,
                    (state.progress * 100.0).round() as u8,
                    MODES[state.mode].risk
                ))),
        )
        .child(divider())
        .child(
            split(
                panel::<Msg>()
                    .title("Stages")
                    .padding(Padding::uniform(1))
                    .gap(1)
                    .child(compact_stage(state))
                    .child(wide_stage(state)),
                policy_preview(state),
            )
            .first_size(62)
            .min_first(42)
            .min_second(28)
            .resizable(false),
        )
}

// Small stage keeps content deliberately dense to catch text clipping and
// geometry effects that move cells outside tight bounds.
fn compact_stage(state: &State) -> Visual<Msg> {
    visual(stage_body("Compact", state, false))
        .progress(stage_progress(state.mode, state.progress))
        .seed(0xC0FF_EE)
        .effect(stage_effect(state.mode, state.progress))
}

// Wide stage uses more rows and columns so the visual wrapper can hit its
// area-sensitive downgrade path on large terminals.
fn wide_stage(state: &State) -> Visual<Msg> {
    visual(stage_body("Wide downgrade", state, true))
        .progress(stage_progress(state.mode, state.progress))
        .seed(0xFACE_FEED)
        .effect(stage_effect(state.mode, state.progress))
}

// This is a local visual comparison: reduced uses each effect's reduced form,
// disabled jumps the original effect to final progress. It keeps the example
// self-contained.
fn policy_preview(state: &State) -> impl Widget<Msg> {
    let effect = stage_effect(state.mode, state.progress);

    panel::<Msg>()
        .title("Motion Policy Preview")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(policy_stage("Normal", state.progress, effect.clone()))
        .child(policy_stage("Reduced", state.progress, effect.reduced()))
        .child(policy_stage("Disabled", 1.0, effect))
        .child(divider())
        .child(theme_slots(state.progress))
}

fn policy_stage(name: &'static str, progress: f64, effect: VisualEffect) -> Visual<Msg> {
    let body = panel::<Msg>()
        .title(name)
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label("deploy rsille-tui").fg(Color::Cyan).bold())
        .child(label("api edge jobs"));

    visual(body).progress(progress).seed(77).effect(effect)
}

fn theme_slots(progress: f64) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Theme Slots")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(slot_stage(
            "modal in",
            progress,
            EffectSlot::ModalEnter,
            ThemeEffects::default(),
        ))
        .child(slot_stage(
            "modal out",
            progress,
            EffectSlot::ModalExit,
            ThemeEffects::default(),
        ))
        .child(slot_stage(
            "toast in",
            progress,
            EffectSlot::ToastEnter,
            ThemeEffects::default(),
        ))
        .child(slot_stage(
            "toast out",
            progress,
            EffectSlot::ToastExit,
            ThemeEffects::default(),
        ))
        .child(slot_stage(
            "focus",
            progress,
            EffectSlot::FocusPulse,
            ThemeEffects::default(),
        ))
        .child(slot_stage(
            "screen",
            progress,
            EffectSlot::ScreenTransition,
            ThemeEffects::default(),
        ))
}

fn slot_stage(
    name: &'static str,
    progress: f64,
    slot: EffectSlot,
    effects: ThemeEffects,
) -> Visual<Msg> {
    visual(
        row::<Msg>()
            .gap(1)
            .child(label(format!("{name:<9}")).fg(Color::Cyan))
            .child(label("theme preset")),
    )
    .progress(progress)
    .seed(0x5107)
    .effect(effects.get(slot))
}

fn stage_body(title: &'static str, state: &State, wide: bool) -> impl Widget<Msg> {
    let mut body = panel::<Msg>()
        .title(title)
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label("Release train").fg(Color::Cyan).bold())
        .child(label(format!("mode: {}", MODES[state.mode].name)))
        .child(progress_bar::<Msg>(state.progress))
        .child(
            row::<Msg>()
                .gap(2)
                .child(label("api").fg(Color::Green))
                .child(label("edge").fg(Color::Yellow))
                .child(label("jobs").fg(Color::Magenta)),
        );

    if wide {
        body = body
            .child(label("us-east  ready   eu-west  canary   ap-south queued"))
            .child(label("cache    warm    stream   live     workers  24/24"))
            .child(label(
                "alerts   none    budget   ok       latency  p95 48ms",
            ));
    }

    body
}

fn stage_progress(mode: usize, progress: f64) -> f64 {
    match mode {
        1 => 1.0,
        _ => progress,
    }
}

fn stage_effect(mode: usize, progress: f64) -> VisualEffect {
    match mode {
        0 => VisualEffect::fade_in(),
        1 => VisualEffect::gradient(
            Color::Rgb(56, 189, 248),
            Color::Rgb(244, 114, 182),
            GradientDirection::Diagonal,
        )
        .phase(progress),
        2 => VisualEffect::shatter().with_seed(42).with_spread(24.0, 8.0),
        3 => VisualEffect::magic_lamp(VisualAnchor::Bottom).squeeze(0.04),
        4 => VisualEffect::stagger_rows(
            0.035,
            VisualEffect::reveal(WipeDirection::LeftToRight).softness(0.04),
        ),
        5 => VisualEffect::dissolve().with_seed(0x5150),
        6 => VisualEffect::parallel(vec![
            VisualEffect::wave(WaveAxis::Rows)
                .amplitude(3.0)
                .wavelength(5.0)
                .phase(progress),
            VisualEffect::gradient(
                Color::Rgb(125, 211, 252),
                Color::Rgb(134, 239, 172),
                GradientDirection::Horizontal,
            )
            .phase(progress * 0.5),
        ]),
        7 => VisualEffect::glitch().with_seed(0xBAD5_EED).intensity(0.85),
        8 => VisualEffect::scanline()
            .density(0.5)
            .intensity(0.42)
            .phase(progress),
        9 => VisualEffect::typewriter().cursor(true),
        10 => VisualEffect::blur_like()
            .radius(2.0)
            .blur_mode(BlurMode::In),
        11 => VisualEffect::highlight_sweep()
            .width(0.18)
            .color(Color::Rgb(255, 255, 180))
            .direction(GradientDirection::Diagonal),
        _ => VisualEffect::sparkle()
            .density(0.12)
            .color(Color::Rgb(255, 255, 200))
            .with_seed(0x5FA2_C1E),
    }
}
