use rustc_hash::{FxHashMap, FxHashSet};

use crate::widget::{Widget, WidgetKey, WidgetPath};

/// How a widget participates in keyboard focus.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FocusConfig {
    /// The widget does not participate in the focus system.
    #[default]
    None,
    /// A leaf focus target that participates in the global Tab order.
    Leaf,
    /// A composite focus target that owns internal navigation state.
    Composite,
    /// A focus scope that can trap Tab and restore previously focused children.
    Scope(FocusScope),
}

/// Policy for entering a focus scope.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ScopeEntry {
    /// Focus the first focusable descendant.
    #[default]
    First,
    /// Restore the last focused descendant when available.
    LastFocused,
    /// Prefer a specific child path under the scope root.
    Child(WidgetKey),
}

/// Focus behavior for scope widgets such as dialogs or grouped forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusScope {
    /// Whether Tab should cycle inside this scope.
    pub trap_tab: bool,
    /// Whether the scope should remember the last focused descendant.
    pub restore_focus: bool,
    /// Which child should receive focus when entering the scope.
    pub entry: ScopeEntry,
}

impl Default for FocusScope {
    fn default() -> Self {
        Self {
            trap_tab: false,
            restore_focus: true,
            entry: ScopeEntry::First,
        }
    }
}

impl FocusScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trap_tab(mut self, trap_tab: bool) -> Self {
        self.trap_tab = trap_tab;
        self
    }

    pub fn restore_focus(mut self, restore_focus: bool) -> Self {
        self.restore_focus = restore_focus;
        self
    }

    pub fn entry(mut self, entry: ScopeEntry) -> Self {
        self.entry = entry;
        self
    }
}

/// Snapshot of the focus-relevant parts of the current widget tree.
#[derive(Debug, Default, Clone)]
pub struct FocusAnalysis {
    live_paths: FxHashSet<WidgetPath>,
    focus_targets: Vec<WidgetPath>,
    focus_target_set: FxHashSet<WidgetPath>,
    scopes: FxHashMap<WidgetPath, FocusScope>,
}

impl FocusAnalysis {
    pub fn analyze<M>(root: &dyn Widget<M>) -> Self {
        let mut analysis = Self::default();
        let mut path = WidgetPath::root();
        Self::walk(root, &mut path, &mut analysis);
        analysis
    }

    fn walk<M>(widget: &dyn Widget<M>, path: &mut WidgetPath, analysis: &mut Self) {
        analysis.live_paths.insert(path.clone());

        match widget.focus_config() {
            FocusConfig::Leaf | FocusConfig::Composite => {
                analysis.focus_target_set.insert(path.clone());
                analysis.focus_targets.push(path.clone());
            }
            FocusConfig::Scope(scope) => {
                analysis.scopes.insert(path.clone(), scope);
            }
            FocusConfig::None => {}
        }

        for (index, child) in widget.children().iter().enumerate() {
            path.push(WidgetKey::for_child(index, child.as_ref()));
            Self::walk(child.as_ref(), path, analysis);
            path.pop();
        }
    }

    pub fn live_paths(&self) -> &FxHashSet<WidgetPath> {
        &self.live_paths
    }

    pub fn focus_targets(&self) -> &[WidgetPath] {
        &self.focus_targets
    }

    pub fn is_live(&self, path: &WidgetPath) -> bool {
        self.live_paths.contains(path)
    }

    pub fn is_focus_target(&self, path: &WidgetPath) -> bool {
        self.focus_target_set.contains(path)
    }

    pub fn scope(&self, path: &WidgetPath) -> Option<&FocusScope> {
        self.scopes.get(path)
    }

    pub fn scopes(&self) -> &FxHashMap<WidgetPath, FocusScope> {
        &self.scopes
    }

    pub fn descendant_targets<'a>(
        &'a self,
        scope_path: &'a WidgetPath,
    ) -> impl Iterator<Item = &'a WidgetPath> + 'a {
        self.focus_targets
            .iter()
            .filter(move |path| path.starts_with(scope_path))
    }

    pub fn first_descendant_target(&self, path: &WidgetPath) -> Option<&WidgetPath> {
        self.focus_targets.iter().find(|candidate| candidate.starts_with(path))
    }
}

