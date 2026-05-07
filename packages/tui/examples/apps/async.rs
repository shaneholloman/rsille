//! Async request patterns — debounce, latest-only search, and request state bridging.
//!
//! Run with: `cargo run -p tui --example async`

use std::time::Duration;

use tui::prelude::*;

const SEARCH_TASK_KEY: &str = "demo.search";
const SEARCH_DEBOUNCE_KEY: &str = "demo.search.debounce";
const PEOPLE: &[&str] = &[
    "Ada Lovelace",
    "Alan Turing",
    "Barbara Liskov",
    "Donald Knuth",
    "Edsger Dijkstra",
    "Frances Allen",
    "Grace Hopper",
    "Guido van Rossum",
    "Ken Thompson",
    "Linus Torvalds",
    "Margaret Hamilton",
    "Radia Perlman",
    "Rusty Russell",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SearchForm {
    query: String,
}

#[derive(Debug, Default)]
struct State {
    form: FormState<SearchForm>,
    search: RequestState<Vec<String>, String>,
    spinner_frame: usize,
}

#[derive(Debug, Clone)]
enum Msg {
    QueryChanged(String),
    RunSearch(String),
    RetryNow,
    CancelSearch,
    Search(RequestEvent<Vec<String>, String>),
    Tick(FrameInfo),
}

fn main() -> WidgetResult<()> {
    App::new(State::default())
        .on_frame(Msg::Tick)
        .run_inline_with_effects(update, view)
}

fn update(state: &mut State, msg: Msg, ctx: &mut UpdateCtx<Msg>) {
    match msg {
        Msg::QueryChanged(query) => {
            state.form.update(|form| form.query = query.clone());

            if query.trim().is_empty() {
                ctx.cancel_debounce(SEARCH_DEBOUNCE_KEY);
                ctx.cancel_task_key(SEARCH_TASK_KEY);
                state.search.clear();
                state.form.set_submitting(false);
                return;
            }

            ctx.debounce(
                SEARCH_DEBOUNCE_KEY,
                Duration::from_millis(250),
                Msg::RunSearch(query),
            );
        }
        Msg::RunSearch(query) => {
            let query = query.trim().to_owned();
            if query.is_empty() {
                return;
            }

            ctx.request(
                Request::new(move |request| {
                    if !request.sleep(Duration::from_millis(550)) {
                        return RequestOutcome::cancelled();
                    }

                    if query.eq_ignore_ascii_case("error") {
                        return RequestOutcome::failure(
                            "Simulated backend error. Try any other keyword.".to_owned(),
                        );
                    }

                    let needle = query.to_lowercase();
                    let matches = PEOPLE
                        .iter()
                        .filter(|name| name.to_lowercase().contains(&needle))
                        .map(|name| (*name).to_owned())
                        .collect::<Vec<_>>();

                    RequestOutcome::success(matches)
                })
                .key(SEARCH_TASK_KEY)
                .label("Directory search")
                .retry(RetryPolicy::fixed(1, Duration::from_millis(150)))
                .timeout(Duration::from_secs(2)),
                Msg::Search,
            );
        }
        Msg::RetryNow => {
            let query = state.form.value().query.trim().to_owned();
            if !query.is_empty() {
                ctx.cancel_debounce(SEARCH_DEBOUNCE_KEY);
                ctx.emit(Msg::RunSearch(query));
            }
        }
        Msg::CancelSearch => {
            ctx.cancel_debounce(SEARCH_DEBOUNCE_KEY);
            ctx.cancel_task_key(SEARCH_TASK_KEY);
        }
        Msg::Search(event) => {
            state.search.apply(event);
            state.form.sync_submitting_with_request(&state.search);
        }
        Msg::Tick(info) => {
            state.spinner_frame = info.frame as usize;
        }
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let query = state.form.value().query.clone();
    let busy = state.form.is_submitting();
    let can_retry = !query.trim().is_empty() && !busy;

    col::<Msg>()
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label("Async Search").bold())
        .child(label(
            "Typing will debounce for 250ms. Re-running the same request key cancels stale work.",
        ))
        .child(
            text_input::<Msg>()
                .key("query")
                .value(query.clone())
                .placeholder("Search people or type `error` to simulate failure")
                .disabled(busy)
                .on_change(Msg::QueryChanged)
                .on_submit(Msg::RunSearch),
        )
        .child(
            row::<Msg>()
                .gap(1)
                .child(
                    button("Retry now")
                        .variant(ButtonVariant::Secondary)
                        .disabled(!can_retry)
                        .on_click(|| Msg::RetryNow),
                )
                .child(
                    button("Cancel")
                        .variant(ButtonVariant::Ghost)
                        .disabled(!busy)
                        .on_click(|| Msg::CancelSearch),
                ),
        )
        .child(status_panel(state))
        .child(results_panel(state))
        .border(BorderStyle::Rounded)
}

fn status_panel(state: &State) -> impl Widget<Msg> {
    let status_line = if let Some(status) = state.search.status() {
        format!(
            "task={} state={:?} attempt={}/{}",
            status.id.get(),
            status.state,
            status.attempt,
            status.max_attempts
        )
    } else {
        "task=none".to_owned()
    };

    col::<Msg>()
        .gap(1)
        .border(BorderStyle::Single)
        .padding(Padding::uniform(1))
        .child(label("Request State").bold())
        .child(label(format!("submitting={}", state.form.is_submitting())))
        .child(label(status_line))
}

fn results_panel(state: &State) -> impl Widget<Msg> {
    let mut panel = col::<Msg>()
        .gap(1)
        .border(BorderStyle::Single)
        .padding(Padding::uniform(1))
        .child(label("Results").bold());

    match state.search.phase() {
        RequestPhase::Idle => {
            panel = panel.child(label("Start typing to trigger a debounced request."));
        }
        RequestPhase::Loading => {
            panel = panel.child(
                loading_indicator::<Msg>()
                    .frame(state.spinner_frame)
                    .label("Searching directory..."),
            );
        }
        RequestPhase::Success(results) if results.is_empty() => {
            panel = panel.child(label("No matching people."));
        }
        RequestPhase::Success(results) => {
            panel = panel.child(
                list::<Msg>()
                    .key("results")
                    .height(8)
                    .items(results.iter().cloned()),
            );
        }
        RequestPhase::Failed(error) => {
            panel = panel.child(label(format!("Error: {error}")).fg(Color::Rgb(255, 120, 120)));
        }
        RequestPhase::Cancelled => {
            panel = panel.child(label("Search cancelled."));
        }
        RequestPhase::TimedOut => {
            panel = panel.child(label("Search timed out."));
        }
        RequestPhase::Panicked => {
            panel = panel.child(label("Search panicked."));
        }
    }

    panel
}
