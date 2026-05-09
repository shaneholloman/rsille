//! Prelude — import everything you need with `use tui::prelude::*;`

pub use crate::animation::{
    AnimationConfig, AnimationSlot, AnimationSpec, AnimationTheme, AreaF, ClipMode,
    Direction as AnimationDirection, Easing, HitTestMode, InitialAnimation, LayoutSnapshot,
    LayoutTransition, MotionPolicy, Presence, Repeat, SharedTransition, Timeline, Transition,
    TransitionEffect,
};
pub use crate::app::{App, FrameInfo, QuitBehavior};
pub use crate::effect::{
    CancellationToken, Effect, Request, RequestContext, RequestEvent, RequestId, RequestOutcome,
    RequestPhase, RequestState, RetryPolicy, Task, TaskId, TaskOutcome, TaskState, TaskStatus,
    UpdateCtx,
};
pub use crate::error::{WidgetError, WidgetResult};
pub use crate::event::{Event, KeyCode, KeyEvent, KeyModifiers};
pub use crate::focus::{FocusConfig, FocusScope, ScopeEntry};
pub use crate::layout::{
    clamp_scroll_offset, col, ensure_item_visible, grid, max_scroll_offset, overlay, row,
    scroll_lines, scroll_offset_for_item, scroll_view, scrollbar, split, stack, AlignItems,
    AnchorRect, Constraints, Direction, Flex, Grid, GridLine, GridPlacement, GridTrack,
    JustifyContent, Overlay, OverlayAnchor, OverlayLayer, OverlayPlacement, ScrollAxis,
    ScrollState, ScrollView, Scrollbar, ScrollbarOrientation, ScrollbarVisibility, Split,
    SplitDirection, SplitSize, SplitState, Stack,
};
pub use crate::shell::{
    CommandDescriptor, CommandRouter, Hotkey, HotkeyRegistry, ModalManager, Navigator,
    Notification, NotificationCenter, NotificationId, NotificationLevel,
};
pub use crate::state::{Derived, FormState, Store, StoreKey};
pub use crate::style::{BorderStyle, Color, Padding, Style, Theme};
pub use crate::widget::{
    EventCtx, EventPhase, FocusRequest, IntoWidget, RenderCtx, Widget, WidgetId, WidgetKey,
    WidgetPath, WidgetStore,
};
pub use crate::widgets::{
    animate, button, calendar, canvas, checkbox, code_viewer, collapsible, command_palette,
    data_table, dialog, diff_viewer, divider, file_explorer, label, list, loading_indicator,
    log_viewer, markdown_viewer, menu, panel, progress_bar, radio_group, select, spacer, switch,
    tabs, text_input, textarea, toggle, tree, Animated, Button, ButtonVariant, Calendar,
    CalendarDate, CalendarState, CanvasContext, CanvasWidget, Checkbox, CodeViewer, Collapsible,
    CommandItem, CommandPalette, CommandPaletteState, ContentViewerState, DataTable,
    DataTableColumn, DataTableNavigationMode, DataTableRow, DataTableSort, DataTableSortDirection,
    DataTableState, Dialog, DiffViewer, Divider, DividerDirection, DividerTextPosition,
    DividerVariant, FileExplorer, FileExplorerItem, FileExplorerItemKind, FileExplorerState, Label,
    List, ListItem, ListState, LoadingIndicator, LogLevel, LogLine, LogViewer, MarkdownViewer,
    Menu, MenuItem, MenuState, Panel, ProgressBar, RadioGroup, RadioGroupState, RadioOption,
    Select, SelectOption, SelectSearchMode, SelectState, SelectionMode, SelectionState, Spacer,
    Switch, TabItem, TableAlign, Tabs, TabsState, TextArea, TextAreaState, TextAreaVariant,
    TextInput, TextInputVariant, Tree, TreeItem, TreeState,
};
