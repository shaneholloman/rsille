use std::time::Duration;

use rsille::canvas::Canvas;
use rsille::tui::prelude::*;

const CODE_SAMPLE: &str = r#"fn draw_probe(canvas: &mut Canvas, frame: u64) {
    let cells = chunk.area().size();
    let dots_w = cells.width as i32 * 2;
    let dots_h = cells.height as i32 * 4;
    let phase = frame as f64 * 0.08;

    for x in 0..dots_w {
        let xf = x as f64;
        canvas.set(xf, dots_h as f64 * 0.5 + (xf * 0.16 + phase).sin() * 8.5);
    }
}"#;

const NOTES: &str = r#"# Showcase shape

This screen keeps the demo readable in an 80-column terminal.

- The braille canvas reads its widget box and fills the available cells.
- Controls update shared app state.
- Tables, logs, overlays, and viewers show real TUI surfaces.
"#;

#[derive(Debug, Clone)]
enum Msg {
    Frame(FrameInfo),
    Tick,
    TabChanged(String),
    FilterChanged(String),
    PhaseChanged(String),
    ModeChanged(String),
    ToggleLive(bool),
    ToggleScan(bool),
    NotesChanged(String),
    DeploymentFocused(String),
    DeploymentOpened(String),
    CellFocused(String, String),
    SelectedDeployments(Vec<String>),
    AlertFocused(String),
    AlertOpened(String),
    FileFocused(String),
    FileOpened(String),
    FileLoad(String),
    PaletteHighlighted(String),
    RunCommand(String),
    TogglePalette,
    ClosePalette,
    OpenDialog,
    CloseDialog,
}

#[derive(Debug)]
struct State {
    frame: u64,
    elapsed_secs: f32,
    pulse: usize,
    tab: String,
    filter: String,
    phase: String,
    mode: String,
    live: bool,
    scan: bool,
    notes: String,
    deployment: String,
    opened_deployment: String,
    cell: String,
    selected: Vec<String>,
    alert: String,
    opened_alert: String,
    file: String,
    opened_file: String,
    load_request: String,
    command: String,
    highlighted_command: String,
    show_palette: bool,
    show_dialog: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            frame: 0,
            elapsed_secs: 0.0,
            pulse: 0,
            tab: "ops".to_owned(),
            filter: String::new(),
            phase: "canary".to_owned(),
            mode: "balanced".to_owned(),
            live: true,
            scan: true,
            notes: "Promote after two clean samples.".to_owned(),
            deployment: String::new(),
            opened_deployment: String::new(),
            cell: String::new(),
            selected: Vec::new(),
            alert: String::new(),
            opened_alert: String::new(),
            file: String::new(),
            opened_file: String::new(),
            load_request: String::new(),
            command: "boot".to_owned(),
            highlighted_command: String::new(),
            show_palette: false,
            show_dialog: false,
        }
    }
}

fn main() -> WidgetResult<()> {
    App::new(State::default())
        .on_frame(Msg::Frame)
        .on_tick(Duration::from_millis(1200), || Msg::Tick)
        .run_inline(update, view)
}

fn update(state: &mut State, msg: Msg) {
    match msg {
        Msg::Frame(info) => {
            state.frame = info.frame;
            state.elapsed_secs = info.since_start.as_secs_f32();
        }
        Msg::Tick => state.pulse = state.pulse.wrapping_add(1),
        Msg::TabChanged(value) => state.tab = value,
        Msg::FilterChanged(value) => state.filter = value,
        Msg::PhaseChanged(value) => state.phase = value,
        Msg::ModeChanged(value) => state.mode = value,
        Msg::ToggleLive(value) => state.live = value,
        Msg::ToggleScan(value) => state.scan = value,
        Msg::NotesChanged(value) => state.notes = value,
        Msg::DeploymentFocused(id) => state.deployment = id,
        Msg::DeploymentOpened(id) => state.opened_deployment = id,
        Msg::CellFocused(row, column) => state.cell = format!("{row}/{column}"),
        Msg::SelectedDeployments(ids) => state.selected = ids,
        Msg::AlertFocused(id) => state.alert = id,
        Msg::AlertOpened(id) => state.opened_alert = id,
        Msg::FileFocused(id) => state.file = id,
        Msg::FileOpened(id) => state.opened_file = id,
        Msg::FileLoad(id) => state.load_request = id,
        Msg::PaletteHighlighted(id) => state.highlighted_command = id,
        Msg::TogglePalette => state.show_palette = !state.show_palette,
        Msg::ClosePalette => state.show_palette = false,
        Msg::OpenDialog => state.show_dialog = true,
        Msg::CloseDialog => state.show_dialog = false,
        Msg::RunCommand(id) => run_command(state, id),
    }
}

