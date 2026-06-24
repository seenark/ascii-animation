use std::collections::BTreeMap;
use std::env;
use std::path::Path;
use std::sync::Mutex;

use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};
use ascii_animation::tui::TuiState;
use ascii_animation::viewport::animation_viewport_size_for_terminal;
use ascii_animation::AsciiAnimError;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

static HOME_LOCK: Mutex<()> = Mutex::new(());

fn galaxy_instance(id: &str) -> AnimationInstance {
    let mut options = BTreeMap::new();
    options.insert("arms".to_string(), OptionValue::Int(3));
    options.insert(
        "palette".to_string(),
        OptionValue::Choice("cosmic".to_string()),
    );

    AnimationInstance {
        id: id.to_string(),
        preset: "galaxy".to_string(),
        options,
        placement: Placement::Center,
        layer: Layer::Normal,
        z_index: 0,
        enabled: true,
    }
}

fn text_art_instance(id: &str) -> AnimationInstance {
    let mut options = BTreeMap::new();
    options.insert("text".to_string(), OptionValue::Text("OK".to_string()));
    options.insert(
        "text-bg".to_string(),
        OptionValue::Choice("none".to_string()),
    );

    AnimationInstance {
        id: id.to_string(),
        preset: "text-art".to_string(),
        options,
        placement: Placement::Center,
        layer: Layer::Normal,
        z_index: 0,
        enabled: true,
    }
}

fn text_edit_registry() -> ascii_animation::presets::PresetRegistry {
    ascii_animation::presets::PresetRegistry::new(vec![
        ascii_animation::presets::galaxy::descriptor(),
        ascii_animation::presets::PresetDescriptor::new(
            "demo",
            "Demo",
            "Text editing demo",
            vec![ascii_animation::presets::OptionDescriptor::text(
                "message", "Message", "HELLO", 12, true,
            )],
            |_options, _seed| unreachable!("text editing test does not render"),
        ),
    ])
}

fn write_scene(scene: &Scene, path: &Path) {
    scene.save_to_path(path).unwrap();
}

#[test]
fn scene_toml_round_trips() {
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![galaxy_instance("galaxy-1")],
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");

    scene.save_to_path(&path).unwrap();
    let loaded = Scene::load_from_path(&path).unwrap();

    assert_eq!(loaded, scene);
}

#[test]
fn single_instance_exports_full_command() {
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![galaxy_instance("galaxy-1")],
    };

    assert_eq!(
        scene.export_command(),
        "ascii-animation run galaxy --arms 3 --palette cosmic"
    );
}

#[test]
fn single_text_art_instance_exports_full_command() {
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![text_art_instance("text-art-1")],
    };

    assert_eq!(
        scene.export_command(),
        "ascii-animation run text-art --text OK --text-bg none"
    );
}

#[test]
fn single_text_art_instance_exports_figlet_font_with_spaces() {
    let mut instance = text_art_instance("text-art-1");
    instance.options.insert(
        "text-font".to_string(),
        OptionValue::Choice("ANSI Regular".to_string()),
    );
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![instance],
    };
    assert_eq!(
        scene.export_command(),
        "ascii-animation run text-art --text OK --text-bg none --text-font 'ANSI Regular'"
    );
}

#[test]
fn single_instance_with_non_default_frame_rate_exports_config_command() {
    let scene = Scene {
        frame_rate: 24,
        color: true,
        instances: vec![galaxy_instance("galaxy-1")],
    };

    assert_eq!(
        scene.export_command(),
        "ascii-animation run --config ~/.config/ascii-animation/scene.toml"
    );
}
#[test]
fn single_instance_with_non_default_metadata_exports_config_command() {
    for instance in [
        AnimationInstance {
            placement: Placement::Right,
            ..galaxy_instance("galaxy-1")
        },
        AnimationInstance {
            layer: Layer::Foreground,
            ..galaxy_instance("galaxy-1")
        },
        AnimationInstance {
            z_index: 2,
            ..galaxy_instance("galaxy-1")
        },
        AnimationInstance {
            enabled: false,
            ..galaxy_instance("galaxy-1")
        },
    ] {
        let scene = Scene {
            frame_rate: 30,
            color: true,
            instances: vec![instance],
        };

        assert_eq!(
            scene.export_command(),
            "ascii-animation run --config ~/.config/ascii-animation/scene.toml"
        );
    }
}

