use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::scene::Scene;
use ascii_animation::tui::{handle_tui_event, render_tui, Surface, TuiAction, TuiState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{backend::TestBackend, Terminal};

fn key(state: &mut TuiState, code: KeyCode) -> TuiAction {
    handle_tui_event(state, Event::Key(KeyEvent::new(code, KeyModifiers::NONE)), &build_default_registry()).unwrap()
}
fn type_text(state: &mut TuiState, text: &str) {
    for ch in text.chars() { key(state, KeyCode::Char(ch)); }
}
fn screen(state: &mut TuiState, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_tui(frame, &build_default_registry(), state)).unwrap();
    terminal.backend().buffer().content.iter().map(|cell| cell.symbol()).collect::<String>()
}
fn startup(dir: &std::path::Path) -> TuiState {
    TuiState::startup_at(dir.join("scene.toml"), &build_default_registry()).unwrap()
}

#[test]
fn keyboard_create_save_variation_reopen_play_and_copy_keep_distinct_targets() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    let shown = screen(&mut state, 80, 24);
    assert!(shown.contains("Presets"));
    assert!(shown.contains("Saved Scenes"));
    key(&mut state, KeyCode::Char('/'));
    type_text(&mut state, "galaxy");
    key(&mut state, KeyCode::Enter);
    state.select_option_by_name("arms").unwrap();
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Scene A");
    key(&mut state, KeyCode::Enter);
    let a_path = state.config_path().to_path_buf();
    let a = Scene::load_from_path(&a_path).unwrap();
    assert_eq!(a.instances[0].options["arms"], OptionValue::Int(4));
    assert!(!state.is_dirty());
    key(&mut state, KeyCode::Char('c'));
    let a_command = match key(&mut state, KeyCode::Enter) { TuiAction::CopyCommand(command) => command, action => panic!("{action:?}") };
    assert!(a_command.contains("run --config"));
    assert!(a_command.contains(a_path.to_str().unwrap()));
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Char('S'));
    type_text(&mut state, "Scene B");
    key(&mut state, KeyCode::Enter);
    let b_path = state.config_path().to_path_buf();
    assert_ne!(a_path, b_path);
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Char('s'));
    assert_eq!(Scene::load_from_path(&a_path).unwrap(), a);
    assert_eq!(Scene::load_from_path(&b_path).unwrap().instances[0].options["arms"], OptionValue::Int(5));
    key(&mut state, KeyCode::Char('l'));
    key(&mut state, KeyCode::Char('p'));
    assert!(state.fullscreen());
    let shown = screen(&mut state, 80, 24);
    assert!(shown.contains("Pause"));
    assert!(shown.contains("Back"));
    assert!(shown.contains("Help"));
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.surface(), Surface::SavedScenes);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, a);
    assert_eq!(state.config_path(), a_path);
    assert_eq!(key(&mut state, KeyCode::Char('q')), TuiAction::Quit);
}

#[test]
fn naming_collisions_cancellation_and_failed_save_never_copy_or_switch_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    let before = state.scene.clone();
    key(&mut state, KeyCode::Char('c'));
    assert!(screen(&mut state, 60, 18).contains("Save and Copy"));
    assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
    type_text(&mut state, "sqc");
    assert_eq!(state.draft_text(), Some("sqc"));
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, before);
    assert!(!dir.path().join("saved-scenes").exists());
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "../outside;echo bad");
    assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
    assert_eq!(state.surface(), Surface::Naming);
    assert!(state.is_dirty());
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "First");
    key(&mut state, KeyCode::Enter);
    let original_path = state.config_path().to_path_buf();
    let original = Scene::load_from_path(&original_path).unwrap();
    key(&mut state, KeyCode::Char('S'));
    type_text(&mut state, "First");
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.surface(), Surface::Confirmation);
    key(&mut state, KeyCode::Esc);
    assert_eq!(Scene::load_from_path(&original_path).unwrap(), original);
    assert_eq!(state.config_path(), original_path);
    key(&mut state, KeyCode::Char('S'));
    type_text(&mut state, "Blocked");
    std::fs::create_dir(dir.path().join("saved-scenes/Blocked.toml")).unwrap();
    key(&mut state, KeyCode::Enter);
    assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
    assert_eq!(state.surface(), Surface::SaveError);
    assert_eq!(state.config_path(), original_path);
    assert_eq!(Scene::load_from_path(&original_path).unwrap(), original);
    std::fs::remove_dir(dir.path().join("saved-scenes/Blocked.toml")).unwrap();
    key(&mut state, KeyCode::Char('r'));
    assert_eq!(state.config_path(), dir.path().join("saved-scenes/Blocked.toml"));
    assert!(!state.is_dirty());
}