fn run_command(state: &mut State, id: String) {
    state.command = id.clone();
    state.show_palette = false;
    match id.as_str() {
        "promote" => state.show_dialog = true,
        "toggle-live" => state.live = !state.live,
        "scan" => state.scan = !state.scan,
        "source" => state.tab = "source".to_owned(),
        "canvas" => state.tab = "canvas".to_owned(),
        _ => {}
    }
}

fn view(state: &State) -> impl Widget<Msg> {
    let app = col::<Msg>()
        .style(Style::default().bg(Color::Rgb(8, 11, 18)))
        .padding(Padding::uniform(1))
        .gap(1)
        .child(header(state))
        .child(nav(state))
        .child(page(state))
        .child(status_bar(state));

    let ui = overlay(app);
    let ui = if state.show_palette {
        ui.layer(
            OverlayLayer::new(command_palette_view())
                .floating(OverlayAnchor::Center)
                .size(54, 12)
                .z_index(20),
        )
        .trap_focus()
    } else {
        ui
    };

    if state.show_dialog {
        ui.layer(
            OverlayLayer::new(promote_dialog(state))
                .floating(OverlayAnchor::Center)
                .size(48, 10)
                .z_index(30),
        )
        .trap_focus()
    } else {
        ui
    }
}

fn header(state: &State) -> impl Widget<Msg> {
    row::<Msg>()
        .style(Style::default().bg(Color::Rgb(13, 18, 28)))
        .padding(Padding::new(0, 1, 1, 1))
        .gap(2)
        .child(label("RSILLE OPS").bold().fg(Color::Rgb(111, 224, 255)))
        .child(label(format!("frame {:<5}", state.frame)).fg(Color::Rgb(163, 177, 198)))
        .child(label(format!("phase {}", state.phase)).fg(Color::Rgb(159, 255, 190)))
        .child(
            loading_indicator::<Msg>()
                .frame(state.frame as usize)
                .label(if state.live { "live" } else { "hold" }),
        )
        .child(
            button("Cmd")
                .variant(ButtonVariant::Secondary)
                .on_click(|| Msg::TogglePalette),
        )
        .child(button("Promote").on_click(|| Msg::OpenDialog).animated())
}

fn nav(state: &State) -> impl Widget<Msg> {
    tabs::<Msg>()
        .key("tabs")
        .selected(state.tab.clone())
        .tabs([
            TabItem::new("ops", "Ops"),
            TabItem::new("canvas", "Canvas"),
            TabItem::new("source", "Source"),
        ])
        .on_change(Msg::TabChanged)
}

fn page(state: &State) -> Box<dyn Widget<Msg>> {
    match state.tab.as_str() {
        "canvas" => Box::new(canvas_page(state)),
        "source" => Box::new(source_page(state)),
        _ => Box::new(ops_page(state)),
    }
}

fn ops_page(state: &State) -> impl Widget<Msg> {
    col::<Msg>()
        .gap(1)
        .child(
            row::<Msg>()
                .gap(1)
                .child(signal_card(state))
                .child(control_card(state)),
        )
        .child(
            row::<Msg>()
                .gap(1)
                .child(alert_card(state))
                .child(metrics_card(state)),
        )
        .child(deployments_card(state))
}

fn canvas_page(state: &State) -> impl Widget<Msg> {
    col::<Msg>().gap(1).child(signal_card(state)).child(
        panel::<Msg>()
            .title("Canvas pipeline")
            .border(BorderStyle::Rounded)
            .padding(Padding::uniform(1))
            .gap(1)
            .child(markdown_viewer::<Msg>(NOTES).key("notes-md").height(8))
            .child(
                code_viewer::<Msg>(CODE_SAMPLE)
                    .key("canvas-code")
                    .language("rs")
                    .height(8),
            ),
    )
}