#[test]
fn multi_instance_exports_config_command() {
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![galaxy_instance("galaxy-1"), galaxy_instance("galaxy-2")],
    };

    assert_eq!(
        scene.export_command(),
        "ascii-animation run --config ~/.config/ascii-animation/scene.toml"
    );
}

#[test]
fn multi_instance_with_single_enabled_exports_config_command() {
    let mut disabled = galaxy_instance("galaxy-1");
    disabled.enabled = false;
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![disabled, galaxy_instance("galaxy-2")],
    };

    assert_eq!(
        scene.export_command(),
        "ascii-animation run --config ~/.config/ascii-animation/scene.toml"
    );
}

#[test]
fn default_config_path_expands_home_directory() {
    let home = directories::BaseDirs::new()
        .unwrap()
        .home_dir()
        .to_path_buf();

    assert_eq!(
        Scene::default_config_path(),
        home.join(".config/ascii-animation/scene.toml")
    );
}

#[test]
fn tui_state_loads_saved_default_scene_on_startup() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    let scene = Scene {
        frame_rate: 12,
        color: false,
        instances: vec![AnimationInstance {
            placement: Placement::Right,
            ..galaxy_instance("saved-galaxy")
        }],
    };
    write_scene(&scene, &path);

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let state = TuiState::load_startup(&registry).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(state.scene.frame_rate, 12);
    assert!(!state.scene.color);
    assert_eq!(state.scene.instances.len(), 1);
    assert_eq!(state.scene.instances[0].id, "saved-galaxy");
    assert_eq!(state.scene.instances[0].placement, Placement::Right);
}

#[test]
fn tui_state_treats_normalized_startup_scene_as_fresh_export() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    let scene = Scene {
        frame_rate: 12,
        color: false,
        instances: vec![AnimationInstance {
            placement: Placement::Right,
            ..galaxy_instance("saved-galaxy")
        }],
    };
    write_scene(&scene, &path);

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let state = TuiState::load_startup(&registry).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(state.export_status(), None);
    assert_ne!(state.scene.instances[0].options, scene.instances[0].options);
}

#[test]
fn tui_state_loads_text_art_scene_with_removed_legacy_options() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    let mut instance = text_art_instance("saved-text-art");
    instance.options.insert(
        "text-font".to_string(),
        OptionValue::Choice("block".to_string()),
    );
    instance.options.insert(
        "text-fill".to_string(),
        OptionValue::Choice("auto".to_string()),
    );
    instance.options.insert("text-scale".to_string(), OptionValue::Float(1.0));
    instance.options.insert("text-spacing".to_string(), OptionValue::Int(2));
    instance.options.insert(
        "text-block-shadow".to_string(),
        OptionValue::Bool(false),
    );
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![instance],
    };
    write_scene(&scene, &path);

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let state = TuiState::load_startup(&registry).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(state.scene.instances[0].id, "saved-text-art");
    assert_eq!(
        state.scene.instances[0].options.get("text-font").unwrap(),
        &OptionValue::Choice("Standard".to_string())
    );
    assert!(!state.scene.instances[0].options.contains_key("text-fill"));
    assert!(!state.scene.instances[0].options.contains_key("text-scale"));
    assert!(!state.scene.instances[0].options.contains_key("text-spacing"));
    assert!(!state.scene.instances[0].options.contains_key("text-block-shadow"));
    assert_eq!(state.export_status(), None);
}

#[test]
fn tui_state_falls_back_to_default_scene_when_default_config_is_missing() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let state = TuiState::load_startup(&registry).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(state.scene.frame_rate, 30);
    assert!(state.scene.color);
    assert_eq!(state.scene.instances.len(), 1);
    assert_eq!(state.scene.instances[0].preset, "galaxy");
    assert_eq!(state.scene.instances[0].placement, Placement::Center);
}