/// Manages keyboard focus navigation across focusable widgets and scopes.
#[derive(Debug, Default)]
pub struct FocusManager {
    analysis: FocusAnalysis,
    focused_path: Option<WidgetPath>,
    scope_memory: FxHashMap<WidgetPath, WidgetPath>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rebuild<M>(&mut self, root: &dyn Widget<M>) {
        let old_focus = self.focused_path.clone();
        self.analysis = FocusAnalysis::analyze(root);
        self.prune_scope_memory();

        self.focused_path = old_focus
            .as_ref()
            .and_then(|path| self.resolve_requested_path(path))
            .or_else(|| self.analysis.focus_targets().first().cloned());

        if let Some(focused) = self.focused_path.clone() {
            self.remember_focus(&focused);
        }
    }

    pub fn current_path(&self) -> Option<&WidgetPath> {
        self.focused_path.as_ref()
    }

    pub fn live_paths(&self) -> &FxHashSet<WidgetPath> {
        self.analysis.live_paths()
    }

    pub fn request_focus(&mut self, path: &WidgetPath) -> bool {
        let next = self.resolve_requested_path(path);
        self.set_focus(next)
    }

    pub fn clear(&mut self) -> bool {
        self.set_focus(None)
    }

    pub fn next(&mut self) {
        if let Some(scope) = self.active_trapping_scope() {
            self.move_within_scope(&scope, true);
        } else {
            self.move_global(true);
        }
    }

    pub fn prev(&mut self) {
        if let Some(scope) = self.active_trapping_scope() {
            self.move_within_scope(&scope, false);
        } else {
            self.move_global(false);
        }
    }

    pub fn next_in_scope(&mut self, scope: Option<&WidgetPath>) {
        if let Some(scope_path) = self.resolve_scope_path(scope) {
            self.move_within_scope(&scope_path, true);
        } else {
            self.move_global(true);
        }
    }

    pub fn prev_in_scope(&mut self, scope: Option<&WidgetPath>) {
        if let Some(scope_path) = self.resolve_scope_path(scope) {
            self.move_within_scope(&scope_path, false);
        } else {
            self.move_global(false);
        }
    }

    pub fn is_focus_within(&self, path: &WidgetPath) -> bool {
        self.focused_path
            .as_ref()
            .map(|focused| focused.starts_with(path))
            .unwrap_or(false)
    }

    fn resolve_requested_path(&self, path: &WidgetPath) -> Option<WidgetPath> {
        if self.analysis.is_focus_target(path) {
            return Some(path.clone());
        }

        if self.analysis.scope(path).is_some() {
            return self.resolve_scope_entry(path);
        }

        if self.analysis.is_live(path) {
            return self.analysis.first_descendant_target(path).cloned();
        }

        None
    }

    fn resolve_scope_entry(&self, scope_path: &WidgetPath) -> Option<WidgetPath> {
        let scope = self.analysis.scope(scope_path)?;

        if matches!(scope.entry, ScopeEntry::LastFocused) {
            if let Some(remembered) = self.scope_memory.get(scope_path) {
                if self.analysis.is_focus_target(remembered) && remembered.starts_with(scope_path)
                {
                    return Some(remembered.clone());
                }
            }
        }

        if let ScopeEntry::Child(ref key) = scope.entry {
            let requested = scope_path.child(key.clone());
            if let Some(target) = self.resolve_requested_path(&requested) {
                if target.starts_with(scope_path) {
                    return Some(target);
                }
            }
        }

        if matches!(scope.entry, ScopeEntry::First) {
            if let Some(target) = self.analysis.first_descendant_target(scope_path) {
                return Some(target.clone());
            }
        }

        if scope.restore_focus {
            if let Some(remembered) = self.scope_memory.get(scope_path) {
                if self.analysis.is_focus_target(remembered) && remembered.starts_with(scope_path)
                {
                    return Some(remembered.clone());
                }
            }
        }

        self.analysis.first_descendant_target(scope_path).cloned()
    }