fn source_page(state: &State) -> impl Widget<Msg> {
    col::<Msg>()
        .gap(1)
        .child(
            panel::<Msg>()
                .title("Files")
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(1)
                .child(file_tree())
                .child(label(format!("Focus: {}", compact(&state.file))))
                .child(label(format!("Open: {}", compact(&state.opened_file))))
                .child(label(format!("Load: {}", compact(&state.load_request)))),
        )
        .child(
            panel::<Msg>()
                .title("Code + logs")
                .border(BorderStyle::Rounded)
                .padding(Padding::uniform(1))
                .gap(1)
                .child(
                    code_viewer::<Msg>(CODE_SAMPLE)
                        .key("code")
                        .language("rs")
                        .height(8),
                )
                .child(
                    log_viewer::<Msg>()
                        .key("source-log")
                        .height(6)
                        .lines(log_lines(state)),
                ),
        )
}

fn signal_card(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Micro canvas")
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(Color::Rgb(7, 13, 23)))
        .child(CanvasScene::new(state))
}

fn control_card(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Controls")
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(Color::Rgb(12, 16, 24)))
        .child(
            switch("Live")
                .checked(state.live)
                .on_change(Msg::ToggleLive)
                .animated(),
        )
        .child(
            switch("Scan")
                .checked(state.scan)
                .on_change(Msg::ToggleScan)
                .animated(),
        )
        .child(
            select::<Msg>()
                .key("phase")
                .height(5)
                .placeholder("Phase")
                .searchable(true)
                .search_mode(SelectSearchMode::Fuzzy)
                .options([
                    SelectOption::new("dev", "dev"),
                    SelectOption::new("canary", "canary"),
                    SelectOption::new("staging", "staging"),
                    SelectOption::new("prod", "prod"),
                ])
                .on_change(Msg::PhaseChanged),
        )
        .child(
            radio_group::<Msg>()
                .key("mode")
                .selected(state.mode.clone())
                .options([
                    RadioOption::new("safe", "safe"),
                    RadioOption::new("balanced", "balanced"),
                    RadioOption::new("fast", "fast"),
                ])
                .on_change(Msg::ModeChanged),
        )
        .child(
            textarea::<Msg>()
                .key("operator-notes")
                .height(4)
                .value(state.notes.clone())
                .placeholder("Notes")
                .on_change(Msg::NotesChanged),
        )
}

fn alert_card(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Alerts")
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(Color::Rgb(12, 16, 24)))
        .child(
            list::<Msg>()
                .key("alerts")
                .height(6)
                .items([
                    ListItem::new("latency", "latency drift"),
                    ListItem::new("warmup", "cache warmup"),
                    ListItem::new("replica", "replica lag"),
                    ListItem::new("render", "render burst"),
                ])
                .on_change(Msg::AlertFocused)
                .on_submit(Msg::AlertOpened),
        )
        .child(label(format!("Focus: {}", compact(&state.alert))))
        .child(label(format!("Open: {}", compact(&state.opened_alert))))
}

fn metrics_card(state: &State) -> impl Widget<Msg> {
    let phase = state.elapsed_secs as f64;
    let draw = (0.55 + phase.sin() * 0.16).clamp(0.0, 1.0);
    let input = (0.38 + (phase * 1.7).cos() * 0.12).clamp(0.0, 1.0);
    let queue = (0.30 + (state.pulse % 5) as f64 * 0.08).clamp(0.0, 1.0);

    panel::<Msg>()
        .title("Signals")
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(Color::Rgb(12, 16, 24)))
        .child(metric_row("draw", draw, Color::Rgb(111, 224, 255)))
        .child(metric_row("input", input, Color::Rgb(159, 255, 190)))
        .child(metric_row("queue", queue, Color::Rgb(255, 204, 102)))
        .child(divider().text("state"))
        .child(label(format!("Cmd: {}", state.command)))
        .child(label(format!("Cell: {}", compact(&state.cell))))
        .child(label(format!("Rows: {}", state.selected.len())))
}

