use std::collections::BTreeMap;
use std::path::Path;

use ascii_animation::presets::OptionValue;
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};
use ascii_animation::AsciiAnimError;

fn galaxy_instance(id: &str) -> AnimationInstance {
    let mut options = BTreeMap::new();
    options.insert("arms".to_string(), OptionValue::Int(3));
    options.insert("palette".to_string(), OptionValue::Choice("cosmic".to_string()));

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
    let home = directories::BaseDirs::new().unwrap().home_dir().to_path_buf();

    assert_eq!(
        Scene::default_config_path(),
        home.join(".config/ascii-animation/scene.toml")
    );
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
