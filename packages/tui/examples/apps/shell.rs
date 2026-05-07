//! App shell primitives — commands, modals, notifications, routes, and tasks.
//!
//! Run with: `cargo run -p tui --example shell`
//! Keys: `j` opens the jobs screen, `b` goes back, `g` starts a background job,
//! `n` pushes a notification, `h` opens help, `Esc` closes the modal, `Ctrl+C` quits.

use std::time::Duration;

use tui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Dashboard,
    Jobs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Modal {
    Help,
}

#[derive(Debug)]
struct State {
    nav: Navigator<Screen>,
    modals: ModalManager<Modal>,
    notifications: NotificationCenter<String>,
    last_task: Option<TaskStatus>,
    completed_jobs: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            nav: Navigator::new(Screen::Dashboard),
            modals: ModalManager::new(),
            notifications: NotificationCenter::new(),
            last_task: None,
            completed_jobs: 0,
        }
    }
}

#[derive(Debug, Clone)]
enum Msg {
    OpenJobs,
    ResetDashboard,
    GoBack,
    StartJob,
    JobFinished,
    OpenHelp,
    CloseModal,
    PushNotice,
    TaskStatus(TaskStatus),
    CleanupNotifications,
}

fn main() -> WidgetResult<()> {
    let mut commands = CommandRouter::new();
    commands
        .register("route.jobs", "Open jobs", || Msg::OpenJobs)
        .description("Push the Jobs screen onto the route stack")
        .hotkey(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    commands
        .register("route.back", "Go back", || Msg::GoBack)
        .description("Pop the current screen")
        .hotkey(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    commands
        .register("route.dashboard", "Reset dashboard", || Msg::ResetDashboard)
        .description("Jump back to the root dashboard screen")
        .hotkey(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::empty()));
    commands
        .register("job.start", "Start job", || Msg::StartJob)
        .description("Spawn a background task with lifecycle updates")
        .hotkey(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()));
    commands
        .register("modal.close", "Close modal", || Msg::CloseModal)
        .description("Close the active modal")
        .hotkey(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));

    App::new(State::default())
        .with_command_router(commands)
        .on_hotkey(
            Hotkey::simple("notice.push", KeyCode::Char('n'), "n")
                .description("Push an app-level notification"),
            || Msg::PushNotice,
        )
        .on_hotkey(
            Hotkey::simple("modal.help", KeyCode::Char('h'), "h")
                .description("Open the help modal"),
            || Msg::OpenHelp,
        )
        .on_tick(Duration::from_millis(250), || Msg::CleanupNotifications)
        .with_quit_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
        .run_inline_with_effects(update, view)
}

