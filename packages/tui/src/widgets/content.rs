//! Content viewers for code, logs, and diffs.

use once_cell::sync::Lazy;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::event::{Event, KeyCode};
use crate::focus::FocusConfig;
use crate::layout::border_renderer;
use crate::layout::{ensure_item_visible, Constraints};
use crate::style::{BorderStyle, Color, Style};
use crate::widget::{EventCtx, EventPhase, RenderCtx, Widget};

static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentViewerState {
    pub scroll_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub level: LogLevel,
    pub message: String,
}

impl LogLine {
    pub fn new(level: LogLevel, message: impl Into<String>) -> Self {
        Self {
            level,
            message: message.into(),
        }
    }
}

impl From<&str> for LogLine {
    fn from(value: &str) -> Self {
        let upper = value.to_ascii_uppercase();
        let level = if upper.contains("ERROR") {
            LogLevel::Error
        } else if upper.contains("WARN") {
            LogLevel::Warn
        } else if upper.contains("DEBUG") {
            LogLevel::Debug
        } else if upper.contains("TRACE") {
            LogLevel::Trace
        } else {
            LogLevel::Info
        };
        Self::new(level, value)
    }
}

impl From<String> for LogLine {
    fn from(value: String) -> Self {
        Self::from(value.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentKind {
    Code,
    Diff,
    Markdown,
}

pub struct CodeViewer<M = ()> {
    code: String,
    language: Option<String>,
    height: u16,
    border: Option<BorderStyle>,
    show_line_numbers: bool,
    custom_style: Option<Style>,
    widget_key: Option<String>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> std::fmt::Debug for CodeViewer<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeViewer")
            .field("language", &self.language)
            .field("height", &self.height)
            .field("border", &self.border)
            .field("show_line_numbers", &self.show_line_numbers)
            .finish()
    }
}

impl<M> CodeViewer<M> {
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            language: None,
            height: 12,
            border: Some(BorderStyle::Single),
            show_line_numbers: true,
            custom_style: None,
            widget_key: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(1);
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn borderless(mut self) -> Self {
        self.border = None;
        self
    }

    pub fn line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }
}

impl<M> Default for CodeViewer<M> {
    fn default() -> Self {
        Self::new("")
    }
}

impl<M: 'static> Widget<M> for CodeViewer<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        render_code(
            chunk,
            ctx,
            ContentKind::Code,
            self.border,
            self.height,
            self.custom_style,
            &self.code,
            self.language.as_deref(),
            self.show_line_numbers,
        );
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        handle_scroll_event(event, ctx, self.code.lines().count(), self.height);
    }

    fn constraints(&self) -> Constraints {
        content_constraints(self.height, self.border, 24)
    }

    fn focus_config(&self) -> FocusConfig {
        FocusConfig::Composite
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

pub struct LogViewer<M = ()> {
    lines: Vec<LogLine>,
    height: u16,
    border: Option<BorderStyle>,
    custom_style: Option<Style>,
    widget_key: Option<String>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> std::fmt::Debug for LogViewer<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogViewer")
            .field("lines", &self.lines)
            .field("height", &self.height)
            .field("border", &self.border)
            .finish()
    }
}

impl<M> LogViewer<M> {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            height: 12,
            border: Some(BorderStyle::Single),
            custom_style: None,
            widget_key: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn line(mut self, line: impl Into<LogLine>) -> Self {
        self.lines.push(line.into());
        self
    }

    pub fn lines<I>(mut self, lines: I) -> Self
    where
        I: IntoIterator,
        I::Item: Into<LogLine>,
    {
        self.lines.extend(lines.into_iter().map(Into::into));
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(1);
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn borderless(mut self) -> Self {
        self.border = None;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }
}

impl<M> Default for LogViewer<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: 'static> Widget<M> for LogViewer<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        render_log(chunk, ctx, self.border, self.custom_style, &self.lines);
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        handle_scroll_event(event, ctx, self.lines.len(), self.height);
    }

    fn constraints(&self) -> Constraints {
        content_constraints(self.height, self.border, 24)
    }

    fn focus_config(&self) -> FocusConfig {
        FocusConfig::Composite
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

pub struct DiffViewer<M = ()> {
    diff: String,
    height: u16,
    border: Option<BorderStyle>,
    custom_style: Option<Style>,
    widget_key: Option<String>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> std::fmt::Debug for DiffViewer<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiffViewer")
            .field("height", &self.height)
            .field("border", &self.border)
            .finish()
    }
}

