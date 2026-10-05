# Keyboard-first TUI and Saved Scenes design

Date: 2026-10-05

Status: **Confirmed design and acceptance criteria. Implementation is tracked by `ascii-animation-lmw`.**

Tracking: design Beads `ascii-animation-92a`; implementation Beads `ascii-animation-lmw`.

## Goal and approved decisions

Make choosing, adjusting, saving, reopening, and playing animations easy without memorizing hidden controls.

The user confirmed:

1. Save means multiple named Saved Scenes, not command bookmarks.
2. Choosing one Preset and adjusting its options is the primary workflow. Multi-instance composition remains supported.
3. Keyboard interaction is more important than mouse interaction. Mouse supports clicks and wheel scrolling, not dragging.
4. The TUI starts with Preset selection and a live preview, with an explicit Saved Scenes entry.
5. Save updates the current Saved Scene; Save As creates another. A conflicting name requires confirmation before overwrite.
6. Copied commands refer to Saved Scenes on the same machine. Self-contained cross-machine commands are not required.
7. Every surface explains available keys and actions, including fullscreen and dialogs.
8. The original design delivery was documentation-only. Implementation is separately authorized under `ascii-animation-lmw`; remote push is not authorized.

Product facts live in [PRODUCT.md](../../../PRODUCT.md). Domain definitions live in [CONTEXT.md](../../../CONTEXT.md). This design replaces the original [2026-06-23 design](2026-06-23-ascii-animation-design.md)'s single-slot restriction and TUI startup flow only when implemented. The old research document is historical evidence, not a description of the current runtime.

## Pre-implementation source facts

These historical findings describe base commit `3eadfe02fe9765e6589817b15cd08d62b153da76`, not the implemented TUI. The linked line ranges refer to that baseline:

