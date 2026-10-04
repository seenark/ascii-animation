use std::time::Duration;

use ascii_animation::presets::{OptionValue, PresetRegistry};
use ascii_animation::runtime::SceneSession;
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

fn galaxy_scene(registry: &PresetRegistry) -> Scene {
    Scene {
        frame_rate: 30,
        color: false,
        instances: vec![AnimationInstance {
            id: "same-display-id".into(),
            preset: "galaxy".into(),
            options: registry.get("galaxy").unwrap().defaults(),
            placement: Placement::Fill,
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    }
}

#[test]
fn pause_and_same_time_redraw_preserve_the_visible_frame() {
    let registry = PresetRegistry::default();
    let scene = galaxy_scene(&registry);
    let mut session = SceneSession::new(scene, &registry, 17).unwrap();
    let initial = session.draw(80, 24).unwrap().to_plain_text();
    session.advance(Duration::from_millis(100));
    let moving = session.draw(80, 24).unwrap().to_plain_text();
    assert_ne!(initial, moving);
    assert_eq!(session.draw(80, 24).unwrap().to_plain_text(), moving);
    session.set_paused(true);
    session.advance(Duration::from_secs(10));
    assert_eq!(session.draw(80, 24).unwrap().to_plain_text(), moving);
    session.set_paused(false);
    session.advance(Duration::from_millis(50));
    let mut expected = SceneSession::new(session.scene().clone(), &registry, 17).unwrap();
    expected.advance(Duration::from_millis(100));
    expected.advance(Duration::from_millis(50));
    assert_eq!(session.draw(80, 24).unwrap().cells(), expected.draw(80, 24).unwrap().cells());
}

#[test]
fn invalid_committed_edit_retains_scene_and_visible_frame() {
    let registry = PresetRegistry::default();
    let mut session = SceneSession::new(galaxy_scene(&registry), &registry, 17).unwrap();
    session.advance(Duration::from_millis(100));
    let previous_scene = session.scene().clone();
    let previous_frame = session.draw(80, 24).unwrap().to_plain_text();
    let mut invalid = previous_scene.clone();
    invalid.instances[0].options.insert("stars".into(), OptionValue::Int(-1));
    let identities = session.entry_ids().into_iter().map(Some).collect::<Vec<_>>();
    assert!(session.apply_scene(invalid, &identities).is_err());
    assert_eq!(session.scene(), &previous_scene);
    assert_eq!(session.draw(80, 24).unwrap().to_plain_text(), previous_frame);
}

#[test]
fn low_fps_interval_is_not_clamped_to_one_tenth_second() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.frame_rate = 2;
    let mut slow = SceneSession::new(scene, &registry, 17).unwrap();
    let mut ordinary = SceneSession::new(galaxy_scene(&registry), &registry, 17).unwrap();
    slow.advance(Duration::from_millis(500));
    for _ in 0..5 {
        ordinary.advance(Duration::from_millis(100));
    }
    assert_eq!(slow.draw(80, 24).unwrap().cells(), ordinary.draw(80, 24).unwrap().cells());
    slow.advance(Duration::from_secs(60));
    for _ in 0..5 {
        ordinary.advance(Duration::from_millis(100));
    }
    assert_eq!(slow.draw(80, 24).unwrap().cells(), ordinary.draw(80, 24).unwrap().cells());
}

#[test]
fn deleting_duplicate_identifier_neighbor_preserves_survivor_seed_and_age() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.instances[0].enabled = false;
    let mut survivor = scene.instances[0].clone();
    survivor.enabled = true;
    scene.instances.push(survivor.clone());
    let mut session = SceneSession::new(scene, &registry, 17).unwrap();
    for _ in 0..10 {
        session.advance(Duration::from_millis(100));
        session.draw(80, 24).unwrap();
    }
    let before = session.draw(80, 24).unwrap().to_plain_text();
    let identity = session.entry_ids()[1];
    let mut edited = session.scene().clone();
    edited.instances.remove(0);
    session.apply_scene(edited.clone(), &[Some(identity)]).unwrap();
    assert_eq!(session.draw(80, 24).unwrap().to_plain_text(), before);
    let mut expected = SceneSession::new(edited, &registry, 18).unwrap();
    for _ in 0..10 { expected.advance(Duration::from_millis(100)); }
    assert_eq!(session.draw(80, 24).unwrap().cells(), expected.draw(80, 24).unwrap().cells());
}

