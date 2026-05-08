use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::Event;
use render::area::Size;
use render::chunk::Chunk;
use render::{Draw, DrawErr, Update};

use crate::animation::{
    AnimationCtx, AnimationStore, MotionPolicy, Presence, Timeline, TimelineFrame, TransitionEffect,
};
use crate::effect::{
    run_task_attempt, sleep_with_cancellation, CancellationToken, Effect, Task, TaskEvent,
    TaskEventSender, TaskId, TaskState, TaskStatus, UpdateCtx,
};
use crate::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::focus::FocusManager;
use crate::shell::{CommandRouter, Hotkey, HotkeyRegistry};
use crate::style::Theme;
use crate::widget::{
    EventCtx, EventPhase, FocusRequest, RenderCtx, Widget, WidgetKey, WidgetPath, WidgetStore,
};
use crate::widgets::text_input::TextInputState;
use crate::widgets::textarea::TextAreaState;
use crate::WidgetResult;

pub type EventHandler<M> = Box<dyn Fn() -> M>;
pub type FrameHandler<M> = Box<dyn Fn(FrameInfo) -> M>;
pub type ThemeResolver<State> = Box<dyn Fn(&State) -> Theme>;

struct TickConfig<M> {
    interval: Duration,
    handler: EventHandler<M>,
}

struct TickRuntime<M> {
    interval: Duration,
    next_fire_at: Instant,
    handler: EventHandler<M>,
}

struct FrameConfig<M> {
    handler: FrameHandler<M>,
}

/// Timing information for a rendered frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub now: Instant,
    pub delta: Duration,
    pub since_start: Duration,
    pub frame: u64,
}

#[derive(Debug)]
struct FrameRuntime {
    started_at: Option<Instant>,
    last_frame_at: Option<Instant>,
    frame: u64,
}

impl FrameRuntime {
    fn new() -> Self {
        Self {
            started_at: None,
            last_frame_at: None,
            frame: 0,
        }
    }

    fn next(&mut self, now: Instant) -> FrameInfo {
        let started_at = *self.started_at.get_or_insert(now);
        let delta = self
            .last_frame_at
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or(Duration::ZERO);
        let info = FrameInfo {
            now,
            delta,
            since_start: now.saturating_duration_since(started_at),
            frame: self.frame,
        };

        self.last_frame_at = Some(now);
        self.frame = self.frame.wrapping_add(1);
        info
    }
}

/// Quit key configuration for the application.
#[derive(Debug, Clone, Default)]
pub enum QuitBehavior {
    /// Default quit key (Esc).
    #[default]
    Default,
    /// Custom quit key (simple key without modifiers).
    CustomKey(KeyCode),
    /// Custom key event (with modifiers like Ctrl+C).
    CustomKeyEvent(KeyEvent),
    /// Disable built-in quit handling.
    Disabled,
}

trait UpdateDriver<State, M> {
    fn handle(&mut self, state: &mut State, message: M, ctx: &mut UpdateCtx<M>);
}

struct SimpleUpdate<F>(F);

impl<State, M, F> UpdateDriver<State, M> for SimpleUpdate<F>
where
    F: Fn(&mut State, M),
{
    fn handle(&mut self, state: &mut State, message: M, _ctx: &mut UpdateCtx<M>) {
        (self.0)(state, message);
    }
}

impl<State, M, F> UpdateDriver<State, M> for F
where
    F: Fn(&mut State, M, &mut UpdateCtx<M>),
{
    fn handle(&mut self, state: &mut State, message: M, ctx: &mut UpdateCtx<M>) {
        (self)(state, message, ctx);
    }
}

// ---------------------------------------------------------------------------
// App – public builder API
// ---------------------------------------------------------------------------

/// Application builder. Create with [`App::new`], configure, then call `.run()`.
pub struct App<State, M = ()> {
    state: State,
    theme: Theme,
    theme_resolver: Option<ThemeResolver<State>>,
    global_key_handlers: HashMap<KeyCode, EventHandler<M>>,
    hotkeys: HotkeyRegistry<M>,
    command_router: Option<CommandRouter<M>>,
    tick_handlers: Vec<TickConfig<M>>,
    frame_handlers: Vec<FrameConfig<M>>,
    quit_behavior: QuitBehavior,
    mouse_capture: bool,
    motion_policy: MotionPolicy,
}

impl<State: std::fmt::Debug, M> std::fmt::Debug for App<State, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("state", &self.state)
            .field("quit_behavior", &self.quit_behavior)
            .finish()
    }
}

