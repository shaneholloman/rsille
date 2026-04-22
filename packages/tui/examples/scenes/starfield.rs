//! Starfield animation demo using an ASCII grid.
//!
//! Run with: `cargo run -p tui --example starfield`
//! Press Esc to quit.

use tui::prelude::*;

const FIELD_WIDTH: usize = 52;
const FIELD_HEIGHT: usize = 14;
const STAR_COUNT: usize = 42;

#[derive(Debug, Clone, Copy)]
enum Msg {
    Frame(FrameInfo),
}

#[derive(Debug, Clone, Copy)]
struct Star {
    x: f32,
    y: f32,
    speed: f32,
    glyph: char,
}

#[derive(Debug)]
struct State {
    frame: u64,
    elapsed_secs: f32,
    respawn_cursor: usize,
    stars: Vec<Star>,
    last_frame_ms: Option<f64>,
    avg_frame_ms: Option<f64>,
}

impl State {
    fn new() -> Self {
        let mut stars = Vec::with_capacity(STAR_COUNT);
        for index in 0..STAR_COUNT {
            stars.push(seed_star(index));
        }

        Self {
            frame: 0,
            elapsed_secs: 0.0,
            respawn_cursor: STAR_COUNT,
            stars,
            last_frame_ms: None,
            avg_frame_ms: None,
        }
    }

    fn frame(&mut self, info: FrameInfo) {
        self.frame = info.frame;
        self.elapsed_secs = info.since_start.as_secs_f32();
        self.record_frame(info.delta);
        let delta_secs = info.delta.as_secs_f32();
        let warp = self.warp_factor();

        for star in &mut self.stars {
            star.x -= star.speed * delta_secs * 10.0 * warp;
            if star.x < 0.0 {
                *star = respawn_star(self.respawn_cursor);
                self.respawn_cursor = self.respawn_cursor.wrapping_add(1);
            }
        }
    }

    fn speed_label(&self) -> String {
        let value = self.warp_factor();
        format!("{value:.2}x")
    }

    fn warp_factor(&self) -> f32 {
        1.0 + (self.elapsed_secs * 1.5).sin() * 0.18
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
        .style(Style::default().bg(Color::Rgb(4, 7, 14)))
        .padding(Padding::uniform(2))
        .gap(1)
        .align_items(AlignItems::Center)
        .child(label("Starfield").bold().fg(Color::Rgb(241, 245, 249)))
        .child(
            label("A scrolling ASCII space scene driven by per-frame object updates.")
                .fg(Color::Rgb(141, 155, 171)),
        )
        .child(divider().text("Animation"))
        .child(
            col::<Msg>()
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(0)
                .style(Style::default().bg(Color::Rgb(8, 12, 22)))
                .children(render_field(state)),
        )
        .child(
            row::<Msg>()
                .gap(2)
                .justify_content(JustifyContent::Center)
                .child(info_panel(
                    "Stars",
                    STAR_COUNT.to_string(),
                    Color::Rgb(104, 211, 255),
                ))
                .child(info_panel(
                    "Warp",
                    state.speed_label(),
                    Color::Rgb(150, 240, 184),
                ))
                .child(info_panel(
                    "Frame",
                    format!("#{}", state.frame),
                    Color::Rgb(255, 205, 98),
                ))
                .child(info_panel(
                    "Delta",
                    state.delta_text(),
                    Color::Rgb(182, 156, 255),
                )),
        )
        .child(label("Esc quits.").fg(Color::Rgb(141, 155, 171)))
}

fn render_field(state: &State) -> Vec<Label<Msg>> {
    let mut grid = vec![vec![' '; FIELD_WIDTH]; FIELD_HEIGHT];

    for star in &state.stars {
        let x = star.x.round() as isize;
        let y = star.y.round() as isize;
        if x >= 0 && x < FIELD_WIDTH as isize && y >= 0 && y < FIELD_HEIGHT as isize {
            grid[y as usize][x as usize] = star.glyph;
        }
    }

    grid.into_iter()
        .enumerate()
        .map(|(row_index, row)| {
            let line: String = row.into_iter().collect();
            let color = if row_index % 3 == 0 {
                Color::Rgb(120, 164, 255)
            } else if row_index % 3 == 1 {
                Color::Rgb(167, 192, 255)
            } else {
                Color::Rgb(212, 224, 255)
            };
            label(line).fg(color)
        })
        .collect()
}

fn seed_star(index: usize) -> Star {
    let x = (FIELD_WIDTH as f32 - 1.0) - (index * 7 % FIELD_WIDTH) as f32;
    let y = (index * 5 % FIELD_HEIGHT) as f32;
    let speed = 0.45 + (index % 5) as f32 * 0.22;
    let glyph = glyph_for(index);
    Star { x, y, speed, glyph }
}

fn respawn_star(seed: usize) -> Star {
    let y = ((seed * 11 + 3) % FIELD_HEIGHT) as f32;
    let speed = 0.5 + (seed % 6) as f32 * 0.18;
    let glyph = glyph_for(seed + 17);
    Star {
        x: FIELD_WIDTH as f32 - 1.0,
        y,
        speed,
        glyph,
    }
}

fn glyph_for(seed: usize) -> char {
    match seed % 4 {
        0 => '.',
        1 => '+',
        2 => '*',
        _ => 'o',
    }
}

fn info_panel(title: &str, value: String, accent: Color) -> impl Widget<Msg> {
    col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(0)
        .style(Style::default().bg(Color::Rgb(8, 12, 22)))
        .child(label(title).fg(Color::Rgb(141, 155, 171)))
        .child(label(value).bold().fg(accent))
}
