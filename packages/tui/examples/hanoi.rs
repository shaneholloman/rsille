//! Towers of Hanoi — automatic animation using the TUI layout primitives.
//!
//! Run with: `cargo run -p tui --example hanoi`
//! Press Esc to quit.

use std::time::{Duration, Instant};

use tui::prelude::*;

const DISK_COUNT: u8 = 5;
const TICK_INTERVAL: Duration = Duration::from_millis(16);
const STEP_INTERVAL: Duration = Duration::from_millis(280);
const RESET_DELAY: Duration = Duration::from_millis(1400);
const TOWER_WIDTH: usize = DISK_COUNT as usize * 2 + 3;

#[derive(Debug, Clone, Copy)]
struct MoveStep {
    from: usize,
    to: usize,
}

#[derive(Debug, Clone, Copy)]
struct LastMove {
    from: usize,
    to: usize,
    disk: u8,
}

#[derive(Debug, Clone, Copy)]
enum Msg {
    Tick,
}

#[derive(Debug)]
struct State {
    towers: [Vec<u8>; 3],
    script: Vec<MoveStep>,
    next_move: usize,
    next_step_at: Instant,
    finished_at: Option<Instant>,
    cycle: usize,
    last_move: Option<LastMove>,
}

impl State {
    fn new() -> Self {
        Self {
            towers: initial_towers(),
            script: solution_script(),
            next_move: 0,
            next_step_at: Instant::now() + STEP_INTERVAL,
            finished_at: None,
            cycle: 1,
            last_move: None,
        }
    }

    fn tick(&mut self, now: Instant) {
        if let Some(finished_at) = self.finished_at {
            if now.duration_since(finished_at) >= RESET_DELAY {
                self.reset(now);
            }
            return;
        }

        if now < self.next_step_at || self.next_move >= self.script.len() {
            return;
        }

        let step = self.script[self.next_move];
        let disk = self.towers[step.from]
            .pop()
            .expect("solution script only contains legal moves");
        self.towers[step.to].push(disk);
        self.last_move = Some(LastMove {
            from: step.from,
            to: step.to,
            disk,
        });
        self.next_move += 1;
        self.next_step_at = now + STEP_INTERVAL;

        if self.next_move == self.script.len() {
            self.finished_at = Some(now);
        }
    }

    fn reset(&mut self, now: Instant) {
        self.towers = initial_towers();
        self.next_move = 0;
        self.next_step_at = now + STEP_INTERVAL;
        self.finished_at = None;
        self.last_move = None;
        self.cycle += 1;
    }

    fn status_text(&self) -> String {
        if let Some(last_move) = self.last_move {
            if self.finished_at.is_some() {
                format!(
                    "Solved. Disk {} completed {} -> {}. Restarting soon.",
                    last_move.disk,
                    tower_name(last_move.from),
                    tower_name(last_move.to)
                )
            } else {
                format!(
                    "Disk {} moved {} -> {}.",
                    last_move.disk,
                    tower_name(last_move.from),
                    tower_name(last_move.to)
                )
            }
        } else {
            "Solving automatically from Tower A to Tower C.".to_owned()
        }
    }

    fn next_move_text(&self) -> String {
        self.script
            .get(self.next_move)
            .map(|step| format!("Next: {} -> {}", tower_name(step.from), tower_name(step.to)))
            .unwrap_or_else(|| "Next: restart animation".to_owned())
    }
}

fn main() -> WidgetResult<()> {
    App::new(State::new())
        .on_tick(TICK_INTERVAL, || Msg::Tick)
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Tick => state.tick(Instant::now()),
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    col::<Msg>()
        .style(Style::default().bg(Color::Rgb(12, 18, 28)))
        .padding(Padding::uniform(2))
        .gap(1)
        .align_items(AlignItems::Center)
        .child(
            label("Towers of Hanoi")
                .bold()
                .fg(Color::Rgb(236, 242, 247)),
        )
        .child(
            label("Automatic animation built from row/col/layout + colored label blocks.")
                .fg(Color::Rgb(155, 169, 180)),
        )
        .child(divider().text("Animation"))
        .child(
            row::<Msg>()
                .gap(3)
                .justify_content(JustifyContent::Center)
                .child(tower_card("Tower A", &state.towers[0]))
                .child(tower_card("Tower B", &state.towers[1]))
                .child(tower_card("Tower C", &state.towers[2])),
        )
        .child(summary_card(state))
}

