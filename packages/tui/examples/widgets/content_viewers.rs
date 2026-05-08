//! Markdown, code, log, and diff viewer widgets.
//!
//! Run with: `cargo run -p tui --example content_viewers`

use tui::prelude::*;

const CODE: &str = r#"pub fn reconcile(items: &[String], query: &str) -> Vec<String> {
    let needle = query.trim().to_lowercase();
    items
        .iter()
        .filter(|item| item.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}"#;

const DIFF: &str = r#"diff --git a/packages/tui/src/widgets/list.rs b/packages/tui/src/widgets/list.rs
@@ -42,6 +42,7 @@ pub struct ListState {
 pub struct ListState {
     pub active_item: Option<String>,
+    pub selection: SelectionState,
     pub scroll_offset: usize,
 }
@@ -118,7 +119,7 @@ impl<M> List<M> {
-            on_submit: None,
+            on_selection_change: None,
             widget_key: None,
         }
     }"#;

const MARKDOWN: &str = r#"# Data Components

The framework now includes first-class viewers for common tool content.

- Markdown summaries
- Syntax-highlighted code
- Log streams
- Unified diffs

> These widgets are focusable and scrollable.

```rust
let viewer = markdown_viewer(markdown);
```"#;

fn main() -> WidgetResult<()> {
    App::new(()).run_inline(|_, _| {}, view)
}

fn view(_: &()) -> impl Widget<()> {
    row::<()>()
        .padding(Padding::uniform(1))
        .gap(2)
        .child(
            col::<()>()
                .gap(1)
                .child(label("Code").bold())
                .child(
                    code_viewer::<()>(CODE)
                        .key("code")
                        .language("rs")
                        .height(10),
                )
                .child(label("Diff").bold())
                .child(diff_viewer::<()>(DIFF).key("diff").height(10)),
        )
        .child(
            col::<()>()
                .gap(1)
                .child(label("Markdown").bold())
                .child(markdown_viewer::<()>(MARKDOWN).key("markdown").height(10))
                .child(label("Logs").bold())
                .child(log_viewer::<()>().key("logs").height(10).lines([
                    LogLine::new(LogLevel::Info, "INFO request started id=req_001"),
                    LogLine::new(LogLevel::Debug, "DEBUG cache hit key=deployments"),
                    LogLine::new(LogLevel::Info, "INFO rendered 128 rows in 4ms"),
                    LogLine::new(LogLevel::Warn, "WARN retry budget is nearly exhausted"),
                    LogLine::new(LogLevel::Error, "ERROR upstream timed out after 2000ms"),
                    LogLine::new(LogLevel::Info, "INFO fallback response served"),
                    LogLine::new(LogLevel::Trace, "TRACE widget tree walked depth=6"),
                ])),
        )
}
