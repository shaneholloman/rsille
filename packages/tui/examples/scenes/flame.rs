//! Procedural flame study with a heat-field simulation.
//!
//! Run with: `cargo run -p tui --example flame`
//! Press Esc to quit.

use tui::prelude::*;

const WIDTH: usize = 48;
const HEIGHT: usize = 26;
const SOURCE_ROWS: usize = 3;
const SIM_STEP_SECS: f32 = 1.0 / 40.0;

#[derive(Debug, Clone, Copy)]
enum Msg {
    Frame(FrameInfo),
}

#[derive(Debug)]
struct State {
    heat: Vec<u8>,
    elapsed_secs: f32,
    accumulator_secs: f32,
    step_count: u32,
}

impl State {
    fn new() -> Self {
        let mut state = Self {
            heat: vec![0; WIDTH * HEIGHT],
            elapsed_secs: 0.0,
            accumulator_secs: 0.0,
            step_count: 0,
        };

        for step in 0..48 {
            state.elapsed_secs = step as f32 * SIM_STEP_SECS;
            state.step();
        }

        state
    }

    fn frame(&mut self, info: FrameInfo) {
        self.elapsed_secs = info.since_start.as_secs_f32();
        self.accumulator_secs += info.delta.as_secs_f32().min(0.1);

        while self.accumulator_secs >= SIM_STEP_SECS {
            self.step();
            self.accumulator_secs -= SIM_STEP_SECS;
        }
    }

    fn step(&mut self) {
        let prev = self.heat.clone();
        let mut next = vec![0; prev.len()];
        let center = 0.5 + 0.045 * (self.elapsed_secs * 0.85).sin();
        let burner_width = 0.43 + 0.035 * (self.elapsed_secs * 0.33).cos();

        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let idx = index(x, y);

                if y >= HEIGHT - SOURCE_ROWS {
                    next[idx] = self.source_heat(x, y, center, burner_width);
                    continue;
                }

                let rise = 1.0 - y as f32 / (HEIGHT.saturating_sub(1) as f32);
                let sway = ((self.elapsed_secs * 1.9 + rise * 5.2).sin() * 1.4).round() as isize;
                let turbulence = if hash01(
                    x as u32 * 17,
                    y as u32 * 29,
                    self.step_count.wrapping_mul(13),
                ) > 0.62
                {
                    1
                } else if hash01(
                    x as u32 * 31,
                    y as u32 * 11,
                    self.step_count.wrapping_mul(7),
                ) < 0.18
                {
                    -1
                } else {
                    0
                };

                let sample_x = x as isize + sway + turbulence;
                let below = sample(&prev, sample_x, y + 1) as u16
                    + sample(&prev, sample_x - 1, y + 1) as u16
                    + sample(&prev, sample_x + 1, y + 1) as u16
                    + sample(&prev, sample_x, (y + 2).min(HEIGHT - 1)) as u16;
                let avg = (below / 4) as u8;

                let cooling = 5
                    + ((HEIGHT - 1 - y) / 4) as u8
                    + (hash01(
                        x as u32 * 13,
                        y as u32 * 19,
                        self.step_count.wrapping_mul(5),
                    ) * 8.0) as u8;

                next[idx] = avg.saturating_sub(cooling);
            }
        }

        self.heat = next;
        self.step_count = self.step_count.wrapping_add(1);
    }

    fn source_heat(&self, x: usize, y: usize, center: f32, burner_width: f32) -> u8 {
        let nx = x as f32 / (WIDTH.saturating_sub(1) as f32);
        let rel = ((nx - center).abs() / burner_width).clamp(0.0, 1.35);
        let shape = (1.0 - rel.powf(1.65)).max(0.0);
        let row_gain = match HEIGHT - 1 - y {
            0 => 1.0,
            1 => 0.90,
            _ => 0.78,
        };

        let pocket = ((nx * 16.0 + self.elapsed_secs * 3.1).sin() * 0.5 + 0.5)
            * ((nx * 7.0 - self.elapsed_secs * 1.7).cos() * 0.5 + 0.5);
        let flicker = 0.72
            + hash01(
                x as u32 * 43,
                y as u32 * 23,
                self.step_count.wrapping_mul(3),
            ) * 0.28;
        let ember_burst = if rel < 0.12 {
            ((self.elapsed_secs * 11.0 + nx * 37.0).sin() * 0.5 + 0.5) * 0.14
        } else {
            0.0
        };

        let intensity = shape * row_gain * (0.78 + 0.22 * pocket) * flicker + ember_burst;
        (intensity * 255.0).clamp(0.0, 255.0) as u8
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
        .style(Style::default().bg(Color::Rgb(4, 3, 7)))
        .padding(Padding::uniform(1))
        .gap(1)
        .align_items(AlignItems::Center)
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(0)
                .style(Style::default().bg(Color::Rgb(8, 5, 10)))
                .children((0..HEIGHT).map(|y| flame_line(state, y))),
        )
        .child(label("Esc quits.").fg(Color::Rgb(116, 98, 92)))
}

fn flame_line(state: &State, y: usize) -> Flex<Msg> {
    let mut line = row::<Msg>().gap(0);

    for x in 0..WIDTH {
        let heat = state.heat[index(x, y)];
        line = line.child(label("  ").bg(heat_to_color(heat)));
    }

    line
}

fn heat_to_color(heat: u8) -> Color {
    match heat {
        0..=7 => Color::Rgb(8, 5, 10),
        8..=18 => Color::Rgb(18, 8, 12),
        19..=32 => Color::Rgb(40, 11, 10),
        33..=50 => Color::Rgb(72, 16, 8),
        51..=72 => Color::Rgb(118, 28, 6),
        73..=96 => Color::Rgb(171, 50, 6),
        97..=124 => Color::Rgb(219, 89, 6),
        125..=152 => Color::Rgb(243, 134, 12),
        153..=180 => Color::Rgb(251, 176, 38),
        181..=208 => Color::Rgb(255, 214, 92),
        209..=232 => Color::Rgb(255, 238, 160),
        _ => Color::Rgb(255, 249, 226),
    }
}

fn sample(heat: &[u8], x: isize, y: usize) -> u8 {
    heat[index(clamp_x(x), y)]
}

fn clamp_x(x: isize) -> usize {
    x.clamp(0, WIDTH.saturating_sub(1) as isize) as usize
}

fn index(x: usize, y: usize) -> usize {
    y * WIDTH + x
}

fn hash01(a: u32, b: u32, c: u32) -> f32 {
    let mut value =
        a.wrapping_mul(0x9e37_79b9) ^ b.wrapping_mul(0x85eb_ca6b) ^ c.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    (value & 0xffff) as f32 / 65_535.0
}