impl<State, M: Clone + std::fmt::Debug + Send + 'static> App<State, M> {
    pub fn new(state: State) -> Self {
        Self {
            state,
            theme: Theme::dark(),
            theme_resolver: None,
            global_key_handlers: HashMap::new(),
            hotkeys: HotkeyRegistry::new(),
            command_router: None,
            tick_handlers: Vec::new(),
            frame_handlers: Vec::new(),
            quit_behavior: QuitBehavior::default(),
            mouse_capture: false,
            motion_policy: MotionPolicy::default(),
        }
    }

    /// Configure the theme used for rendering.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.theme_resolver = None;
        self
    }

    /// Resolve the theme from application state before each render.
    pub fn with_theme_from<F>(mut self, resolver: F) -> Self
    where
        F: Fn(&State) -> Theme + 'static,
    {
        self.theme_resolver = Some(Box::new(resolver));
        self
    }

    /// Configure global animation behavior.
    pub fn with_motion_policy(mut self, motion_policy: MotionPolicy) -> Self {
        self.motion_policy = motion_policy;
        self
    }

    /// Register a global keyboard shortcut.
    pub fn on_key<F>(mut self, key: KeyCode, handler: F) -> Self
    where
        F: Fn() -> M + 'static,
    {
        self.global_key_handlers.insert(key, Box::new(handler));
        self
    }

    /// Register a richer hotkey with metadata and modifier support.
    pub fn on_hotkey<F>(mut self, hotkey: Hotkey, handler: F) -> Self
    where
        F: Fn() -> M + 'static,
    {
        self.hotkeys.bind(hotkey, handler);
        self
    }

    /// Install a reusable hotkey registry.
    pub fn with_hotkeys(mut self, hotkeys: HotkeyRegistry<M>) -> Self {
        self.hotkeys = hotkeys;
        self
    }

    /// Install a command router that can dispatch commands from hotkeys.
    pub fn with_command_router(mut self, command_router: CommandRouter<M>) -> Self {
        self.command_router = Some(command_router);
        self
    }

    /// Register a periodic timer that emits a message at the given interval.
    ///
    /// Missed ticks are coalesced into a single message, so a busy frame does
    /// not cause a backlog of timer messages.
    pub fn on_tick<F>(mut self, interval: Duration, handler: F) -> Self
    where
        F: Fn() -> M + 'static,
    {
        let interval = if interval.is_zero() {
            Duration::from_millis(1)
        } else {
            interval
        };
        self.tick_handlers.push(TickConfig {
            interval,
            handler: Box::new(handler),
        });
        self
    }

    /// Register a callback that runs once per frame.
    ///
    /// This is a good fit for time-based animation because the callback
    /// receives real frame timing data.
    pub fn on_frame<F>(mut self, handler: F) -> Self
    where
        F: Fn(FrameInfo) -> M + 'static,
    {
        self.frame_handlers.push(FrameConfig {
            handler: Box::new(handler),
        });
        self
    }

    /// Configure the quit key.
    pub fn with_quit_key(mut self, key: KeyCode) -> Self {
        self.quit_behavior = QuitBehavior::CustomKey(key);
        self
    }

    /// Configure quit key with modifiers (e.g., Ctrl+C).
    pub fn with_quit_key_event(mut self, key_event: KeyEvent) -> Self {
        self.quit_behavior = QuitBehavior::CustomKeyEvent(key_event);
        self
    }

    /// Disable built-in quit key handling.
    pub fn disable_quit_key(mut self) -> Self {
        self.quit_behavior = QuitBehavior::Disabled;
        self
    }

    /// Enable mouse capture for wheel, click, and drag interactions.
    pub fn enable_mouse_capture(mut self) -> Self {
        self.mouse_capture = true;
        self
    }

    /// Run the application in full-screen mode.
    pub fn run<F, V, W>(self, update: F, view: V) -> WidgetResult<()>
    where
        F: Fn(&mut State, M),
        V: Fn(&State) -> W,
        W: Widget<M> + 'static,
    {
        let view_fn = move |state: &State| -> Box<dyn Widget<M>> { Box::new(view(state)) };
        self.run_with_options(SimpleUpdate(update), view_fn, false)
    }

    /// Run the application in inline (non-fullscreen) mode.
    pub fn run_inline<F, V, W>(self, update: F, view: V) -> WidgetResult<()>
    where
        F: Fn(&mut State, M),
        V: Fn(&State) -> W,
        W: Widget<M> + 'static,
    {
        let view_fn = move |state: &State| -> Box<dyn Widget<M>> { Box::new(view(state)) };
        self.run_with_options(SimpleUpdate(update), view_fn, true)
    }

    /// Run the application with an update context that can schedule effects.
    pub fn run_with_effects<F, V, W>(self, update: F, view: V) -> WidgetResult<()>
    where
        F: Fn(&mut State, M, &mut UpdateCtx<M>),
        V: Fn(&State) -> W,
        W: Widget<M> + 'static,
    {
        let view_fn = move |state: &State| -> Box<dyn Widget<M>> { Box::new(view(state)) };
        self.run_with_options(update, view_fn, false)
    }

    /// Run the application inline with an update context that can schedule effects.
    pub fn run_inline_with_effects<F, V, W>(self, update: F, view: V) -> WidgetResult<()>
    where
        F: Fn(&mut State, M, &mut UpdateCtx<M>),
        V: Fn(&State) -> W,
        W: Widget<M> + 'static,
    {
        let view_fn = move |state: &State| -> Box<dyn Widget<M>> { Box::new(view(state)) };
        self.run_with_options(update, view_fn, true)
    }

    fn run_with_options<U, V>(self, update: U, view: V, inline_mode: bool) -> WidgetResult<()>
    where
        U: UpdateDriver<State, M>,
        V: Fn(&State) -> Box<dyn Widget<M>>,
    {
        let (width, height) = crossterm::terminal::size()?;
        let inline_max_height: u16 = 50;

        let (buffer_height, initial_used_height) = if inline_mode {
            let layout = view(&self.state);
            let required = layout.constraints().min_height;
            let used = required.min(inline_max_height).min(height);
            (inline_max_height.min(height), used)
        } else {
            (height, height)
        };
        let App {
            state,
            theme,
            theme_resolver,
            global_key_handlers,
            hotkeys,
            command_router,
            tick_handlers,
            frame_handlers,
            quit_behavior,
            mouse_capture,
            motion_policy,
        } = self;

        let (task_sender, task_receiver) = mpsc::channel();

        let runtime = AppRuntime {
            state,
            theme,
            theme_resolver,
            update_fn: update,
            view_fn: view,
            store: WidgetStore::new(),
            animation_store: RefCell::new(AnimationStore::new()),
            geometry: RefCell::new(HashMap::new()),
            focus: FocusManager::new(),
            cached_tree: None,
            presence_previous_tree: None,
            presence_previous_geometry: HashMap::new(),
            exiting_visuals: Vec::new(),
            messages: Vec::new(),
            should_quit: false,
            quit_behavior,
            global_key_handlers,
            hotkeys,
            command_router,
            tick_handlers: tick_handlers
                .into_iter()
                .map(|tick| TickRuntime {
                    interval: tick.interval,
                    next_fire_at: Instant::now() + tick.interval,
                    handler: tick.handler,
                })
                .collect(),
            frame_handlers: frame_handlers
                .into_iter()
                .map(|frame| frame.handler)
                .collect(),
            frame_runtime: FrameRuntime::new(),
            animation_epoch: Instant::now(),
            animation_frame: 0,
            motion_policy,
            inline_mode,
            inline_max_height,
            task_sender,
            task_receiver,
            debounces: HashMap::new(),
            next_debounce_nonce: 0,
            next_task_id: 0,
            tasks: HashMap::new(),
            task_keys: HashMap::new(),
            task_order: Vec::new(),
        };

        let mut builder = render::Builder::new();
        builder
            .enable_raw_mode()
            .clear(false)
            .append_newline(false)
            .enable_hide_cursor()
            .disable_exit_code();
        if mouse_capture {
            builder.enable_mouse_capture();
        }

        if inline_mode {
            builder
                .inline_mode(true)
                .inline_max_height(buffer_height)
                .frame_limit(60)
                .size((width, buffer_height));
        } else {
            builder.enable_all().frame_limit(60).size((width, height));
        }

        let mut event_loop = builder.build_event_loop(runtime);

        if inline_mode {
            event_loop.set_initial_used_height(initial_used_height);
        }

        event_loop
            .run()
            .map_err(|e| crate::error::WidgetError::render_error(e.to_string()))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AppRuntime – implements Draw + Update for the render event loop
// ---------------------------------------------------------------------------

struct TaskRecord<M> {
    status: TaskStatus,
    cancellation: CancellationToken,
    status_handler: Option<crate::effect::StatusHandler<M>>,
}

struct AppRuntime<State, U, V, M> {
    state: State,
    theme: Theme,
    theme_resolver: Option<ThemeResolver<State>>,
    update_fn: U,
    view_fn: V,
    store: WidgetStore,
    animation_store: RefCell<AnimationStore>,
    geometry: RefCell<HashMap<WidgetPath, render::area::Area>>,
    focus: FocusManager,
    cached_tree: Option<Box<dyn Widget<M>>>,
    presence_previous_tree: Option<Box<dyn Widget<M>>>,
    presence_previous_geometry: HashMap<WidgetPath, render::area::Area>,
    exiting_visuals: Vec<ExitingVisual<M>>,
    messages: Vec<M>,
    should_quit: bool,
    quit_behavior: QuitBehavior,
    global_key_handlers: HashMap<KeyCode, Box<dyn Fn() -> M>>,
    hotkeys: HotkeyRegistry<M>,
    command_router: Option<CommandRouter<M>>,
    tick_handlers: Vec<TickRuntime<M>>,
    frame_handlers: Vec<FrameHandler<M>>,
    frame_runtime: FrameRuntime,
    animation_epoch: Instant,
    animation_frame: u64,
    motion_policy: MotionPolicy,
    inline_mode: bool,
    inline_max_height: u16,
    task_sender: Sender<TaskEvent<M>>,
    task_receiver: Receiver<TaskEvent<M>>,
    debounces: HashMap<String, u64>,
    next_debounce_nonce: u64,
    next_task_id: u64,
    tasks: HashMap<TaskId, TaskRecord<M>>,
    task_keys: HashMap<String, TaskId>,
    task_order: Vec<TaskId>,
}

struct ExitingVisual<M> {
    root: Box<dyn Widget<M>>,
    nodes: Vec<ExitingNode>,
}

struct ExitingNode {
    path: WidgetPath,
    bounds: render::area::Area,
}

fn exit_display_area(area: render::area::Area, frames: &[TimelineFrame]) -> render::area::Area {
    frames
        .iter()
        .fold(area, |display_area, frame| match frame.transition.effect {
            TransitionEffect::Collapse | TransitionEffect::Expand => {
                vertical_area_for_exit(display_area, 1.0 - frame.progress)
            }
            TransitionEffect::ScaleFromCenter => {
                scale_area_for_exit(display_area, 1.0 - frame.progress)
            }
            TransitionEffect::Layout(_)
            | TransitionEffect::Fade
            | TransitionEffect::BorderEmphasis => display_area,
        })
}

fn vertical_area_for_exit(area: render::area::Area, progress: f64) -> render::area::Area {
    let height = ((area.height() as f64) * progress.clamp(0.0, 1.0)).round() as u16;
    render::area::Area::new(area.pos(), (area.width(), height).into())
}

fn scale_area_for_exit(area: render::area::Area, progress: f64) -> render::area::Area {
    let progress = progress.clamp(0.0, 1.0);
    let width = ((area.width() as f64) * progress).round() as u16;
    let height = ((area.height() as f64) * progress).round() as u16;
    let x = area.x() + area.width().saturating_sub(width) / 2;
    let y = area.y() + area.height().saturating_sub(height) / 2;

    render::area::Area::new((x, y).into(), (width, height).into())
}

impl<State, U, V, M: Send + 'static> AppRuntime<State, U, V, M> {
    const MAX_TRACKED_TASKS: usize = 256;

    fn ensure_tree(&mut self)
    where
        V: Fn(&State) -> Box<dyn Widget<M>>,
    {
        if self.cached_tree.is_none() {
            let tree = (self.view_fn)(&self.state);
            self.focus.rebuild(tree.as_ref());
            self.retain_state_for_live_and_exiting_paths();
            self.cached_tree = Some(tree);
        }
    }

    fn exiting_paths(&self) -> Vec<WidgetPath> {
        self.exiting_visuals
            .iter()
            .flat_map(|visual| visual.nodes.iter().map(|node| node.path.clone()))
            .collect()
    }

    fn retain_state_for_live_and_exiting_paths(&mut self) {
        let live_paths: Vec<WidgetPath> = self.focus.live_paths().iter().cloned().collect();
        let exiting_paths = self.exiting_paths();
        let mut live_shared_ids = HashSet::new();
        if let Some(tree) = self.cached_tree.as_ref() {
            let mut path = WidgetPath::root();
            Self::collect_shared_transition_ids(tree.as_ref(), &mut path, &mut live_shared_ids);
        }
        let should_retain = |path: &WidgetPath| {
            live_paths.iter().any(|live| live == path)
                || exiting_paths.iter().any(|exit| path.starts_with(exit))
        };

        self.store.retain_active(should_retain);
        let mut animation_store = self.animation_store.borrow_mut();
        animation_store.retain_active(|path| {
            live_paths.iter().any(|live| live == path)
                || exiting_paths.iter().any(|exit| path.starts_with(exit))
        });
        animation_store.retain_shared_layouts(|id| live_shared_ids.contains(id));
    }

    fn collect_presence_declarations(
        widget: &dyn Widget<M>,
        path: &mut WidgetPath,
        declarations: &mut HashMap<WidgetPath, Presence>,
    ) {
        if let Some(presence) = widget.presence() {
            declarations.insert(path.clone(), presence.clone());
        }

        for (index, child) in widget.children().iter().enumerate() {
            path.push(WidgetKey::for_child(index, child.as_ref()));
            Self::collect_presence_declarations(child.as_ref(), path, declarations);
            path.pop();
        }
    }

    fn collect_shared_transition_ids(
        widget: &dyn Widget<M>,
        path: &mut WidgetPath,
        ids: &mut HashSet<String>,
    ) {
        if let Some(shared) = widget.shared_transition() {
            ids.insert(shared.id.clone());
        }

        for (index, child) in widget.children().iter().enumerate() {
            path.push(WidgetKey::for_child(index, child.as_ref()));
            Self::collect_shared_transition_ids(child.as_ref(), path, ids);
            path.pop();
        }
    }

    fn start_exit_visuals(
        &mut self,
        previous_tree: Box<dyn Widget<M>>,
        previous_geometry: HashMap<WidgetPath, render::area::Area>,
        now: Instant,
    ) {
        let mut previous_presence = HashMap::new();
        let mut path = WidgetPath::root();
        Self::collect_presence_declarations(
            previous_tree.as_ref(),
            &mut path,
            &mut previous_presence,
        );

        if previous_presence.is_empty() {
            return;
        }

        let live_paths: Vec<WidgetPath> = self.focus.live_paths().iter().cloned().collect();
        let mut candidates: Vec<(WidgetPath, render::area::Area, Timeline)> = previous_presence
            .into_iter()
            .filter_map(|(path, presence)| {
                if live_paths.iter().any(|live| live == &path) {
                    return None;
                }

                let timeline = presence.exit?;
                let bounds = previous_geometry.get(&path).copied()?;
                Some((path, bounds, timeline))
            })
            .collect();

        candidates.sort_by_key(|(path, _, _)| path.len());

        let mut nodes: Vec<ExitingNode> = Vec::new();
        for (path, bounds, timeline) in candidates {
            if nodes.iter().any(|node| path.starts_with(&node.path)) {
                continue;
            }

            self.animation_store.borrow_mut().track_timeline(
                &path,
                "exit",
                timeline.clone(),
                true,
                now,
                self.motion_policy,
            );
            nodes.push(ExitingNode { path, bounds });
        }

        if !nodes.is_empty() {
            self.exiting_visuals.push(ExitingVisual {
                root: previous_tree,
                nodes,
            });
        }
    }

    fn sync_theme(&mut self) {
        if let Some(resolver) = self.theme_resolver.as_ref() {
            self.theme = resolver(&self.state);
        }
    }

    fn build_event_route<'a>(
        tree: &'a dyn Widget<M>,
        target_path: Option<&WidgetPath>,
    ) -> Vec<(WidgetPath, &'a dyn Widget<M>)> {
        let mut route = vec![(WidgetPath::root(), tree)];
        let Some(target_path) = target_path else {
            return route;
        };

        let mut current_widget = tree;
        let mut current_path = WidgetPath::root();

        for key in target_path.as_slice() {
            let children = current_widget.children();
            let next = match key {
                WidgetKey::Index(idx) => children.get(*idx).map(|child| child.as_ref()),
                WidgetKey::Named(name) => children
                    .iter()
                    .find(|child| child.key() == Some(name.as_str()))
                    .map(|child| child.as_ref()),
            };

            let Some(next_widget) = next else {
                break;
            };

            current_path.push(key.clone());
            route.push((current_path.clone(), next_widget));
            current_widget = next_widget;
        }

        route
    }

    fn dispatch_to_widget(
        widget: &dyn Widget<M>,
        event: &Event,
        store: &mut WidgetStore,
        messages: &mut Vec<M>,
        path: WidgetPath,
        focused_path: Option<WidgetPath>,
        geometry: &HashMap<WidgetPath, render::area::Area>,
        phase: EventPhase,
        already_handled: bool,
    ) -> crate::widget::EventOutcome {
        let mut ctx = EventCtx::new(
            store,
            messages,
            path,
            focused_path,
            geometry,
            phase,
            already_handled,
        );
        widget.handle_event(event, &mut ctx);
        ctx.finish()
    }

    fn apply_focus_request(focus: &mut FocusManager, request: Option<FocusRequest>) {
        match request {
            Some(FocusRequest::Set(path)) => {
                focus.request_focus(&path);
            }
            Some(FocusRequest::Clear) => {
                focus.clear();
            }
            Some(FocusRequest::Next) => {
                focus.next();
            }
            Some(FocusRequest::Prev) => {
                focus.prev();
            }
            Some(FocusRequest::NextInScope(scope)) => {
                focus.next_in_scope(scope.as_ref());
            }
            Some(FocusRequest::PrevInScope(scope)) => {
                focus.prev_in_scope(scope.as_ref());
            }
            None => {}
        }
    }

    fn dispatch_widget_event(
        tree: &dyn Widget<M>,
        event: &Event,
        store: &mut WidgetStore,
        messages: &mut Vec<M>,
        focus: &mut FocusManager,
        geometry: &HashMap<WidgetPath, render::area::Area>,
    ) -> bool {
        let route = Self::build_event_route(tree, focus.current_path());
        let mut handled = false;
        let focused_snapshot = focus.current_path().cloned();
        let ancestor_len = route.len().saturating_sub(1);

        for (path, widget) in route.iter().take(ancestor_len) {
            let outcome = Self::dispatch_to_widget(
                *widget,
                event,
                store,
                messages,
                path.clone(),
                focused_snapshot.clone(),
                geometry,
                EventPhase::Capture,
                handled,
            );
            handled |= outcome.handled;
            Self::apply_focus_request(focus, outcome.focus_request);
            if outcome.stop_propagation {
                return handled;
            }
        }

        if let Some((path, widget)) = route.last() {
            let outcome = Self::dispatch_to_widget(
                *widget,
                event,
                store,
                messages,
                path.clone(),
                focused_snapshot.clone(),
                geometry,
                EventPhase::Target,
                handled,
            );
            handled |= outcome.handled;
            Self::apply_focus_request(focus, outcome.focus_request);
            if outcome.stop_propagation {
                return handled;
            }
        }

        for (path, widget) in route.iter().take(ancestor_len).rev() {
            let outcome = Self::dispatch_to_widget(
                *widget,
                event,
                store,
                messages,
                path.clone(),
                focused_snapshot.clone(),
                geometry,
                EventPhase::Bubble,
                handled,
            );
            handled |= outcome.handled;
            Self::apply_focus_request(focus, outcome.focus_request);
            if outcome.stop_propagation {
                return handled;
            }
        }

        handled
    }

    fn animate_widget_tree(
        widget: &dyn Widget<M>,
        path: &mut WidgetPath,
        store: &mut AnimationStore,
        focused_path: Option<&WidgetPath>,
        now: Instant,
        motion_policy: MotionPolicy,
        animation_theme: crate::animation::AnimationTheme,
    ) -> bool {
        let mut needs_render = {
            let mut ctx = AnimationCtx::with_policy(
                store,
                path.clone(),
                focused_path,
                now,
                motion_policy,
                animation_theme,
            );
            widget.animate(&mut ctx)
        };

        for (index, child) in widget.children().iter().enumerate() {
            path.push(WidgetKey::for_child(index, child.as_ref()));
            needs_render |= Self::animate_widget_tree(
                child.as_ref(),
                path,
                store,
                focused_path,
                now,
                motion_policy,
                animation_theme,
            );
            path.pop();
        }

        needs_render
    }

    fn animation_now(&mut self) -> Instant {
        if !self.motion_policy.deterministic {
            return Instant::now();
        }

        let now = self.animation_epoch
            + self
                .motion_policy
                .deterministic_step
                .saturating_mul(self.animation_frame.min(u32::MAX as u64) as u32);
        self.animation_frame = self.animation_frame.wrapping_add(1);
        now
    }

    fn animate_widgets(&mut self) -> bool {
        if self.cached_tree.is_none() {
            return false;
        }

        let focused_path = self.focus.current_path().cloned();
        let mut path = WidgetPath::root();
        let now = self.animation_now();
        let animation_theme = self.theme.animations;
        let tree = self.cached_tree.as_ref().unwrap();
        let mut store = self.animation_store.borrow_mut();
        let mut needs_render = Self::animate_widget_tree(
            tree.as_ref(),
            &mut path,
            &mut store,
            focused_path.as_ref(),
            now,
            self.motion_policy,
            animation_theme,
        );
        needs_render |= store.advance(now);
        needs_render
    }

    fn render_exiting_visuals(&self, chunk: &mut Chunk, ctx: &RenderCtx, now: Instant) {
        for visual in &self.exiting_visuals {
            for node in &visual.nodes {
                let frames = self
                    .animation_store
                    .borrow()
                    .timeline_frames(&node.path, "exit", now);
                let display_area = exit_display_area(node.bounds, &frames);

                if display_area.width() == 0 || display_area.height() == 0 {
                    continue;
                }

                let _ = Self::render_widget_at_path(
                    visual.root.as_ref(),
                    node.path.as_slice(),
                    chunk,
                    ctx,
                    display_area,
                );
            }
        }
    }

    fn render_widget_at_path(
        widget: &dyn Widget<M>,
        path: &[WidgetKey],
        chunk: &mut Chunk,
        ctx: &RenderCtx,
        area: render::area::Area,
    ) -> bool {
        let Some((key, rest)) = path.split_first() else {
            let _ = chunk.with_clip(area, |child_chunk| {
                widget.render(child_chunk, ctx);
            });
            return true;
        };

        let children = widget.children();
        let child = match key {
            WidgetKey::Index(index) => children.get(*index).map(|child| child.as_ref()),
            WidgetKey::Named(name) => children
                .iter()
                .find(|child| child.key() == Some(name.as_str()))
                .map(|child| child.as_ref()),
        };

        let Some(child) = child else {
            return false;
        };

        let child_ctx = ctx.child_ctx(key.clone());
        Self::render_widget_at_path(child, rest, chunk, &child_ctx, area)
    }

    fn prune_completed_exit_visuals(&mut self, now: Instant) {
        let animation_store = self.animation_store.borrow();
        for visual in &mut self.exiting_visuals {
            visual
                .nodes
                .retain(|node| animation_store.timeline_is_active(&node.path, "exit", now));
        }
        drop(animation_store);

        self.exiting_visuals
            .retain(|visual| !visual.nodes.is_empty());
    }

    fn queue_tick_messages(&mut self) {
        if self.tick_handlers.is_empty() {
            return;
        }

        let now = Instant::now();

        for tick in &mut self.tick_handlers {
            if now < tick.next_fire_at {
                continue;
            }

            self.messages.push((tick.handler)());

            while tick.next_fire_at <= now {
                tick.next_fire_at += tick.interval;
            }
        }
    }

    fn queue_frame_messages(&mut self) {
        if self.frame_handlers.is_empty() {
            return;
        }

        let info = self.frame_runtime.next(Instant::now());
        for handler in &self.frame_handlers {
            self.messages.push(handler(info));
        }
    }

    fn task_statuses(&self) -> Vec<TaskStatus> {
        self.task_order
            .iter()
            .filter_map(|task_id| self.tasks.get(task_id).map(|record| record.status.clone()))
            .collect()
    }

    fn record_task_status(&mut self, status: TaskStatus) {
        let Some(record) = self.tasks.get_mut(&status.id) else {
            return;
        };

        record.status = status.clone();

        if let Some(handler) = record.status_handler.as_ref() {
            if let Some(message) = handler(status) {
                self.messages.push(message);
            }
        }
    }

    fn drain_task_events(&mut self) {
        while let Ok(event) = self.task_receiver.try_recv() {
            match event {
                TaskEvent::Message(message) => self.messages.push(message),
                TaskEvent::Status(status) => self.record_task_status(status),
                TaskEvent::Debounced {
                    key,
                    nonce,
                    message,
                } => {
                    if self.debounces.get(&key) == Some(&nonce) {
                        self.debounces.remove(&key);
                        self.messages.push(message);
                    }
                }
            }
        }
    }

    fn cancel_task(&mut self, task_id: TaskId) {
        let mut status = None;

        if let Some(record) = self.tasks.get_mut(&task_id) {
            if record.status.state.is_terminal() || record.status.state == TaskState::Cancelling {
                return;
            }

            record.cancellation.cancel();
            let mut next = record.status.clone();
            next.state = TaskState::Cancelling;
            next.updated_at = Instant::now();
            status = Some(next);
        }

        if let Some(status) = status {
            self.record_task_status(status);
        }
    }

    fn cancel_task_key(&mut self, key: &str) {
        if let Some(task_id) = self.task_keys.get(key).copied() {
            self.cancel_task(task_id);
        }
    }

    fn prune_finished_tasks(&mut self) {
        while self.tasks.len() > Self::MAX_TRACKED_TASKS {
            let Some(index) = self.task_order.iter().position(|task_id| {
                self.tasks
                    .get(task_id)
                    .map(|record| record.status.state.is_terminal())
                    .unwrap_or(false)
            }) else {
                break;
            };

            let task_id = self.task_order.remove(index);
            if let Some(record) = self.tasks.remove(&task_id) {
                if let Some(key) = record.status.key {
                    if self.task_keys.get(&key) == Some(&task_id) {
                        self.task_keys.remove(&key);
                    }
                }
            }
        }
    }

    fn spawn_task(&mut self, task: Task<M>) {
        if let Some(key) = task.key.as_deref() {
            if let Some(existing) = self.task_keys.get(key).copied() {
                self.cancel_task(existing);
            }
        }

        let task_id = TaskId(self.next_task_id);
        self.next_task_id = self.next_task_id.wrapping_add(1);

        let created_at = Instant::now();
        let status = TaskStatus::new(
            task_id,
            task.key.clone(),
            task.label.clone(),
            TaskState::Queued,
            1,
            task.retry.max_attempts(),
            created_at,
        );
        let cancellation = CancellationToken::new();

        if let Some(key) = task.key.as_ref() {
            self.task_keys.insert(key.clone(), task_id);
        }
        self.task_order.push(task_id);
        self.tasks.insert(
            task_id,
            TaskRecord {
                status: status.clone(),
                cancellation: cancellation.clone(),
                status_handler: task.status_handler.clone(),
            },
        );
        self.record_task_status(status);
        self.prune_finished_tasks();

        let sender = TaskEventSender::new(self.task_sender.clone());
        let key = task.key.clone();
        let label = task.label.clone();
        let retry = task.retry;
        let timeout = task.timeout;
        let runner = task.runner.clone();

        thread::spawn(move || {
            let mut attempt = 1;

            loop {
                let state = run_task_attempt(
                    task_id,
                    key.clone(),
                    label.clone(),
                    created_at,
                    attempt,
                    retry,
                    timeout,
                    cancellation.clone(),
                    sender.clone(),
                    runner.clone(),
                );

                if state == TaskState::RetryScheduled {
                    sender.send(TaskEvent::Status(TaskStatus::new(
                        task_id,
                        key.clone(),
                        label.clone(),
                        TaskState::RetryScheduled,
                        attempt,
                        retry.max_attempts(),
                        created_at,
                    )));

                    if !sleep_with_cancellation(&cancellation, retry.delay()) {
                        let cancelled_state = TaskState::Cancelled;
                        sender.send(TaskEvent::Status(TaskStatus::new(
                            task_id,
                            key.clone(),
                            label.clone(),
                            cancelled_state,
                            attempt,
                            retry.max_attempts(),
                            created_at,
                        )));
                        break;
                    }

                    attempt += 1;
                    continue;
                }

                sender.send(TaskEvent::Status(TaskStatus::new(
                    task_id,
                    key.clone(),
                    label.clone(),
                    state,
                    attempt,
                    retry.max_attempts(),
                    created_at,
                )));
                break;
            }
        });
    }

    fn apply_effect(&mut self, effect: Effect<M>) {
        match effect {
            Effect::None => {}
            Effect::Message(message) => self.messages.push(message),
            Effect::Batch(effects) => {
                for effect in effects {
                    self.apply_effect(effect);
                }
            }
            Effect::Spawn(task) => self.spawn_task(task),
            Effect::CancelTask(task_id) => self.cancel_task(task_id),
            Effect::CancelTaskKey(key) => self.cancel_task_key(&key),
            Effect::After(duration, message) => {
                let sender = self.task_sender.clone();
                thread::spawn(move || {
                    thread::sleep(duration);
                    let _ = sender.send(TaskEvent::Message(message));
                });
            }
            Effect::Debounce {
                key,
                duration,
                message,
            } => {
                let nonce = self.next_debounce_nonce;
                self.next_debounce_nonce = self.next_debounce_nonce.wrapping_add(1);
                self.debounces.insert(key.clone(), nonce);

                let sender = self.task_sender.clone();
                thread::spawn(move || {
                    thread::sleep(duration);
                    let _ = sender.send(TaskEvent::Debounced {
                        key,
                        nonce,
                        message,
                    });
                });
            }
            Effect::CancelDebounce(key) => {
                self.debounces.remove(&key);
            }
        }
    }
}

