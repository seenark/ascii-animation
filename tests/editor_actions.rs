use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::tui::{handle_tui_event, TuiAction, TuiState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

fn key(state: &mut TuiState, code: KeyCode) -> TuiAction {
    handle_tui_event(state, Event::Key(KeyEvent::new(code, KeyModifiers::NONE)), &build_default_registry()).unwrap()
}

#[test]
fn cancelling_text_draft_preserves_committed_text_and_accepts_shortcuts_as_text() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.add_instance("text-art", &registry).unwrap();
    state.select_option_by_name("text").unwrap();
    let before = state.scene.clone();
    key(&mut state, KeyCode::Enter);
    for ch in " q?cf ".chars() { key(&mut state, KeyCode::Char(ch)); }
    assert_eq!(state.scene, before);
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, before);
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::End);
    key(&mut state, KeyCode::Char('!'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.selected_instance().options["text"], OptionValue::Text("HELLO!".into()));
}

#[test]
fn text_cursor_deletion_and_resize_preserve_draft_and_focus() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.add_instance("text-art", &registry).unwrap();
    state.select_option_by_name("text").unwrap();
    let focus = state.focus();
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Home);
    key(&mut state, KeyCode::Delete);
    key(&mut state, KeyCode::Char('Y'));
    key(&mut state, KeyCode::End);
    key(&mut state, KeyCode::Backspace);
    key(&mut state, KeyCode::Left);
    key(&mut state, KeyCode::Char('!'));
    state.resize(60, 18);
    key(&mut state, KeyCode::Tab);
    assert_eq!(state.draft_text(), Some("YEL!L"));
    assert_eq!(state.focus(), focus);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.selected_instance().options["text"], OptionValue::Text("YEL!L".into()));
    assert_eq!(state.focus(), focus);
}

#[test]
fn search_cancel_keeps_live_scene_clock_and_browser_accepts_shortcut_characters() {
    use std::time::Duration;
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    let scene = state.scene.clone();
    let dirty = state.is_dirty();
    let focus = state.focus();
    key(&mut state, KeyCode::Char('a'));
    for ch in " q?cf ".chars() { key(&mut state, KeyCode::Char(ch)); }
    assert_eq!(state.draft_text(), Some(" q?cf "));
    state.advance(Duration::from_millis(80));
    state.preview_text(48, 18);
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, scene);
    assert_eq!(state.is_dirty(), dirty);
    assert_eq!(state.focus(), focus);
    assert!((state.elapsed_seconds() - 0.08).abs() < 0.000001);
    assert!(!state.is_paused());
    key(&mut state, KeyCode::Char('a'));
    for ch in "text-art".chars() { key(&mut state, KeyCode::Char(ch)); }
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.selected_instance().preset, "text-art");
    assert_eq!(state.scene.instances.len(), 2);
    assert_eq!(state.focus(), ascii_animation::tui::PaneFocus::Inspector);
}

#[test]
fn choice_draft_cancels_without_committing_and_restores_inspector_focus() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.select_option_by_name("palette").unwrap();
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Tab);
    assert_eq!(state.surface(), ascii_animation::tui::Surface::Editor);
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, scene);
    assert_eq!(state.focus(), ascii_animation::tui::PaneFocus::Inspector);
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Enter);
    assert_ne!(state.selected_instance().options["palette"], scene.instances[0].options["palette"]);
}

#[test]
fn deletion_and_replacement_require_confirmation_and_final_entry_remains() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Char('d'));
    assert_eq!(state.scene.instances.len(), 1);
    assert!(state.status().is_some());
    state.add_instance("galaxy", &registry).unwrap();
    key(&mut state, KeyCode::BackTab);
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Char('d'));
    assert_eq!(state.scene, scene);
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, scene);
    key(&mut state, KeyCode::Char('r'));
    for ch in "text-art".chars() { key(&mut state, KeyCode::Char(ch)); }
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, scene);
    assert_eq!(state.surface(), ascii_animation::tui::Surface::Confirmation);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.selected_instance().preset, "text-art");
    assert_eq!(state.selected_instance().placement, ascii_animation::scene::Placement::Center);
    key(&mut state, KeyCode::Char('d'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene.instances.len(), 1);
}

#[test]
fn dirty_quit_cancel_and_failed_save_retain_scene_and_export_never_copies_stale_config() {
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    let original = TuiState::default_with_registry(&registry).unwrap().scene;
    original.save_to_path(&path).unwrap();
    let mut state = TuiState::load_from_path(&path, &registry).unwrap();
    state.add_instance("galaxy", &registry).unwrap();
    let scene = state.scene.clone();
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    key(&mut state, KeyCode::Char('q'));
    key(&mut state, KeyCode::Char('s'));
    assert_eq!(state.surface(), ascii_animation::tui::Surface::SaveError);
    assert_eq!(state.scene, scene);
    assert!(state.is_dirty());
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Char('c'));
    assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
    assert!(state.is_dirty());
    std::fs::remove_dir(&path).unwrap();
    let action = key(&mut state, KeyCode::Char('r'));
    assert_eq!(action, TuiAction::CopyCommand(state.export_command()));
    assert_eq!(ascii_animation::scene::Scene::load_from_path(&path).unwrap(), scene);
    assert!(!state.is_dirty());
    handle_tui_event(&mut state, ascii_animation::tui::TuiEvent::Clipboard(Err("clipboard unavailable".into())), &build_default_registry()).unwrap();
    assert!(!state.is_dirty());
    assert!(state.copy_status().unwrap().contains("clipboard unavailable"));
    assert_eq!(state.export_command(), match action { TuiAction::CopyCommand(command) => command, _ => unreachable!() });
}

