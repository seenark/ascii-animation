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

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn failed_atomic_save_preserves_the_existing_target_scene() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("s".repeat(255));
    let original = scene();
    std::fs::write(&path, toml::to_string_pretty(&original).unwrap()).unwrap();
    let mut edited = original.clone();
    edited.color = true;
    let error = edited.save_to_path(&path).unwrap_err();
    match error {
        ascii_animation::AsciiAnimError::SceneConfigWrite { path: failed_path, source } => {
            assert_eq!(failed_path, path);
            let name_too_long = if cfg!(target_os = "macos") { 63 } else { 36 };
            assert_eq!(source.raw_os_error(), Some(name_too_long));
        }
        other => panic!("expected a filesystem save failure, got {other}"),
    }
    assert_eq!(Scene::load_from_path(&path).unwrap(), original);
}