fn click_label(state: &mut TuiState, label: &str, width: u16, height: u16) -> TuiAction {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| render_tui(frame, &build_default_registry(), state)).unwrap();
    let buffer = terminal.backend().buffer();
    let (column, row) = (0..height).find_map(|row| {
        let text = (0..width).map(|column| buffer[(column, row)].symbol()).collect::<String>();
        text.find(label).map(|column| (column as u16, row))
    }).unwrap_or_else(|| panic!("Missing clickable {label}"));
    handle_tui_event(state, Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers: KeyModifiers::NONE }), &build_default_registry()).unwrap()
}

#[test]
fn mouse_uses_visible_actions_modal_ownership_wheel_selection_and_fresh_resize_targets() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    click_label(&mut state, "Enter Edit", 120, 38);
    state.select_option_by_name("frame-rate").unwrap();
    let before = state.scene.clone();
    screen(&mut state, 120, 38);
    handle_tui_event(&mut state, Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: 2, row: 4, modifiers: KeyModifiers::NONE }), &registry).unwrap();
    assert_eq!(state.scene, before);
    click_label(&mut state, "s Save", 120, 38);
    type_text(&mut state, "Mouse Scene");
    let draft = state.draft_text().map(str::to_owned);
    handle_tui_event(&mut state, Event::Resize(36, 10), &registry).unwrap();
    handle_tui_event(&mut state, Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column: 110, row: 36, modifiers: KeyModifiers::NONE }), &registry).unwrap();
    assert_eq!(state.draft_text(), draft.as_deref());
    assert!(state.is_dirty());
    click_label(&mut state, "Enter Save", 36, 10);
    assert_eq!(Scene::load_from_path(state.config_path()).unwrap(), before);
    click_label(&mut state, "F1 Help/Actions", 36, 10);
    for _ in 0..4 { key(&mut state, KeyCode::Down); }
    click_label(&mut state, "l: Saved Scenes", 36, 10);
    assert_eq!(state.surface(), Surface::SavedScenes);
    click_label(&mut state, "p Play", 36, 10);
    assert!(state.fullscreen());
    click_label(&mut state, "Space Pause", 36, 10);
    assert!(state.is_paused());
    click_label(&mut state, "Esc Back", 36, 10);
    assert_eq!(state.surface(), Surface::SavedScenes);
}

