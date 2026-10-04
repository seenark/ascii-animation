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
- `matrix` — seeded digital rain with bright heads and fading ASCII trails
- `starfield` — forward flight through perspective-projected stars
- `plasma` — a dense, changing mathematical field
- `fire` — persistent heat with fuel, cooling, and wind
- `confetti` — recurring ASCII particle bursts with drag and gravity

`text-art` also offers `decrypt` and `scattered`. Both reveal the configured FIGlet lettering, hold the completed or hidden text when requested, and repeat. Existing fonts and effects remain available.

The app has two entrypoints:

- `ascii-animation run` — run one preset directly or load a saved scene config
- `ascii-animation tui` — open the interactive editor with live preview

Saved scenes live at `~/.config/ascii-animation/scene.toml`.

CLI playback and editor preview use the same persistent Scene session. Pausing freezes its clock; resizing only the viewport does not restart animations. State-changing edits restart only affected instances and are marked in the inspector. Seeds remain stable during a live session, including reordering and duplicate display identifiers. Export starts a fresh session from committed settings, not from the current preview frame.

New presets default to the same Center placement, Normal layer, and zero Z-index as existing presets. Choose Fill and Background in Layout for a full-field background; this uses configuration-backed export.

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

Run the new presets with their namespaced options:

```sh
ascii-animation run matrix --matrix-density 0.7 --matrix-trail 12 --seed 17 --no-color
ascii-animation run starfield --starfield-count 200 --starfield-streak 3
ascii-animation run plasma --plasma-frequency 1.2 --plasma-contrast 1.5
ascii-animation run fire --fire-fuel 1 --fire-cooling 0.06 --fire-wind 0.5
ascii-animation run confetti --confetti-count 160 --confetti-repeat 3
ascii-animation run text-art --text "HELLO" --text-effect decrypt --text-speed 1 --text-hold-visible-seconds 2
ascii-animation run text-art --text "HELLO" --text-effect scattered --text-hold-hidden-seconds 0.5
```

The new preset options cover density/speed/trail/palette for Matrix, count/speed/streak/palette for Starfield, frequency/speed/contrast/palette for Plasma, fuel/cooling/wind/palette for Fire, and count/repeat/palette for Confetti. Each flag uses its preset prefix. New preset glyphs are printable ASCII and remain readable with `--no-color`.

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

The editor opens directly into a live preview. Wide terminals show the Scene list and inspector beside it. At 80–109 columns and at least 24 rows, the preview sits above the editor. Smaller terminals use full-width Preview, Scene, and Edit views; Tab switches focus and the active view. The inspector scrolls to keep its selected field visible.
Field help shows exact CLI flags, including options in the Layout group; Scene-only controls are labeled separately. A cropped-Canvas warning remains visible in fullscreen preview without resizing the animation or its FIGlet glyphs.

Core controls:

- `Tab` / `Shift+Tab` — move focus between Preview, Scene, and Edit
- `Up` / `Down` — select an instance or inspector field
- `Left` / `Right` — adjust a value; hold Shift for faster numeric changes
- `Enter` — edit text or choices, or commit an edit
- `Escape` — cancel the current edit, browser, or panel; it does not quit
- `a` — open the searchable preset browser; Enter adds and selects its highlighted preset
- Scene focus: `d` / `Delete` removes an instance with confirmation; `r` replaces its preset with confirmation; `[` / `]` changes instance order
- Edit Layout: adjust Enabled, Placement, Layer, and Z-index; the final instance cannot be deleted
- `Space` — pause or resume without restarting
- `f` — toggle fullscreen preview
- `s` — save `~/.config/ascii-animation/scene.toml`
- `c` — open the wrapped, scrollable export panel
- `?` — open complete keyboard help
- `q` — request quit; unsaved changes offer Save, Discard, and Cancel

Text edits support cursor movement, Home/End, Backspace, and Delete. Printable shortcut characters remain text while editing or searching. Escape restores the committed value and keeps the live session running.
Invalid drafts keep the committed Scene unchanged. The inspector shows a concise validation reason beside the draft and in the status line, including at the minimum supported 36-column layout.

The unsaved marker applies to every Scene, including new defaults and normalized startup settings. A failed save keeps the Scene in memory and leaves prior saved data recoverable. Invalid startup files offer Reload or an unsaved default without overwriting the original.

Export uses committed settings only. Configuration-backed export offers **Save and Copy** when the Scene is unsaved, and copies only after saving succeeds. Clipboard failure leaves the full command visible for manual copying. Viewing export never silently saves.

Adapted algorithms and exact upstream revisions are recorded in `THIRD_PARTY_NOTICES`, including their MIT notices and text-effect attribution chain.

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
