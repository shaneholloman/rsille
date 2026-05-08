//! Generic animation wrapper widgets.

use render::area::Area;

use crate::animation::{AnimationSpec, ClipMode, LayoutTransition, Presence, Transition};
use crate::focus::FocusConfig;
use crate::layout::Constraints;
use crate::widget::{IntoWidget, RenderCtx, Widget, WidgetKey};

/// Wrap a widget with framework-level animation declarations.
pub fn animate<M>(child: impl IntoWidget<M>) -> Animated<M> {
    Animated::new(child)
}

/// Generic animation wrapper.
///
/// The wrapper keeps the child identity stable under its own path and can
/// animate the area passed to the child during render. Presence declarations
/// are stored here so lifecycle-aware runtimes can honor the same widget API.
pub struct Animated<M = ()> {
    child: Box<dyn Widget<M>>,
    layout_transition: Option<LayoutTransition>,
    presence: Presence,
    widget_key: Option<String>,
}

impl<M> std::fmt::Debug for Animated<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Animated")
            .field("layout_transition", &self.layout_transition)
            .field("presence", &self.presence)
            .finish()
    }
}

impl<M> Animated<M> {
    pub fn new(child: impl IntoWidget<M>) -> Self {
        Self {
            child: child.into_widget(),
            layout_transition: None,
            presence: Presence::default(),
            widget_key: None,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn layout(mut self, spec: AnimationSpec) -> Self {
        self.layout_transition = Some(LayoutTransition::size_and_position(spec));
        self
    }

    pub fn layout_transition(mut self, transition: LayoutTransition) -> Self {
        self.layout_transition = Some(transition);
        self
    }

    pub fn enter(mut self, transition: Transition) -> Self {
        self.presence = self.presence.enter(transition);
        self
    }

    pub fn exit(mut self, transition: Transition) -> Self {
        self.presence = self.presence.exit(transition);
        self
    }

    pub fn presence(mut self, presence: Presence) -> Self {
        self.presence = presence;
        self
    }

    pub fn presence_config(&self) -> &Presence {
        &self.presence
    }

    fn render_child(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let child_ctx = ctx.child_ctx(WidgetKey::for_child(0, self.child.as_ref()));
        self.child.render(chunk, &child_ctx);
    }

    fn render_with_area(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx, area: Area) {
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        if let Ok(mut child_chunk) = chunk.from_area(area) {
            self.render_child(&mut child_chunk, ctx);
        }
    }
}

impl<M> Widget<M> for Animated<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        let target = chunk.area();
        if target.width() == 0 || target.height() == 0 {
            return;
        }

        ctx.record_bounds(target);

        let Some(transition) = self.layout_transition else {
            self.render_child(chunk, ctx);
            return;
        };

        let displayed = ctx.track_layout("layout", target, transition);
        match transition.clip {
            ClipMode::None => self.render_with_area(chunk, ctx, displayed),
            ClipMode::ClipToAnimatedBounds => {
                let _ =
                    chunk.with_clip(displayed, |child_chunk| self.render_child(child_chunk, ctx));
            }
            ClipMode::ClipToTargetBounds => {
                if let Some(clipped) = displayed.clamp_to(&target) {
                    let _ =
                        chunk.with_clip(clipped, |child_chunk| self.render_child(child_chunk, ctx));
                }
            }
        }
    }

    fn constraints(&self) -> Constraints {
        self.child.constraints()
    }

    fn focus_config(&self) -> FocusConfig {
        FocusConfig::None
    }

    fn children(&self) -> &[Box<dyn Widget<M>>] {
        std::slice::from_ref(&self.child)
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}
