use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

use render::area::Area;
use render::chunk::Chunk;
use smallvec::SmallVec;

use crate::animation::{
    AnimationCtx, AnimationStore, ClipMode, LayoutSnapshot, LayoutTransition, MotionPolicy,
    Presence, SharedTransition, Timeline, TimelineFrame,
};
use crate::event::Event;
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::style::Style;
use crate::style::Theme;

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
pub trait Widget<M> {
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

    /// Advance component-level animation state for this widget.
    ///
    /// Return `true` when the animation changed a value or needs another frame.
    fn animate(&self, _ctx: &mut AnimationCtx) -> bool {
        false
    }

    /// Presence declaration used by the runtime for enter/exit lifecycle animation.
    fn presence(&self) -> Option<&Presence> {
        None
    }

    /// Layout transition declaration used by parent layout containers.
    fn layout_transition(&self) -> Option<LayoutTransition> {
        None
    }

    /// Shared layout transition declaration used across widget paths.
    fn shared_transition(&self) -> Option<&SharedTransition> {
        None
    }

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

    fn animate(&self, ctx: &mut AnimationCtx) -> bool {
        (**self).animate(ctx)
    }

    fn presence(&self) -> Option<&Presence> {
        (**self).presence()
    }

    fn layout_transition(&self) -> Option<LayoutTransition> {
        (**self).layout_transition()
    }

    fn shared_transition(&self) -> Option<&SharedTransition> {
        (**self).shared_transition()
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
/// Provides read-only access to the [`WidgetStore`], focus information,
/// and the current render theme.
#[derive(Clone, Copy)]
enum AnimationStoreView<'a> {
    ReadOnly(&'a AnimationStore),
    Mutable(&'a RefCell<AnimationStore>),
}

impl AnimationStoreView<'_> {
    fn value(self, path: &WidgetPath, channel: &str) -> Option<f64> {
        match self {
            Self::ReadOnly(store) => store.value(path, channel),
            Self::Mutable(store) => store.borrow().value(path, channel),
        }
    }

    fn style(self, path: &WidgetPath, channel: &str) -> Option<Style> {
        match self {
            Self::ReadOnly(store) => store.style(path, channel),
            Self::Mutable(store) => store.borrow().style(path, channel),
        }
    }

    fn layout_snapshot(self, path: &WidgetPath, channel: &str) -> Option<LayoutSnapshot> {
        match self {
            Self::ReadOnly(store) => store.layout_snapshot(path, channel),
            Self::Mutable(store) => store.borrow().layout_snapshot(path, channel),
        }
    }

    fn shared_layout_snapshot(self, id: &str) -> Option<LayoutSnapshot> {
        match self {
            Self::ReadOnly(store) => store.shared_layout_snapshot(id),
            Self::Mutable(store) => store.borrow().shared_layout_snapshot(id),
        }
    }

    fn track_timeline(
        self,
        path: &WidgetPath,
        channel: &str,
        timeline: Timeline,
        restart: bool,
        now: std::time::Instant,
        motion_policy: MotionPolicy,
    ) -> Vec<TimelineFrame> {
        match self {
            Self::ReadOnly(store) => store.timeline_frames(path, channel, now),
            Self::Mutable(store) => {
                store
                    .borrow_mut()
                    .track_timeline(path, channel, timeline, restart, now, motion_policy)
                    .0
            }
        }
    }
}

pub struct RenderCtx<'a> {
    store: &'a WidgetStore,
    animation_store: AnimationStoreView<'a>,
    theme: &'a Theme,
    focused_path: Option<&'a WidgetPath>,
    current_path: WidgetPath,
    geometry: &'a RefCell<HashMap<WidgetPath, Area>>,
    now: std::time::Instant,
    motion_policy: MotionPolicy,
    layout_target: Option<Area>,
    layout_managed: bool,
}

impl<'a> RenderCtx<'a> {
    pub fn new(
        store: &'a WidgetStore,
        animation_store: &'a AnimationStore,
        theme: &'a Theme,
        focused_path: Option<&'a WidgetPath>,
        geometry: &'a RefCell<HashMap<WidgetPath, Area>>,
    ) -> Self {
        Self {
            store,
            animation_store: AnimationStoreView::ReadOnly(animation_store),
            theme,
            focused_path,
            current_path: WidgetPath::root(),
            geometry,
            now: std::time::Instant::now(),
            motion_policy: MotionPolicy::default(),
            layout_target: None,
            layout_managed: false,
        }
    }