#[test]
fn malformed_startup_recovery_does_not_overwrite_file_and_reload_can_recover() {
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    std::fs::write(&path, "not valid toml [").unwrap();
    let mut state = TuiState::load_from_path(&path, &registry).unwrap();
    assert_eq!(state.surface(), ascii_animation::tui::Surface::Recovery);
    assert!(state.startup_error().is_some());
    key(&mut state, KeyCode::Enter);
    assert!(state.is_dirty());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "not valid toml [");
    let mut recovery = TuiState::load_from_path(&path, &registry).unwrap();
    let mut saved = TuiState::default_with_registry(&registry).unwrap().scene;
    saved.instances[0].options.insert("arms".into(), OptionValue::Int(5));
    saved.save_to_path(&path).unwrap();
    key(&mut recovery, KeyCode::Char('r'));
    assert_eq!(recovery.surface(), ascii_animation::tui::Surface::None);
    assert_eq!(recovery.selected_instance().options["arms"], OptionValue::Int(5));
    assert!(!recovery.is_dirty());
    assert_eq!(key(&mut recovery, KeyCode::Char('q')), TuiAction::Quit);
}

fn plain(text: ratatui::text::Text<'_>) -> String {
    text.lines.into_iter().map(|line| line.spans.into_iter().map(|s| s.content.into_owned()).collect::<String>()).collect::<Vec<_>>().join("\n")
}

#[test]
fn temporary_browser_and_cancel_do_not_restart_live_simulation_on_resize() {
    use std::time::Duration;
    let registry = build_default_registry();
    let mut scene = TuiState::default_with_registry(&registry).unwrap().scene;
    scene.instances[0].preset = "fire".into();
    scene.instances[0].options = registry.get("fire").unwrap().defaults();
    let mut state = TuiState::from_scene(scene, &registry).unwrap();
    let mut uninterrupted = TuiState::from_scene(state.scene.clone(), &registry).unwrap();
    for _ in 0..8 {
        state.advance(Duration::from_millis(40));
        uninterrupted.advance(Duration::from_millis(40));
        state.preview_text(48, 18);
        uninterrupted.preview_text(48, 18);
    }
    key(&mut state, KeyCode::Char('a'));
    for ch in "fire".chars() { key(&mut state, KeyCode::Char(ch)); }
    state.resize(80, 24);
    for _ in 0..8 {
        state.advance(Duration::from_millis(40));
        uninterrupted.advance(Duration::from_millis(40));
        state.preview_text(78, 8);
        uninterrupted.preview_text(48, 18);
    }
    key(&mut state, KeyCode::Esc);
    assert_eq!(plain(state.preview_text(48, 18)), plain(uninterrupted.preview_text(48, 18)));
    key(&mut state, KeyCode::Char(' '));
    let paused = plain(state.preview_text(48, 18));
    state.advance(Duration::from_secs(30));
    key(&mut state, KeyCode::Char('f'));
    state.resize(120, 38);
    state.preview_text(118, 32);
    assert_eq!(plain(state.preview_text(48, 18)), paused);
    key(&mut state, KeyCode::Char(' '));
    state.advance(Duration::from_millis(40));
    uninterrupted.advance(Duration::from_millis(40));
    assert_eq!(plain(state.preview_text(48, 18)), plain(uninterrupted.preview_text(48, 18)));
}

#[test]
fn shared_geometry_defines_nonzero_hidden_preview_and_responsive_views() {
    use ascii_animation::viewport::{animation_viewport_size_for_terminal, editor_layout, EditorLayout};
    use ascii_animation::tui::tui_layout;
    use ratatui::layout::Rect;
    for (width, height, mode) in [(120, 38, EditorLayout::Wide), (80, 24, EditorLayout::Medium), (60, 18, EditorLayout::Small)] {
        assert_eq!(editor_layout(width, height), mode);
        let layout = tui_layout(Rect::new(0, 0, width, height));
        let dimensions = animation_viewport_size_for_terminal(width, height);
        assert_eq!(dimensions, (layout.preview.width - 2, layout.preview.height - 2));
        assert!(dimensions.0 > 0 && dimensions.1 > 0);
    }
    let mut state = TuiState::default_with_registry(&build_default_registry()).unwrap();
    state.resize(12, 4);
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Char('a'));
    key(&mut state, KeyCode::Right);
    assert_eq!(state.scene, scene);
    assert_eq!(state.surface(), ascii_animation::tui::Surface::None);
}