fn deployments_card(state: &State) -> impl Widget<Msg> {
    panel::<Msg>()
        .title("Deployments")
        .border(BorderStyle::Rounded)
        .padding(Padding::uniform(1))
        .gap(1)
        .style(Style::default().bg(Color::Rgb(10, 14, 22)))
        .child(
            text_input::<Msg>()
                .key("filter")
                .value(state.filter.clone())
                .placeholder("filter service")
                .on_change(Msg::FilterChanged),
        )
        .child(
            data_table::<Msg>()
                .key("deployments")
                .height(8)
                .navigation_mode(DataTableNavigationMode::Cell)
                .multi_select(true)
                .filter_query_opt((!state.filter.trim().is_empty()).then(|| state.filter.clone()))
                .sort(DataTableSort::new("svc", DataTableSortDirection::Asc))
                .columns([
                    DataTableColumn::new("Svc")
                        .id("svc")
                        .width(13)
                        .sortable(true)
                        .filterable(true),
                    DataTableColumn::new("Zone")
                        .id("zone")
                        .width(7)
                        .filterable(true),
                    DataTableColumn::new("ms")
                        .id("ms")
                        .width(5)
                        .align(TableAlign::Right),
                    DataTableColumn::new("State").id("state").width(8),
                ])
                .rows([
                    DataTableRow::new("api", ["api", "iad", "31", "green"]),
                    DataTableRow::new("tui", ["tui", "sfo", "44", "green"]),
                    DataTableRow::new("canvas", ["canvas", "hkg", "68", "amber"]),
                    DataTableRow::new("render", ["render", "fra", "39", "green"]),
                    DataTableRow::new("events", ["events", "nrt", "92", "amber"]),
                ])
                .on_change(Msg::DeploymentFocused)
                .on_cell_change(Msg::CellFocused)
                .on_submit(Msg::DeploymentOpened)
                .on_selection_change(Msg::SelectedDeployments),
        )
        .child(label(format!(
            "Focus {} | Open {}",
            compact(&state.deployment),
            compact(&state.opened_deployment)
        )))
}

fn status_bar(state: &State) -> impl Widget<Msg> {
    row::<Msg>()
        .gap(2)
        .style(Style::default().bg(Color::Rgb(13, 18, 28)))
        .padding(Padding::new(0, 1, 1, 1))
        .child(label("Tab move focus").fg(Color::Rgb(163, 177, 198)))
        .child(label("Enter activate").fg(Color::Rgb(163, 177, 198)))
        .child(label("Esc quit").fg(Color::Rgb(163, 177, 198)))
        .child(
            label(format!("hint {}", compact(&state.highlighted_command)))
                .fg(Color::Rgb(111, 224, 255)),
        )
}

fn command_palette_view() -> impl Widget<Msg> {
    command_palette::<Msg>()
        .key("commands")
        .height(10)
        .title("Commands")
        .prompt(">")
        .items([
            CommandItem::new("promote", "Promote canary").keywords(["release", "ship"]),
            CommandItem::new("toggle-live", "Toggle live").keywords(["pause", "resume"]),
            CommandItem::new("scan", "Toggle scan").keywords(["canvas", "probe"]),
            CommandItem::new("canvas", "Open canvas page").keywords(["braille", "draw"]),
            CommandItem::new("source", "Open source page").keywords(["code", "files"]),
        ])
        .on_change(Msg::PaletteHighlighted)
        .on_submit(Msg::RunCommand)
        .on_close(|| Msg::ClosePalette)
}

fn promote_dialog(state: &State) -> impl Widget<Msg> {
    dialog::<Msg>()
        .title("Promote")
        .border(BorderStyle::Double)
        .padding(Padding::uniform(1))
        .gap(1)
        .child(label("Promote selected canary?").bold())
        .child(label(format!(
            "phase={} selected={}",
            state.phase,
            state.selected.len()
        )))
        .child(
            row::<Msg>()
                .gap(2)
                .child(
                    button("Cancel")
                        .variant(ButtonVariant::Secondary)
                        .on_click(|| Msg::CloseDialog),
                )
                .child(button("Apply").on_click(|| Msg::CloseDialog).animated()),
        )
}