impl<M> DiffViewer<M> {
    pub fn new(diff: impl Into<String>) -> Self {
        Self {
            diff: diff.into(),
            height: 12,
            border: Some(BorderStyle::Single),
            custom_style: None,
            widget_key: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(1);
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn borderless(mut self) -> Self {
        self.border = None;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }
}

pub struct MarkdownViewer<M = ()> {
    markdown: String,
    height: u16,
    border: Option<BorderStyle>,
    custom_style: Option<Style>,
    widget_key: Option<String>,
    _phantom: std::marker::PhantomData<M>,
}

impl<M> std::fmt::Debug for MarkdownViewer<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarkdownViewer")
            .field("height", &self.height)
            .field("border", &self.border)
            .finish()
    }
}

impl<M> MarkdownViewer<M> {
    pub fn new(markdown: impl Into<String>) -> Self {
        Self {
            markdown: markdown.into(),
            height: 12,
            border: Some(BorderStyle::Single),
            custom_style: None,
            widget_key: None,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn key(mut self, name: impl Into<String>) -> Self {
        self.widget_key = Some(name.into());
        self
    }

    pub fn height(mut self, height: u16) -> Self {
        self.height = height.max(1);
        self
    }

    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = Some(border);
        self
    }

    pub fn borderless(mut self) -> Self {
        self.border = None;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.custom_style = Some(style);
        self
    }
}

impl<M> Default for MarkdownViewer<M> {
    fn default() -> Self {
        Self::new("")
    }
}

impl<M: 'static> Widget<M> for MarkdownViewer<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        render_code(
            chunk,
            ctx,
            ContentKind::Markdown,
            self.border,
            self.height,
            self.custom_style,
            &self.markdown,
            Some("md"),
            false,
        );
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        handle_scroll_event(event, ctx, self.markdown.lines().count(), self.height);
    }

    fn constraints(&self) -> Constraints {
        content_constraints(self.height, self.border, 24)
    }

    fn focus_config(&self) -> FocusConfig {
        FocusConfig::Composite
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

impl<M> Default for DiffViewer<M> {
    fn default() -> Self {
        Self::new("")
    }
}

impl<M: 'static> Widget<M> for DiffViewer<M> {
    fn render(&self, chunk: &mut render::chunk::Chunk, ctx: &RenderCtx) {
        render_code(
            chunk,
            ctx,
            ContentKind::Diff,
            self.border,
            self.height,
            self.custom_style,
            &self.diff,
            Some("diff"),
            false,
        );
    }

    fn handle_event(&self, event: &Event, ctx: &mut EventCtx<M>) {
        handle_scroll_event(event, ctx, self.diff.lines().count(), self.height);
    }

    fn constraints(&self) -> Constraints {
        content_constraints(self.height, self.border, 24)
    }

    fn focus_config(&self) -> FocusConfig {
        FocusConfig::Composite
    }

    fn key(&self) -> Option<&str> {
        self.widget_key.as_deref()
    }
}

fn render_code(
    chunk: &mut render::chunk::Chunk,
    ctx: &RenderCtx,
    kind: ContentKind,
    border: Option<BorderStyle>,
    height: u16,
    custom_style: Option<Style>,
    text: &str,
    language: Option<&str>,
    show_line_numbers: bool,
) {
    let area = chunk.area();
    if area.width() == 0 || area.height() == 0 {
        return;
    }

    let theme = ctx.theme();
    let base_style = custom_style
        .map(|style| style.merge(theme.styles.surface))
        .unwrap_or(theme.styles.surface)
        .to_render_style();
    let border_style = if ctx.is_focused() {
        theme.styles.border_focused.to_render_style()
    } else {
        theme.styles.border.to_render_style()
    };
    let muted_style = theme.styles.text_muted.to_render_style();

    let _ = chunk.fill(0, 0, area.width(), area.height(), ' ', base_style);
    let (content_x, content_y, content_width, content_height) =
        content_area(chunk, border, border_style);
    if content_width == 0 || content_height == 0 {
        return;
    }

    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return;
    }

    let state = ctx.state_or_default::<ContentViewerState>();
    let visible_rows = content_height.min(height) as usize;
    let scroll_offset = ensure_item_visible(
        state.scroll_offset.min(lines.len().saturating_sub(1)),
        state.scroll_offset,
        visible_rows,
    );
    let line_number_width = if show_line_numbers {
        lines.len().to_string().width() + 2
    } else {
        0
    };
    let text_width = content_width.saturating_sub(line_number_width as u16) as usize;

    let syntax = language
        .and_then(|name| SYNTAX_SET.find_syntax_by_token(name))
        .or_else(|| SYNTAX_SET.find_syntax_plain_text().into());
    let theme = THEME_SET
        .themes
        .get("base16-ocean.dark")
        .or_else(|| THEME_SET.themes.values().next());

    for row in 0..visible_rows {
        let line_index = scroll_offset + row;
        if line_index >= lines.len() {
            break;
        }

        let y = content_y + row as u16;
        if show_line_numbers {
            let number = format!("{:>width$} ", line_index + 1, width = line_number_width - 1);
            let _ = chunk.set_string(content_x, y, &number, muted_style);
        }

        let text_x = content_x + line_number_width as u16;
        match kind {
            ContentKind::Diff => {
                draw_diff_line(chunk, text_x, y, text_width, lines[line_index], base_style)
            }
            ContentKind::Markdown => {
                draw_markdown_line(chunk, text_x, y, text_width, lines[line_index], base_style)
            }
            ContentKind::Code => {
                if let (Some(syntax), Some(theme)) = (syntax, theme) {
                    let mut highlighter = HighlightLines::new(syntax, theme);
                    if let Ok(ranges) = highlighter.highlight_line(lines[line_index], &SYNTAX_SET) {
                        draw_highlighted_line(chunk, text_x, y, text_width, &ranges, base_style);
                    } else {
                        draw_plain_line(
                            chunk,
                            text_x,
                            y,
                            text_width,
                            lines[line_index],
                            base_style,
                        );
                    }
                } else {
                    draw_plain_line(chunk, text_x, y, text_width, lines[line_index], base_style);
                }
            }
        }
    }
}

