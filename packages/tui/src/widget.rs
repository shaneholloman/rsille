use std::any::Any;
use std::collections::HashMap;
use std::fmt;

use render::chunk::Chunk;
use smallvec::SmallVec;

use crate::event::Event;
use crate::focus::FocusConfig;
use crate::layout::Constraints;

// ---------------------------------------------------------------------------
// WidgetKey & WidgetPath – stable widget identity
// ---------------------------------------------------------------------------

/// A single segment in a widget path.
///
/// Widgets can be identified by their positional index (default) or by a
/// user-assigned stable name. Named keys survive reordering of siblings,
/// making persistent state (scroll offsets, cursor positions, …) robust
/// against dynamic list changes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidgetKey {
    Index(usize),
    Named(String),
}

impl WidgetKey {
    /// Determine the key for a child widget: use the widget's own key if set,
    /// otherwise fall back to its positional index.
    pub fn for_child<M>(index: usize, widget: &dyn Widget<M>) -> Self {
        match widget.key() {
            Some(name) => WidgetKey::Named(name.to_owned()),
            None => WidgetKey::Index(index),
        }
    }
}

impl From<usize> for WidgetKey {
    fn from(i: usize) -> Self {
        WidgetKey::Index(i)
    }
}

impl From<&str> for WidgetKey {
    fn from(s: &str) -> Self {
        WidgetKey::Named(s.to_owned())
    }
}

impl From<String> for WidgetKey {
    fn from(s: String) -> Self {
        WidgetKey::Named(s)
    }
}

impl fmt::Display for WidgetKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WidgetKey::Index(i) => write!(f, "{i}"),
            WidgetKey::Named(s) => write!(f, "\"{s}\""),
        }
    }
}

/// Path from the root of the widget tree to a specific widget.
///
/// Each segment is a [`WidgetKey`] identifying the child at that level.
/// Uses `SmallVec` to avoid heap allocation for typical tree depths (≤ 8).
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct WidgetPath(SmallVec<[WidgetKey; 8]>);

impl WidgetPath {
    pub fn root() -> Self {
        Self(SmallVec::new())
    }

    pub fn child(&self, key: impl Into<WidgetKey>) -> Self {
        let mut p = self.0.clone();
        p.push(key.into());
        Self(p)
    }

    pub fn push(&mut self, key: impl Into<WidgetKey>) {
        self.0.push(key.into());
    }

    pub fn pop(&mut self) -> Option<WidgetKey> {
        self.0.pop()
    }

    pub fn as_slice(&self) -> &[WidgetKey] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn starts_with(&self, prefix: &WidgetPath) -> bool {
        self.0.starts_with(prefix.as_slice())
    }

    pub fn ancestors_inclusive(&self) -> Vec<WidgetPath> {
        let mut ancestors = Vec::with_capacity(self.len() + 1);
        ancestors.push(WidgetPath::root());

        let mut path = WidgetPath::root();
        for key in self.as_slice() {
            path.push(key.clone());
            ancestors.push(path.clone());
        }

        ancestors
    }
}

impl fmt::Debug for WidgetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WidgetPath[")?;
        for (i, key) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " → ")?;
            }
            write!(f, "{key}")?;
        }
        write!(f, "]")
    }
}