fn update(state: &mut State, msg: Msg, ctx: &mut UpdateCtx<Msg>) {
    match msg {
        Msg::OpenJobs => state.nav.push(Screen::Jobs),
        Msg::ResetDashboard => state.nav.reset(Screen::Dashboard),
        Msg::GoBack => {
            state.nav.go_back();
        }
        Msg::StartJob => {
            state.notifications.push_timed(
                NotificationLevel::Info,
                "Started background sync".to_owned(),
                Duration::from_secs(3),
            );

            ctx.spawn(
                Task::new(|task| {
                    if !task.sleep(Duration::from_millis(650)) {
                        return TaskOutcome::Cancelled;
                    }

                    task.emit(Msg::JobFinished);
                    TaskOutcome::Complete
                })
                .key("demo-sync")
                .label("Demo project sync")
                .timeout(Duration::from_secs(2))
                .retry(RetryPolicy::fixed(1, Duration::from_millis(200)))
                .on_status(Msg::TaskStatus),
            );
        }
        Msg::JobFinished => {
            state.completed_jobs += 1;
            state.notifications.push_timed(
                NotificationLevel::Success,
                format!("Background sync finished ({})", state.completed_jobs),
                Duration::from_secs(4),
            );
        }
        Msg::OpenHelp => state.modals.open(Modal::Help),
        Msg::CloseModal => {
            state.modals.close_top();
        }
        Msg::PushNotice => {
            state.notifications.push_timed(
                NotificationLevel::Warning,
                "Modal, router, task, and notification helpers are all app-managed.".to_owned(),
                Duration::from_secs(4),
            );
        }
        Msg::TaskStatus(status) => {
            if matches!(
                status.state,
                TaskState::TimedOut | TaskState::Failed | TaskState::Panicked
            ) {
                state.notifications.push_timed(
                    NotificationLevel::Error,
                    format!("Task {:?} ended as {:?}", status.key, status.state),
                    Duration::from_secs(5),
                );
            }
            state.last_task = Some(status);
        }
        Msg::CleanupNotifications => {
            state.notifications.prune_expired(ctx.now());
        }
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let base = col::<Msg>()
        .gap(1)
        .padding(Padding::uniform(1))
        .child(header(state))
        .child(screen_panel(state))
        .child(task_panel(state))
        .child(notification_panel(state))
        .border(BorderStyle::Single);

    let mut root = overlay(base).key("app-shell");

    if state.modals.is_open() {
        root = root
            .layer(
                OverlayLayer::new(help_modal())
                    .floating(OverlayAnchor::Center)
                    .size(56, 10)
                    .z_index(20),
            )
            .trap_focus();
    }

    root
}

fn header(state: &State) -> impl Widget<Msg> {
    let route = state
        .nav
        .stack()
        .iter()
        .map(|screen| match screen {
            Screen::Dashboard => "Dashboard",
            Screen::Jobs => "Jobs",
        })
        .collect::<Vec<_>>()
        .join(" / ");

    row::<Msg>()
        .gap(2)
        .child(label("Shell Helpers").bold())
        .child(label(format!("Route: {route}")).fg(Color::Rgb(166, 178, 189)))
        .child(label(
            "j jobs | b back | d dashboard | g task | n notice | h help",
        ))
}

fn screen_panel(state: &State) -> impl Widget<Msg> {
    let body = match state.nav.current() {
        Screen::Dashboard => {
            "Dashboard uses Navigator for screen history. Push Jobs with `j`, then come back with `b`."
        }
        Screen::Jobs => {
            "Jobs is just another screen on the route stack. Start a background task with `g`."
        }
    };

    col::<Msg>()
        .gap(1)
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .child(label("Current Screen").bold())
        .child(label(body))
}

fn task_panel(state: &State) -> impl Widget<Msg> {
    let status = match state.last_task.as_ref() {
        Some(status) => format!(
            "Task {:?} -> {:?} (attempt {}/{})",
            status.key, status.state, status.attempt, status.max_attempts
        ),
        None => "No background task started yet.".to_owned(),
    };

    col::<Msg>()
        .gap(1)
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .child(label("Task Runtime").bold())
        .child(label(status))
}

fn notification_panel(state: &State) -> impl Widget<Msg> {
    let notifications = if state.notifications.items().is_empty() {
        vec![label::<Msg>("No notifications yet.")]
    } else {
        state
            .notifications
            .items()
            .iter()
            .rev()
            .take(3)
            .map(|item| {
                let prefix = match item.level {
                    NotificationLevel::Info => "[info]",
                    NotificationLevel::Success => "[ok]",
                    NotificationLevel::Warning => "[warn]",
                    NotificationLevel::Error => "[err]",
                };
                label(format!("{prefix} {}", item.payload))
            })
            .collect()
    };

    col::<Msg>()
        .gap(1)
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .child(label("Notifications").bold())
        .children(notifications)
}

fn help_modal() -> impl Widget<Msg> {
    col::<Msg>()
        .gap(1)
        .border(BorderStyle::Double)
        .padding(Padding::uniform(1))
        .style(Style::default().bg(Color::Rgb(25, 29, 36)))
        .child(label("Help Modal").bold())
        .child(divider())
        .child(label(
            "This modal is owned by ModalManager and rendered through overlay().",
        ))
        .child(label(
            "Esc closes the modal, while Ctrl+C quits the whole app.",
        ))
        .child(label(
            "The command router handles route and task shortcuts; notifications are app-scoped.",
        ))
}
