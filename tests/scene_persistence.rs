use std::io::Read;

use ascii_animation::presets::PresetRegistry;
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

fn scene() -> Scene {
    let registry = PresetRegistry::default();
    Scene { frame_rate: 30, color: false, instances: vec![AnimationInstance {
        id: "galaxy-1".into(), preset: "galaxy".into(),
        options: registry.get("galaxy").unwrap().defaults(),
        placement: Placement::Center, layer: Layer::Normal, z_index: 0, enabled: true,
    }] }
}

#[test]
fn saving_replaces_complete_scene_without_modifying_previous_reader_data() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("scene.toml");
    let mut current = scene();
    current.save_to_path(&path).unwrap();
    let previous_text = std::fs::read_to_string(&path).unwrap();
    let mut previous_reader = std::fs::File::open(&path).unwrap();
    current.color = true;
    current.save_to_path(&path).unwrap();
    let mut recoverable = String::new();
    previous_reader.read_to_string(&mut recoverable).unwrap();
    assert_eq!(recoverable, previous_text);
    assert_eq!(Scene::load_from_path(&path).unwrap(), current);
}

#[test]
fn failed_save_keeps_scene_and_prior_recoverable_file() {
    let directory = tempfile::tempdir().unwrap();
    let prior_path = directory.path().join("scene.toml");
    let current = scene();
    current.save_to_path(&prior_path).unwrap();
    let prior_text = std::fs::read_to_string(&prior_path).unwrap();
    let rejected_path = directory.path().join("cannot-replace-directory");
    std::fs::create_dir(&rejected_path).unwrap();
    let mut edited = current.clone();
    edited.color = true;
    assert!(edited.save_to_path(&rejected_path).is_err());
    assert_eq!(std::fs::read_to_string(&prior_path).unwrap(), prior_text);
    assert!(edited.color);
}