impl fmt::Display for WidgetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, key) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            write!(f, "{key}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------

/// Core widget trait that all TUI components implement.
///
/// Widgets are **immutable descriptions** of UI. All mutable state lives in
/// [`WidgetStore`] (accessed via contexts), not inside the widget itself.
///
/// The generic parameter `M` is the message type for the application.
pub trait Widget<M>: Send + Sync {
    /// Render the widget into the provided chunk.
    ///
    /// Read persistent state from `ctx.state::<T>()` and focus info from
    /// `ctx.is_focused()`. The widget draws at relative coordinates within the chunk.
    fn render(&self, chunk: &mut Chunk, ctx: &RenderCtx);

    /// Handle an event routed to this widget by the framework.
    ///
    /// Widgets receive events during capture, target, and bubble phases.
    /// Write persistent state via `ctx.state_mut::<T>()`, emit messages via
    /// `ctx.emit(msg)`, and request focus changes via the focus helpers.
    fn handle_event(&self, _event: &Event, _ctx: &mut EventCtx<M>) {}

    /// Return size constraints for layout computation.
    fn constraints(&self) -> Constraints;

    /// How this widget participates in keyboard focus.
    fn focus_config(&self) -> FocusConfig {
        FocusConfig::None
    }

    /// Return child widgets (for containers like Flex/Grid).
    /// Leaf widgets return the default empty slice.
    fn children(&self) -> &[Box<dyn Widget<M>>] {
        &[]
    }

    /// Return a stable key for this widget.
    ///
    /// When set, the framework uses this key (instead of the positional index)
    /// to identify the widget in the tree. This makes persistent state survive
    /// sibling reordering — useful for dynamic lists.
    fn key(&self) -> Option<&str> {
        None
    }
}

impl<M> Widget<M> for Box<dyn Widget<M>> {
    fn render(&self, chunk: &mut Chunk, ctx: &RenderCtx) {
        (**self).render(chunk, ctx)
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        (**self).handle_event(event, ctx)
    }

    fn constraints(&self) -> Constraints {
        (**self).constraints()
    }

    fn focus_config(&self) -> FocusConfig {
        (**self).focus_config()
    }

    fn children(&self) -> &[Box<dyn Widget<M>>] {
        (**self).children()
    }

    fn key(&self) -> Option<&str> {
        (**self).key()
    }
}

// ---------------------------------------------------------------------------
// RenderCtx – immutable context passed during rendering
// ---------------------------------------------------------------------------

/// Context passed to [`Widget::render`].
///
/// Provides read-only access to the [`WidgetStore`] and focus information.
pub struct RenderCtx<'a> {
    store: &'a WidgetStore,
    focused_path: Option<&'a WidgetPath>,
    current_path: WidgetPath,
}

impl<'a> RenderCtx<'a> {
    pub fn new(store: &'a WidgetStore, focused_path: Option<&'a WidgetPath>) -> Self {
        Self {
            store,
            focused_path,
            current_path: WidgetPath::root(),
        }
    }

    /// Whether the current widget has keyboard focus.
    pub fn is_focused(&self) -> bool {
        self.focused_path
            .map(|fp| *fp == self.current_path)
            .unwrap_or(false)
    }

    /// Whether the currently focused widget is inside this subtree.
    pub fn is_focus_within(&self) -> bool {
        self.focused_path
            .map(|fp| fp.starts_with(&self.current_path))
            .unwrap_or(false)
    }

    /// The first descendant key along the focused path, relative to this widget.
    pub fn focused_descendant_key(&self) -> Option<&WidgetKey> {
        let focused = self.focused_path?;
        if !focused.starts_with(&self.current_path) || focused.len() <= self.current_path.len() {
            return None;
        }
        focused.as_slice().get(self.current_path.len())
    }

    /// The globally focused path.
    pub fn focused_path(&self) -> Option<&WidgetPath> {
        self.focused_path
    }