fn file_tree() -> impl Widget<Msg> {
    file_explorer::<Msg>()
        .key("files")
        .height(10)
        .multi_select(true)
        .items([
            FileExplorerItem::directory("packages", "packages")
                .child(
                    FileExplorerItem::directory("packages/canvas", "canvas")
                        .child(FileExplorerItem::file("canvas.rs", "canvas.rs"))
                        .child(FileExplorerItem::file("braille.rs", "braille.rs")),
                )
                .child(
                    FileExplorerItem::directory("packages/tui", "tui")
                        .child(FileExplorerItem::file("app.rs", "app.rs"))
                        .child(FileExplorerItem::lazy_directory("widgets", "widgets")),
                ),
            FileExplorerItem::file("Cargo.toml", "Cargo.toml"),
            FileExplorerItem::file("README.md", "README.md"),
        ])
        .on_change(Msg::FileFocused)
        .on_open(Msg::FileOpened)
        .on_load_children(Msg::FileLoad)
}

fn metric_row(name: &str, value: f64, color: Color) -> impl Widget<Msg> {
    row::<Msg>()
        .gap(1)
        .child(label(format!("{name:<5}")).fg(Color::Rgb(163, 177, 198)))
        .child(
            progress_bar::<Msg>(value)
                .width(22)
                .label(format!("{:>3}%", (value * 100.0).round() as u8))
                .fill_style(Style::default().fg(color))
                .animated(),
        )
}

#[derive(Debug, Clone)]
struct CanvasScene {
    frame: u64,
    elapsed_secs: f32,
    pulse: usize,
    live: bool,
    scan: bool,
    mode: String,
    phase: String,
}

impl CanvasScene {
    fn new(state: &State) -> Self {
        Self {
            frame: state.frame,
            elapsed_secs: state.elapsed_secs,
            pulse: state.pulse,
            live: state.live,
            scan: state.scan,
            mode: state.mode.clone(),
            phase: state.phase.clone(),
        }
    }
}

impl Widget<Msg> for CanvasScene {
    fn render(&self, chunk: &mut rsille::render::chunk::Chunk, _ctx: &RenderCtx) {
        let area = chunk.area();
        if area.width() == 0 || area.height() == 0 {
            return;
        }

        let lines = render_canvas_scene(self, area.width(), area.height());

        for (row, line) in lines.into_iter().take(area.height() as usize).enumerate() {
            let style = Style::default().fg(canvas_row_color(row)).to_render_style();
            let display = truncate_chars(&line, area.width() as usize);
            let _ = chunk.set_string(0, row as u16, &display, style);
        }
    }

    fn constraints(&self) -> Constraints {
        Constraints {
            min_width: 34,
            max_width: None,
            min_height: 8,
            max_height: None,
            flex: Some(1.0),
        }
    }

    fn key(&self) -> Option<&str> {
        Some("micro-canvas")
    }
}

fn render_canvas_scene(state: &CanvasScene, cell_width: u16, cell_height: u16) -> Vec<String> {
    if cell_width == 0 || cell_height == 0 {
        return Vec::new();
    }

    let left_width = cell_width / 2;
    let right_width = cell_width.saturating_sub(left_width + 1);
    let top_height = cell_height / 2;
    let bottom_height = cell_height.saturating_sub(top_height + 1);

    if right_width < 4 || bottom_height < 2 {
        return render_panel(state, cell_width, cell_height, PanelKind::Radar);
    }

    let radar = render_panel(state, left_width, top_height, PanelKind::Radar);
    let wave = render_panel(state, right_width, top_height, PanelKind::Wave);
    let spectrum = render_panel(state, left_width, bottom_height, PanelKind::Spectrum);
    let portal = render_panel(state, right_width, bottom_height, PanelKind::Portal);

    let mut lines = Vec::with_capacity(cell_height as usize);
    for row in 0..top_height as usize {
        lines.push(format!("{} {}", line_at(&radar, row), line_at(&wave, row)));
    }
    lines.push(" ".repeat(cell_width as usize));
    for row in 0..bottom_height as usize {
        lines.push(format!(
            "{} {}",
            line_at(&spectrum, row),
            line_at(&portal, row)
        ));
    }
    lines
}

#[derive(Debug, Clone, Copy)]
enum PanelKind {
    Radar,
    Wave,
    Spectrum,
    Portal,
}