fn render_log(
    chunk: &mut render::chunk::Chunk,
    ctx: &RenderCtx,
    border: Option<BorderStyle>,
    custom_style: Option<Style>,
    lines: &[LogLine],
) {
    let area = chunk.area();
    if area.width() == 0 || area.height() == 0 {
        return;
    }

    let theme = ctx.theme();
    let base_style = custom_style
        .map(|style| style.merge(theme.styles.surface))
        .unwrap_or(theme.styles.surface)
        .to_render_style();
    let border_style = if ctx.is_focused() {
        theme.styles.border_focused.to_render_style()
    } else {
        theme.styles.border.to_render_style()
    };

    let _ = chunk.fill(0, 0, area.width(), area.height(), ' ', base_style);
    let (content_x, content_y, content_width, content_height) =
        content_area(chunk, border, border_style);
    if content_width == 0 || content_height == 0 || lines.is_empty() {
        return;
    }

    let state = ctx.state_or_default::<ContentViewerState>();
    let visible_rows = content_height as usize;
    let scroll_offset = state.scroll_offset.min(lines.len().saturating_sub(1));

    for row in 0..visible_rows {
        let line_index = scroll_offset + row;
        if line_index >= lines.len() {
            break;
        }
        let line = &lines[line_index];
        let style = log_style(line.level);
        let text = truncate_to_width(&line.message, content_width as usize);
        let _ = chunk.set_string(
            content_x,
            content_y + row as u16,
            &text,
            style.to_render_style(),
        );
    }
}

fn content_area(
    chunk: &mut render::chunk::Chunk,
    border: Option<BorderStyle>,
    border_style: render::style::Style,
) -> (u16, u16, u16, u16) {
    let area = chunk.area();
    if let Some(border) = border {
        if area.width() < 2 || area.height() < 2 {
            return (0, 0, 0, 0);
        }
        border_renderer::render_border(chunk, border, border_style);
        (1, 1, area.width() - 2, area.height() - 2)
    } else {
        (0, 0, area.width(), area.height())
    }
}

fn handle_scroll_event<M>(event: &Event, ctx: &mut EventCtx<M>, total_lines: usize, height: u16) {
    if ctx.phase() != EventPhase::Target || total_lines == 0 {
        return;
    }

    let Event::Key(key_event) = event else {
        return;
    };

    let visible_rows = height.max(1) as usize;
    let state = ctx.state_mut::<ContentViewerState>();
    let max_offset = total_lines.saturating_sub(visible_rows);
    match key_event.code {
        KeyCode::Up => state.scroll_offset = state.scroll_offset.saturating_sub(1),
        KeyCode::Down => state.scroll_offset = (state.scroll_offset + 1).min(max_offset),
        KeyCode::PageUp => state.scroll_offset = state.scroll_offset.saturating_sub(visible_rows),
        KeyCode::PageDown => {
            state.scroll_offset = (state.scroll_offset + visible_rows).min(max_offset);
        }
        KeyCode::Home => state.scroll_offset = 0,
        KeyCode::End => state.scroll_offset = max_offset,
        _ => return,
    }

    ctx.set_handled();
}

fn content_constraints(height: u16, border: Option<BorderStyle>, min_width: u16) -> Constraints {
    let border_size = if border.is_some() { 2 } else { 0 };
    Constraints {
        min_width: min_width + border_size,
        max_width: None,
        min_height: height,
        max_height: Some(height),
        flex: Some(1.0),
    }
}