#[test]
fn printable_unicode_input_uses_character_boundaries_and_stays_in_its_editor() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    key(&mut state, KeyCode::Char('a'));
    for ch in "é界q".chars() { key(&mut state, KeyCode::Char(ch)); }
    key(&mut state, KeyCode::Left);
    key(&mut state, KeyCode::Backspace);
    key(&mut state, KeyCode::Home);
    key(&mut state, KeyCode::Delete);
    assert_eq!(state.draft_text(), Some("q"));
    key(&mut state, KeyCode::Esc);
    state.add_instance("text-art", &registry).unwrap();
    state.select_option_by_name("text").unwrap();
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Home);
    key(&mut state, KeyCode::Char('é'));
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Left);
    key(&mut state, KeyCode::Backspace);
    assert_eq!(state.draft_text(), Some("HELLO"));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.selected_instance().options["text"], OptionValue::Text("HELLO".into()));
}

#[test]
fn fullscreen_blocks_hidden_edits_and_restores_selected_field_focus() {
    use ascii_animation::tui::PaneFocus;
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.select_option_by_name("arms").unwrap();
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Char('f'));
    assert!(state.fullscreen());
    assert_eq!(state.focus(), PaneFocus::Preview);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, scene);
    key(&mut state, KeyCode::Esc);
    assert!(!state.fullscreen());
    assert_eq!(state.focus(), PaneFocus::Inspector);
    assert_eq!(state.selected_option_name(), Some("arms"));
    key(&mut state, KeyCode::Right);
    assert_eq!(state.selected_instance().options["arms"], OptionValue::Int(4));
}

#[test]
fn color_preview_uses_terminal_styles_without_ansi_and_monochrome_keeps_same_cells() {
    use ratatui::style::Color;
    let registry = build_default_registry();
    let mut color = TuiState::default_with_registry(&registry).unwrap();
    let mut scene = color.scene.clone();
    scene.color = false;
    let mut monochrome = TuiState::from_scene(scene, &registry).unwrap();
    let color_frame = color.preview_text(48, 18);
    let monochrome_frame = monochrome.preview_text(48, 18);
    assert!(color_frame.lines.iter().flat_map(|line| &line.spans).any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _)))));
    assert!(monochrome_frame.lines.iter().flat_map(|line| &line.spans).all(|span| span.style.fg.is_none()));
    assert_eq!(plain(color_frame), plain(monochrome_frame));
    assert!(!plain(color.preview_text(48, 18)).contains('\u{1b}'));
}

#[test]
fn tiny_resize_cannot_accept_invisible_discard_confirmation() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Char('q'));
    state.resize(12, 4);
    assert_eq!(key(&mut state, KeyCode::Left), TuiAction::Continue);
    assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
    assert_eq!(key(&mut state, KeyCode::Char('d')), TuiAction::Continue);
    assert_eq!(state.scene, scene);
    assert!(state.is_dirty());
    state.resize(36, 10);
    assert_eq!(key(&mut state, KeyCode::Char('c')), TuiAction::Continue);
    key(&mut state, KeyCode::Char('q'));
    assert_eq!(key(&mut state, KeyCode::Char('d')), TuiAction::Quit);
}

#[test]
fn invalid_text_reports_field_error_without_committing_and_can_be_corrected() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.add_instance("text-art", &registry).unwrap();
    state.select_option_by_name("text").unwrap();
    state.resize(36, 10);
    let previous_scene = state.scene.clone();
    let previous_status = state.status().map(str::to_owned);
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('é'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, previous_scene);
    assert_eq!(state.surface(), ascii_animation::tui::Surface::Editor);
    assert_ne!(state.status(), previous_status.as_deref());
    key(&mut state, KeyCode::Backspace);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, previous_scene);
    assert_eq!(state.surface(), ascii_animation::tui::Surface::None);
    assert_eq!(state.status(), previous_status.as_deref());
}

#[test]
fn wide_layout_keeps_sidebar_and_preview_bounded_across_u16_terminal_widths() {
    use ascii_animation::tui::tui_layout;
    use ascii_animation::viewport::animation_viewport_size_for_terminal;
    use ratatui::layout::Rect;
    for width in [110, 2184, 2185, 10000, u16::MAX] {
        // Rect::new caps area and can change the layout mode before this seam runs.
        let area = Rect { x: 0, y: 0, width, height: 38 };
        let layout = tui_layout(area);
        assert!((32..=42).contains(&layout.options.width));
        assert_eq!(layout.preview.x, layout.options.width);
        for pane in [layout.options, layout.preview] {
            assert!(u32::from(pane.x) + u32::from(pane.width) <= u32::from(width));
            assert!(u32::from(pane.y) + u32::from(pane.height) <= u32::from(area.height));
        }
        let (viewport_width, viewport_height) = animation_viewport_size_for_terminal(width, 38);
        assert!(viewport_width <= layout.preview.width);
        assert!(viewport_height <= layout.preview.height);
    }
}
