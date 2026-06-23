use std::collections::BTreeMap;
use std::env;
use std::path::Path;
use std::sync::Mutex;

use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};
use ascii_animation::tui::TuiState;
use ascii_animation::AsciiAnimError;
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
fn tui_state_falls_back_to_default_scene_when_default_config_is_unreadable() {
    let _home_lock = HOME_LOCK.lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    std::fs::create_dir_all(&path).unwrap();

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
fn tui_state_export_command_writes_current_config_snapshot() {
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
    let config_path = command
        .strip_prefix("ascii-animation run --config ")
        .map(|path| match path.strip_prefix("~/") {
            Some(relative) => home.join(relative),
            None => home.join(path),
        })
        .expect("config-backed export command");
    let exported_scene = Scene::load_from_path(&config_path).unwrap();

    match original_home {
        Some(value) => env::set_var("HOME", value),
        None => env::remove_var("HOME"),
    }

    assert_eq!(command, "ascii-animation run --config ~/.config/ascii-animation/tui-export.toml");
    assert_eq!(exported_scene, state.scene);
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
    assert_eq!(instance.preset, "galaxy");
}
#[test]
fn tui_state_cycles_through_custom_placement_without_collapsing_it() {
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
