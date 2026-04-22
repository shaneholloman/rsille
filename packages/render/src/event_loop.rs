use std::{
    io::{stdout, Stdout},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{position, Hide, MoveToNextLine, Show},
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEvent},
    execute,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use log::{error, info, warn};

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

        let (event_tx, event_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();

        let event_thread = self.make_event_thread(event_tx, stop_tx);
        let render_thread = self.make_render_thread(event_rx, stop_rx);

        event_thread.join().map_err(DrawErr::thread_panic)?;
        render_thread.join().map_err(DrawErr::thread_panic)?;

        info!(target: "render::event_loop", "event loop stopped");

        Ok(())
    }

    fn make_event_thread(
        &self,
        event_tx: mpsc::Sender<Event>,
        stop_tx: mpsc::Sender<()>,
    ) -> thread::JoinHandle<()> {
        let exit_code = self.exit_code;
        thread::spawn(move || event_thread(event_tx, stop_tx, exit_code))
    }

    fn make_render_thread(
        mut self,
        event_rx: mpsc::Receiver<Event>,
        stop_rx: mpsc::Receiver<()>,
    ) -> thread::JoinHandle<()> {
        let frame_interval = self
            .frame_limit
            .filter(|fps| *fps > 0)
            .map(|fps| Duration::from_secs_f64(1.0 / fps as f64));
        let max_event_per_frame = self.max_event_per_frame;

        thread::spawn(move || {
            let mut pending_initial_frame = true;
            let mut next_frame_at = Instant::now();

            loop {
                if stop_rx.try_recv().is_ok() {
                    break;
                }

                let mut events = Vec::new();
                match wait_for_frame_or_event(
                    &event_rx,
                    frame_interval,
                    next_frame_at,
                    pending_initial_frame,
                ) {
                    WaitOutcome::Event(event) => events.push(event),
                    WaitOutcome::Timeout => {}
                    WaitOutcome::Disconnected => {
                        if stop_rx.try_recv().is_err() {
                            warn!("Event channel closed, stopping render thread");
                        }
                        break;
                    }
                }

                drain_pending_events(&event_rx, &mut events, max_event_per_frame);

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
        })
    }
}

enum WaitOutcome {
    Event(Event),
    Timeout,
    Disconnected,
}

fn wait_for_frame_or_event(
    event_rx: &mpsc::Receiver<Event>,
    frame_interval: Option<Duration>,
    next_frame_at: Instant,
    pending_initial_frame: bool,
) -> WaitOutcome {
    if pending_initial_frame {
        return WaitOutcome::Timeout;
    }

    if frame_interval.is_some() {
        match event_rx.recv_timeout(next_frame_at.saturating_duration_since(Instant::now())) {
            Ok(event) => WaitOutcome::Event(event),
            Err(mpsc::RecvTimeoutError::Timeout) => WaitOutcome::Timeout,
            Err(mpsc::RecvTimeoutError::Disconnected) => WaitOutcome::Disconnected,
        }
    } else {
        match event_rx.recv() {
            Ok(event) => WaitOutcome::Event(event),
            Err(_) => WaitOutcome::Disconnected,
        }
    }
}

fn drain_pending_events(
    event_rx: &mpsc::Receiver<Event>,
    events: &mut Vec<Event>,
    max_event_per_frame: usize,
) {
    while events.len() < max_event_per_frame {
        match event_rx.try_recv() {
            Ok(event) => events.push(event),
            Err(mpsc::TryRecvError::Empty) => break,
            Err(mpsc::TryRecvError::Disconnected) => break,
        }
    }
}

fn event_thread(
    event_tx: mpsc::Sender<Event>,
    stop_tx: mpsc::Sender<()>,
    exit_code: Option<KeyEvent>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            error!("Failed to create tokio runtime: {}", e);
            let _ = stop_tx.send(());
            return;
        }
    };

    rt.block_on(async move {
        let mut reader = EventStream::new();

        loop {
            match reader.next().await {
                Some(Ok(event)) => {
                    if let Some(exit_key) = exit_code {
                        if let Event::Key(key_event) = event {
                            if key_event == exit_key {
                                let _ = stop_tx.send(());
                                break;
                            }
                        }
                    }

                    if event_tx.send(event).is_err() {
                        break;
                    }
                }
                Some(Err(e)) => {
                    error!("Error reading event: {}", e);
                }
                None => {
                    warn!("Event stream ended");
                    let _ = stop_tx.send(());
                    break;
                }
            }
        }
    });
}