#[test]
fn complete_composition_and_shared_settings_survive_reopen_without_browse_mutation() {
    use ascii_animation::scene::{Layer, Placement};
    use std::time::Duration;
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    state.add_instance("galaxy", &registry).unwrap();
    state.select_option_by_name("enabled").unwrap();
    key(&mut state, KeyCode::Right);
    state.set_selected_placement(Placement::Custom { x: 3, y: 2, width: 18, height: 9 }, &registry).unwrap();
    state.select_option_by_name("layer").unwrap();
    key(&mut state, KeyCode::Right);
    state.select_option_by_name("z-index").unwrap();
    key(&mut state, KeyCode::Right);
    state.move_selected_instance(-1, &registry).unwrap();
    state.select_option_by_name("frame-rate").unwrap();
    key(&mut state, KeyCode::Right);
    state.select_option_by_name("color").unwrap();
    key(&mut state, KeyCode::Right);
    assert_eq!(state.scene.instances[0].layer, Layer::Foreground);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Composition");
    key(&mut state, KeyCode::Enter);
    let expected = state.scene.clone();
    state.advance(Duration::from_millis(80));
    let age = state.elapsed_seconds();
    key(&mut state, KeyCode::Char('l'));
    screen(&mut state, 60, 18);
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, expected);
    assert_eq!(state.elapsed_seconds(), age);
    key(&mut state, KeyCode::Char('l'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, expected);
    assert!(!state.scene.instances[0].enabled);
    assert_eq!(state.scene.instances[0].z_index, 1);
    assert_eq!(state.scene.frame_rate, 31);
    assert!(!state.scene.color);
    assert!(!state.is_dirty());
}

#[test]
fn unsaved_transition_cancel_and_save_failure_keep_work_and_do_not_quit() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Char('q'));
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, scene);
    key(&mut state, KeyCode::Char('q'));
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Guard Scene");
    std::fs::write(dir.path().join("saved-scenes"), "not a directory").unwrap();
    assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
    assert_eq!(state.surface(), Surface::SaveError);
    assert_eq!(state.scene, scene);
    assert!(state.is_dirty());
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.surface(), Surface::Quit);
    assert_eq!(key(&mut state, KeyCode::Char('c')), TuiAction::Continue);
    assert_eq!(state.scene, scene);
}

#[test]
fn invalid_library_and_existing_default_are_recoverable_without_replacement() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("saved-scenes")).unwrap();
    let invalid = dir.path().join("saved-scenes/Broken.toml");
    std::fs::write(&invalid, "invalid toml [").unwrap();
    let default_path = dir.path().join("scene.toml");
    let original = TuiState::default_with_registry(&build_default_registry()).unwrap().scene;
    original.save_to_path(&default_path).unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Char('l'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.surface(), Surface::SavedScenes);
    key(&mut state, KeyCode::Char('e'));
    assert_eq!(state.surface(), Surface::Recovery);
    assert!(screen(&mut state, 80, 24).contains("File unchanged"));
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Down);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, original);
    assert_eq!(state.config_path(), default_path);
    assert_eq!(std::fs::read_to_string(invalid).unwrap(), "invalid toml [");
    assert_eq!(Scene::load_from_path(&default_path).unwrap(), original);
}

#[test]
fn compact_browser_panes_and_resize_keep_search_draft_and_editor_clock() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    for (width, height) in [(120, 38), (80, 24), (60, 18), (36, 10)] {
        let shown = screen(&mut state, width, height);
        assert!(shown.contains("Presets"));
        assert!(shown.contains("Help/Actions"));
        key(&mut state, KeyCode::Char('w'));
        assert!(screen(&mut state, width, height).contains("Live preview"));
        key(&mut state, KeyCode::Char('w'));
    }
    key(&mut state, KeyCode::Char('/'));
    type_text(&mut state, "sqc");
    let scene = state.scene.clone();
    screen(&mut state, 12, 4);
    assert_eq!(state.draft_text(), Some("sqc"));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.scene, scene);
    screen(&mut state, 36, 10);
    assert_eq!(state.draft_text(), Some("sqc"));
    assert!(!dir.path().join("saved-scenes").exists());
}

#[test]
fn saving_guarded_open_of_current_scene_reloads_just_saved_configuration() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Char('/'));
    type_text(&mut state, "galaxy");
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Same Scene");
    key(&mut state, KeyCode::Enter);
    state.select_option_by_name("arms").unwrap();
    key(&mut state, KeyCode::Right);
    let edited = state.scene.clone();
    key(&mut state, KeyCode::Char('l'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.surface(), Surface::Quit);
    key(&mut state, KeyCode::Char('s'));
    assert_eq!(state.scene, edited);
    assert_eq!(Scene::load_from_path(state.config_path()).unwrap(), edited);
    assert!(!state.is_dirty());
    state.select_option_by_name("arms").unwrap();
    key(&mut state, KeyCode::Right);
    assert_eq!(state.scene.instances[0].options["arms"], OptionValue::Int(5));
}