    fn set_focus(&mut self, next: Option<WidgetPath>) -> bool {
        if self.focused_path == next {
            return false;
        }
        self.focused_path = next;
        if let Some(focused) = self.focused_path.clone() {
            self.remember_focus(&focused);
        }
        true
    }

    fn move_global(&mut self, forward: bool) {
        let targets = self.analysis.focus_targets();
        if targets.is_empty() {
            return;
        }

        let next_index = match self
            .focused_path
            .as_ref()
            .and_then(|path| targets.iter().position(|candidate| candidate == path))
        {
            Some(index) if forward => (index + 1) % targets.len(),
            Some(0) => targets.len() - 1,
            Some(index) => index - 1,
            None if forward => 0,
            None => targets.len() - 1,
        };

        self.set_focus(Some(targets[next_index].clone()));
    }

    fn move_within_scope(&mut self, scope_path: &WidgetPath, forward: bool) {
        let targets: Vec<&WidgetPath> = self.analysis.descendant_targets(scope_path).collect();
        if targets.is_empty() {
            return;
        }

        let next_index = match self
            .focused_path
            .as_ref()
            .and_then(|path| targets.iter().position(|candidate| *candidate == path))
        {
            Some(index) if forward => (index + 1) % targets.len(),
            Some(0) => targets.len() - 1,
            Some(index) => index - 1,
            None if forward => 0,
            None => targets.len() - 1,
        };

        self.set_focus(Some(targets[next_index].clone()));
    }

    fn resolve_scope_path(&self, requested: Option<&WidgetPath>) -> Option<WidgetPath> {
        if let Some(path) = requested {
            return self.analysis.scope(path).map(|_| path.clone());
        }

        let focused = self.focused_path.as_ref()?;
        self.ancestor_scope_paths(focused).pop()
    }

    fn active_trapping_scope(&self) -> Option<WidgetPath> {
        let focused = self.focused_path.as_ref()?;
        self.ancestor_scope_paths(focused)
            .into_iter()
            .rev()
            .find(|scope_path| {
                self.analysis
                    .scope(scope_path)
                    .map(|scope| scope.trap_tab)
                    .unwrap_or(false)
            })
    }

    fn ancestor_scope_paths(&self, path: &WidgetPath) -> Vec<WidgetPath> {
        let mut scopes = Vec::new();
        for ancestor in path.ancestors_inclusive() {
            if self.analysis.scope(&ancestor).is_some() {
                scopes.push(ancestor);
            }
        }
        scopes
    }

    fn remember_focus(&mut self, focused: &WidgetPath) {
        for ancestor in focused.ancestors_inclusive() {
            if self.analysis.scope(&ancestor).is_some() {
                self.scope_memory.insert(ancestor, focused.clone());
            }
        }
    }

