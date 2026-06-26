# ascii-animation

Terminal-only ASCII animation app written in Rust.

## How it works

`ascii-animation` uses code-defined preset descriptors as the source of truth for:

- CLI flags
- TUI controls
- scene config validation
- command export

A scene contains one or more animation instances. Each instance points at a preset, validated option values, placement, layer, and z-index. Rendering composes all instances into a terminal frame buffer, then prints ANSI truecolor output or monochrome output with `--no-color`.

Current built-in presets:

- `galaxy` — rotating ASCII spiral galaxy
- `text-art` — animated ASCII text with font/effect/background options

The app has two entrypoints:

- `ascii-animation run` — run one preset directly or load a saved scene config
- `ascii-animation tui` — open the interactive editor with live preview

Saved scenes live at `~/.config/ascii-animation/scene.toml`.

## Install

This project is installed from source. npm is not part of the install flow.

### Prerequisites

- [mise](https://mise.jdx.dev/) or an equivalent Rust toolchain setup
- Rust stable

If you use mise, the repo already pins Rust in `mise.toml`:

```sh
mise use -g github:seenark/ascii-animation
```

### Install the binary

```sh
cargo install --path .
```

That places `ascii-animation` in Cargo's bin directory.

### Run without installing

```sh
cargo run -- run galaxy
cargo run -- tui
```

## How to use

### 1. Run a preset directly

```sh
ascii-animation run galaxy
```

Override preset options with flags:

```sh
ascii-animation run galaxy --arms 4 --stars 800 --palette nebula --gradient starry
```

Run text art directly:

```sh
ascii-animation run text-art --text "HELLO" --text-font Block --text-effect wave
```

Disable ANSI color output:

```sh
ascii-animation run galaxy --no-color
```

Use a fixed seed for repeatable output:

```sh
ascii-animation run galaxy --seed 17
```

### 2. Open the TUI editor

```sh
ascii-animation tui
```

Core controls:

- `↑` / `↓` — choose option
- `←` / `→` — change selected option
- `Enter` — start editing a text field
- `Tab` / `Shift+Tab` — switch animation instance
- `a` / `d` — add or delete an instance
- `p` / `P` — cycle preset
- `m` / `M` — cycle placement
- `l` / `L` — cycle layer
- `[` / `]` — change z-index
- `s` — save `~/.config/ascii-animation/scene.toml`
- `c` — copy the exported CLI command
- `q` / `Esc` — quit

### 3. Run a saved scene

If `~/.config/ascii-animation/scene.toml` exists, these commands load it:

```sh
ascii-animation run --scene default
ascii-animation run
```

You can also point at an explicit config file:

```sh
ascii-animation run --config ./scene.toml
```

### 4. Export behavior

- Single-instance scenes export as a direct command such as `ascii-animation run galaxy ...`
- Multi-instance or non-directly-exportable scenes export as:

```sh
ascii-animation run --config ~/.config/ascii-animation/scene.toml
```

## Development

```sh
cargo test
cargo run -- run galaxy
cargo run -- tui
```