fn render_panel(
    state: &CanvasScene,
    cell_width: u16,
    cell_height: u16,
    kind: PanelKind,
) -> Vec<String> {
    if cell_width == 0 || cell_height == 0 {
        return Vec::new();
    }

    let mut canvas = Canvas::new();
    canvas.set_bound(
        (0, cell_width.saturating_sub(1) as i32),
        (0, cell_height.saturating_sub(1) as i32),
    );
    canvas.fixed_bound(true);

    let dot_width = cell_width as i32 * 2;
    let dot_height = cell_height as i32 * 4;
    let speed = if state.live { 2.2 } else { 0.25 };
    let phase = state.elapsed_secs as f64 * speed;
    let cx = (dot_width - 1) as f64 / 2.0;
    let cy = (dot_height - 1) as f64 / 2.0;

    match kind {
        PanelKind::Radar => draw_radar(state, &mut canvas, dot_width, dot_height, cx, cy, phase),
        PanelKind::Wave => draw_wave(state, &mut canvas, dot_width, dot_height, cy, phase),
        PanelKind::Spectrum => draw_spectrum(state, &mut canvas, dot_width, dot_height, phase),
        PanelKind::Portal => draw_portal(state, &mut canvas, dot_width, dot_height, cx, cy, phase),
    }

    let mut bytes = Vec::new();
    let _ = canvas.print_on(&mut bytes, false);
    String::from_utf8_lossy(&bytes)
        .lines()
        .map(str::to_owned)
        .collect()
}

fn draw_radar(
    state: &CanvasScene,
    canvas: &mut Canvas,
    dot_width: i32,
    dot_height: i32,
    cx: f64,
    cy: f64,
    phase: f64,
) {
    let star_count = (dot_width * dot_height / 16).clamp(24, 140);
    for seed in 0..star_count {
        let drift = (state.frame / 3) as i32;
        let x = (seed * 29 + drift * 3).rem_euclid(dot_width);
        let y = (seed * 17 + drift + seed / 7).rem_euclid(dot_height);
        if (seed + state.pulse as i32) % 5 != 0 {
            canvas.set(x, y);
        }
    }

    let max_radius = (dot_height as f64 * 0.42).max(4.0);
    for scale in [0.42, 0.68, 0.92] {
        let radius = max_radius * scale;
        let steps = (dot_width as usize * 2).clamp(48, 160);
        for i in 0..steps {
            let t = i as f64 / steps as f64 * std::f64::consts::TAU + phase * 0.08;
            canvas.set(cx + t.cos() * radius * 1.75, cy + t.sin() * radius);
        }
    }

    let sweep = phase * 0.85;
    for r in 0..dot_height.min(dot_width / 2) {
        let r = r as f64;
        canvas.set(cx + sweep.cos() * r * 1.9, cy + sweep.sin() * r);
        canvas.set(
            cx + (sweep - 0.13).cos() * r * 1.6,
            cy + (sweep - 0.13).sin() * r,
        );
    }
}

fn draw_wave(
    state: &CanvasScene,
    canvas: &mut Canvas,
    dot_width: i32,
    dot_height: i32,
    cy: f64,
    phase: f64,
) {
    for x in 0..dot_width {
        let xf = x as f64;
        let y = cy + (xf * 0.16 + phase).sin() * (dot_height as f64 * 0.23);
        canvas.set(xf, y);
        if state.scan && x % 2 == 0 {
            let y2 = cy + (xf * 0.10 - phase * 0.8).cos() * (dot_height as f64 * 0.34);
            canvas.set(xf, y2);
        }
    }

    let particle_count = if state.scan { dot_width } else { dot_width / 2 };
    for seed in 0..particle_count {
        let xf = (seed * 5 + state.frame as i32).rem_euclid(dot_width) as f64;
        let drift = (seed as f64 * 0.37 + phase).sin();
        let y = cy
            + (xf * 0.13 + phase * 1.4).sin() * (dot_height as f64 * 0.18)
            + drift * (dot_height as f64 * 0.12);
        canvas.set(xf, y);
        if seed % 5 == 0 {
            canvas.set(xf + 1.0, y + drift.signum());
        }
    }
}