#[test]
fn tui_state_surfaces_default_scene_io_errors() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    std::fs::create_dir_all(&path).unwrap();

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let err = match TuiState::load_startup(&registry) {
        Ok(_) => panic!("expected startup load to fail for non-missing default scene I/O error"),
        Err(err) => err,
    };

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert!(matches!(err, AsciiAnimError::Terminal(message) if message.contains("Is a directory")));
}

#[test]
fn tui_state_export_command_leaves_unsaved_config_scene_stale() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let saved_path = home.join(".config/ascii-animation/scene.toml");
    let saved_scene = Scene {
        frame_rate: 12,
        color: false,
        instances: vec![AnimationInstance {
            placement: Placement::Right,
            ..galaxy_instance("saved-galaxy")
        }],
    };
    write_scene(&saved_scene, &saved_path);

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let mut state = TuiState::load_startup(&registry).unwrap();
    state.scene.frame_rate = 24;
    state.scene.color = true;
    state.scene.instances[0].placement = Placement::Custom {
        x: 3,
        y: 2,
        width: 12,
        height: 8,
    };

    let command = state.export_command();
    let status = state.export_status().unwrap();
    let exported_scene = Scene::load_from_path(&saved_path).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(
        command,
        "ascii-animation run --config ~/.config/ascii-animation/scene.toml"
    );
    assert_eq!(exported_scene, saved_scene);
    assert_ne!(exported_scene, state.scene);
    assert_eq!(
        status,
        "config export is stale until you press s to save ~/.config/ascii-animation/scene.toml"
    );
}

#[test]
fn tui_state_save_updates_config_export_snapshot() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let saved_path = home.join(".config/ascii-animation/scene.toml");
    write_scene(
        &Scene {
            frame_rate: 30,
            color: true,
            instances: vec![galaxy_instance("galaxy-1")],
        },
        &saved_path,
    );

    let original_home = env::var_os("HOME");
    env::set_var("HOME", home);

    let registry = build_default_registry();
    let mut state = TuiState::load_startup(&registry).unwrap();
    state.add_instance("galaxy", &registry).unwrap();

    state.save_default_scene().unwrap();

    let exported_scene = Scene::load_from_path(&saved_path).unwrap();
    let status = state.export_status();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(exported_scene, state.scene);
    assert_eq!(status, None);
}

#[test]
fn load_from_path_rejects_unknown_preset() {
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![AnimationInstance {
            preset: "unknown".to_string(),
            ..galaxy_instance("galaxy-1")
        }],
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    write_scene(&scene, &path);

    let err = Scene::load_from_path(&path).unwrap_err();

    assert!(matches!(
        err,
        AsciiAnimError::UnknownPreset { name } if name == "unknown"
    ));
}

