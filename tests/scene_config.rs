use std::collections::BTreeMap;
use std::path::Path;

use ascii_animation::presets::{build_default_registry, OptionValue};
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};
use ascii_animation::tui::TuiState;
use ascii_animation::AsciiAnimError;


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


    let registry = build_default_registry();
    let state = TuiState::load_from_path(&path, &registry).unwrap();


    assert_eq!(state.scene.frame_rate, 12);
    assert!(!state.scene.color);
    assert_eq!(state.scene.instances.len(), 1);
    assert_eq!(state.scene.instances[0].id, "saved-galaxy");
    assert_eq!(state.scene.instances[0].placement, Placement::Right);
}

#[test]
fn normalized_startup_scene_requires_save_before_config_export() {
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


    let registry = build_default_registry();
    let state = TuiState::load_from_path(&path, &registry).unwrap();


    assert!(state.is_dirty());
    assert!(state.export_status().is_some());
    assert_ne!(state.scene.instances[0].options, scene.instances[0].options);
}

#[test]
fn tui_state_loads_text_art_scene_with_removed_legacy_options() {
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


    let registry = build_default_registry();
    let state = TuiState::load_from_path(&path, &registry).unwrap();


    assert_eq!(state.scene.instances[0].id, "saved-text-art");
    assert_eq!(
        state.scene.instances[0].options.get("text-font").unwrap(),
        &OptionValue::Choice("Standard".to_string())
    );
    assert!(!state.scene.instances[0].options.contains_key("text-fill"));
    assert!(!state.scene.instances[0].options.contains_key("text-scale"));
    assert!(!state.scene.instances[0].options.contains_key("text-spacing"));
    assert!(!state.scene.instances[0].options.contains_key("text-block-shadow"));
    assert!(state.is_dirty());
}

#[test]
fn tui_state_falls_back_to_default_scene_when_default_config_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".config/ascii-animation/scene.toml");


    let registry = build_default_registry();
    let state = TuiState::load_from_path(&path, &registry).unwrap();


    assert_eq!(state.scene.frame_rate, 30);
    assert!(state.scene.color);
    assert_eq!(state.scene.instances.len(), 1);
    assert_eq!(state.scene.instances[0].preset, "galaxy");
    assert_eq!(state.scene.instances[0].placement, Placement::Center);
}

#[test]
fn tui_state_surfaces_default_scene_io_errors() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let path = home.join(".config/ascii-animation/scene.toml");
    std::fs::create_dir_all(&path).unwrap();


    let registry = build_default_registry();
    let state = TuiState::load_from_path(&path, &registry).unwrap();


    assert!(state.startup_error().is_some());
    assert_eq!(state.surface(), ascii_animation::tui::Surface::Recovery);
}

#[test]
fn tui_state_export_command_leaves_unsaved_config_scene_stale() {
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


    let registry = build_default_registry();
    let mut state = TuiState::load_from_path(&saved_path, &registry).unwrap();
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


    assert!(command.contains(&saved_path.to_string_lossy().to_string()));
    assert_eq!(exported_scene, saved_scene);
    assert_ne!(exported_scene, state.scene);
    assert!(!status.is_empty());
}

#[test]
fn tui_state_save_updates_config_export_snapshot() {
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


    let registry = build_default_registry();
    let mut state = TuiState::load_from_path(&saved_path, &registry).unwrap();
    state.add_instance("galaxy", &registry).unwrap();

    state.save_default_scene().unwrap();

    let exported_scene = Scene::load_from_path(&saved_path).unwrap();
    let status = state.export_status();


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