- [Scene persistence](../../../src/scene.rs#L84-L163) writes full Scene TOML to a path, using a temporary file and rename. TUI Save currently targets one configured file.
- [Current command export](../../../src/scene.rs#L165-L200) uses direct arguments for a simple Scene and a config reference for other Scenes. A command referencing the shared default file does not independently preserve an earlier Scene after that file is overwritten.
- [CLI inputs](../../../src/cli.rs#L242-L294) already accept `run --config <path>` and direct Preset arguments. The existing `--scene` input recognizes `default`; arbitrary named Scene lookup is not implemented.
- [Current hints](../../../src/tui.rs#L775-L808) vary by editor context. [Fullscreen rendering](../../../src/tui.rs#L725-L734) only shows its return hint when the Canvas is cropped.
- The inspected TUI event loop handles key and resize events, not mouse actions. Named Saved Scenes and mouse interaction remain new work.

## Language and persistence contract

- A **Preset** is an animation type. A **Scene** is the complete composition. A **Saved scene** is a named persisted Scene. Do not label saved configurations as Presets.
- An animation instance is not a Layer. Use **Animations** for the instance list; reserve **Layer** for background, normal, or foreground priority.
- Persist the complete existing Scene configuration, including shared settings, instance options, enabled states, Placement, Layer, Z-index, and order. Do not build a shell-command bookmark store.
- Each Saved Scene has a distinct configuration target. Saving Scene B must not change Scene A or make A's command load B.
- A Saved Scene command is a reference to that Scene's current saved configuration. Later Save operations on that same Scene intentionally change what the command plays; the command is not an immutable historical snapshot.
- Save As preserves the original saved target and makes the new Saved Scene the current editing target after success.
- Continue using full Scene TOML and the existing atomic-save approach. The exact library directory and name-to-filename mapping are implementation choices, not new product features.
- Names are labels, not executable shell input. Validate names before deriving paths, prevent directory traversal, and quote the resolved config path when generating a command. Ordinary spaces in names must work.
- The existing default Scene remains accessible without silently moving, overwriting, or deleting its file. Existing direct CLI flags, `--config`, and `--scene default` retain their meanings.
- Save does not record video, current frame, elapsed time, or simulation history. Reopening or direct playback starts a new runtime session; frame-for-frame replay is not promised.

## Screen hierarchy

### Preset selection

The entry surface contains a selectable Preset list, its description, and a live preview. Selection updates preview without saving or committing an edit. Enter or the visible Edit action creates a new single-instance editing Scene from the selected Preset.

Saved Scenes, Help, and Quit are visible actions. Preserve searchable Preset discovery. Starting a new Scene must not overwrite an existing save.

### Editor

Keep the live preview dominant and group descriptor-derived controls beside it. Show the current Saved Scene name, unsaved status, and paused/running state. Every selected option has its label, value, help, and relevant controls.

Make the simple path short: choose a Preset, adjust values, Save, and Copy Command. Do not require visiting a composition screen to adjust one animation.

Provide a visible Animations action for adding, selecting, removing, replacing, enabling, or reordering instances and editing their Placement, Layer, and Z-index. Keep this separate from the primary option controls so a single-instance Scene does not resemble a mandatory multi-layer setup wizard.

### Saved Scenes

Show named entries with a preview and visible Open/Edit, Play, and Copy Command actions. Opening restores the full composition, not just the selected instance. Play presents fullscreen playback; returning restores the invoking surface.

An empty collection explains how to create the first Saved Scene. An unreadable or invalid entry shows an actionable error; it must not be replaced with a default file automatically. The current default file remains discoverable.

### Supporting surfaces

Use short, consistent dialogs for naming a Scene, editing a value, confirming name conflicts, confirming deletion or Preset replacement, handling unsaved work, displaying/copying commands, showing Help, and recovering from read/save errors.

Fullscreen is still a UI surface. It has a persistent compact control strip even when the Canvas fits. Do not depend on a cropping warning to tell users how to return.

## Functional layout intent

These are functional schematics, not screenshots or a replacement visual identity. Use the existing terminal rendering and control conventions; terminal fonts remain user-controlled.

```text
Preset selection
+----------------------+-----------------------------------+
| Presets              | Live preview                      |
| Selected item        |                                   |
| Other items          |                                   |
| Search               | Selected Preset description       |
+----------------------+-----------------------------------+
| Edit selected | Saved Scenes | Help | Quit                |
| Contextual keys and visible focus                        |
+----------------------------------------------------------+
```

```text
Editor
+----------------------------------------------------------+
| Scene name | Unsaved/saved | Running/paused               |
+----------------------+-----------------------------------+
| Grouped options      | Live preview                      |
| Selected value/help  |                                   |
| Animations           |                                   |
+----------------------+-----------------------------------+
| Save | Save As | Copy Command | New | Saved Scenes        |
| Contextual keys | Pause | Fullscreen | Help | Quit         |
+----------------------------------------------------------+
```

```text
Saved Scenes
+----------------------+-----------------------------------+
| Saved Scene names    | Selected Scene preview            |
| Selected item        |                                   |
| Other saved items    |                                   |
+----------------------+-----------------------------------+
| Open/Edit | Play | Copy Command | New | Back | Help        |
| Contextual keys and visible focus                        |
+----------------------------------------------------------+
```

In compact terminals, show one work pane at a time with a visible way to switch panes; keep current selection, draft, and unsaved work. Action strips wrap or expose remaining actions through a labeled, keyboard-accessible and clickable menu. Do not truncate key/action pairs into unreadable fragments.

Preserve the shared viewport rule between editor preview and direct playback. A layout change must not silently change composition, crop saved data, or reset a simulation.

## End-to-end workflows

### Create and save

1. Select a Preset and inspect the live preview.
2. Activate Edit and adjust its options.
3. Activate Save. For a new Scene, enter a name and confirm.
4. On successful save, show the name and saved state. The Scene appears in Saved Scenes.
5. Canceling naming retains the editing Scene and unsaved state. A failed save retains the draft and reports a recoverable error.

### Update or create a variation

Save on an opened Saved Scene updates only its current target. Save As asks for a name and writes another complete Scene; the original saved configuration remains unchanged.

If another Saved Scene already has the requested name, show which Scene would be overwritten and require explicit confirmation. Cancel leaves both saved files and the editing state unchanged. A failed save must not mark the editor clean or switch its current target.

### Reopen and play

Select a Saved Scene to preview it. Open/Edit restores it to the editor. Play enters fullscreen; Pause/Resume and Back remain available by keyboard and mouse. Returning from playback restores the invoking surface and selection.

Browsing alone does not replace the current editing Scene. Before actually replacing unsaved work, offer Save / Discard / Cancel.

### Copy a playback command

For a Saved Scene, Copy Command emits `ascii-animation run --config <quoted-resolved-path>` for that Scene, including single-instance Scenes. Direct Preset CLI flags remain available outside this saved-reference workflow.

If the editing Scene is new or dirty, label the action **Save and Copy**. A new Scene first needs a name; an existing Scene saves to its own target. Copy occurs only after a successful save. Canceled or failed saves must not copy a stale command or change the clipboard.

Explain that the command uses a local Scene file. Show the command and its target; allow scrolling long commands. A clipboard failure preserves the saved file and displays the command for manual copying. Do not claim copy success when clipboard access fails.

### Leave unsaved work

New Scene, Open/Edit of another Scene, and Quit must not silently discard changes. The guard offers Save / Discard / Cancel. Save failure keeps the editor open. Discard requires an explicit choice; Cancel returns to the same editing state.

Canceling a value draft changes neither the committed Scene nor its saved state. A partially typed value is not silently saved or exported.

## Keyboard-first interaction

- Every action has a keyboard route. Mouse availability never changes the available feature set.
- Keep existing contextual bindings where their meanings remain valid: arrows for selection/adjustment, Tab/Shift+Tab for visible focus targets, Enter for activation or commit, Escape for cancel/back, Space for Pause/Resume, `f` for fullscreen, `s` for Save, `c` for Copy, `?` for Help, and `q` for Quit. Retain instance operations and faster numeric adjustment.
- Assign discoverable, terminal-compatible bindings to New, Saved Scenes, and Save As. Display their actual bindings beside their actions; do not depend on modifier combinations the terminal cannot distinguish.
- Text/search/name entry owns printable keys. Typing a letter such as `s`, `c`, or `q` must not save, copy, or quit.
- Focus is always visible and navigation is predictable. Scroll lists and controls to keep the selected item visible.
- Escape cancels a draft or closes the current surface before it can leave the editor. It does not silently quit or commit.
- Do not hide essential actions behind hover, mouse-only controls, or an unexplained icon.

## Mouse supplement and action hints

A click focuses/selects an item or activates its labeled action. Supply clickable adjustment controls for values and visible Open/Edit/Play/confirm/cancel actions; do not require double-clicking or dragging. Text entry can still use the keyboard.

Wheel events scroll the relevant list, option pane, command, or help text. Do not treat incidental scrolling as a value edit. Modal surfaces own input: clicks must not activate obscured background actions.

Clickable actions and keyboard shortcuts perform the same action with the same validation and data-loss guards. Use actual rendered geometry for hit targets; after resizing, no old or off-screen target may fire. Enable terminal mouse reporting only for the TUI and restore it on exit. Unsupported reporting must not block keyboard workflows.

Every surface has contextual hints with an action verb and its actual key, a visible Help route, and readable feedback. Cover Preset selection, editor panes, Animations, Saved Scenes, value/name entry, overwrite/delete/replace confirmation, unsaved-work guard, command display, Help, recovery/errors, fullscreen, and the too-small-terminal warning. Hints need not list every global shortcut simultaneously, but all actions must be discoverable through a labeled control or the visible Help/action menu.

## Acceptance criteria

These are implementation acceptance scenarios, not claims of completed tests.

### UX and workflows

- **UX-1:** A fresh TUI opens in Preset selection with live preview and visible Saved Scenes access. Selecting another Preset does not write a file; Edit opens the selected Preset with its descriptor defaults.
- **UX-2:** A keyboard-only user completes choose, edit, Save, Save As, reopen, Play, return, Copy Command, and Quit without a mouse. Focus and selection remain visible throughout.
- **UX-3:** Changing an option updates preview with the descriptor's validation and restart behavior. Canceling text/choice entry leaves the committed value unchanged.
- **UX-4:** Multi-instance composition remains accessible. After saving and reopening a composition, options, disabled instances, Placement, Layer, Z-index, order, frame rate, and color are preserved.
- **UX-5:** Browsing Presets or Saved Scenes and canceling returns to the prior editor state. Replacing unsaved work or quitting requires Save / Discard / Cancel; failed Save does not leave the editor.

### Saved Scenes and commands

- **SAVE-1:** First Save asks for a name, persists the complete Scene, adds a named library entry, and marks the editor saved only after success. Cancel keeps the unsaved Scene and creates no file.
- **SAVE-2:** Save updates Scene A without changing Scene B. A previously copied command for A still targets A after B is saved.
- **SAVE-3:** Save As from A creates B, makes B the current saved target, and leaves A's saved configuration unchanged. Later Save updates B, not A.
- **SAVE-4:** A name conflict identifies the existing Scene and requires confirmation. Cancel or failed write preserves the prior target and editor state.
- **SAVE-5:** A failed save leaves the existing saved file intact, preserves unsaved changes, and shows a readable error with retry/cancel guidance.
- **SAVE-6:** Names with spaces produce valid commands. Names cannot escape the Scene storage location or inject shell syntax into the generated command.
- **SAVE-7:** Missing, empty, or unreadable library states explain how to proceed. An invalid saved entry is not automatically overwritten; existing default configuration stays accessible and unchanged.
- **CMD-1:** Copy of a clean Saved Scene references that Scene's actual config target. Running that command loads the saved composition rather than whichever Scene was most recently edited.
- **CMD-2:** Copy of a new or dirty Scene shows Save and Copy. Name cancellation, overwrite cancellation, or save failure produces no copy and leaves the clipboard unchanged.
- **CMD-3:** Clipboard failure does not undo a successful save. The command remains visible for manual copying, with an honest failure message.
- **CMD-4:** The command surface explains its local-file dependency. Existing direct CLI flags, `--config`, and `--scene default` retain their meanings.

### Key/action guidance

- **HELP-1:** Every surface in the inventory above shows relevant action/key pairs and a Help or action-menu route. No displayed action requires guessing an unlabeled key or icon.
- **HELP-2:** Fullscreen always shows Pause/Resume, Back, and Help controls, including when the Canvas fits and no cropping warning is present.
- **HELP-3:** Name, search, and text-entry surfaces visibly distinguish typing from action shortcuts. Typing shortcut letters edits text instead of invoking global actions.
- **HELP-4:** Long commands, option lists, Help, and errors can be scrolled. Selected items and confirmation buttons remain reachable and visible in compact layouts.

### Mouse without weakening keyboard access

- **MOUSE-1:** Clicking visible list entries, fields, adjustment controls, and action buttons performs the same operation and validation as keyboard input. Each activation occurs once; no hover or drag is required.
- **MOUSE-2:** Wheel scrolling affects the relevant visible pane and cannot edit an option unintentionally. An open dialog prevents background actions from receiving clicks.
- **MOUSE-3:** Resizing updates hit targets. A click on an old, hidden, or clipped button location cannot activate that action.
- **MOUSE-4:** With no mouse events, all keyboard workflows still work. Exiting the TUI restores cursor, raw mode, alternate screen, and mouse reporting.

### Responsive behavior and preserved rendering

- **LAYOUT-1:** Exercise the main surfaces at 120x38, 80x24, 60x18, and 36x10. Every action remains reachable, key/action labels remain intelligible, and selection does not scroll out of view.
- **LAYOUT-2:** Below the supported minimum, show a clear resize warning and retain work. Resizing back restores selection and drafts without a stale mouse target.
- **LAYOUT-3:** Changing focus, opening Help, browsing, and resizing do not mutate saved composition or arbitrarily restart the editor session. Preserve existing renderer lifecycle and shared preview/direct viewport behavior.

## Verification required when implemented

Run the actual TUI in an isolated temporary config directory. Complete the keyboard-only path first; then repeat core actions with clicks and wheel events. Capture the main, compact, fullscreen, dialog, and error surfaces. Verify A/B saved-file isolation and execute each copied command against its saved file. Exercise failed writes, canceled naming/overwrite, clipboard failure, and terminal restoration.

Use existing behavioral tests where appropriate, but source-string checks and mocked action forwarding are not proof of usability. This design-only task does not run those implementation checks or claim visual verification.

## Boundaries and implementation handoff

Keep the Rust terminal stack, preset descriptors, Scene model, existing animations, composition rules, renderer lifecycle, and CLI input contracts. Reuse current option validation, draft handling, save/export safety, clipboard feedback, viewport helpers, and recovery behavior rather than establishing parallel conventions.

Do not add video/GIF export, runtime snapshots, frame-perfect replay, drag-based placement, command bookmarks, cloud sync, cross-machine bundles, arbitrary shell execution, a web UI, or a new visual identity. Saved Scene rename/delete, autosave, and history/versioning are not part of the approved feature set.

Storage naming and the new actions' exact keys remain implementation choices constrained by this document. They must not change the approved workflows or weaken keyboard access. Production code, existing user files, README claims about shipped features, and git history are intentionally unchanged by this design delivery.

[Showing lines 1-238 of 239. Use :239 to continue]