#[test]
fn load_from_path_accepts_text_art_scene() {
    let scene = Scene {
        frame_rate: 30,
        color: true,
        instances: vec![text_art_instance("text-art-1")],
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("text-art.toml");
    write_scene(&scene, &path);

    let loaded = Scene::load_from_path(&path).unwrap();

    assert_eq!(loaded.instances[0].preset, "text-art");
}

#[test]
fn load_from_path_rejects_unknown_option_key() {
    let mut instance = galaxy_instance("galaxy-1");
    instance
        .options
        .insert("mystery".to_string(), OptionValue::Int(7));
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![instance],
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    write_scene(&scene, &path);

    let err = Scene::load_from_path(&path).unwrap_err();

    assert!(matches!(
        err,
        AsciiAnimError::UnknownOption { preset, option }
            if preset == "galaxy" && option == "mystery"
    ));
}

#[test]
fn load_from_path_rejects_invalid_option_value() {
    let mut instance = galaxy_instance("galaxy-1");
    instance.options.insert(
        "palette".to_string(),
        OptionValue::Choice("invalid".to_string()),
    );
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![instance],
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    write_scene(&scene, &path);

    let err = Scene::load_from_path(&path).unwrap_err();

    assert!(matches!(
        err,
        AsciiAnimError::InvalidChoice { option, actual, .. }
            if option == "palette" && actual == "invalid"
    ));
}
#[test]
fn load_from_path_rejects_empty_scenes() {
    let scene = Scene {
        frame_rate: 24,
        color: false,
        instances: vec![],
    };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.toml");
    write_scene(&scene, &path);

    let err = Scene::load_from_path(&path).unwrap_err();

    assert!(matches!(err, AsciiAnimError::EmptyScene));
}

#[test]
fn tui_state_starts_with_galaxy_and_exports_command() {
    let registry = build_default_registry();
    let state = TuiState::default_with_registry(&registry).unwrap();

    assert_eq!(state.scene.instances.len(), 1);
    assert_eq!(state.scene.instances[0].preset, "galaxy");
    assert!(state
        .export_command()
        .starts_with("ascii-animation run galaxy"));
}

#[test]
fn tui_layout_places_options_on_left_and_preview_on_right() {
    let layout = ascii_animation::tui::tui_layout(Rect::new(0, 0, 100, 40));

    assert_eq!(layout.options.x, 0);
    assert_eq!(layout.options.width, 30);
    assert_eq!(layout.preview.x, 30);
    assert_eq!(layout.preview.width, 70);
    assert_eq!(layout.options.height, 40);
    assert_eq!(layout.preview.height, 40);
}

#[test]
fn tui_copy_hotkey_returns_whole_export_command() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    let key_event = KeyEvent {
        code: KeyCode::Char('c'),
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let expected = state.export_command();

    let action = ascii_animation::tui::handle_tui_key(&mut state, key_event, &registry).unwrap();

    assert_eq!(
        action,
        ascii_animation::tui::TuiAction::CopyCommand(expected)
    );
}

#[test]
fn tui_state_cycles_to_text_art_and_exposes_text_option() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();

    for _ in 0..2 {
        if state.selected_instance().preset == "text-art" {
            break;
        }
        state.cycle_selected_preset(&registry, 1).unwrap();
    }

    assert_eq!(state.selected_instance().id, "text-art-1");
    state.select_option_by_name("text").unwrap();
}

#[test]
fn tui_text_editing_updates_text_art_content() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.cycle_selected_preset(&registry, 1).unwrap();
    state.select_option_by_name("text").unwrap();

    let enter = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let push_o = KeyEvent {
        code: KeyCode::Char('O'),
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let backspace = KeyEvent {
        code: KeyCode::Backspace,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let push_k = KeyEvent {
        code: KeyCode::Char('K'),
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    ascii_animation::tui::handle_tui_key(&mut state, enter, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, push_o, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, backspace, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, push_k, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, enter, &registry).unwrap();

    assert_eq!(
        state.selected_instance().options.get("text"),
        Some(&OptionValue::Text("HELLOK".to_string()))
    );
    assert!(!state.editing_text());
}

#[test]
fn tui_text_art_text_editing_allows_more_than_twelve_chars() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.cycle_selected_preset(&registry, 1).unwrap();
    state.select_option_by_name("text").unwrap();
    state.begin_text_edit();

    for ch in "ABCDEFGHIJK".chars() {
        state.push_selected_text_char(ch).unwrap();
    }

    assert_eq!(
        state.selected_instance().options.get("text"),
        Some(&OptionValue::Text("HELLOABCDEFGHIJK".to_string()))
    );
}

#[test]
fn tui_text_art_overflow_choice_cycles_between_extend_and_slide() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.cycle_selected_preset(&registry, 1).unwrap();
    state.select_option_by_name("text-overflow").unwrap();

    assert_eq!(
        state.selected_instance().options.get("text-overflow"),
        Some(&OptionValue::Choice("extend".to_string()))
    );

    state.adjust_selected_option(1).unwrap();
    assert_eq!(
        state.selected_instance().options.get("text-overflow"),
        Some(&OptionValue::Choice("slide".to_string()))
    );

    state.adjust_selected_option(1).unwrap();
    assert_eq!(
        state.selected_instance().options.get("text-overflow"),
        Some(&OptionValue::Choice("extend".to_string()))
    );
}

