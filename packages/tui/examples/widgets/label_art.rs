//! Label widget rendering interactive preformatted character art.
//!
//! Run with: `cargo run -p tui --example label_art`

use tui::prelude::*;

const ORBIT_FRAMES: [&str; 4] = [
    r#"
              .-.
          .-'     '-.
        .'    01     '.
       /    .----.     \
      ;    /  rs  \     ;
      |    \      /     |
      ;     '----'     ;
       \       *       /
        '.           .'
          '-._____.-'
"#,
    r#"
              .-.
          .-'  *  '-.
        .'           '.
       /    .----.     \
      ;    /  rs  \     ;
      |    \      /  01 |
      ;     '----'     ;
       \               /
        '.           .'
          '-._____.-'
"#,
    r#"
              .-.
          .-'     '-.
        .'           '.
       /    .----.     \
      ;    /  rs  \     ;
      |  * \      /     |
      ;     '----'     ;
       \      01       /
        '.           .'
          '-._____.-'
"#,
    r#"
              .-.
          .-'     '-.
        .'           '.
       /    .----.  *  \
      ;    /  rs  \     ;
      |    \      /     |
      ; 01  '----'     ;
       \               /
        '.           .'
          '-._____.-'
"#,
];

const SIGNAL_FRAMES: [&str; 4] = [
    r#"
╭────────────────────────╮
│  ░█▀▀░▀█▀░█▀▀░█▀█      │
│  ░▀▀█░░█░░█░█░█░█      │
│  ░▀▀▀░▀▀▀░▀▀▀░▀░▀      │
│                        │
│  ███░░░░░░░░░░░░       │
╰────────────────────────╯
"#,
    r#"
╭────────────────────────╮
│  ░█▀▀░▀█▀░█▀▀░█▀█      │
│  ░▀▀█░░█░░█░█░█░█      │
│  ░▀▀▀░▀▀▀░▀▀▀░▀░▀      │
│                        │
│  ███████░░░░░░░░       │
╰────────────────────────╯
"#,
    r#"
╭────────────────────────╮
│  ░█▀▀░▀█▀░█▀▀░█▀█      │
│  ░▀▀█░░█░░█░█░█░█      │
│  ░▀▀▀░▀▀▀░▀▀▀░▀░▀      │
│                        │
│  ███████████░░░░       │
╰────────────────────────╯
"#,
    r#"
╭────────────────────────╮
│  ░█▀▀░▀█▀░█▀▀░█▀█      │
│  ░▀▀█░░█░░█░█░█░█      │
│  ░▀▀▀░▀▀▀░▀▀▀░▀░▀      │
│                        │
│  ███████████████       │
╰────────────────────────╯
"#,
];

const TERMINAL_FRAMES: [&str; 4] = [
    r#"
┌────────────────────────────┐
│ $ cargo run --example art  │
│                            │
│ layout: fixed              │
│ align : center             │
│ cells : unicode-aware      │
│                            │
│ ████████████░░░░░░░░       │
└────────────────────────────┘
"#,
    r#"
┌────────────────────────────┐
│ $ cargo run --example art  │
│                            │
│ layout: fixed              │
│ align : center             │
│ cells : unicode-aware      │
│                            │
│ ███████████████░░░░░       │
└────────────────────────────┘
"#,
    r#"
┌────────────────────────────┐
│ $ cargo run --example art  │
│                            │
│ layout: fixed              │
│ align : center             │
│ cells : unicode-aware      │
│                            │
│ ██████████████████░░       │
└────────────────────────────┘
"#,
    r#"
┌────────────────────────────┐
│ $ cargo run --example art  │
│                            │
│ layout: fixed              │
│ align : center             │
│ cells : unicode-aware      │
│                            │
│ ████████████████████       │
└────────────────────────────┘
"#,
];

const GALLERY: [ArtPiece; 3] = [
    ArtPiece {
        title: "Orbit",
        frames: &ORBIT_FRAMES,
        accent: Color::Rgb(111, 194, 255),
    },
    ArtPiece {
        title: "Signal",
        frames: &SIGNAL_FRAMES,
        accent: Color::Rgb(241, 199, 94),
    },
    ArtPiece {
        title: "Terminal",
        frames: &TERMINAL_FRAMES,
        accent: Color::Rgb(150, 240, 184),
    },
];