#[test]
fn successful_save_and_copy_survives_clipboard_failure_with_manual_command_visible() {
    use ascii_animation::tui::TuiEvent;
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('c'));
    key(&mut state, KeyCode::Enter);
    type_text(&mut state, "Local scene with spaces");
    let command = match key(&mut state, KeyCode::Enter) {
        TuiAction::CopyCommand(command) => command,
        action => panic!("Expected successful Save and Copy, got {action:?}"),
    };
    let saved = Scene::load_from_path(state.config_path()).unwrap();
    handle_tui_event(&mut state, TuiEvent::Clipboard(Err("clipboard unavailable".into())), &build_default_registry()).unwrap();
    assert!(!state.is_dirty());
    assert_eq!(Scene::load_from_path(state.config_path()).unwrap(), saved);
    let shown = screen(&mut state, 120, 38);
    assert!(shown.contains("clipboard unavailable"));
    assert!(shown.contains("run --config"));
    assert!(shown.contains("local Scene file"));
    assert!(command.contains("'"));
    assert!(command.contains("Local scene with spaces.toml"));
    handle_tui_event(&mut state, TuiEvent::Clipboard(Ok(())), &build_default_registry()).unwrap();
    assert!(state.copy_status().unwrap().contains("Copied command"));
}

#[test]
fn fullscreen_extended_text_matches_direct_playback_viewport_with_controls_outside_canvas() {
    use ascii_animation::presets::OptionKind;
    use ascii_animation::runtime::{scene_viewport_size_for_terminal, SceneSession};
    use ascii_animation::scene::{AnimationInstance, Layer, Placement};
    let registry = build_default_registry();
    let descriptor = registry.get("text-art").unwrap();
    let mut options = descriptor.defaults();
    let overflow = descriptor.visible_options(&options).into_iter().find(|field| {
        matches!(field.kind(), OptionKind::Choice { choices } if choices.iter().any(|choice| choice == "extend"))
    }).expect("Text supports an extended Canvas").name().to_string();
    options.insert(overflow, OptionValue::Choice("extend".into()));
    options.insert("text".into(), OptionValue::Text("A".repeat(30)));
    options.insert("text-font".into(), OptionValue::Choice("Standard".into()));
    options.insert("text-effect".into(), OptionValue::Choice("none".into()));
    let scene = Scene { frame_rate: 30, color: false, instances: vec![AnimationInstance {
        id: "wide-text".into(), preset: "text-art".into(), options, placement: Placement::Center, layer: Layer::Normal, z_index: 0, enabled: true,
    }] };
    let (width, height) = scene_viewport_size_for_terminal(&scene, &registry, 120, 38).unwrap();
    assert_eq!(width, 120);
    let mut direct = SceneSession::new(scene.clone(), &registry, 0).unwrap();
    let expected = direct.draw(width, height).unwrap();
    let mut state = TuiState::from_scene(scene, &registry).unwrap();
    key(&mut state, KeyCode::Char('f'));
    let mut terminal = Terminal::new(TestBackend::new(120, 38)).unwrap();
    terminal.draw(|frame| render_tui(frame, &registry, &mut state)).unwrap();
    let buffer = terminal.backend().buffer();
    for y in 0..height {
        let actual = (0..width).map(|x| buffer[(x, y + 1)].symbol()).collect::<String>();
        let expected = (0..width).map(|x| expected.get(x, y).unwrap().ch).collect::<String>();
        assert_eq!(actual, expected);
    }
    let shown = screen(&mut state, 120, 38);
    assert!(shown.contains("Pause"));
    assert!(shown.contains("Back"));
    assert!(shown.contains("Help"));
}