#[test]
fn tui_copy_status_reports_success_and_failure() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();

    state.set_copy_status(Ok(()));
    assert_eq!(state.copy_status().unwrap(), "Copied command to clipboard");

    state.set_copy_status(Err("clipboard unavailable".to_string()));
    assert_eq!(
        state.copy_status().unwrap(),
        "Copy failed: clipboard unavailable"
    );
}

#[test]
fn tui_text_option_editing_updates_content() {
    let registry = text_edit_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.cycle_selected_preset(&registry, 1).unwrap();
    state.select_option_by_name("message").unwrap();

    let enter = KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let push_o = KeyEvent {
        code: KeyCode::Char('O'),
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let backspace = KeyEvent {
        code: KeyCode::Backspace,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };
    let push_k = KeyEvent {
        code: KeyCode::Char('K'),
        modifiers: KeyModifiers::SHIFT,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    };

    ascii_animation::tui::handle_tui_key(&mut state, enter, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, push_o, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, backspace, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, push_k, &registry).unwrap();
    ascii_animation::tui::handle_tui_key(&mut state, enter, &registry).unwrap();

    assert_eq!(
        state.selected_instance().options.get("message"),
        Some(&OptionValue::Text("HELLOK".to_string()))
    );
    assert!(!state.editing_text());
}
#[test]
fn tui_state_can_adjust_integer_option() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.select_option_by_name("arms").unwrap();

    state.adjust_selected_option(1).unwrap();

    assert_eq!(
        state.scene.instances[0]
            .options
            .get("arms")
            .unwrap()
            .as_cli_value(),
        "4"
    );
}

#[test]
fn tui_state_applies_descriptor_integer_steps() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.select_option_by_name("stars").unwrap();

    state.adjust_selected_option(1).unwrap();

    assert_eq!(
        state.scene.instances[0]
            .options
            .get("stars")
            .unwrap()
            .as_cli_value(),
        "650"
    );
}

#[test]
fn tui_state_clamps_integer_option_to_descriptor_bounds() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.select_option_by_name("arms").unwrap();

    for _ in 0..10 {
        state.adjust_selected_option(-1).unwrap();
    }

    assert_eq!(
        state.scene.instances[0]
            .options
            .get("arms")
            .unwrap()
            .as_cli_value(),
        "1"
    );
}

#[test]
fn tui_state_clamps_float_option_to_descriptor_bounds() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.select_option_by_name("noise").unwrap();

    for _ in 0..100 {
        state.adjust_selected_option(1).unwrap();
    }

    assert_eq!(
        state.scene.instances[0]
            .options
            .get("noise")
            .unwrap()
            .as_cli_value(),
        "0.5"
    );
}

#[test]
fn tui_state_can_add_remove_and_select_instances() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();

    state.add_instance("galaxy", &registry).unwrap();

    assert_eq!(state.selected_instance().id, "galaxy-2");
    assert_eq!(state.scene.instances.len(), 2);

    state.cycle_selected_instance(-1, &registry).unwrap();
    assert_eq!(state.selected_instance().id, "galaxy-1");

    state.cycle_selected_instance(1, &registry).unwrap();
    state.remove_selected_instance(&registry).unwrap();

    assert_eq!(state.scene.instances.len(), 1);
    assert_eq!(state.selected_instance().id, "galaxy-1");
}

