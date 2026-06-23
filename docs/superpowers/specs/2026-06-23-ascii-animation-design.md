# ASCII Animation Rust App Design

Date: 2026-06-23

## Goal

Build a terminal-only Rust app for running preset ASCII animations. Users can run animations directly from the CLI, edit and preview scenes in a TUI, and export copyable CLI commands for use elsewhere.

The first preset is `galaxy`, ported from the working HTML prototype. Future presets should be easy to add in code without duplicating CLI, TUI, config, and validation plumbing.

## Domain model

### Preset

A `Preset` is code-defined animation capability. It owns:

- Stable preset name, e.g. `galaxy`.
- Human label and short description.
- Option descriptors: name, type, default value, allowed range or choices, display label, and whether changing the option requires rebuilding animation state.
- Renderer factory for creating animation state from validated option values.

Preset descriptors are the source of truth for CLI flags, TUI controls, scene config validation, and command export.

### Animation instance

An `AnimationInstance` is one configured preset inside a scene. It contains:

- Instance id.
- Preset name.
- Typed option values.
- Placement: `center`, `top`, `bottom`, `left`, `right`, `fill`, or explicit custom rectangle.
- Layer: `background`, `normal`, or `foreground`.
- `z_index` inside the layer.
- Visibility/enabled flag.

### Scene

A `Scene` is the complete terminal composition. It contains one or more animation instances plus scene-level defaults such as frame rate, color mode, and terminal sizing behavior.

When multiple animations overlap, the final terminal cell is selected by:

1. Layer priority: `foreground` over `normal` over `background`.
2. Higher `z_index` within the same layer.
3. Later instance order as the final tie-breaker.

Only non-space cells participate in overwrite decisions. This keeps transparent regions simple and predictable.

### Frame buffer

Rendering writes into a terminal `FrameBuffer` of cells:

- Character.
- Optional RGB color.
- Layer metadata used during composition.

The renderer can emit ANSI truecolor output or plain monochrome output for `--no-color`.

## Runtime behavior

### Direct CLI

The CLI supports running a single preset directly:

```sh
ascii-animation run galaxy --arms 3 --stars 600 --palette cosmic --gradient smooth
```

It also supports running a saved scene:

```sh
ascii-animation run --scene default
```

Direct animation runs continuously until interrupted with Ctrl-C. The CLI should cleanly restore terminal state on exit.

### TUI

The TUI opens with a live preview and editor:

```sh
ascii-animation tui
```

The TUI supports:

- Selecting presets.
- Adding/removing animation instances.
- Editing descriptor-derived options.
- Changing placement, layer, and z-index.
- Previewing changes immediately.
- Saving scenes.
- Showing/copying the equivalent command for direct CLI use.

### Config

Scene files live in:

```text
~/.config/ascii-animation/scene.toml
```

The initial version uses this single default scene file. Named scene files are out of scope.

For a single animation instance, command export produces a full copyable `ascii-animation run <preset> ...` command. For scenes with multiple instances, command export produces `ascii-animation run --config ~/.config/ascii-animation/scene.toml`.

## First preset: `galaxy`

The first preset ports the tested HTML rotating galaxy behavior to terminal Rust.

Options:

- `arms`: integer, 1 to 10, default 3, rebuilds stars.
- `stars`: integer, 100 to 1200, step 50, default 600, rebuilds stars.
- `speed`: integer degrees per second, 1 to 60, default 20.
- `size`: integer, 20 to 100, default 70, rebuilds stars.
- `twist`: float, 0.0 to 1.0, default 0.45, rebuilds stars.
- `noise`: float, 0.0 to 0.5, default 0.15, rebuilds stars.
- `glow`: float, 0.0 to 1.0, default 0.45.
- `twinkle`: float, 0.0 to 1.0, default 0.35.
- `palette`: choice, default `cosmic`; choices `cosmic`, `stardust`, `nebula`, `rainbow`, `ice`, `mono`.
- `gradient`: choice, default `smooth`; choices `smooth`, `classic`, `starry`, `block`.

The Rust renderer should use seeded random generation so tests can assert deterministic output. Runtime may use a random seed unless the user supplies one.

## Suggested Rust structure

```text
src/
  main.rs
  cli.rs
  tui.rs
  scene.rs
  render/
    mod.rs
    buffer.rs
    ansi.rs
    layout.rs
  presets/
    mod.rs
    galaxy.rs
```

Responsibilities:

- `cli`: parse commands and flags with `clap`, map them to scenes or animation instances.
- `tui`: render live preview and descriptor-generated controls with `ratatui`/`crossterm`.
- `scene`: define `Scene`, `AnimationInstance`, placement, layers, config load/save, validation.
- `render`: define `FrameBuffer`, composition, terminal sizing, ANSI/no-color output.
- `presets`: register code-defined presets and provide renderers.

## Error handling

Errors should be explicit and user-facing:

- Unknown preset name.
- Unknown option name.
- Invalid option type.
- Out-of-range numeric option.
- Invalid choice option.
- Scene config parse failure.
- Terminal initialization or restore failure.

Validation should happen before entering the render loop whenever possible.

## Testing strategy

Tests should cover behavior, not implementation details:

1. Preset descriptor validation accepts defaults and rejects invalid ranges/choices.
2. Scene TOML round-trips through `~/.config/ascii-animation/scene.toml` shape.
3. Layer composition picks foreground over normal/background.
4. `z_index` and instance order resolve same-layer overlaps.
5. ANSI renderer emits colors by default and no escape codes with `--no-color`.
6. Galaxy renderer produces deterministic frames with a fixed seed.
7. CLI command export can be parsed back into the same animation options for a simple single-preset scene.

## Decisions

- Use descriptor-driven presets rather than manually wiring each preset into CLI/TUI/config.
- Use layer priority for overlap resolution.
- Support both single-command export and config-backed scene execution.
- Store the default scene at `~/.config/ascii-animation/scene.toml`.
- Support ANSI truecolor with a monochrome fallback.
- Keep animation presets code-defined; no external preset scripting or plugin system in the first version.

## Out of scope for the first implementation

- Browser rendering.
- External plugin loading.
- User-authored animation scripts.
- Audio or image export.
- Networked sharing of scenes.