#[test]
fn tab_focus_can_cancel_naming_and_play_saved_scene_without_shortcut_or_mouse() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    let scene = state.scene.clone();
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Canceled");
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.surface(), Surface::None);
    assert_eq!(state.scene, scene);
    assert!(!dir.path().join("saved-scenes/Canceled.toml").exists());
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Tabbed");
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('l'));
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Tab);
    key(&mut state, KeyCode::Enter);
    assert!(state.fullscreen());
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.surface(), Surface::SavedScenes);
}

#[test]
fn legacy_saved_options_open_as_unsaved_normalization_without_referencing_or_overwriting_raw_file() {
    let dir = tempfile::tempdir().unwrap();
    let default_path = dir.path().join("scene.toml");
    let mut legacy = TuiState::default_with_registry(&build_default_registry()).unwrap().scene;
    legacy.instances[0].options.remove("arms");
    legacy.instances[0].options.insert("legacy-option".into(), OptionValue::Int(9));
    legacy.save_to_path(&default_path).unwrap();
    let original = std::fs::read(&default_path).unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Char('l'));
    assert!(screen(&mut state, 80, 24).contains("Open/Edit"));
    assert_eq!(std::fs::read(&default_path).unwrap(), original);
    assert_eq!(key(&mut state, KeyCode::Char('p')), TuiAction::Continue);
    assert_eq!(state.surface(), Surface::None);
    assert!(!state.fullscreen());
    assert_eq!(state.config_path(), default_path);
    assert!(state.is_dirty());
    assert!(!state.scene.instances[0].options.contains_key("legacy-option"));
    assert_eq!(state.scene.instances[0].options["arms"], OptionValue::Int(3));
    assert_eq!(std::fs::read(&default_path).unwrap(), original);
    key(&mut state, KeyCode::Char('c'));
    assert!(screen(&mut state, 80, 24).contains("Save and Copy"));
    assert_eq!(std::fs::read(&default_path).unwrap(), original);
    let command = match key(&mut state, KeyCode::Enter) {
        TuiAction::CopyCommand(command) => command,
        action => panic!("{action:?}"),
    };
    assert!(command.contains(default_path.to_str().unwrap()));
    assert_eq!(Scene::load_from_path(&default_path).unwrap(), state.scene);
    assert!(!state.is_dirty());
    key(&mut state, KeyCode::Esc);

    std::fs::create_dir(dir.path().join("saved-scenes")).unwrap();
    let named_path = dir.path().join("saved-scenes/Legacy.toml");
    legacy.save_to_path(&named_path).unwrap();
    let named_original = std::fs::read(&named_path).unwrap();
    key(&mut state, KeyCode::Right);
    let current_work = state.scene.clone();
    key(&mut state, KeyCode::Char('l'));
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.surface(), Surface::Quit);
    key(&mut state, KeyCode::Esc);
    assert_eq!(state.scene, current_work);
    assert_eq!(state.config_path(), default_path);
    assert_eq!(std::fs::read(&named_path).unwrap(), named_original);
    assert_eq!(key(&mut state, KeyCode::Char('c')), TuiAction::Continue);
    assert_eq!(state.surface(), Surface::Quit);
    key(&mut state, KeyCode::Char('d'));
    assert_eq!(state.config_path(), named_path);
    assert!(state.is_dirty());
    assert!(!state.scene.instances[0].options.contains_key("legacy-option"));
    assert_eq!(std::fs::read(&named_path).unwrap(), named_original);
}

