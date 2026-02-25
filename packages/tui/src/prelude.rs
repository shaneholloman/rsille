//! Prelude — import everything you need with `use tui::prelude::*;`

pub use crate::app::{App, QuitBehavior};
pub use crate::error::{WidgetError, WidgetResult};
pub use crate::event::{Event, KeyCode, KeyEvent, KeyModifiers};
pub use crate::layout::{
    col, grid, row, AlignItems, Constraints, Direction, Flex, Grid, GridLine, GridPlacement,
    GridTrack, JustifyContent,
};
pub use crate::style::{BorderStyle, Color, Padding, Style, ThemeManager};
pub use crate::widget::{EventCtx, IntoWidget, RenderCtx, Widget, WidgetStore};
pub use crate::widgets::{
    button, divider, label, spacer, text_input, Button, ButtonVariant, Divider, DividerDirection,
    DividerTextPosition, DividerVariant, Label, Spacer, TextInput, TextInputVariant,
};
