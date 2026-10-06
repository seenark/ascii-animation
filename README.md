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

Named Saved Scenes live at `~/.config/ascii-animation/saved-scenes/<name>.toml`. The existing `~/.config/ascii-animation/scene.toml` remains a separate default configuration and is available in the Saved Scenes library.

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

Use `ascii-animation --help` or `ascii-animation run --help` to view CLI options.


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

The TUI starts with searchable Preset selection and a live preview. Select a Preset and press Enter to edit its descriptor defaults, or press `l` to browse Saved Scenes. Browsing does not save or replace the current editing Scene.

The editor groups options beside the preview and shows the current name, saved/unsaved state, and playback state. Wide terminals show both panes. Medium terminals stack them; compact terminals show one work pane at a time. Selection stays visible at 120x38, 80x24, 60x18, and 36x10. Below 36x10, resize guidance replaces editing while retaining work and drafts.

Core controls:

- `Tab` / `Shift+Tab` — move pane, search, or dialog focus
- `Up` / `Down` — select entries or fields; scroll Help, commands, and errors
- `Left` / `Right` — adjust a value; hold Shift for faster numeric changes
- `Enter` — activate, edit text/choices, or commit
- `Escape` — cancel or return without quitting or committing a draft
- `/` in Preset selection — enter search; Tab switches between typing and list navigation
- `s` — Save; a new Scene first asks for a name, while an opened Scene updates its own target
- `S` — Save As; create a separate target and leave the original unchanged
- `c` — open Copy Command; new or dirty Scenes require explicit Save and Copy
- `n` — choose a Preset for a new Scene
- `l` — browse Saved Scenes; Enter opens, `p` plays fullscreen, and `c` opens its command
- `v` — open Animations for advanced composition
- Animations: `a` adds, `r` replaces with confirmation, `d`/Delete removes with confirmation, and `[`/`]` reorders
- Inspector Layout (advanced): Enabled, Placement, Layer, Z-index, and custom region controls
- `Space` — pause/resume without restarting; `f` enters fullscreen
- `F1` — contextual Help/Actions on every surface; `?` also opens Help outside typing
- `q` — request Quit; unsaved work offers Save, Discard, and Cancel

Fullscreen always shows Pause/Resume, Back, and Help, even when the Canvas fits. Help/Actions exposes controls that do not fit the compact action strip. Mouse clicks use the same validated actions; wheel scrolling selects or scrolls the relevant pane without editing values. Dragging is not required.

Text, search, and names own printable shortcut characters. Text edits support arrows, Home/End, Backspace, and Delete. Escape restores committed values; invalid drafts keep the committed Scene unchanged.

Saved names allow 1–80 ASCII letters, digits, spaces, `-`, and `_`. Save As to an existing name identifies the target and asks before overwriting. Canceled or failed saves preserve the editing Scene, existing files, and current target. New/Open/Quit never silently discard unsaved work.

The library previews and reopens complete Scenes, including all instances, disabled states, options, Placement, Layer, Z-index, order, frame rate, and color. Existing default configurations remain accessible. Obsolete or invalid options are normalized in memory; Play/Copy routes through Open/Edit and an explicit Save when updating the file is necessary. Invalid or unreadable entries and directory errors remain recoverable without overwriting files.

Copy for a clean Saved Scene always references its own safely quoted absolute config path, even for one instance. Save and Copy finishes naming, conflict confirmation, and persistence before touching the clipboard. Clipboard failure retains the saved file and a scrollable manual command with an honest error. Commands depend on local files: saving B does not change A, but a later Save on A intentionally updates what A's existing command plays.

Save captures configuration, not elapsed time, simulation history, a frame, or a video. Reopening or running the command starts a fresh animation session.

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

Named Saved Scenes can be played without opening the TUI:

```sh
ascii-animation run --config "$HOME/.config/ascii-animation/saved-scenes/Scene A.toml"
```

### 4. Export behavior

- The TUI copies references to each Saved Scene's own config file, not references to whichever Scene was saved last.
- Existing direct Preset flags and CLI command generation remain available. `--scene default` still refers only to the existing default configuration; arbitrary named lookup uses `--config`.

## Development

```sh
cargo test
cargo run -- run galaxy
cargo run -- tui
```