#[cfg(unix)]
#[test]
fn unreadable_library_remains_visible_while_readable_default_can_play_copy_and_open() {
    use std::os::unix::fs::PermissionsExt;
    struct RestorePermissions {
        path: std::path::PathBuf,
        permissions: std::fs::Permissions,
    }
    impl Drop for RestorePermissions {
        fn drop(&mut self) { let _ = std::fs::set_permissions(&self.path, self.permissions.clone()); }
    }
    let dir = tempfile::tempdir().unwrap();
    let default_path = dir.path().join("scene.toml");
    let scene = TuiState::default_with_registry(&build_default_registry()).unwrap().scene;
    scene.save_to_path(&default_path).unwrap();
    let original = std::fs::read(&default_path).unwrap();
    let library = dir.path().join("saved-scenes");
    std::fs::create_dir(&library).unwrap();
    let named_path = library.join("Hidden Scene.toml");
    scene.save_to_path(&named_path).unwrap();
    let named_original = std::fs::read(&named_path).unwrap();
    let restore = RestorePermissions { path: library.clone(), permissions: std::fs::metadata(&library).unwrap().permissions() };
    std::fs::set_permissions(&library, std::fs::Permissions::from_mode(0)).unwrap();
    if std::fs::read_dir(&library).is_ok() { return; } // Privileged execution can bypass mode bits.

    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Char('l'));
    assert!(screen(&mut state, 120, 32).contains("Cannot read Saved Scenes"));
    key(&mut state, KeyCode::Char('e'));
    assert_eq!(state.surface(), Surface::Recovery);
    assert!(screen(&mut state, 120, 32).contains("permissions"));
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Char('p'));
    assert!(state.fullscreen());
    key(&mut state, KeyCode::Esc);
    assert!(screen(&mut state, 120, 32).contains("Cannot read Saved Scenes"));
    key(&mut state, KeyCode::Char('c'));
    let command = match key(&mut state, KeyCode::Enter) {
        TuiAction::CopyCommand(command) => command,
        action => panic!("{action:?}"),
    };
    assert!(command.contains(default_path.to_str().unwrap()));
    key(&mut state, KeyCode::Esc);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.config_path(), default_path);
    assert_eq!(state.scene, scene);
    assert!(!state.is_dirty());
    assert_eq!(std::fs::read(&default_path).unwrap(), original);
    key(&mut state, KeyCode::Char('l'));
    drop(restore);
    key(&mut state, KeyCode::Char('r'));
    let shown = screen(&mut state, 120, 32);
    assert!(shown.contains("Hidden Scene"));
    assert!(!shown.contains("Cannot read Saved Scenes"));
    assert_eq!(std::fs::read(&default_path).unwrap(), original);
    assert_eq!(std::fs::read(&named_path).unwrap(), named_original);
}