    pub(crate) fn with_runtime(
        store: &'a WidgetStore,
        animation_store: &'a RefCell<AnimationStore>,
        theme: &'a Theme,
        focused_path: Option<&'a WidgetPath>,
        geometry: &'a RefCell<HashMap<WidgetPath, Area>>,
        now: std::time::Instant,
        motion_policy: MotionPolicy,
    ) -> Self {
        Self {
            store,
            animation_store: AnimationStoreView::Mutable(animation_store),
            theme,
            focused_path,
            current_path: WidgetPath::root(),
            geometry,
            now,
            motion_policy,
            layout_target: None,
            layout_managed: false,
        }
    }

    /// The theme for the current render pass.
    pub fn theme(&self) -> &Theme {
        self.theme
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
    pub fn state<T: Default + 'static>(&self) -> Option<&T> {
        self.store.get::<T>(&self.current_path)
    }

    /// Read the current animation value for this widget and channel.
    pub fn animation_value(&self, channel: &str) -> Option<f64> {
        self.animation_store.value(&self.current_path, channel)
    }

    /// Read the current layout animation snapshot for this widget and channel.
    pub fn layout_animation(&self, channel: &str) -> Option<LayoutSnapshot> {
        self.animation_store
            .layout_snapshot(&self.current_path, channel)
    }

    /// Read the current shared layout animation snapshot by shared id.
    pub fn shared_layout_animation(&self, id: &str) -> Option<LayoutSnapshot> {
        self.animation_store.shared_layout_snapshot(id)
    }

    /// Read the current style animation value for this widget and channel.
    pub fn animation_style(&self, channel: &str) -> Option<Style> {
        self.animation_store.style(&self.current_path, channel)
    }

    /// Track a target area and return the animated display area.
    pub fn track_layout(&self, channel: &str, target: Area, transition: LayoutTransition) -> Area {
        let AnimationStoreView::Mutable(store) = self.animation_store else {
            return target;
        };

        let (displayed, _) = store.borrow_mut().track_layout(
            &self.current_path,
            channel,
            target,
            transition,
            self.now,
            self.motion_policy,
        );
        displayed
    }

    /// Track a shared target area and return the animated display area.
    pub fn track_shared_layout(
        &self,
        id: &str,
        target: Area,
        transition: LayoutTransition,
    ) -> Area {
        let AnimationStoreView::Mutable(store) = self.animation_store else {
            return target;
        };

        let (displayed, _) = store.borrow_mut().track_shared_layout(
            id,
            &self.current_path,
            target,
            transition,
            self.now,
            self.motion_policy,
        );
        displayed
    }

    /// Track a timeline and return transition frames active at the current render instant.
    pub fn track_timeline(
        &self,
        channel: &str,
        timeline: Timeline,
        restart: bool,
    ) -> Vec<TimelineFrame> {
        self.animation_store.track_timeline(
            &self.current_path,
            channel,
            timeline,
            restart,
            self.now,
            self.motion_policy,
        )
    }

