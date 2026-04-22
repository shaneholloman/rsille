use std::{
    io::{stdout, Stdout},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{position, Hide, MoveToNextLine, Show},
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEvent},
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use log::{error, info, warn};
use tokio::time::Instant as TokioInstant;

use crate::{Builder, DrawErr, DrawUpdate, Render};

struct TerminalGuard {
    hide_cursor: bool,
    mouse_capture: bool,
    raw_mode: bool,
    alt_screen: bool,
}

impl TerminalGuard {
    /// Create and initialize terminal guard with the given settings
    fn new(
        alt_screen: bool,
        raw_mode: bool,
        mouse_capture: bool,
        hide_cursor: bool,
    ) -> Result<Self, DrawErr> {
        if alt_screen {
            execute!(stdout(), EnterAlternateScreen).map_err(DrawErr::TerminalSetup)?;
            execute!(stdout(), Clear(ClearType::All)).map_err(DrawErr::TerminalSetup)?;
        }
        if raw_mode {
            crossterm::terminal::enable_raw_mode().map_err(DrawErr::TerminalSetup)?;
        }
        if mouse_capture {
            execute!(stdout(), EnableMouseCapture).map_err(DrawErr::TerminalSetup)?;
        }
        if hide_cursor {
            execute!(stdout(), Hide).map_err(DrawErr::TerminalSetup)?;
        }

        Ok(Self {
            hide_cursor,
            mouse_capture,
            raw_mode,
            alt_screen,
        })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.hide_cursor {
            let _ = execute!(stdout(), Show);
        }
        if self.mouse_capture {
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        if self.raw_mode {
            let _ = crossterm::terminal::disable_raw_mode();
        }
        if self.alt_screen {
            let _ = execute!(stdout(), LeaveAlternateScreen);
        }
    }
}

pub struct EventLoop<T> {
    render: Render<Stdout, T>,
    raw_mode: bool,
    exit_code: Option<KeyEvent>,
    max_event_per_frame: usize,
    frame_limit: Option<u16>,
    alt_screen: bool,
    mouse_capture: bool,
    hide_cursor: bool,
    inline_mode: bool,
}

impl<T> EventLoop<T>
where
    T: DrawUpdate + Send + Sync + 'static,
{
    pub(super) fn from_builder(builder: &Builder, thing: T) -> Self
    where
        T: DrawUpdate + Send + Sync + 'static,
    {
        Self {
            render: Render::from_builder(builder, thing, stdout()),
            raw_mode: builder.enable_raw_mode,
            exit_code: builder.exit_code,
            max_event_per_frame: builder.max_event_per_frame,
            frame_limit: builder.frame_limit,
            alt_screen: builder.enable_alt_screen,
            mouse_capture: builder.enable_mouse_capture,
            hide_cursor: builder.enable_hide_cursor,
            inline_mode: builder.inline_mode,
        }
    }

    pub fn max_event_per_frame(&mut self, max_event_per_frame: usize) -> &mut Self {
        self.max_event_per_frame = max_event_per_frame;
        self
    }

    pub fn frame_limit(&mut self, frame_limit: Option<u16>) -> &mut Self {
        self.frame_limit = frame_limit;
        self
    }

    pub fn exit_code(&mut self, exit_code: KeyEvent) -> &mut Self {
        self.exit_code = Some(exit_code);
        self
    }

    pub fn disable_exit_code(&mut self) -> &mut Self {
        self.exit_code = None;
        self
    }

    pub fn enable_alt_screen(&mut self) -> &mut Self {
        self.alt_screen = true;
        self
    }

    pub fn disable_alt_screen(&mut self) -> &mut Self {
        self.alt_screen = false;
        self
    }

    pub fn enable_mouse_capture(&mut self) -> &mut Self {
        self.mouse_capture = true;
        self
    }

    pub fn disable_mouse_capture(&mut self) -> &mut Self {
        self.mouse_capture = false;
        self
    }

    pub fn hide_cursor_when_render(&mut self) -> &mut Self {
        self.hide_cursor = true;
        self
    }

    pub fn show_cursor_when_render(&mut self) -> &mut Self {
        self.hide_cursor = false;
        self
    }

    /// Set initial used height for inline mode
    ///
    /// This should be called before run() to set the initial rendering height
    /// without reallocating the buffer. Only effective in inline mode.
    pub fn set_initial_used_height(&mut self, height: u16) -> &mut Self {
        self.render.set_used_height(height);
        self
    }

    pub fn run(mut self) -> Result<(), DrawErr> {
        let _guard = TerminalGuard::new(
            self.alt_screen,
            self.raw_mode,
            self.mouse_capture,
            self.hide_cursor,
        )?;

        info!(
            target: "render::event_loop",
            "event loop started: mode={}, frame_limit={:?}, raw_mode={}, alt_screen={}",
            if self.inline_mode { "inline" } else { "fullscreen" },
            self.frame_limit,
            self.raw_mode,
            self.alt_screen
        );

        if self.inline_mode {
            match position() {
                Ok((x, y)) => {
                    self.render.pos = (x, y).into();
                    if x != 0 {
                        execute!(stdout(), MoveToNextLine(1))?;
                        self.render.pos.down(1);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to get cursor position in inline mode: {}, using (0, 0)",
                        e
                    );
                    self.render.pos = (0, 0).into();
                }
            }
        }

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(DrawErr::RuntimeCreation)?;

        rt.block_on(self.run_async())?;

        info!(target: "render::event_loop", "event loop stopped");

        Ok(())
    }

    async fn run_async(mut self) -> Result<(), DrawErr> {
        let frame_interval = self
            .frame_limit
            .filter(|fps| *fps > 0)
            .map(|fps| Duration::from_secs_f64(1.0 / fps as f64));
        let mut pending_initial_frame = true;
        let mut next_frame_at = Instant::now();
        let mut reader = EventStream::new();

        loop {
            let mut stop_after_iteration = false;
            let mut events = Vec::new();

            match wait_for_frame_or_event(
                &mut reader,
                self.exit_code,
                frame_interval,
                next_frame_at,
                pending_initial_frame,
            )
            .await
            {
                WaitOutcome::Event(event) => events.push(event),
                WaitOutcome::Timeout => {}
                WaitOutcome::ExitRequested => {
                    info!(
                        target: "render::event_loop",
                        "Automatic exit key pressed"
                    );
                    break;
                }
                WaitOutcome::Disconnected => break,
            }

            match drain_pending_events(
                &mut reader,
                self.exit_code,
                &mut events,
                self.max_event_per_frame,
            ) {
                DrainOutcome::Continue => {}
                DrainOutcome::ExitRequested => {
                    info!(
                        target: "render::event_loop",
                        "Automatic exit key pressed"
                    );
                    stop_after_iteration = true;
                }
                DrainOutcome::Disconnected => {
                    stop_after_iteration = true;
                }
            }

            pending_initial_frame = false;
            let frame_started_at = Instant::now();

            for event in &events {
                if let Event::Resize(width, height) = event {
                    self.render.resize((*width, *height).into());
                }
            }

            if let Err(e) = self.render.on_events(&events) {
                error!("Error processing events: {}", e);
            }
            let needs_render = match self.render.update() {
                Ok(needs_render) => needs_render,
                Err(e) => {
                    error!("Error updating render state: {}", e);
                    false
                }
            };

            if self.render.thing().should_quit() {
                info!(
                    target: "render::event_loop",
                    "Application requested quit"
                );
                break;
            }

            let current_size = self.render.size();
            if let Some(new_size) = self.render.thing().required_size(current_size) {
                if self.inline_mode {
                    self.render.set_used_height(new_size.height);
                } else {
                    self.render.resize(new_size);
                }
            }

            if !events.is_empty() || needs_render || self.render.has_pending_changes() {
                if let Err(e) = self.render.render() {
                    error!("Error rendering: {}", e);
                }
            }

            if stop_after_iteration {
                break;
            }

            if let Some(interval) = frame_interval {
                if next_frame_at <= frame_started_at {
                    next_frame_at = frame_started_at + interval;
                }

                let finished_at = Instant::now();
                while next_frame_at <= finished_at {
                    next_frame_at += interval;
                }
            }
        }

        Ok(())
    }
}

enum WaitOutcome {
    Event(Event),
    Timeout,
    Disconnected,
    ExitRequested,
}

enum DrainOutcome {
    Continue,
    Disconnected,
    ExitRequested,
}

async fn wait_for_frame_or_event(
    reader: &mut EventStream,
    exit_code: Option<KeyEvent>,
    frame_interval: Option<Duration>,
    next_frame_at: Instant,
    pending_initial_frame: bool,
) -> WaitOutcome {
    if pending_initial_frame {
        return WaitOutcome::Timeout;
    }

    loop {
        if frame_interval.is_some() {
            let event = reader.next();
            tokio::pin!(event);

            let sleep = tokio::time::sleep_until(TokioInstant::from_std(next_frame_at));
            tokio::pin!(sleep);

            match tokio::select! {
                maybe_event = &mut event => Some(maybe_event),
                _ = &mut sleep => None,
            } {
                Some(Some(Ok(event))) => {
                    if is_exit_event(&event, exit_code) {
                        return WaitOutcome::ExitRequested;
                    }
                    return WaitOutcome::Event(event);
                }
                Some(Some(Err(e))) => {
                    error!("Error reading event: {}", e);
                }
                Some(None) => {
                    warn!("Event stream ended");
                    return WaitOutcome::Disconnected;
                }
                None => return WaitOutcome::Timeout,
            }
        } else {
            match reader.next().await {
                Some(Ok(event)) => {
                    if is_exit_event(&event, exit_code) {
                        return WaitOutcome::ExitRequested;
                    }
                    return WaitOutcome::Event(event);
                }
                Some(Err(e)) => {
                    error!("Error reading event: {}", e);
                }
                None => {
                    warn!("Event stream ended");
                    return WaitOutcome::Disconnected;
                }
            }
        }
    }
}

fn drain_pending_events(
    reader: &mut EventStream,
    exit_code: Option<KeyEvent>,
    events: &mut Vec<Event>,
    max_event_per_frame: usize,
) -> DrainOutcome {
    while events.len() < max_event_per_frame {
        match reader.next().now_or_never() {
            Some(Some(Ok(event))) => {
                if is_exit_event(&event, exit_code) {
                    return DrainOutcome::ExitRequested;
                }
                events.push(event);
            }
            Some(Some(Err(e))) => {
                error!("Error reading event: {}", e);
            }
            Some(None) => {
                warn!("Event stream ended");
                return DrainOutcome::Disconnected;
            }
            None => break,
        }
    }

    DrainOutcome::Continue
}

fn is_exit_event(event: &Event, exit_code: Option<KeyEvent>) -> bool {
    matches!(
        (event, exit_code),
        (Event::Key(key_event), Some(exit_key)) if *key_event == exit_key
    )
}
