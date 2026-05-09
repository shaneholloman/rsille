# rsille showcase

A compact production-style terminal demo for the local `rsille` crate. It keeps
the default view readable in a typical 80-column terminal while showing a
braille canvas that fills its allocated widget box, focusable controls, tables,
logs, overlays, and source viewers.

Run it from the repository root:

```bash
cargo run --manifest-path examples/rsille-showcase/Cargo.toml
```

Useful controls:

- `Tab` moves focus through widgets.
- Arrow keys navigate lists, tabs, tables, and menus.
- `Enter` or `Space` activates focused controls.
- `Esc` quits the app.
