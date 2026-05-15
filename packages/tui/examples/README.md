# TUI Examples

These examples are intentionally small. Each file should demonstrate one TUI
widget or one framework feature, including its common variants. Larger app-like
showcases belong in the repository-level `examples/` directory as separate
projects.

## Categories

- `basics/`: minimal first-run examples.
- `layout/`: layout functions and containers such as flex, grid, overlay, scroll, split, and stack.
- `motion/`: animation and visual-effect wrappers.
- `themes/`: theme and styling system examples.
- `widgets/`: focused examples for built-in widgets.

## Running Examples

Example names stay flat even though files are grouped in subdirectories:

```bash
cargo run -p tui --example hello
cargo run -p tui --example flex
cargo run -p tui --example grid
cargo run -p tui --example button
cargo run -p tui --example text_input
cargo run -p tui --example data_table
cargo run -p tui --example command_palette
cargo run -p tui --example visual
```

Use this to check the full set:

```bash
cargo check -p tui --examples
```

## Adding A New Example

1. Add one file for one widget or feature.
2. Show important variants in that file, but avoid building a full app.
3. Add a matching `[[example]]` entry in `packages/tui/Cargo.toml`.
4. If the example needs multiple screens, domain data, or orchestration, create
   a separate project under the repository-level `examples/` directory instead.