#[test]
fn command_focus_visibly_identifies_the_control_that_enter_activates_at_every_supported_size() {
    use ascii_animation::tui::TuiEvent;
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Command Focus");
    key(&mut state, KeyCode::Enter);
    let path = state.config_path().to_path_buf();
    let saved_bytes = std::fs::read(&path).unwrap();
    for (width, height) in [(120, 38), (80, 24), (60, 18), (36, 10)] {
        key(&mut state, KeyCode::Char('l'));
        if !state.is_paused() { key(&mut state, KeyCode::Char(' ')); }
        key(&mut state, KeyCode::Char('c'));
        assert!(screen(&mut state, width, height).contains("[>c "));
        key(&mut state, KeyCode::Tab);
        let back = screen(&mut state, width, height);
        assert!(back.contains("[>Esc "));
        assert!(!back.contains("[>c "));
        assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
        assert_eq!(state.surface(), Surface::SavedScenes);
        assert_eq!(std::fs::read(&path).unwrap(), saved_bytes);

        key(&mut state, KeyCode::Char('c'));
        key(&mut state, KeyCode::Tab);
        key(&mut state, KeyCode::BackTab);
        key(&mut state, KeyCode::PageDown);
        assert!(screen(&mut state, width, height).contains("[>c "));
        let command = match key(&mut state, KeyCode::Enter) {
            TuiAction::CopyCommand(command) => command,
            action => panic!("{action:?}"),
        };
        assert!(command.contains(path.to_str().unwrap()));
        handle_tui_event(&mut state, TuiEvent::Clipboard(Err("clipboard unavailable".into())), &build_default_registry()).unwrap();
        assert!(screen(&mut state, width, height).contains("[>c "));
        key(&mut state, KeyCode::Tab);
        handle_tui_event(&mut state, TuiEvent::Clipboard(Ok(())), &build_default_registry()).unwrap();
        assert!(screen(&mut state, width, height).contains("[>Esc "));
        assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
        assert_eq!(state.surface(), Surface::SavedScenes);
        assert!(state.is_paused());
        key(&mut state, KeyCode::Esc);
    }
    key(&mut state, KeyCode::Right);
    let unsaved = state.scene.clone();
    assert!(state.is_dirty());
    for (width, height) in [(120, 38), (80, 24), (60, 18), (36, 10)] {
        key(&mut state, KeyCode::Char('c'));
        assert!(screen(&mut state, width, height).contains("[>c "));
        key(&mut state, KeyCode::Tab);
        assert!(screen(&mut state, width, height).contains("[>Esc "));
        assert_eq!(key(&mut state, KeyCode::Enter), TuiAction::Continue);
        assert_eq!(state.surface(), Surface::None);
        assert_eq!(state.scene, unsaved);
        assert_eq!(std::fs::read(&path).unwrap(), saved_bytes);
    }
    key(&mut state, KeyCode::Char('c'));
    assert!(screen(&mut state, 36, 10).contains("[>c "));
    assert!(matches!(key(&mut state, KeyCode::Enter), TuiAction::CopyCommand(_)));
    assert_eq!(Scene::load_from_path(&path).unwrap(), unsaved);
    assert!(!state.is_dirty());
}

#[test]
fn compact_description_scrolling_reads_every_line_with_keyboard_and_wheel() {
    use crossterm::event::{MouseEvent, MouseEventKind};
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Char('/'));
    type_text(&mut state, "fire");
    key(&mut state, KeyCode::Esc);
    let mut terminal = Terminal::new(TestBackend::new(36, 10)).unwrap();
    let mut lines = Vec::new();
    for _ in 0..registry.get("fire").unwrap().description().chars().count() {
        terminal.draw(|frame| render_tui(frame, &registry, &mut state)).unwrap();
        let line = (1..35).map(|column| terminal.backend().buffer()[(column, 5)].symbol()).collect::<String>();
        let line = line.trim().to_owned();
        if line.is_empty() { break; }
        lines.push(line);
        key(&mut state, KeyCode::PageDown);
    }
    assert_eq!(lines.join(" "), registry.get("fire").unwrap().description());
    for _ in 0..lines.len() { key(&mut state, KeyCode::PageUp); }
    terminal.draw(|frame| render_tui(frame, &registry, &mut state)).unwrap();
    handle_tui_event(&mut state, Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: 2, row: 5, modifiers: KeyModifiers::NONE }), &registry).unwrap();
    terminal.draw(|frame| render_tui(frame, &registry, &mut state)).unwrap();
    let second = (1..35).map(|column| terminal.backend().buffer()[(column, 5)].symbol()).collect::<String>();
    assert_eq!(second.trim(), lines.get(1).map(String::as_str).unwrap_or(""));
}

#[test]
fn help_menu_mouse_cancel_matches_keyboard_activation_but_help_back_preserves_name_draft() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    let work = state.scene.clone();
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Canceled from Help");
    key(&mut state, KeyCode::F(1));
    click_label(&mut state, "Esc: Cancel", 80, 24);
    assert_eq!(state.surface(), Surface::None);
    assert_eq!(state.scene, work);
    assert!(!dir.path().join("saved-scenes/Canceled from Help.toml").exists());

    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Canceled from Help");
    key(&mut state, KeyCode::F(1));
    key(&mut state, KeyCode::Down);
    key(&mut state, KeyCode::Enter);
    assert_eq!(state.surface(), Surface::None);
    assert_eq!(state.scene, work);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Retained in Help");
    key(&mut state, KeyCode::F(1));
    click_label(&mut state, "Esc Back", 80, 24);
    assert_eq!(state.surface(), Surface::Naming);
    assert_eq!(state.draft_text(), Some("Retained in Help"));
    assert!(!dir.path().join("saved-scenes").exists());
}

