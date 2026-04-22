# TUI Examples

Examples are grouped by intent so the directory stays navigable as the set grows.

## Categories

- `basics/`: minimal starting points and first-run examples
- `layout/`: composition and layout primitives
- `widgets/`: focused widget behavior demos
- `scenes/`: richer animated or visual showcase examples

## Running Examples

Example commands stay flat even though files are grouped in subdirectories:

```bash
cargo run -p tui --example hello
cargo run -p tui --example tree
cargo run -p tui --example flame
```

## Adding A New Example

1. Put the file in the most appropriate category directory.
2. Add a matching `[[example]]` entry in `packages/tui/Cargo.toml`.
3. Keep the example name short and stable so commands stay easy to remember.

If a new class of examples starts to form, add a new top-level category instead of flattening everything back into `examples/`.