fn tower_card(name: &str, tower: &[u8]) -> impl Widget<Msg> {
    let mut card = col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(0)
        .style(Style::default().bg(Color::Rgb(20, 28, 41)))
        .child(label(name).bold().fg(Color::Rgb(230, 236, 241)))
        .child(spacer::<Msg>().height(1));

    for line in tower_lines(tower) {
        card = card.child(line);
    }

    card.child(spacer::<Msg>().height(1))
        .child(label(format!("{:^width$}", " ", width = TOWER_WIDTH)).bg(Color::Rgb(130, 146, 166)))
}

fn tower_lines(tower: &[u8]) -> Vec<Flex<Msg>> {
    let mut lines = Vec::with_capacity(DISK_COUNT as usize);

    for level in (0..DISK_COUNT as usize).rev() {
        let disk = tower.get(level).copied();
        lines.push(block_line(disk));
    }

    lines
}

fn block_line(disk: Option<u8>) -> Flex<Msg> {
    let width = disk.map(disk_width).unwrap_or(1);
    let color = disk.map(disk_color).unwrap_or(Color::Rgb(110, 126, 144));
    let pad = ((TOWER_WIDTH - width) / 2) as u16;

    row::<Msg>()
        .gap(0)
        .child(spacer::<Msg>().width(pad))
        .child(label(" ".repeat(width)).bg(color))
        .child(spacer::<Msg>().width(pad))
}

fn summary_card(state: &State) -> impl Widget<Msg> {
    col::<Msg>()
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(Color::Rgb(20, 28, 41)))
        .child(label("Status").bold().fg(Color::Rgb(230, 236, 241)))
        .child(
            label(format!(
                "Move: {} / {}",
                state.next_move,
                state.script.len()
            ))
            .fg(Color::Rgb(120, 220, 190)),
        )
        .child(label(format!("Cycle: {}", state.cycle)).fg(Color::Rgb(155, 169, 180)))
        .child(label(state.status_text()).fg(Color::Rgb(241, 199, 94)))
        .child(label(state.next_move_text()).fg(Color::Rgb(111, 194, 255)))
        .child(label("Esc quits.").fg(Color::Rgb(155, 169, 180)))
}

fn initial_towers() -> [Vec<u8>; 3] {
    [(1..=DISK_COUNT).rev().collect(), Vec::new(), Vec::new()]
}

fn solution_script() -> Vec<MoveStep> {
    let mut moves = Vec::new();
    build_solution(DISK_COUNT, 0, 2, 1, &mut moves);
    moves
}

fn build_solution(disks: u8, from: usize, to: usize, aux: usize, moves: &mut Vec<MoveStep>) {
    if disks == 0 {
        return;
    }

    build_solution(disks - 1, from, aux, to, moves);
    moves.push(MoveStep { from, to });
    build_solution(disks - 1, aux, to, from, moves);
}

fn disk_width(disk: u8) -> usize {
    usize::from(disk) * 2 + 1
}

fn disk_color(disk: u8) -> Color {
    match disk {
        1 => Color::Rgb(255, 99, 132),
        2 => Color::Rgb(255, 159, 64),
        3 => Color::Rgb(255, 205, 86),
        4 => Color::Rgb(75, 192, 192),
        5 => Color::Rgb(54, 162, 235),
        _ => Color::Rgb(153, 102, 255),
    }
}

fn tower_name(index: usize) -> &'static str {
    match index {
        0 => "A",
        1 => "B",
        2 => "C",
        _ => "?",
    }
}