#[derive(Debug, Clone, Copy)]
enum Msg {
    Frame(FrameInfo),
    PreviousArt,
    NextArt,
    TogglePause,
    CycleTheme,
    CycleHorizontalAlign,
    CycleVerticalAlign,
}

#[derive(Debug)]
struct State {
    frame: u64,
    selected: usize,
    paused: bool,
    theme: usize,
    horizontal_align: usize,
    vertical_align: usize,
}

#[derive(Debug, Clone, Copy)]
struct ArtPiece {
    title: &'static str,
    frames: &'static [&'static str],
    accent: Color,
}

#[derive(Debug, Clone, Copy)]
struct Palette {
    background: Color,
    surface: Color,
    surface_alt: Color,
    text: Color,
    muted: Color,
}

impl Default for State {
    fn default() -> Self {
        Self {
            frame: 0,
            selected: 0,
            paused: false,
            theme: 0,
            horizontal_align: 1,
            vertical_align: 1,
        }
    }
}

impl State {
    fn current_art(&self) -> ArtPiece {
        GALLERY[self.selected]
    }

    fn current_frame(&self) -> &'static str {
        let art = self.current_art();
        let index = ((self.frame / 8) as usize) % art.frames.len();
        art.frames[index].trim_matches('\n')
    }

    fn horizontal_align(&self) -> HorizontalAlign {
        match self.horizontal_align % 3 {
            0 => HorizontalAlign::Left,
            1 => HorizontalAlign::Center,
            _ => HorizontalAlign::Right,
        }
    }

    fn vertical_align(&self) -> VerticalAlign {
        match self.vertical_align % 3 {
            0 => VerticalAlign::Top,
            1 => VerticalAlign::Middle,
            _ => VerticalAlign::Bottom,
        }
    }

    fn status(&self) -> &'static str {
        if self.paused {
            "paused"
        } else {
            "playing"
        }
    }

    fn theme_name(&self) -> &'static str {
        match self.theme % 3 {
            0 => "blueprint",
            1 => "amber",
            _ => "mint",
        }
    }

    fn palette(&self) -> Palette {
        match self.theme % 3 {
            0 => Palette {
                background: Color::Rgb(5, 9, 18),
                surface: Color::Rgb(8, 12, 22),
                surface_alt: Color::Rgb(12, 19, 34),
                text: Color::Rgb(241, 245, 249),
                muted: Color::Rgb(149, 161, 176),
            },
            1 => Palette {
                background: Color::Rgb(22, 15, 6),
                surface: Color::Rgb(38, 24, 8),
                surface_alt: Color::Rgb(55, 34, 10),
                text: Color::Rgb(255, 242, 214),
                muted: Color::Rgb(207, 166, 106),
            },
            _ => Palette {
                background: Color::Rgb(4, 18, 15),
                surface: Color::Rgb(7, 34, 29),
                surface_alt: Color::Rgb(9, 48, 41),
                text: Color::Rgb(224, 255, 246),
                muted: Color::Rgb(130, 207, 187),
            },
        }
    }
}

fn main() -> WidgetResult<()> {
    App::new(State::default())
        .on_frame(Msg::Frame)
        .on_key(KeyCode::Left, || Msg::PreviousArt)
        .on_key(KeyCode::Right, || Msg::NextArt)
        .on_key(KeyCode::Char(' '), || Msg::TogglePause)
        .on_key(KeyCode::Char('t'), || Msg::CycleTheme)
        .on_key(KeyCode::Char('h'), || Msg::CycleHorizontalAlign)
        .on_key(KeyCode::Char('v'), || Msg::CycleVerticalAlign)
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Frame(info) => {
            if !state.paused {
                state.frame = info.frame;
            }
        }
        Msg::PreviousArt => {
            state.selected = state.selected.checked_sub(1).unwrap_or(GALLERY.len() - 1);
        }
        Msg::NextArt => {
            state.selected = (state.selected + 1) % GALLERY.len();
        }
        Msg::TogglePause => state.paused = !state.paused,
        Msg::CycleTheme => state.theme = (state.theme + 1) % 3,
        Msg::CycleHorizontalAlign => state.horizontal_align = (state.horizontal_align + 1) % 3,
        Msg::CycleVerticalAlign => state.vertical_align = (state.vertical_align + 1) % 3,
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let art = state.current_art();
    let palette = state.palette();
    let pulse = ((state.frame / 12) % 4) as usize;

    col::<Msg>()
        .style(Style::default().bg(palette.background))
        .padding(Padding::uniform(1))
        .gap(1)
        .child(header(state, palette))
        .child(
            row::<Msg>()
                .gap(2)
                .child(gallery_panel(state, art, palette, pulse))
                .child(control_panel(state, art, palette)),
        )
        .child(timeline(state, art.accent, palette, pulse))
}

