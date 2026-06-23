use std::collections::BTreeMap;

use ascii_animation::presets::OptionValue;
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

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
fn default_config_path_uses_mandated_shape() {
    assert_eq!(
        Scene::default_config_path(),
        std::path::PathBuf::from("~/.config/ascii-animation/scene.toml")
    );
}
