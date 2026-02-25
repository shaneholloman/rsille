use crate::widget::Widget;

/// Manages keyboard focus navigation across focusable widgets.
///
/// The framework builds the focus chain by walking the widget tree — widgets
/// themselves do not participate in focus chain construction.
#[derive(Debug, Default)]
pub struct FocusManager {
    /// Ordered list of focusable widget paths (tree indices from root).
    chain: Vec<Vec<usize>>,
    /// Current focus index into `chain`. `usize::MAX` means nothing focused.
    index: usize,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            chain: Vec::new(),
            index: usize::MAX,
        }
    }

    /// Rebuild the focus chain by walking the widget tree.
    /// Automatically preserves focus on the same path if it still exists,
    /// otherwise focuses the first focusable widget.
    pub fn rebuild<M>(&mut self, root: &dyn Widget<M>) {
        let old_path = self.current_path().map(|p| p.to_vec());

        self.chain.clear();
        let mut path = Vec::new();
        Self::collect_focusable(root, &mut path, &mut self.chain);

        // Try to keep focus on the same path
        if let Some(old) = old_path {
            if let Some(pos) = self.chain.iter().position(|p| *p == old) {
                self.index = pos;
                return;
            }
        }
        // Fall back to first focusable widget
        self.index = if self.chain.is_empty() {
            usize::MAX
        } else {
            0
        };
    }

    /// Focus the next widget (Tab).
    pub fn next(&mut self) {
        if self.chain.is_empty() {
            return;
        }
        if self.index == usize::MAX {
            self.index = 0;
        } else {
            self.index = (self.index + 1) % self.chain.len();
        }
    }

    /// Focus the previous widget (Shift+Tab).
    pub fn prev(&mut self) {
        if self.chain.is_empty() {
            return;
        }
        if self.index == usize::MAX || self.index == 0 {
            self.index = self.chain.len() - 1;
        } else {
            self.index -= 1;
        }
    }

    /// The path of the currently focused widget, or `None`.
    pub fn current_path(&self) -> Option<&[usize]> {
        self.chain.get(self.index).map(|v| v.as_slice())
    }

    /// All focusable widget paths (used for store cleanup).
    pub fn active_paths(&self) -> &[Vec<usize>] {
        &self.chain
    }

    /// Recursively collect focusable widgets into `out`.
    fn collect_focusable<M>(
        widget: &dyn Widget<M>,
        path: &mut Vec<usize>,
        out: &mut Vec<Vec<usize>>,
    ) {
        if widget.focusable() {
            out.push(path.clone());
        }
        for (i, child) in widget.children().iter().enumerate() {
            path.push(i);
            Self::collect_focusable(child.as_ref(), path, out);
            path.pop();
        }
    }
}