fn draw_highlighted_line(
    chunk: &mut render::chunk::Chunk,
    x: u16,
    y: u16,
    width: usize,
    ranges: &[(SyntectStyle, &str)],
    fallback: render::style::Style,
) {
    let mut cursor_x = x;
    let mut remaining = width;

    for (style, text) in ranges {
        if remaining == 0 {
            break;
        }
        let segment = truncate_to_width(text, remaining);
        if segment.is_empty() {
            continue;
        }
        let render_style = Style::default()
            .fg(Color::Rgb(
                style.foreground.r,
                style.foreground.g,
                style.foreground.b,
            ))
            .to_render_style();
        let _ = chunk.set_string(cursor_x, y, &segment, render_style);
        let segment_width = segment.width() as u16;
        cursor_x = cursor_x.saturating_add(segment_width);
        remaining = remaining.saturating_sub(segment_width as usize);
    }

    if remaining == width {
        let _ = chunk.set_char(x, y, ' ', fallback);
    }
}

fn draw_plain_line(
    chunk: &mut render::chunk::Chunk,
    x: u16,
    y: u16,
    width: usize,
    line: &str,
    style: render::style::Style,
) {
    let text = truncate_to_width(line, width);
    let _ = chunk.set_string(x, y, &text, style);
}

fn draw_diff_line(
    chunk: &mut render::chunk::Chunk,
    x: u16,
    y: u16,
    width: usize,
    line: &str,
    fallback: render::style::Style,
) {
    let style = if line.starts_with('+') && !line.starts_with("+++") {
        Style::default()
            .fg(Color::Rgb(134, 239, 172))
            .bg(Color::Rgb(20, 83, 45))
            .to_render_style()
    } else if line.starts_with('-') && !line.starts_with("---") {
        Style::default()
            .fg(Color::Rgb(252, 165, 165))
            .bg(Color::Rgb(127, 29, 29))
            .to_render_style()
    } else if line.starts_with("@@") {
        Style::default()
            .fg(Color::Rgb(125, 211, 252))
            .bg(Color::Rgb(12, 74, 110))
            .to_render_style()
    } else {
        fallback
    };
    let text = truncate_to_width(line, width);
    let _ = chunk.fill(x, y, width as u16, 1, ' ', style);
    let _ = chunk.set_string(x, y, &text, style);
}

fn draw_markdown_line(
    chunk: &mut render::chunk::Chunk,
    x: u16,
    y: u16,
    width: usize,
    line: &str,
    fallback: render::style::Style,
) {
    let trimmed = line.trim_start();
    let style = if trimmed.starts_with("# ") {
        Style::default()
            .fg(Color::Rgb(125, 211, 252))
            .bold()
            .to_render_style()
    } else if trimmed.starts_with("## ") || trimmed.starts_with("### ") {
        Style::default()
            .fg(Color::Rgb(147, 197, 253))
            .bold()
            .to_render_style()
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
        Style::default()
            .fg(Color::Rgb(226, 232, 240))
            .to_render_style()
    } else if trimmed.starts_with("```") || trimmed.starts_with('`') {
        Style::default()
            .fg(Color::Rgb(196, 181, 253))
            .bg(Color::Rgb(39, 39, 42))
            .to_render_style()
    } else if trimmed.starts_with('>') {
        Style::default()
            .fg(Color::Rgb(148, 163, 184))
            .italic()
            .to_render_style()
    } else {
        fallback
    };

    let text = truncate_to_width(line, width);
    let _ = chunk.set_string(x, y, &text, style);
}

fn log_style(level: LogLevel) -> Style {
    match level {
        LogLevel::Trace => Style::default().fg(Color::Rgb(148, 163, 184)),
        LogLevel::Debug => Style::default().fg(Color::Rgb(147, 197, 253)),
        LogLevel::Info => Style::default().fg(Color::Rgb(226, 232, 240)),
        LogLevel::Warn => Style::default().fg(Color::Rgb(253, 224, 71)),
        LogLevel::Error => Style::default().fg(Color::Rgb(252, 165, 165)).bold(),
    }
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    let mut out = String::new();
    let mut width = 0;

    for ch in text.chars() {
        let char_width = ch.width().unwrap_or(0);
        if width + char_width > max_width {
            break;
        }
        out.push(ch);
        width += char_width;
    }

    out
}

pub fn code_viewer<M>(code: impl Into<String>) -> CodeViewer<M> {
    CodeViewer::new(code)
}

pub fn log_viewer<M>() -> LogViewer<M> {
    LogViewer::new()
}

pub fn diff_viewer<M>(diff: impl Into<String>) -> DiffViewer<M> {
    DiffViewer::new(diff)
}

pub fn markdown_viewer<M>(markdown: impl Into<String>) -> MarkdownViewer<M> {
    MarkdownViewer::new(markdown)
}