    /// Read persistent state stored for this widget.
    /// Returns `None` if no state has been stored yet.
    pub fn state<T: Default + Send + Sync + 'static>(&self) -> Option<&T> {
        self.store.get::<T>(&self.current_path)
    }

    /// Read persistent state, or return a default reference if absent.
    /// This is a convenience that avoids `unwrap_or` at every call-site
    /// by falling back to a leaked static default. Use sparingly.
    pub fn state_or_default<T: Default + Send + Sync + 'static>(&self) -> &T {
        self.store.get::<T>(&self.current_path).unwrap_or_else(|| {
            // Safe: Default is computed once per type via OnceLock-like pattern
            // We use a thread-local to avoid leaking.
            // For rendering purposes, returning a stack reference is fine because
            // we only need it for the duration of the render call.
            // We use a small trick: store the default in a thread-local.
            thread_local! {
                static DEFAULTS: std::cell::RefCell<HashMap<std::any::TypeId, Box<dyn Any>>> =
                    std::cell::RefCell::new(HashMap::new());
            }
            DEFAULTS.with(|defaults| {
                let mut map = defaults.borrow_mut();
                let entry = map
                    .entry(std::any::TypeId::of::<T>())
                    .or_insert_with(|| Box::new(T::default()));
                // SAFETY: the borrow_mut guard is dropped but the Box lives in
                // thread-local storage for the entire thread lifetime. We extend
                // the lifetime to 'a which is bounded by the render call.
                let ptr: *const T = entry.downcast_ref::<T>().unwrap();
                unsafe { &*ptr }
            })
        })
    }

    /// Create a child context for rendering a child widget.
    ///
    /// The `key` identifies the child — use [`WidgetKey::for_child`] to
    /// automatically pick a named key when the child widget provides one,
    /// falling back to the positional index.
    pub fn child_ctx(&self, key: impl Into<WidgetKey>) -> RenderCtx<'a> {
        RenderCtx {
            store: self.store,
            focused_path: self.focused_path,
            current_path: self.current_path.child(key),
        }
    }

    /// The current widget path in the tree.
    pub fn path(&self) -> &WidgetPath {
        &self.current_path
    }
}

// ---------------------------------------------------------------------------
// EventCtx – mutable context passed during event handling
// ---------------------------------------------------------------------------

/// The current event routing phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Capture,
    Target,
    Bubble,
}

/// Requested focus change emitted by event handlers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRequest {
    Set(WidgetPath),
    Clear,
    Next,
    Prev,
    NextInScope(Option<WidgetPath>),
    PrevInScope(Option<WidgetPath>),
}

/// Final event outcome collected after a widget has handled an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventOutcome {
    pub handled: bool,
    pub stop_propagation: bool,
    pub focus_request: Option<FocusRequest>,
}

/// Context passed to [`Widget::handle_event`].
///
/// Provides mutable access to the [`WidgetStore`] and message collection.
pub struct EventCtx<'a, M> {
    store: &'a mut WidgetStore,
    messages: &'a mut Vec<M>,
    path: WidgetPath,
    focused_path: Option<WidgetPath>,
    phase: EventPhase,
    handled: bool,
    stop_propagation: bool,
    focus_request: Option<FocusRequest>,
}

impl<'a, M> EventCtx<'a, M> {
    pub fn new(
        store: &'a mut WidgetStore,
        messages: &'a mut Vec<M>,
        path: WidgetPath,
        focused_path: Option<WidgetPath>,
        phase: EventPhase,
    ) -> Self {
        Self {
            store,
            messages,
            path,
            focused_path,
            phase,
            handled: false,
            stop_propagation: false,
            focus_request: None,
        }
    }

