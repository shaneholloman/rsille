//! Layout and shared area animations.
//!
//! Run with: `cargo run -p tui --example layout_animation`
//! Use Tab to focus buttons. Enter/Space activates the focused button.

use std::time::Duration;

use tui::prelude::*;

#[derive(Debug, Clone)]
enum Msg {
    Tick,
    Shuffle,
    SelectNext,
    ToggleDetail,
}

#[derive(Debug)]
struct State {
    order: Vec<usize>,
    selected: usize,
    detail_open: bool,
    step: usize,
}

#[derive(Debug, Clone, Copy)]
struct Job {
    id: &'static str,
    title: &'static str,
    status: &'static str,
    color: Color,
}

const JOBS: [Job; 5] = [
    Job {
        id: "ingest",
        title: "Ingest events",
        status: "queued",
        color: Color::Blue,
    },
    Job {
        id: "index",
        title: "Index search",
        status: "running",
        color: Color::Cyan,
    },
    Job {
        id: "audit",
        title: "Audit trail",
        status: "waiting",
        color: Color::Yellow,
    },
    Job {
        id: "deploy",
        title: "Deploy edge",
        status: "ready",
        color: Color::Green,
    },
    Job {
        id: "notify",
        title: "Notify teams",
        status: "blocked",
        color: Color::Magenta,
    },
];

fn main() -> WidgetResult<()> {
    App::new(State {
        order: (0..JOBS.len()).collect(),
        selected: 1,
        detail_open: false,
        step: 0,
    })
    .on_tick(Duration::from_millis(1200), || Msg::Tick)
    .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Tick => {
            state.step = state.step.wrapping_add(1);
            rotate_order(state);
            state.selected = (state.selected + 1) % JOBS.len();
            state.detail_open = state.step % 2 == 0;
        }
        Msg::Shuffle => rotate_order(state),
        Msg::SelectNext => state.selected = (state.selected + 1) % JOBS.len(),
        Msg::ToggleDetail => state.detail_open = !state.detail_open,
    }
}

fn rotate_order(state: &mut State) {
    if !state.order.is_empty() {
        state.order.rotate_left(1);
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Layout Motion")
        .padding(Padding::uniform(1))
        .gap(1)
        .child(
            row::<Msg>()
                .gap(2)
                .child(button("Shuffle").on_click(|| Msg::Shuffle).animated())
                .child(button("Next").on_click(|| Msg::SelectNext).animated())
                .child(button("Detail").on_click(|| Msg::ToggleDetail).animated()),
        )
        .child(divider())
        .child(
            row::<Msg>()
                .gap(3)
                .child(job_list(state))
                .child(detail_stage(state)),
        )
}

fn job_list(state: &State) -> impl Widget<Msg> {
    let mut list = panel::<Msg>()
        .title("Build Queue")
        .padding(Padding::uniform(1))
        .gap(1);

    for index in state.order.iter().copied() {
        let selected = index == state.selected;
        list = list.child(job_row(JOBS[index], selected, state.detail_open));
    }

    list
}

fn job_row(job: Job, selected: bool, detail_open: bool) -> Animated<Msg> {
    let marker = if selected { ">" } else { " " };
    let style = if selected {
        Style::default().fg(job.color).bold()
    } else {
        Style::default()
    };
    let spec = AnimationSpec::new(Duration::from_millis(360), Easing::EaseInOut);

    let row_widget = row::<Msg>()
        .gap(1)
        .child(label(marker).style(style))
        .child(label(format!("{:<16}", job.title)).style(style))
        .child(label(job.status).fg(job.color));

    let row = animate(row_widget)
        .key(job.id)
        .layout_transition(LayoutTransition::position(spec));

    if selected && !detail_open {
        row.shared("active-job").layout(spec)
    } else {
        row
    }
}

fn detail_stage(state: &State) -> Box<dyn Widget<Msg>> {
    let spec = AnimationSpec::new(Duration::from_millis(360), Easing::EaseInOut);
    let job = JOBS[state.selected];

    if state.detail_open {
        return animate(
            panel::<Msg>()
                .title("Active Job")
                .padding(Padding::uniform(1))
                .gap(1)
                .child(label(job.title).fg(job.color).bold())
                .child(label(format!("State: {}", job.status)))
                .child(progress_bar::<Msg>((state.step % 10) as f64 / 9.0).animated()),
        )
        .key("active-detail")
        .shared_transition("active-job", LayoutTransition::size_and_position(spec))
        .enter(Transition::scale_from_center())
        .exit(Transition::collapse())
        .into_widget();
    }

    panel::<Msg>()
        .title("Active Job")
        .padding(Padding::uniform(1))
        .child(label("Select a row, then open the detail card."))
        .into_widget()
}
