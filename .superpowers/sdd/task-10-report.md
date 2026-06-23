# Task 10 Report

## Status
DONE_WITH_CONCERNS

## Summary
- Rejected mixed direct-run inputs so `--scene`/`--config` now fail fast when combined with a positional preset or preset-specific flags instead of silently ignoring them.
- Moved runtime rendering onto descriptor-registered renderer factories so every registered preset is dispatched through the registry path instead of a `galaxy` hard-code.
- Expanded TUI scene editing state and controls to add/remove instances, cycle selected instances/presets, and edit placement/layer/`z_index` alongside preset options.
- Added focused regressions for CLI input conflicts, registry-driven runtime dispatch, and TUI scene-structure editing.

## Files Changed
- `src/cli.rs`
- `src/error.rs`
- `src/presets/galaxy.rs`
- `src/presets/mod.rs`
- `src/runtime.rs`
- `src/tui.rs`
- `tests/cli.rs`
- `tests/preset_validation.rs`
- `tests/scene_config.rs`

## Exact Test Commands and Outputs
1. Red:
   - Command: `cargo test --test cli`
   - Output:
     ```text
     Failed compiling new coverage before implementation; first failure was the new registered-preset dispatch test still calling a 4-arg `PresetDescriptor::new(...)` while production only exposed the old descriptor shape.
     ```
2. Red:
   - Command: `cargo test --test scene_config`
   - Output:
     ```text
     Failed compiling new TUI coverage before implementation because `TuiState` did not yet expose instance-management APIs such as `selected_instance`, `add_instance`, `cycle_selected_instance`, `remove_selected_instance`, `set_selected_placement`, `cycle_selected_layer`, `adjust_selected_z_index`, and `cycle_selected_preset`.
     ```
3. Green:
   - Command: `cargo test --test cli`
   - Output:
     ```text
     ✓ cargo test: 11 passed (1 suite, 0.00s)
     ```
4. Green:
   - Command: `cargo test --test scene_config`
   - Output:
     ```text
     ✓ cargo test: 15 passed (1 suite, 0.01s)
     ```
5. Full regression:
   - Command: `cargo test`
   - Output:
     ```text
     ✓ cargo test: 49 passed (8 suites, 0.03s)
     ```

## Self-Review Notes
- The conflicting-input guard only rejects preset-specific inputs (`preset`, `--arms`, `--stars`, `--speed`, `--size`, `--twist`, `--noise`, `--glow`, `--twinkle`, `--palette`, `--gradient`) and intentionally still allows scene-level overrides like `--no-color` and `--seed` when loading `--scene`/`--config`.
- Runtime dispatch now calls `PresetDescriptor::create_renderer(...)`, so adding a new preset requires wiring a renderer factory into its descriptor rather than touching `runtime.rs`.
- TUI editing stays minimal but complete for the spec/review scope: keyboard-only controls expose scene structure without introducing unsupported config fields or extra preset abstractions.

## Concerns
- The worktree still contains unrelated pre-existing changes in `src/render/ansi.rs`, `src/render/buffer.rs`, `src/render/layout.rs`, `src/scene.rs`, `tests/galaxy.rs`, plus untracked `.superpowers/sdd/progress.md` and `target/`; they were left untouched.

## Commit
- `ea5e0b4db9da8d47945d56665d14efeba837529b` — `fix: close final branch review gaps`
- `816491948d4154d0db549e3e7c945852e78b5156` — `docs: add task 10 fix report`

## Final Review Fixes
- Commit: `da3c61836f98ca22ac367d23fa9d2dc8026771c5` — `fix: address final branch review findings`
- Summary:
  - `Scene::export_command()` now falls back to `ascii-animation run --config ~/.config/ascii-animation/scene.toml` whenever a single-instance scene has non-default placement, layer, `z_index`, or `enabled` metadata.
  - Direct runtime exit now requires actual Ctrl-C; `q` and `Esc` no longer exit `ascii-animation run ...`.
  - Added focused regressions for both findings.
- Files changed:
  - `src/scene.rs`
  - `src/runtime.rs`
  - `tests/scene_config.rs`
- Exact commands and outputs:
  1. `cargo test --test scene_config`
     ```text
     ✓ cargo test: 16 passed (1 suite, 0.01s)
     ```
  2. `cargo test --test cli`
     ```text
     ✓ cargo test: 11 passed (1 suite, 0.00s)
     ```
  3. `cargo test`
     ```text
     ✓ cargo test: 51 passed (8 suites, 0.03s)
     ```
- Self-review notes:
  - Export fallback stays minimal by checking whether the single instance still matches the direct-run CLI shape.
  - Runtime quit handling now matches the spec exactly instead of sharing the TUI's `q`/`Esc` shortcuts.
- Concerns:
  - Unrelated pre-existing worktree changes remain in `src/render/ansi.rs`, `src/render/buffer.rs`, `src/render/layout.rs`, `tests/galaxy.rs`, plus untracked `.superpowers/sdd/progress.md` and `target/`.

## Final verification addendum
- Relevant fix commits: `ea5e0b4 fix: close final branch review gaps`, `8164919 docs: add task 10 fix report`, `3414d78 docs: note task 10 worktree concerns`.
- Fresh verification on current worktree:
  - `cargo test --test cli` -> `✓ cargo test: 15 passed (1 suite, 0.01s)`
  - `cargo test --test scene_config` -> `✓ cargo test: 23 passed (1 suite, 0.01s)`
  - `cargo test` -> `✓ cargo test: 65 passed (8 suites, 0.03s)`
