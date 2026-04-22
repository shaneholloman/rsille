//! Equalizer animation demo with peak hold bars.
//!
//! Run with: `cargo run -p tui --example equalizer`
//! Press Esc to quit.

use tui::prelude::*;

const CHANNELS: usize = 8;
const LEVEL_HEIGHT: usize = 10;
const CHANNEL_NAMES: [&str; CHANNELS] =
    ["KICK", "BASS", "SNARE", "VOX", "PAD", "LEAD", "FX", "AIR"];

#[derive(Debug, Clone, Copy)]
enum Msg {
    Frame(FrameInfo),
}

#[derive(Debug)]
struct State {
    frame: u64,
    elapsed_secs: f32,
    peaks: [usize; CHANNELS],
    last_frame_ms: Option<f64>,
    avg_frame_ms: Option<f64>,
}

impl State {
    fn new() -> Self {
        Self {
            frame: 0,
            elapsed_secs: 0.0,
            peaks: [0; CHANNELS],
            last_frame_ms: None,
            avg_frame_ms: None,
        }
    }

    fn frame(&mut self, info: FrameInfo) {
        self.frame = info.frame;
        self.elapsed_secs = info.since_start.as_secs_f32();
        self.record_frame(info.delta);

        for channel in 0..CHANNELS {
            let level = channel_level(self.elapsed_secs, channel);
            let peak = &mut self.peaks[channel];
            if level >= *peak {
                *peak = level;
            } else if self.frame % 2 == 0 {
                *peak = peak.saturating_sub(1);
            }
        }
    }

    fn average_level(&self) -> usize {
        (0..CHANNELS)
            .map(|channel| channel_level(self.elapsed_secs, channel))
            .sum::<usize>()
            / CHANNELS
    }

    fn record_frame(&mut self, delta: std::time::Duration) {
        let frame_ms = delta.as_secs_f64() * 1_000.0;
        if self.frame == 0 {
            self.last_frame_ms = Some(frame_ms);
            self.avg_frame_ms = Some(frame_ms);
            return;
        }

        self.last_frame_ms = Some(frame_ms);
        self.avg_frame_ms = Some(match self.avg_frame_ms {
            Some(avg) => avg * 0.8 + frame_ms * 0.2,
            None => frame_ms,
        });
    }

    fn delta_text(&self) -> String {
        match (self.last_frame_ms, self.avg_frame_ms) {
            (Some(last), Some(avg)) => format!("{last:.1} ms | avg {avg:.1}"),
            _ => "warming up".to_owned(),
        }
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
        .style(Style::default().bg(Color::Rgb(9, 12, 20)))
        .padding(Padding::uniform(2))
        .gap(1)
        .align_items(AlignItems::Center)
        .child(
            label("Studio Equalizer")
                .bold()
                .fg(Color::Rgb(242, 246, 250)),
        )
        .child(
            label("Per-channel motion, peak hold, and vertical meter composition.")
                .fg(Color::Rgb(149, 161, 176)),
        )
        .child(divider().text("Animation"))
        .child(
            row::<Msg>()
                .gap(1)
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::End)
                .children((0..CHANNELS).map(|channel| meter_column(state, channel))),
        )
        .child(
            row::<Msg>()
                .gap(2)
                .justify_content(JustifyContent::Center)
                .child(info_card(
                    "Average",
                    format!("{:02}/10", state.average_level()),
                    Color::Rgb(96, 208, 255),
                ))
                .child(info_card(
                    "Frame",
                    format!("#{}", state.frame),
                    Color::Rgb(127, 233, 170),
                ))
                .child(info_card(
                    "Delta",
                    state.delta_text(),
                    Color::Rgb(186, 157, 255),
                ))
                .child(info_card(
                    "Mode",
                    "Auto mix".to_owned(),
                    Color::Rgb(255, 200, 92),
                )),
        )
        .child(label("Esc quits.").fg(Color::Rgb(149, 161, 176)))
}

fn meter_column(state: &State, channel: usize) -> Flex<Msg> {
    let level = channel_level(state.elapsed_secs, channel);
    let peak = state.peaks[channel];
    let accent = channel_accent(channel);
    let mut meter = col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(0)
        .style(Style::default().bg(Color::Rgb(20, 24, 36)))
        .child(
            label(CHANNEL_NAMES[channel])
                .bold()
                .fg(Color::Rgb(232, 237, 243)),
        )
        .child(spacer::<Msg>().height(1));

    for row in (1..=LEVEL_HEIGHT).rev() {
        meter = meter.child(level_row(row, level, peak, accent));
    }

    meter
        .child(spacer::<Msg>().height(1))
        .child(label(format!("{:02}", level)).fg(Color::Rgb(149, 161, 176)))
}

fn level_row(row: usize, level: usize, peak: usize, accent: Color) -> impl Widget<Msg> {
    if row <= level {
        label("      ").bg(fill_color(row, accent))
    } else if row == peak {
        label("======").fg(Color::Rgb(250, 243, 196))
    } else {
        label("      ").bg(Color::Rgb(34, 40, 56))
    }
}

fn channel_level(elapsed_secs: f32, channel: usize) -> usize {
    let channel = channel as f32;
    let base = ((elapsed_secs * 2.4 + channel * 0.75).sin() * 0.5 + 0.5) * 6.2;
    let pulse = ((elapsed_secs * 1.2 + channel * 1.31).cos() * 0.5 + 0.5) * 3.2;
    let level = 1.0 + base + pulse;
    level.round().clamp(1.0, LEVEL_HEIGHT as f32) as usize
}

fn channel_accent(channel: usize) -> Color {
    match channel {
        0 => Color::Rgb(255, 106, 136),
        1 => Color::Rgb(255, 159, 64),
        2 => Color::Rgb(255, 213, 79),
        3 => Color::Rgb(112, 225, 172),
        4 => Color::Rgb(76, 201, 240),
        5 => Color::Rgb(90, 153, 255),
        6 => Color::Rgb(170, 118, 255),
        _ => Color::Rgb(255, 135, 213),
    }
}

fn fill_color(row: usize, accent: Color) -> Color {
    let scale = 0.45 + row as f32 / LEVEL_HEIGHT as f32 * 0.7;
    match accent {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32 * scale).clamp(0.0, 255.0)) as u8,
            ((g as f32 * scale).clamp(0.0, 255.0)) as u8,
            ((b as f32 * scale).clamp(0.0, 255.0)) as u8,
        ),
        color => color,
    }
}

fn info_card(title: &str, value: String, accent: Color) -> impl Widget<Msg> {
    col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(0)
        .style(Style::default().bg(Color::Rgb(20, 24, 36)))
        .child(label(title).fg(Color::Rgb(149, 161, 176)))
        .child(label(value).bold().fg(accent))
}