impl<State, U, V, M> Draw for AppRuntime<State, U, V, M>
where
    U: UpdateDriver<State, M>,
    V: Fn(&State) -> Box<dyn Widget<M>>,
    M: Clone + std::fmt::Debug + Send + 'static,
{
    fn draw(&mut self, mut chunk: Chunk) -> Result<Size, DrawErr> {
        let size = chunk.area().size();

        self.sync_theme();

        // Rebuild widget tree from current state
        let tree = (self.view_fn)(&self.state);

        // Rebuild focus chain
        self.focus.rebuild(tree.as_ref());

        let render_now = self.animation_now();
        if let Some(previous_tree) = self.presence_previous_tree.take() {
            let previous_geometry = std::mem::take(&mut self.presence_previous_geometry);
            self.start_exit_visuals(previous_tree, previous_geometry, render_now);
        }

        self.retain_state_for_live_and_exiting_paths();

        // Render
        let focused_path = self.focus.current_path();
        self.geometry.borrow_mut().clear();
        let ctx = RenderCtx::with_runtime(
            &self.store,
            &self.animation_store,
            &self.theme,
            focused_path,
            &self.geometry,
            render_now,
            self.motion_policy,
        );
        tree.render(&mut chunk, &ctx);
        self.render_exiting_visuals(&mut chunk, &ctx, render_now);
        self.prune_completed_exit_visuals(render_now);
        self.retain_state_for_live_and_exiting_paths();

        // Cache tree for event handling in on_events()
        self.cached_tree = Some(tree);

        Ok(size)
    }
}