#[test]
fn live_visual_reconfiguration_does_not_restart_local_age() {
    let registry = PresetRegistry::default();
    let mut session = SceneSession::new(galaxy_scene(&registry), &registry, 17).unwrap();
    for _ in 0..10 { session.advance(Duration::from_millis(100)); }
    let before = session.draw(80, 24).unwrap().to_plain_text();
    let mut edited = session.scene().clone();
    edited.instances[0].options.insert("palette".into(), OptionValue::Choice("ice".into()));
    let identities = session.entry_ids().into_iter().map(Some).collect::<Vec<_>>();
    assert!(session.apply_scene(edited.clone(), &identities).unwrap().is_empty());
    assert_eq!(session.draw(80, 24).unwrap().to_plain_text(), before);
    let mut expected = SceneSession::new(edited, &registry, 17).unwrap();
    for _ in 0..10 { expected.advance(Duration::from_millis(100)); }
    assert_eq!(session.draw(80, 24).unwrap().cells(), expected.draw(80, 24).unwrap().cells());
}

#[test]
fn newly_added_instance_begins_at_local_age_zero_after_long_session() {
    let registry = PresetRegistry::default();
    let mut session = SceneSession::new(galaxy_scene(&registry), &registry, 17).unwrap();
    for _ in 0..100 { session.advance(Duration::from_millis(100)); }
    let mut edited = session.scene().clone();
    edited.instances[0].enabled = false;
    edited.instances.push(galaxy_scene(&registry).instances.remove(0));
    let previous = session.entry_ids()[0];
    session.apply_scene(edited, &[Some(previous), None]).unwrap();
    let mut fresh = SceneSession::new(galaxy_scene(&registry), &registry, 18).unwrap();
    assert_eq!(session.draw(80, 24).unwrap().to_plain_text(), fresh.draw(80, 24).unwrap().to_plain_text());
}

#[test]
fn elapsed_fire_heat_uses_previous_fuel_before_committing_a_live_edit() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.instances[0].preset = "fire".into();
    scene.instances[0].options = registry.get("fire").unwrap().defaults();
    let mut without_redraw = SceneSession::new(scene.clone(), &registry, 17).unwrap();
    let mut with_redraw = SceneSession::new(scene, &registry, 17).unwrap();
    for _ in 0..3 {
        without_redraw.advance(Duration::from_millis(100));
        with_redraw.advance(Duration::from_millis(100));
        with_redraw.draw(110, 46).unwrap();
    }
    for session in [&mut without_redraw, &mut with_redraw] {
        let mut edited = session.scene().clone();
        edited.instances[0].options.insert("fire-fuel".into(), OptionValue::Float(0.0));
        let identities = session.entry_ids().into_iter().map(Some).collect::<Vec<_>>();
        session.apply_scene(edited, &identities).unwrap();
    }
    assert_eq!(without_redraw.draw(110, 46).unwrap().cells(), with_redraw.draw(110, 46).unwrap().cells());
}

#[test]
fn disabled_fire_progresses_and_viewport_resize_does_not_reset_heat() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.instances[0].preset = "fire".into();
    scene.instances[0].options = registry.get("fire").unwrap().defaults();
    let mut visible = SceneSession::new(scene.clone(), &registry, 17).unwrap();
    scene.instances[0].enabled = false;
    let mut disabled = SceneSession::new(scene, &registry, 17).unwrap();
    for _ in 0..10 {
        visible.advance(Duration::from_millis(100));
        disabled.advance(Duration::from_millis(100));
        visible.draw(110, 46).unwrap();
        assert!(disabled.draw(40, 16).unwrap().cells().iter().all(|cell| cell.ch == ' '));
    }
    let mut enabled_scene = disabled.scene().clone();
    enabled_scene.instances[0].enabled = true;
    let identities = disabled.entry_ids().into_iter().map(Some).collect::<Vec<_>>();
    assert!(disabled.apply_scene(enabled_scene, &identities).unwrap().is_empty());
    assert_eq!(disabled.draw(110, 46).unwrap().cells(), visible.draw(110, 46).unwrap().cells());
    let before = visible.draw(110, 46).unwrap().cells().to_vec();
    visible.draw(1, 1).unwrap();
    assert_eq!(visible.draw(110, 46).unwrap().cells(), before);
}

