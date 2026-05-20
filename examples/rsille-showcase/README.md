# rsille showcase

A larger terminal demo for the local `rsille` crate. It is intentionally split
into two kinds of examples:

- practical surfaces that look like tools people actually use: repository
  browsing, file preview, diff review, process monitoring, and a calculator.
- a pure showoff surface that exists to make the braille canvas feel fast,
  dense, animated, and visually surprising.

Run it from the repository root:

```bash
cargo run --manifest-path examples/rsille-showcase/Cargo.toml
```

Tabs:

- `Workbench` is a compact repo review cockpit with a real file tree, source
  preview, generated diff, markdown notes, and structured logs.
- `Top` is an enhanced process monitor with filtering, sortable table data,
  multi-selection, inspector metrics, and a live resource timeline canvas.
- `Calculator` is a small arithmetic tool with controlled input, a clickable
  keypad, submit history, and parser feedback.
- `Showoff` is the theatrical side: particle tunnel, wireframe cube, and signal
  field scenes drawn with the rsille braille canvas.

Useful controls:

- `Tab` moves focus through widgets.
- Arrow keys navigate tabs, file trees, lists, tables, menus, and radio groups.
- `Enter` or `Space` activates focused controls.
- Use the `Cmd` button to open the command palette.
- `Esc` quits the app.
