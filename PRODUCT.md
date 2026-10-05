# Product

<!-- impeccable:product-schema 1 -->

## Platform

Terminal: a Rust CLI and TUI. This is not a web or mobile application.

## Users

People choosing, configuring, composing, and running ASCII animations in a terminal. The confirmed primary workflow is choosing one animation and adjusting its options; composing multiple animation instances remains available.

## Product Purpose

Make it easy to try an animation, understand its controls, adjust it with a live preview, save a reusable named Scene, and run that Scene again from a command.

## Operating Context

- The TUI supports experimentation and editing; the CLI supports direct playback without opening the TUI.
- Keyboard interaction is primary. Mouse clicks and scrolling supplement keyboard interaction.
- The TUI starts with Preset selection and a live preview, with a visible entry to Saved Scenes.
- Copied commands for Saved Scenes target saved configuration on the same machine. Moving the command alone to another machine is not the approved portability contract.

## Capabilities and Constraints

### Existing product foundation

- Presets are defined in code. Their descriptors supply option definitions for CLI input, TUI controls, validation, and command export.
- A Scene contains configured animation instances and shared playback settings. Placement, Layer, Z-index, and instance order define composition.
- Persistence stores complete Scene configuration as TOML. Named Saved Scenes use `~/.config/ascii-animation/saved-scenes/<name>.toml`; the existing default file remains `~/.config/ascii-animation/scene.toml`.
- Direct preset flags and config-backed CLI playback exist. Runtime animation state is separate from serialized Scene configuration.

### Implemented keyboard-first workflows

- Store multiple named Saved Scenes, not a list of raw shell commands.
- Save asks for a name for a new Scene and updates the current Saved Scene thereafter. Save As creates a separate Saved Scene. Reusing another Scene's name requires overwrite confirmation.
- Saved Scenes can be previewed, reopened for editing, played, and referenced by a copied command.
- Save stores configuration, not a video, current frame, elapsed playback, or simulation history.
- Every surface explains its available actions and keyboard controls, including fullscreen, dialogs, and error states.
- All actions remain accessible by keyboard, with visible focus and predictable navigation. Mouse clicks and wheel scrolling provide an additional route; dragging is outside this design.
- Failed or canceled saves must not lose work, overwrite a valid saved file, or copy a command that does not match saved configuration.

## Product Principles

- Optimize the single-Preset editing path without removing multi-instance composition.
- Treat keyboard access as the primary interaction contract, not a fallback for mouse users.
- Make actions discoverable where they are used; do not require memorizing shortcuts or hovering.
- Keep Saved Scenes as the configuration source of truth and generate playback commands from them.
- Preserve work and make destructive transitions explicit.

## Accessibility & Inclusion

Provide visible focus, contextual key hints, complete keyboard access, readable errors, and usable controls in compact terminals. Mouse reporting may be unavailable; keyboard workflows must not depend on it. Hover-only controls are not acceptable.

## Evidence on Hand

The requirements above were confirmed in the design interview tracked by Beads `ascii-animation-92a` and implemented under `ascii-animation-lmw`. Locked Rust tests, check, and build passed. Actual PTY journeys exercised keyboard and mouse workflows, named-file isolation and generated commands, complete composition, all four supported layouts, fullscreen, dialogs, failed writes, compatibility recovery, and terminal restoration. Clipboard failure was exercised through the production event/render interface; automation did not overwrite the native clipboard.

See the [confirmed workflow and acceptance criteria](docs/superpowers/specs/2026-10-05-keyboard-first-saved-scenes-ux-design.md) and the domain glossary in [CONTEXT.md](CONTEXT.md).

[Showing lines 1-59 of 60. Use :60 to continue]