#[test]
fn tui_state_can_edit_selected_instance_structure() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.add_instance("galaxy", &registry).unwrap();
    state
        .set_selected_placement(Placement::Right, &registry)
        .unwrap();
    state.cycle_selected_layer(1);
    state.adjust_selected_z_index(3);
    state.cycle_selected_preset(&registry, 1).unwrap();

    let instance = state.selected_instance();
    assert_eq!(instance.placement, Placement::Right);
    assert_eq!(instance.layer, Layer::Foreground);
    assert_eq!(instance.z_index, 3);
    assert_eq!(instance.preset, "text-art");
}
#[test]
fn tui_state_restores_edited_custom_placement_after_cycling_back() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();

    state
        .set_selected_placement(Placement::Fill, &registry)
        .unwrap();
    state.cycle_selected_placement(1, &registry).unwrap();
    assert!(matches!(
        state.selected_instance().placement,
        Placement::Custom { .. }
    ));

    state
        .set_selected_placement(
            Placement::Custom {
                x: 2,
                y: 3,
                width: 20,
                height: 10,
            },
            &registry,
        )
        .unwrap();
    state.cycle_selected_placement(1, &registry).unwrap();
    assert_eq!(state.selected_instance().placement, Placement::Center);

    state.cycle_selected_placement(-1, &registry).unwrap();
    assert_eq!(
        state.selected_instance().placement,
        Placement::Custom {
            x: 2,
            y: 3,
            width: 20,
            height: 10,
        }
    );
}

#[test]
fn tui_state_can_edit_custom_placement_fields() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state
        .set_selected_placement(
            Placement::Custom {
                x: 2,
                y: 3,
                width: 20,
                height: 10,
            },
            &registry,
        )
        .unwrap();

    state.select_option_by_name("placement-x").unwrap();
    state.adjust_selected_option(3).unwrap();
    state.select_option_by_name("placement-y").unwrap();
    state.adjust_selected_option(-10).unwrap();
    state.select_option_by_name("placement-width").unwrap();
    state.adjust_selected_option(-25).unwrap();
    state.select_option_by_name("placement-height").unwrap();
    state.adjust_selected_option(5).unwrap();

    assert_eq!(
        state.selected_instance().placement,
        Placement::Custom {
            x: 5,
            y: 0,
            width: 1,
            height: 15,
        }
    );
}

fn ratatui_text_to_plain_text(text: ratatui::text::Text<'_>) -> String {
    text.lines
        .into_iter()
        .map(|line| {
            line.spans
                .into_iter()
                .map(|span| span.content.into_owned())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn shared_animation_viewport_matches_tui_preview_inner_size() {
    let layout = ascii_animation::tui::tui_layout(Rect::new(0, 0, 120, 40));

    assert_eq!(
        animation_viewport_size_for_terminal(120, 40),
        (
            layout.preview.width.saturating_sub(2).max(1),
            layout.preview.height.saturating_sub(2).max(1),
        )
    );
}

#[test]
fn tui_preview_uses_same_centered_viewport_as_direct_run() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    state.scene.color = false;

    let preview = state.preview_text(&registry, 0.0, 40, 16);
    let runtime = ascii_animation::runtime::render_centered_scene_frame(
        &state.scene,
        &registry,
        0,
        0.0,
        40,
        16,
    )
    .unwrap();

    assert_eq!(ratatui_text_to_plain_text(preview), runtime.to_plain_text());
}
#[test]
fn tui_preview_uses_ratatui_styles_for_color_output() {
    let registry = build_default_registry();
    let mut state = TuiState::default_with_registry(&registry).unwrap();
    let color_preview = state.preview_text(&registry, 0.0, 40, 16);
    let color_spans: Vec<_> = color_preview
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .collect();
    assert!(color_spans
        .iter()
        .any(|span| !span.content.contains("\u{1b}")));
    assert!(color_spans
        .iter()
        .any(|span| matches!(span.style.fg, Some(Color::Rgb(_, _, _)))));

    state.scene.color = false;
    let monochrome_preview = state.preview_text(&registry, 0.0, 40, 16);
    let monochrome_spans: Vec<_> = monochrome_preview
        .lines
        .iter()
        .flat_map(|line| line.spans.iter())
        .collect();

    assert!(monochrome_spans
        .iter()
        .all(|span| !span.content.contains("\u{1b}")));
    assert!(monochrome_spans.iter().all(|span| span.style.fg.is_none()));
}
