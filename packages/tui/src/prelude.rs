//! Prelude — import everything you need with `use tui::prelude::*;`

pub use crate::app::{App, FrameInfo, QuitBehavior};
pub use crate::effect::{
    CancellationToken, Effect, RetryPolicy, Task, TaskId, TaskOutcome, TaskState, TaskStatus,
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
    EventCtx, EventPhase, FocusRequest, IntoWidget, RenderCtx, Widget, WidgetKey, WidgetPath,
    WidgetStore,
};
pub use crate::widgets::{
    button, calendar, checkbox, collapsible, data_table, dialog, divider, label, list,
    loading_indicator, menu, panel, progress_bar, radio_group, select, spacer, switch, tabs,
    text_input, textarea, toggle, tree, Button, ButtonVariant, Calendar, CalendarDate,
    CalendarState, Checkbox, Collapsible, DataTable, DataTableColumn, DataTableRow, DataTableState,
    Dialog, Divider, DividerDirection, DividerTextPosition, DividerVariant, Label, List, ListItem,
    ListState, LoadingIndicator, Menu, MenuItem, MenuState, Panel, ProgressBar, RadioGroup,
    RadioGroupState, RadioOption, Select, SelectOption, SelectState, Spacer, Switch, TabItem,
    TableAlign, Tabs, TabsState, TextArea, TextAreaState, TextAreaVariant, TextInput,
    TextInputVariant, Tree, TreeItem, TreeState,
};