impl<State, U, V, M> Update for AppRuntime<State, U, V, M>
where
    U: UpdateDriver<State, M>,
    V: Fn(&State) -> Box<dyn Widget<M>>,
    M: Clone + std::fmt::Debug + Send + 'static,
{
    fn on_events(&mut self, events: &[Event]) -> Result<(), DrawErr> {
        // Reset TextInputState.modified_this_batch so we sync from parent when value differs
        self.store
            .for_each_state_mut::<TextInputState, _>(|_, s| s.modified_this_batch = false);
        self.store
            .for_each_state_mut::<TextAreaState, _>(|_, s| s.modified_this_batch = false);

        // Ensure we have a tree (may be missing on first frame before draw)
        self.ensure_tree();

        let tree = self.cached_tree.as_ref().unwrap();
        let geometry = self.geometry.borrow();

        for event in events {
            if let Event::Resize(_, _) = event {
                continue;
            }

            if Self::dispatch_widget_event(
                tree.as_ref(),
                event,
                &mut self.store,
                &mut self.messages,
                &mut self.focus,
                &geometry,
            ) {
                continue;
            }

            // Only handle keyboard events after widget propagation.
            let key_event = match event {
                Event::Key(k) => k,
                _ => continue,
            };

            // 1. Tab / Shift+Tab → default focus navigation
            match key_event.code {
                KeyCode::Tab if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                    self.focus.prev();
                    continue;
                }
                KeyCode::Tab => {
                    self.focus.next();
                    continue;
                }
                _ => {}
            }

            // 2. Quit key
            let should_quit = match &self.quit_behavior {
                QuitBehavior::Default => {
                    key_event.code == KeyCode::Esc && key_event.modifiers.is_empty()
                }
                QuitBehavior::CustomKey(k) => {
                    key_event.code == *k && key_event.modifiers.is_empty()
                }
                QuitBehavior::CustomKeyEvent(ke) => {
                    key_event.code == ke.code && key_event.modifiers == ke.modifiers
                }
                QuitBehavior::Disabled => false,
            };
            if should_quit {
                self.should_quit = true;
                return Ok(());
            }

            // 3. Command router
            if let Some(router) = self.command_router.as_ref() {
                if let Some(message) = router.dispatch_hotkey(key_event) {
                    self.messages.push(message);
                    continue;
                }
            }

            // 4. Rich hotkey registry
            if let Some(message) = self.hotkeys.resolve(key_event) {
                self.messages.push(message);
                continue;
            }

            // 5. Legacy global key handlers
            if let Some(handler) = self.global_key_handlers.get(&key_event.code) {
                self.messages.push(handler());
            }
        }

        Ok(())
    }

    fn update(&mut self) -> Result<bool, DrawErr> {
        self.drain_task_events();
        self.queue_tick_messages();
        self.queue_frame_messages();

        let mut processed_messages = false;

        while !self.messages.is_empty() {
            let batch = std::mem::take(&mut self.messages);
            processed_messages = true;

            for message in batch {
                let mut effects = Vec::new();
                let statuses = self.task_statuses();
                let task_keys = self.task_keys.clone();
                let now = Instant::now();

                {
                    let mut ctx = UpdateCtx::new(&mut effects, statuses, task_keys, now);
                    self.update_fn.handle(&mut self.state, message, &mut ctx);
                }

                for effect in effects {
                    self.apply_effect(effect);
                }
            }

            self.drain_task_events();
            self.prune_finished_tasks();
        }

        if processed_messages {
            // Preserve the last rendered tree so draw() can play exit presence
            // for widgets that disappear from the next view tree.
            if self.presence_previous_tree.is_none() {
                self.presence_previous_tree = self.cached_tree.take();
                self.presence_previous_geometry = self.geometry.borrow().clone();
            } else {
                self.cached_tree = None;
            }
        }

        self.sync_theme();
        self.ensure_tree();
        let needs_animation_frame = self.animate_widgets();

        Ok(processed_messages || needs_animation_frame)
    }

    fn should_quit(&self) -> bool {
        self.should_quit
    }

    fn required_size(&self, current_size: Size) -> Option<Size> {
        if !self.inline_mode {
            return None;
        }
        if let Some(ref tree) = self.cached_tree {
            let required = tree.constraints().min_height;
            let height = required.min(self.inline_max_height);
            if height != current_size.height {
                return Some(Size {
                    width: current_size.width,
                    height,
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation::{AnimationSpec, Easing, Transition};

    #[test]
    fn exit_collapse_reduces_height_without_moving_origin() {
        let area = render::area::Area::new((2, 3).into(), (10, 6).into());
        let frame = TimelineFrame {
            transition: Transition::new(
                TransitionEffect::Collapse,
                AnimationSpec::new(Duration::from_millis(100), Easing::Linear),
            ),
            progress: 0.5,
            complete: false,
        };

        let display = exit_display_area(area, &[frame]);

        assert_eq!(display.x(), 2);
        assert_eq!(display.y(), 3);
        assert_eq!(display.width(), 10);
        assert_eq!(display.height(), 3);
    }

    #[test]
    fn exit_scale_from_center_collapses_around_center() {
        let area = render::area::Area::new((2, 4).into(), (10, 6).into());
        let frame = TimelineFrame {
            transition: Transition::new(
                TransitionEffect::ScaleFromCenter,
                AnimationSpec::new(Duration::from_millis(100), Easing::Linear),
            ),
            progress: 0.5,
            complete: false,
        };

        let display = exit_display_area(area, &[frame]);

        assert_eq!(display.x(), 4);
        assert_eq!(display.y(), 5);
        assert_eq!(display.width(), 5);
        assert_eq!(display.height(), 3);
    }
}