fn draw_portal(
    state: &CanvasScene,
    canvas: &mut Canvas,
    dot_width: i32,
    dot_height: i32,
    cx: f64,
    cy: f64,
    phase: f64,
) {
    let max_radius = (dot_height as f64 * 0.46).max(4.0);
    let twist = match state.mode.as_str() {
        "safe" => 0.42,
        "fast" => 0.95,
        _ => 0.68,
    };

    for arm in 0..5 {
        let offset = arm as f64 / 5.0 * std::f64::consts::TAU;
        for step in 0..42 {
            let p = step as f64 / 42.0;
            let radius = 1.5 + p * max_radius;
            let angle = offset + phase * twist + p * std::f64::consts::TAU * 1.45;
            let flare = 1.2 + p * 0.9;
            canvas.set(cx + angle.cos() * radius * flare, cy + angle.sin() * radius);
            if step % 6 == 0 {
                canvas.set(
                    cx + (angle + 0.18).cos() * radius * flare,
                    cy + (angle + 0.18).sin() * radius,
                );
            }
        }
    }

    for ring in [0.22, 0.38, 0.56] {
        let radius = max_radius * ring;
        let steps = (dot_width as usize * 2).clamp(36, 120);
        for i in 0..steps {
            let t = i as f64 / steps as f64 * std::f64::consts::TAU - phase * 0.25;
            if i % 3 != 0 {
                canvas.set(cx + t.cos() * radius * 1.7, cy + t.sin() * radius);
            }
        }
    }

    for burst in 0..18 {
        let angle = burst as f64 / 18.0 * std::f64::consts::TAU + phase * 1.3;
        let radius = max_radius * (0.55 + (burst % 5) as f64 * 0.09);
        canvas.line(
            (cx + angle.cos() * 2.0, cy + angle.sin() * 1.0),
            (cx + angle.cos() * radius * 1.9, cy + angle.sin() * radius),
        );
    }

    for core in 0..10 {
        let t = core as f64 / 10.0 * std::f64::consts::TAU + phase * 2.2;
        canvas.set(cx + t.cos() * 2.4, cy + t.sin() * 1.4);
    }
    canvas.set(cx, cy);

    if state.phase == "prod" {
        for rift in 0..3 {
            let angle = phase * 0.7 + rift as f64 * 2.1;
            canvas.line(
                (cx, cy),
                (
                    cx + angle.cos() * max_radius * 2.0,
                    cy + angle.sin() * max_radius,
                ),
            );
        }
    }
}

fn draw_spectrum(
    state: &CanvasScene,
    canvas: &mut Canvas,
    dot_width: i32,
    dot_height: i32,
    phase: f64,
) {
    let bands = (dot_width / 5).clamp(6, 18);
    let band_gap = (dot_width / bands.max(1)).max(3);
    for band in 0..bands {
        let height =
            2 + ((band as f64 * 0.8 + phase * 1.2).sin().abs() * dot_height as f64 * 0.25) as i32;
        let x = 2 + band * band_gap;
        for y in 0..height {
            canvas.set(x, y);
            if band % 2 == 0 {
                canvas.set(x + 1, y);
            }
        }
    }

    let noise = if state.scan {
        dot_width / 2
    } else {
        dot_width / 4
    };
    for seed in 0..noise {
        let x = (seed * 11 + state.pulse as i32).rem_euclid(dot_width);
        let y = (seed * 7 + (phase * 3.0) as i32).rem_euclid(dot_height);
        canvas.set(x, y);
    }
}

fn line_at(lines: &[String], row: usize) -> String {
    lines.get(row).cloned().unwrap_or_default()
}

fn canvas_row_color(index: usize) -> Color {
    match index % 4 {
        0 => Color::Rgb(111, 224, 255),
        1 => Color::Rgb(159, 255, 190),
        2 => Color::Rgb(186, 170, 255),
        _ => Color::Rgb(255, 204, 102),
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn log_lines(state: &State) -> Vec<LogLine> {
    vec![
        LogLine::new(
            LogLevel::Info,
            format!("INFO frame={} phase={}", state.frame, state.phase),
        ),
        LogLine::new(
            LogLevel::Debug,
            format!("DEBUG mode={} scan={}", state.mode, state.scan),
        ),
        LogLine::new(
            LogLevel::Info,
            format!("INFO deployment={}", compact(&state.deployment)),
        ),
        LogLine::new(
            LogLevel::Warn,
            format!("WARN alert={}", compact(&state.alert)),
        ),
        LogLine::new(LogLevel::Trace, "TRACE widget tree rendered"),
    ]
}

fn compact(value: &str) -> &str {
    if value.is_empty() {
        "none"
    } else {
        value
    }
}
