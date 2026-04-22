//! Focused wave study with layered water motion.
//!
//! Run with: `cargo run -p tui --example wave`
//! Press Esc to quit.

use tui::prelude::*;

const WIDTH: usize = 50;
const HEIGHT: usize = 24;

#[derive(Debug, Clone, Copy)]
enum Msg {
    Frame(FrameInfo),
}

#[derive(Debug)]
struct State {
    elapsed_secs: f32,
}

impl State {
    fn new() -> Self {
        Self { elapsed_secs: 0.0 }
    }

    fn frame(&mut self, info: FrameInfo) {
        self.elapsed_secs = info.since_start.as_secs_f32();
    }
}

fn main() -> WidgetResult<()> {
    App::new(State::new())
        .on_frame(Msg::Frame)
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Frame(info) => state.frame(info),
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    col::<Msg>()
        .style(Style::default().bg(Color::Rgb(3, 10, 22)))
        .padding(Padding::uniform(1))
        .gap(1)
        .align_items(AlignItems::Center)
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(0)
                .style(Style::default().bg(Color::Rgb(5, 14, 30)))
                .children((0..HEIGHT).map(|y| wave_line(state, y))),
        )
        .child(label("Esc quits.").fg(Color::Rgb(104, 124, 148)))
}

fn wave_line(state: &State, y: usize) -> Flex<Msg> {
    let mut line = row::<Msg>().gap(0);

    for x in 0..WIDTH {
        line = line.child(label("  ").bg(cell_color(state, x, y)));
    }

    line
}

fn cell_color(state: &State, x: usize, y: usize) -> Color {
    let t = state.elapsed_secs;
    let nx = x as f32 / (WIDTH.saturating_sub(1) as f32);
    let ny = y as f32 / (HEIGHT.saturating_sub(1) as f32);

    let far_wave = 0.22 + 0.030 * (nx * 7.0 + t * 0.55).sin() + 0.012 * (nx * 3.0 - t * 0.30).cos();
    let upper_wave =
        0.38 + 0.042 * (nx * 9.0 - t * 0.95).sin() + 0.012 * (nx * 4.8 + t * 0.55).cos();
    let mid_wave =
        0.56 + 0.055 * (nx * 12.0 + t * 1.45).sin() + 0.016 * (nx * 5.5 - t * 0.70).cos();
    let front_wave =
        0.76 + 0.080 * (nx * 15.5 - t * 2.05).sin() + 0.022 * (nx * 6.5 + t * 1.10).cos();

    if near_wave(ny, front_wave, 0.016) {
        return if stripe(nx, t, 28.0) {
            Color::Rgb(246, 253, 255)
        } else {
            Color::Rgb(212, 244, 249)
        };
    }
    if near_wave(ny, mid_wave, 0.013) {
        return if stripe(nx, t + 0.8, 22.0) {
            Color::Rgb(150, 232, 235)
        } else {
            Color::Rgb(103, 202, 212)
        };
    }
    if near_wave(ny, upper_wave, 0.011) {
        return Color::Rgb(85, 175, 194);
    }
    if near_wave(ny, far_wave, 0.010) {
        return Color::Rgb(64, 139, 170);
    }

    if ny < far_wave {
        return if stripe(nx, t * 0.35, 13.0) {
            Color::Rgb(10, 36, 68)
        } else {
            Color::Rgb(7, 28, 57)
        };
    }
    if ny < upper_wave {
        return if stripe(nx, t * 0.55, 16.0) {
            Color::Rgb(12, 59, 96)
        } else {
            Color::Rgb(10, 48, 82)
        };
    }
    if ny < mid_wave {
        return if stripe(nx, t * 0.85, 18.0) {
            Color::Rgb(10, 87, 129)
        } else {
            Color::Rgb(8, 73, 115)
        };
    }
    if ny < front_wave {
        return if stripe(nx, t * 1.25, 22.0) {
            Color::Rgb(8, 118, 158)
        } else {
            Color::Rgb(7, 100, 141)
        };
    }

    if stripe(nx, t * 1.60, 24.0) {
        Color::Rgb(7, 92, 130)
    } else {
        Color::Rgb(5, 74, 110)
    }
}

fn near_wave(y: f32, line: f32, width: f32) -> bool {
    (y - line).abs() <= width
}

fn stripe(nx: f32, t: f32, density: f32) -> bool {
    ((nx * density - t * 1.35).sin() * 0.5 + 0.5) > 0.60
}
