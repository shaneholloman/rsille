# TUI Examples

Examples are grouped by intent so the directory stays navigable as the set grows.

## Categories

- `basics/`: minimal starting points and first-run examples
- `themes/`: theme and styling system demos
- `layout/`: composition and layout primitives
- `widgets/`: focused widget behavior demos
- `apps/`: end-to-end application patterns and state orchestration
- `scenes/`: richer animated or visual showcase examples

## Running Examples

Example commands stay flat even though files are grouped in subdirectories:

```bash
cargo run -p tui --example hello
cargo run -p tui --example primitives
cargo run -p tui --example theme
cargo run -p tui --example async
cargo run -p tui --example controls
cargo run -p tui --example animation
cargo run -p tui --example layout_animation
cargo run -p tui --example presence_animation
cargo run -p tui --example visual_effects
cargo run -p tui --example label_art
cargo run -p tui --example tree
cargo run -p tui --example flame
```

`visual_effects` is the visual regression entry for the post-processing engine:
it cycles through fade, gradient, shatter, magic-lamp, wipe, dissolve, wave, and
glitch while showing compact, wide, reduced, and disabled-motion stages.

## Adding A New Example

1. Put the file in the most appropriate category directory.
2. Add a matching `[[example]]` entry in `packages/tui/Cargo.toml`.
3. Keep the example name short and stable so commands stay easy to remember.

If a new class of examples starts to form, add a new top-level category instead of flattening everything back into `examples/`.