    fn prune_scope_memory(&mut self) {
        self.scope_memory.retain(|scope_path, focused_path| {
            self.analysis.scope(scope_path).is_some()
                && self.analysis.is_focus_target(focused_path)
                && focused_path.starts_with(scope_path)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{FocusConfig, FocusManager, FocusScope, ScopeEntry};
    use crate::layout::Constraints;
    use crate::widget::{Widget, WidgetKey, WidgetPath};

    struct TestWidget {
        key: Option<&'static str>,
        focus: FocusConfig,
        children: Vec<Box<dyn Widget<()>>>,
    }

    impl TestWidget {
        fn node(key: Option<&'static str>, focus: FocusConfig) -> Self {
            Self {
                key,
                focus,
                children: Vec::new(),
            }
        }

        fn with_children(mut self, children: Vec<Box<dyn Widget<()>>>) -> Self {
            self.children = children;
            self
        }
    }

    impl Widget<()> for TestWidget {
        fn render(&self, _chunk: &mut render::chunk::Chunk, _ctx: &crate::widget::RenderCtx) {}

        fn constraints(&self) -> Constraints {
            Constraints::fixed(1, 1)
        }

        fn focus_config(&self) -> FocusConfig {
            self.focus.clone()
        }

        fn children(&self) -> &[Box<dyn Widget<()>>] {
            &self.children
        }

        fn key(&self) -> Option<&str> {
            self.key
        }
    }

    fn path(parts: &[&str]) -> WidgetPath {
        let mut path = WidgetPath::root();
        for part in parts {
            path.push(WidgetKey::Named((*part).to_owned()));
        }
        path
    }

    #[test]
    fn scope_request_uses_explicit_entry() {
        let tree = TestWidget::node(None, FocusConfig::None).with_children(vec![
            Box::new(
                TestWidget::node(
                    Some("dialog"),
                    FocusConfig::Scope(
                        FocusScope::new()
                            .entry(ScopeEntry::Child(WidgetKey::Named("second".into()))),
                    ),
                )
                .with_children(vec![
                    Box::new(TestWidget::node(Some("first"), FocusConfig::Leaf)),
                    Box::new(TestWidget::node(Some("second"), FocusConfig::Leaf)),
                ]),
            ),
            Box::new(TestWidget::node(Some("outside"), FocusConfig::Leaf)),
        ]);

        let mut focus = FocusManager::new();
        focus.rebuild(&tree);

        assert!(focus.request_focus(&path(&["outside"])));
        assert!(focus.request_focus(&path(&["dialog"])));
        assert_eq!(
            focus.current_path().cloned(),
            Some(path(&["dialog", "second"]))
        );
    }

    #[test]
    fn last_focused_entry_restores_previous_descendant() {
        let tree = TestWidget::node(None, FocusConfig::None).with_children(vec![
            Box::new(
                TestWidget::node(
                    Some("dialog"),
                    FocusConfig::Scope(
                        FocusScope::new()
                            .restore_focus(true)
                            .entry(ScopeEntry::LastFocused),
                    ),
                )
                .with_children(vec![
                    Box::new(TestWidget::node(Some("first"), FocusConfig::Leaf)),
                    Box::new(TestWidget::node(Some("second"), FocusConfig::Leaf)),
                ]),
            ),
            Box::new(TestWidget::node(Some("outside"), FocusConfig::Leaf)),
        ]);

        let mut focus = FocusManager::new();
        focus.rebuild(&tree);

        assert!(focus.request_focus(&path(&["dialog", "second"])));
        assert!(focus.request_focus(&path(&["outside"])));
        assert!(focus.request_focus(&path(&["dialog"])));
        assert_eq!(
            focus.current_path().cloned(),
            Some(path(&["dialog", "second"]))
        );
    }

    #[test]
    fn trapping_scope_cycles_within_scope() {
        let tree = TestWidget::node(None, FocusConfig::None).with_children(vec![
            Box::new(
                TestWidget::node(
                    Some("scope"),
                    FocusConfig::Scope(FocusScope::new().trap_tab(true)),
                )
                .with_children(vec![
                    Box::new(TestWidget::node(Some("a"), FocusConfig::Leaf)),
                    Box::new(TestWidget::node(Some("b"), FocusConfig::Leaf)),
                ]),
            ),
            Box::new(TestWidget::node(Some("outside"), FocusConfig::Leaf)),
        ]);

        let mut focus = FocusManager::new();
        focus.rebuild(&tree);
        assert!(focus.request_focus(&path(&["outside"])));
        assert!(focus.request_focus(&path(&["scope", "a"])));

        focus.next();
        assert_eq!(focus.current_path().cloned(), Some(path(&["scope", "b"])));

        focus.next();
        assert_eq!(focus.current_path().cloned(), Some(path(&["scope", "a"])));

        assert!(focus.request_focus(&path(&["outside"])));
        focus.next();
        assert_eq!(focus.current_path().cloned(), Some(path(&["scope", "a"])));
    }
}
