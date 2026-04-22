//! Prelude — import everything you need with `use tui::prelude::*;`

pub use crate::app::{App, FrameInfo, QuitBehavior};
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
pub use crate::style::{BorderStyle, Color, Padding, Style, Theme};
pub use crate::widget::{
    EventCtx, EventPhase, FocusRequest, IntoWidget, RenderCtx, Widget, WidgetKey, WidgetPath,
    WidgetStore,
};
pub use crate::widgets::{
    button, calendar, data_table, divider, label, list, select, spacer, text_input, tree, Button,
    ButtonVariant, Calendar, CalendarDate, CalendarState, DataTable, DataTableColumn, DataTableRow,
    DataTableState, Divider, DividerDirection, DividerTextPosition, DividerVariant, Label, List,
    ListItem, ListState, Select, SelectOption, SelectState, Spacer, TableAlign, TextInput,
    TextInputVariant, Tree, TreeItem, TreeState,
};