#[test]
fn resolved_region_resize_rebuilds_only_dimension_dependent_entries_with_original_seed() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.instances[0].preset = "fire".into();
    scene.instances[0].options = registry.get("fire").unwrap().defaults();
    scene.instances[0].placement = Placement::Center;
    let mut session = SceneSession::new(scene, &registry, 17).unwrap();
    for _ in 0..10 {
        session.advance(Duration::from_millis(100));
        session.draw(110, 46).unwrap();
    }
    let mut enlarged = session.scene().clone();
    enlarged.instances[0].placement = Placement::Fill;
    let identity = session.entry_ids()[0];
    assert_eq!(session.apply_scene(enlarged.clone(), &[Some(identity)]).unwrap(), vec![identity]);
    let mut fresh = SceneSession::new(enlarged, &registry, 17).unwrap();
    assert_eq!(session.draw(110, 46).unwrap().cells(), fresh.draw(110, 46).unwrap().cells());
}

#[test]
fn reordered_duplicate_identifiers_keep_their_heat_and_live_seed() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.instances[0].preset = "fire".into();
    scene.instances[0].options = registry.get("fire").unwrap().defaults();
    scene.instances[0].placement = Placement::Custom { x: 0, y: 0, width: 40, height: 20 };
    let mut neighbor = scene.instances[0].clone();
    neighbor.placement = Placement::Custom { x: 55, y: 0, width: 40, height: 20 };
    scene.instances.push(neighbor);
    let mut session = SceneSession::new(scene, &registry, 17).unwrap();
    for _ in 0..10 {
        session.advance(Duration::from_millis(100));
        session.draw(110, 46).unwrap();
    }
    let before = session.draw(110, 46).unwrap().to_plain_text();
    let mut reordered = session.scene().clone();
    reordered.instances.swap(0, 1);
    let mut ids = session.entry_ids();
    ids.swap(0, 1);
    assert!(session.apply_scene(reordered, &ids.into_iter().map(Some).collect::<Vec<_>>()).unwrap().is_empty());
    assert_eq!(session.draw(110, 46).unwrap().to_plain_text(), before);
}

#[test]
fn initial_seed_addition_wraps_and_deleted_creation_ordinals_are_not_reused() {
    let registry = PresetRegistry::default();
    let mut scene = galaxy_scene(&registry);
    scene.instances[0].enabled = false;
    scene.instances.push(galaxy_scene(&registry).instances.remove(0));
    let mut session = SceneSession::new(scene, &registry, u64::MAX).unwrap();
    let mut wrapped = SceneSession::new(galaxy_scene(&registry), &registry, 0).unwrap();
    assert_eq!(session.draw(110, 46).unwrap().to_plain_text(), wrapped.draw(110, 46).unwrap().to_plain_text());
    let survivor = session.entry_ids()[1];
    let mut scene = session.scene().clone();
    scene.instances.remove(0);
    scene.instances[0].enabled = false;
    session.apply_scene(scene.clone(), &[Some(survivor)]).unwrap();
    scene.instances.push(galaxy_scene(&registry).instances.remove(0));
    session.apply_scene(scene, &[Some(survivor), None]).unwrap();
    let mut expected = SceneSession::new(galaxy_scene(&registry), &registry, 1).unwrap();
    assert_eq!(session.draw(110, 46).unwrap().to_plain_text(), expected.draw(110, 46).unwrap().to_plain_text());
}