    /// Read persistent state, or return a default reference if absent.
    /// This is a convenience that avoids `unwrap_or` at every call-site
    /// by falling back to a leaked static default. Use sparingly.
    pub fn state_or_default<T: Default + 'static>(&self) -> &T {
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
            animation_store: self.animation_store,
            theme: self.theme,
            focused_path: self.focused_path,
            current_path: self.current_path.child(key),
            geometry: self.geometry,
            now: self.now,
            motion_policy: self.motion_policy,
            layout_target: None,
            layout_managed: false,
        }
    }

    pub(crate) fn child_ctx_with_layout(
        &self,
        key: impl Into<WidgetKey>,
        target: Area,
    ) -> RenderCtx<'a> {
        RenderCtx {
            store: self.store,
            animation_store: self.animation_store,
            theme: self.theme,
            focused_path: self.focused_path,
            current_path: self.current_path.child(key),
            geometry: self.geometry,
            now: self.now,
            motion_policy: self.motion_policy,
            layout_target: Some(target),
            layout_managed: true,
        }
    }

    /// Return the logical target area when a parent container has already
    /// applied a layout transition for this widget.
    pub fn layout_target(&self) -> Option<Area> {
        self.layout_target
    }

    pub(crate) fn layout_is_managed(&self) -> bool {
        self.layout_managed
    }

    pub(crate) fn prepare_child_layout<M>(
        &self,
        key: WidgetKey,
        child: &dyn Widget<M>,
        target: Area,
    ) -> (Area, RenderCtx<'a>, ClipMode) {
        let path = self.current_path.child(key.clone());
        self.geometry.borrow_mut().insert(path.clone(), target);

        if let Some(shared) = child.shared_transition() {
            let display = match self.animation_store {
                AnimationStoreView::ReadOnly(_) => target,
                AnimationStoreView::Mutable(store) => {
                    store
                        .borrow_mut()
                        .track_shared_layout(
                            &shared.id,
                            &path,
                            target,
                            shared.layout,
                            self.now,
                            self.motion_policy,
                        )
                        .0
                }
            };
            return (
                display,
                self.child_ctx_with_layout(key, target),
                shared.layout.clip,
            );
        }

        if let Some(transition) = child.layout_transition() {
            let display = match self.animation_store {
                AnimationStoreView::ReadOnly(_) => target,
                AnimationStoreView::Mutable(store) => {
                    store
                        .borrow_mut()
                        .track_layout(
                            &path,
                            "layout",
                            target,
                            transition,
                            self.now,
                            self.motion_policy,
                        )
                        .0
                }
            };
            return (
                display,
                self.child_ctx_with_layout(key, target),
                transition.clip,
            );
        }

        (target, self.child_ctx(key), ClipMode::None)
    }

    pub(crate) fn render_child_at<M>(
        &self,
        chunk: &mut Chunk,
        key: WidgetKey,
        child: &dyn Widget<M>,
        target: Area,
    ) {
        let (display, child_ctx, clip) = self.prepare_child_layout(key, child, target);
        if display.width() == 0 || display.height() == 0 {
            return;
        }

        let render_area = match clip {
            ClipMode::None | ClipMode::ClipToAnimatedBounds => display,
            ClipMode::ClipToTargetBounds => {
                let Some(clipped) = display.clamp_to(&target) else {
                    return;
                };
                clipped
            }
        };

        let _ = chunk.with_clip(render_area, |child_chunk| {
            child.render(child_chunk, &child_ctx);
        });
    }

    /// The current widget path in the tree.
    pub fn path(&self) -> &WidgetPath {
        &self.current_path
    }

    /// Record the rendered bounds for the current widget.
    pub fn record_bounds(&self, area: Area) {
        self.geometry
            .borrow_mut()
            .insert(self.current_path.clone(), area);
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
    geometry: &'a HashMap<WidgetPath, Area>,
    phase: EventPhase,
    already_handled: bool,
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
        geometry: &'a HashMap<WidgetPath, Area>,
        phase: EventPhase,
        already_handled: bool,
    ) -> Self {
        Self {
            store,
            messages,
            path,
            focused_path,
            geometry,
            phase,
            already_handled,
            handled: false,
            stop_propagation: false,
            focus_request: None,
        }
    }

    /// Get or create mutable persistent state for this widget.
    pub fn state_mut<T: Default + 'static>(&mut self) -> &mut T {
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

    /// Whether an earlier handler in the current routing pass already handled this event.
    pub fn was_handled(&self) -> bool {
        self.already_handled
    }

    /// The last rendered bounds for the current widget.
    pub fn bounds(&self) -> Option<Area> {
        self.geometry.get(&self.path).copied()
    }

    /// The last rendered bounds for an arbitrary widget path.
    pub fn bounds_for(&self, path: &WidgetPath) -> Option<Area> {
        self.geometry.get(path).copied()
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
    states: HashMap<WidgetPath, Box<dyn Any>>,
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
    pub fn get_or_default<T: Default + 'static>(&mut self, path: WidgetPath) -> &mut T {
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