    /// Get or create mutable persistent state for this widget.
    pub fn state_mut<T: Default + Send + Sync + 'static>(&mut self) -> &mut T {
        self.store.get_or_default::<T>(self.path.clone())
    }

    /// Emit a message that will be delivered to the application's `update` function.
    pub fn emit(&mut self, message: M) {
        self.messages.push(message);
        self.handled = true;
    }

    /// The current widget path in the tree.
    pub fn path(&self) -> &WidgetPath {
        &self.path
    }

    /// The globally focused path.
    pub fn focused_path(&self) -> Option<&WidgetPath> {
        self.focused_path.as_ref()
    }

    /// The current routing phase.
    pub fn phase(&self) -> EventPhase {
        self.phase
    }

    /// Mark the event as handled.
    pub fn set_handled(&mut self) {
        self.handled = true;
    }

    /// Stop event propagation after the current handler returns.
    pub fn stop_propagation(&mut self) {
        self.handled = true;
        self.stop_propagation = true;
    }

    /// Request focus for the current widget.
    pub fn request_focus_self(&mut self) {
        self.request_focus(self.path.clone());
    }

    /// Request focus for a specific widget path.
    pub fn request_focus(&mut self, path: WidgetPath) {
        self.handled = true;
        self.focus_request = Some(FocusRequest::Set(path));
    }

    /// Clear the current focus.
    pub fn clear_focus(&mut self) {
        self.handled = true;
        self.focus_request = Some(FocusRequest::Clear);
    }

    /// Move focus to the next global focus target.
    pub fn focus_next(&mut self) {
        self.handled = true;
        self.focus_request = Some(FocusRequest::Next);
    }

    /// Move focus to the previous global focus target.
    pub fn focus_prev(&mut self) {
        self.handled = true;
        self.focus_request = Some(FocusRequest::Prev);
    }

    /// Move focus to the next target within the given scope.
    pub fn focus_next_in_scope(&mut self, scope: Option<WidgetPath>) {
        self.handled = true;
        self.focus_request = Some(FocusRequest::NextInScope(scope));
    }

    /// Move focus to the previous target within the given scope.
    pub fn focus_prev_in_scope(&mut self, scope: Option<WidgetPath>) {
        self.handled = true;
        self.focus_request = Some(FocusRequest::PrevInScope(scope));
    }

    pub fn finish(self) -> EventOutcome {
        EventOutcome {
            handled: self.handled,
            stop_propagation: self.stop_propagation,
            focus_request: self.focus_request,
        }
    }
}

// ---------------------------------------------------------------------------
// WidgetStore – persistent state store across view() rebuilds
// ---------------------------------------------------------------------------

/// Stores widget-internal persistent state (cursor positions, scroll offsets, etc.)
/// keyed by the widget's [`WidgetPath`] in the tree.
///
/// State survives `view()` rebuilds because the store lives in the [`AppRuntime`],
/// not inside widget instances.
#[derive(Default)]
pub struct WidgetStore {
    states: HashMap<WidgetPath, Box<dyn Any + Send + Sync>>,
}

impl WidgetStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read state for the given path. Returns `None` if no state is stored.
    pub fn get<T: 'static>(&self, path: &WidgetPath) -> Option<&T> {
        self.states.get(path)?.downcast_ref()
    }

    /// Get or insert a default state for the given path.
    pub fn get_or_default<T: Default + Send + Sync + 'static>(
        &mut self,
        path: WidgetPath,
    ) -> &mut T {
        self.states
            .entry(path)
            .or_insert_with(|| Box::new(T::default()))
            .downcast_mut()
            .expect("WidgetStore type mismatch: a different type was stored at this path")
    }

    /// Apply a function to all states of type T. Used for frame-level resets.
    pub fn for_each_state_mut<T: 'static, F>(&mut self, mut f: F)
    where
        F: FnMut(&WidgetPath, &mut T),
    {
        for (path, state) in self.states.iter_mut() {
            if let Some(s) = state.downcast_mut::<T>() {
                f(path, s);
            }
        }
    }

    /// Remove entries whose paths are not in the active set.
    /// Called after building the widget tree to clean up stale state.
    pub fn retain_active<F>(&mut self, mut is_active: F)
    where
        F: FnMut(&WidgetPath) -> bool,
    {
        self.states.retain(|path, _| is_active(path));
    }
}

// ---------------------------------------------------------------------------
// IntoWidget – ergonomic conversion for the builder API
// ---------------------------------------------------------------------------

/// Trait for converting types into boxed widgets, used by container `.child()` methods.
pub trait IntoWidget<M> {
    fn into_widget(self) -> Box<dyn Widget<M>>;
}

impl<M, W: Widget<M> + 'static> IntoWidget<M> for W {
    fn into_widget(self) -> Box<dyn Widget<M>> {
        Box::new(self)
    }
}