#[test]
fn wheel_on_compact_preview_does_not_change_hidden_saved_selection_or_editor_controls() {
    use ascii_animation::tui::EditorView;
    use crossterm::event::{MouseEvent, MouseEventKind};
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Char('/'));
    type_text(&mut state, "galaxy");
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Alpha");
    key(&mut state, KeyCode::Enter);
    let alpha_path = state.config_path().to_path_buf();
    let alpha = state.scene.clone();
    state.select_option_by_name("arms").unwrap();
    key(&mut state, KeyCode::Right);
    key(&mut state, KeyCode::Char('S'));
    type_text(&mut state, "Beta");
    key(&mut state, KeyCode::Enter);
    for (width, height) in [(60, 18), (36, 10)] {
        key(&mut state, KeyCode::Char('l'));
        key(&mut state, KeyCode::Char('w'));
        screen(&mut state, width, height);
        handle_tui_event(&mut state, Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: 2, row: 4, modifiers: KeyModifiers::NONE }), &registry).unwrap();
        key(&mut state, KeyCode::Char('w'));
        key(&mut state, KeyCode::Enter);
        assert_eq!(state.config_path(), alpha_path);
        assert_eq!(state.scene, alpha);
        let selected = state.selected_option_name().unwrap().to_owned();
        key(&mut state, KeyCode::Esc);
        screen(&mut state, width, height);
        handle_tui_event(&mut state, Event::Mouse(MouseEvent { kind: MouseEventKind::ScrollDown, column: 2, row: 4, modifiers: KeyModifiers::NONE }), &registry).unwrap();
        assert_eq!(state.view(), EditorView::Preview);
        assert_eq!(state.selected_option_name(), Some(selected.as_str()));
        assert_eq!(state.scene, alpha);
        assert!(!state.is_dirty());
    }
}

#[test]
fn saved_preview_pause_is_available_by_visible_mouse_control_and_help_action() {
    use ascii_animation::tui::TuiEvent;
    use std::time::Duration;
    let registry = build_default_registry();
    let dir = tempfile::tempdir().unwrap();
    let mut state = startup(dir.path());
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('s'));
    type_text(&mut state, "Paused Preview");
    key(&mut state, KeyCode::Enter);
    key(&mut state, KeyCode::Char('l'));
    handle_tui_event(&mut state, TuiEvent::Advance(Duration::from_millis(50)), &registry).unwrap();
    click_label(&mut state, "Space Pause/Resume", 36, 10);
    assert!(state.is_paused());
    let before = state.preview_text(80, 20);
    let elapsed = state.elapsed_seconds();
    handle_tui_event(&mut state, TuiEvent::Advance(Duration::from_millis(50)), &registry).unwrap();
    assert_eq!(state.elapsed_seconds(), elapsed);
    assert_eq!(state.preview_text(80, 20), before);
    key(&mut state, KeyCode::F(1));
    click_label(&mut state, "Space: Pause/Resume", 80, 24);
    assert_eq!(state.surface(), Surface::SavedScenes);
    assert!(!state.is_paused());
    handle_tui_event(&mut state, TuiEvent::Advance(Duration::from_millis(50)), &registry).unwrap();
    assert!(state.elapsed_seconds() > elapsed);
    assert!(!state.is_dirty());
    assert_eq!(Scene::load_from_path(state.config_path()).unwrap(), state.scene);
}
