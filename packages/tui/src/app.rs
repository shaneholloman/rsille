use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::Event;
use render::area::Size;
use render::chunk::Chunk;
use render::{Draw, DrawErr, Update};

use crate::event::{KeyCode, KeyEvent, KeyModifiers};
use crate::focus::FocusManager;
use crate::widget::{EventCtx, RenderCtx, Widget, WidgetStore};
use crate::widgets::text_input::TextInputState;
use crate::WidgetResult;

pub type EventHandler<M> = Arc<dyn Fn() -> M + Send + Sync>;

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


// ---------------------------------------------------------------------------
// App – public builder API
// ---------------------------------------------------------------------------

/// Application builder. Create with [`App::new`], configure, then call `.run()`.
pub struct App<State, M = ()> {
    state: State,
    global_key_handlers: HashMap<KeyCode, EventHandler<M>>,
    quit_behavior: QuitBehavior,
}

impl<State: std::fmt::Debug, M> std::fmt::Debug for App<State, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("state", &self.state)
            .field("quit_behavior", &self.quit_behavior)
            .finish()
    }
}

impl<State, M: Clone + std::fmt::Debug + Send + Sync + 'static> App<State, M> {
    pub fn new(state: State) -> Self {
        Self {
            state,
            global_key_handlers: HashMap::new(),
            quit_behavior: QuitBehavior::default(),
        }
    }

    /// Register a global keyboard shortcut.
    pub fn on_key<F>(mut self, key: KeyCode, handler: F) -> Self
    where
        F: Fn() -> M + Send + Sync + 'static,
    {
        self.global_key_handlers.insert(key, Arc::new(handler));
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

    /// Run the application in full-screen mode.
    pub fn run<F, V, W>(self, update: F, view: V) -> WidgetResult<()>
    where
        F: Fn(&mut State, M) + Send + Sync + 'static,
        V: Fn(&State) -> W + Send + Sync + 'static,
        W: Widget<M> + 'static,
        State: Send + Sync + 'static,
    {
        let view_fn = move |state: &State| -> Box<dyn Widget<M>> { Box::new(view(state)) };
        self.run_with_options(update, view_fn, false)
    }

    /// Run the application in inline (non-fullscreen) mode.
    pub fn run_inline<F, V, W>(self, update: F, view: V) -> WidgetResult<()>
    where
        F: Fn(&mut State, M) + Send + Sync + 'static,
        V: Fn(&State) -> W + Send + Sync + 'static,
        W: Widget<M> + 'static,
        State: Send + Sync + 'static,
    {
        let view_fn = move |state: &State| -> Box<dyn Widget<M>> { Box::new(view(state)) };
        self.run_with_options(update, view_fn, true)
    }

    fn run_with_options<F, V>(self, update: F, view: V, inline_mode: bool) -> WidgetResult<()>
    where
        F: Fn(&mut State, M) + Send + Sync + 'static,
        V: Fn(&State) -> Box<dyn Widget<M>> + Send + Sync + 'static,
        State: Send + Sync + 'static,
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

        // Build global key handler map (Box instead of Arc for EventRouter)
        let mut global_handlers: HashMap<KeyCode, Box<dyn Fn() -> M + Send + Sync>> =
            HashMap::new();
        for (key, handler) in &self.global_key_handlers {
            let h = handler.clone();
            global_handlers.insert(*key, Box::new(move || h()));
        }

        let runtime = AppRuntime {
            state: self.state,
            update_fn: update,
            view_fn: view,
            store: WidgetStore::new(),
            focus: FocusManager::new(),
            cached_tree: None,
            messages: Vec::new(),
            should_quit: false,
            quit_behavior: self.quit_behavior,
            global_key_handlers: global_handlers,
            inline_mode,
            inline_max_height,
        };

        let mut builder = render::Builder::new();
        builder
            .enable_raw_mode()
            .clear(false)
            .append_newline(false)
            .enable_hide_cursor()
            .disable_exit_code();

        if inline_mode {
            builder
                .inline_mode(true)
                .inline_max_height(buffer_height)
                .frame_limit(60)
                .size((width, buffer_height));
        } else {
            builder
                .enable_all()
                .frame_limit(60)
                .size((width, height));
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

struct AppRuntime<State, F, V, M> {
    state: State,
    update_fn: F,
    view_fn: V,
    store: WidgetStore,
    focus: FocusManager,
    cached_tree: Option<Box<dyn Widget<M>>>,
    messages: Vec<M>,
    should_quit: bool,
    quit_behavior: QuitBehavior,
    global_key_handlers: HashMap<KeyCode, Box<dyn Fn() -> M + Send + Sync>>,
    inline_mode: bool,
    inline_max_height: u16,
}

impl<State, F, V, M> Draw for AppRuntime<State, F, V, M>
where
    F: Fn(&mut State, M),
    V: Fn(&State) -> Box<dyn Widget<M>>,
    M: Clone + std::fmt::Debug,
{
    fn draw(&mut self, mut chunk: Chunk) -> Result<Size, DrawErr> {
        let size = chunk.area().size();

        // Rebuild widget tree from current state
        let tree = (self.view_fn)(&self.state);

        // Rebuild focus chain
        self.focus.rebuild(tree.as_ref());

        // Render
        let focused_path = self.focus.current_path();
        let ctx = RenderCtx::new(&self.store, focused_path);
        tree.render(&mut chunk, &ctx);

        // Cache tree for event handling in on_events()
        self.cached_tree = Some(tree);

        Ok(size)
    }
}

impl<State, F, V, M> Update for AppRuntime<State, F, V, M>
where
    F: Fn(&mut State, M),
    V: Fn(&State) -> Box<dyn Widget<M>>,
    M: Clone + std::fmt::Debug + 'static,
{
    fn on_events(&mut self, events: &[Event]) -> Result<(), DrawErr> {
        // Reset TextInputState.modified_this_batch so we sync from parent when value differs
        self.store
            .for_each_state_mut::<TextInputState, _>(|_, s| s.modified_this_batch = false);

        // Ensure we have a tree (may be missing on first frame before draw)
        if self.cached_tree.is_none() {
            let tree = (self.view_fn)(&self.state);
            self.focus.rebuild(tree.as_ref());
            self.cached_tree = Some(tree);
        }

        let tree = self.cached_tree.as_ref().unwrap();

        for event in events {
            if let Event::Resize(_, _) = event {
                continue;
            }

            // Only handle keyboard events
            let key_event = match event {
                Event::Key(k) => k,
                _ => continue,
            };

            // 1. Tab / Shift+Tab → focus navigation
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

            // 3. Global key handlers
            if let Some(handler) = self.global_key_handlers.get(&key_event.code) {
                self.messages.push(handler());
                continue;
            }

            // 4. Route to focused widget
            if let Some(focus_path) = self.focus.current_path() {
                let focus_path = focus_path.to_vec();
                // Navigate tree to the focused widget
                let mut widget: &dyn Widget<M> = tree.as_ref();
                let mut valid = true;
                for &idx in &focus_path {
                    let children = widget.children();
                    if let Some(child) = children.get(idx) {
                        widget = child.as_ref();
                    } else {
                        valid = false;
                        break;
                    }
                }

                if valid {
                    let mut messages = Vec::new();
                    {
                        let mut ctx =
                            EventCtx::new(&mut self.store, &mut messages, focus_path);
                        widget.handle_event(event, &mut ctx);
                    }
                    self.messages.extend(messages);
                }
            }
        }

        Ok(())
    }

    fn update(&mut self) -> Result<bool, DrawErr> {
        let had_messages = !self.messages.is_empty();
        for msg in self.messages.drain(..) {
            (self.update_fn)(&mut self.state, msg);
        }

        if had_messages {
            // Invalidate cached tree so draw() rebuilds it
            self.cached_tree = None;
        }

        // Always return true to keep the render loop running
        Ok(true)
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