fn header(state: &State, palette: Palette) -> impl Widget<Msg> {
    row::<Msg>()
        .justify_content(JustifyContent::SpaceBetween)
        .child(label("Label Character Art").bold().fg(palette.text))
        .child(
            label(format!(
                "{} | theme {} | frame {}",
                state.status(),
                state.theme_name(),
                state.frame
            ))
            .fg(palette.muted),
        )
}

fn gallery_panel(state: &State, art: ArtPiece, palette: Palette, pulse: usize) -> impl Widget<Msg> {
    let accent = pulse_color(art.accent, pulse);

    panel::<Msg>()
        .title(format!(" {} ", art.title))
        .padding(Padding::uniform(1))
        .style(Style::default().bg(palette.surface))
        .child(
            label(state.current_frame())
                .constraints(Constraints {
                    min_width: 56,
                    max_width: Some(56),
                    min_height: 10,
                    max_height: None,
                    flex: Some(1.0),
                })
                .align(state.horizontal_align())
                .valign(state.vertical_align())
                .fg(accent),
        )
}

fn control_panel(state: &State, art: ArtPiece, palette: Palette) -> impl Widget<Msg> {
    panel::<Msg>()
        .title(" Controls ")
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(palette.surface_alt))
        .child(
            label(format!("Selected: {}", art.title))
                .bold()
                .fg(art.accent),
        )
        .child(label(format!("Horizontal: {:?}", state.horizontal_align())).fg(palette.text))
        .child(label(format!("Vertical: {:?}", state.vertical_align())).fg(palette.text))
        .child(label(format!("Playback: {}", state.status())).fg(palette.text))
        .child(divider())
        .child(
            row::<Msg>()
                .gap(1)
                .child(button("Previous").on_click(|| Msg::PreviousArt))
                .child(button("Next").on_click(|| Msg::NextArt)),
        )
        .child(
            row::<Msg>()
                .gap(1)
                .child(
                    button(if state.paused { "Play" } else { "Pause" })
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::TogglePause),
                )
                .child(
                    button("Theme")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::CycleTheme),
                ),
        )
        .child(
            row::<Msg>()
                .gap(1)
                .child(
                    button("H Align")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::CycleHorizontalAlign),
                )
                .child(
                    button("V Align")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::CycleVerticalAlign),
                ),
        )
        .child(label("Keys: Left/Right, Space, T, H, V").fg(palette.muted))
}

fn timeline(state: &State, accent: Color, palette: Palette, pulse: usize) -> impl Widget<Msg> {
    let cells = progress_cells(((state.frame / 2) % 36) as usize, pulse);

    panel::<Msg>()
        .borderless()
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(palette.background))
        .child(
            label(cells)
                .width(72)
                .align(HorizontalAlign::Center)
                .fg(pulse_color(accent, pulse)),
        )
}

fn progress_cells(active: usize, pulse: usize) -> String {
    let mut cells = String::with_capacity(36);
    for index in 0..36 {
        let ch = if index < active {
            '█'
        } else if index == active {
            match pulse % 3 {
                0 => '▓',
                1 => '▒',
                _ => '░',
            }
        } else {
            '░'
        };
        cells.push(ch);
    }
    cells
}

fn pulse_color(base: Color, pulse: usize) -> Color {
    let Color::Rgb(r, g, b) = base else {
        return base;
    };
    let lift = (pulse as u8).saturating_mul(14);
    Color::Rgb(
        r.saturating_add(lift),
        g.saturating_add(lift / 2),
        b.saturating_add(lift / 3),
    )
}
