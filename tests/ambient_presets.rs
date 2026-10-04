use std::time::Duration;

use ascii_animation::presets::{OptionValue, PresetRegistry};
use ascii_animation::runtime::SceneSession;
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

fn scene(preset: &str) -> Scene {
    Scene {
        frame_rate: 30,
        color: true,
        instances: vec![AnimationInstance {
            id: "ambient".into(),
            preset: preset.into(),
            options: PresetRegistry::default().get(preset).unwrap().defaults(),
            placement: Placement::Custom { x: 0, y: 0, width: 30, height: 12 },
            layer: Layer::Normal,
            z_index: 0,
            enabled: true,
        }],
    }
}

fn advance(session: &mut SceneSession, milliseconds: u64) {
    for _ in 0..milliseconds / 10 {
        session.advance(Duration::from_millis(10));
        session.draw(110, 46).unwrap();
    }
}

#[test]
fn plasma_redraw_is_pure_and_visual_edits_change_without_restart() {
    let registry = PresetRegistry::default();
    let mut session = SceneSession::new(scene("plasma"), &registry, 7).unwrap();
    advance(&mut session, 500);
    let original = session.draw(110, 46).unwrap().cells().to_vec();
    assert_eq!(original, session.draw(110, 46).unwrap().cells());
    let mut changed = session.scene().clone();
    changed.instances[0].options.insert("plasma-contrast".into(), OptionValue::Float(2.0));
    let ids = session.entry_ids();
    assert!(session.apply_scene(changed, &ids.into_iter().map(Some).collect::<Vec<_>>()).unwrap().is_empty());
    assert_ne!(original, session.draw(110, 46).unwrap().cells());
    assert!(session.draw(110, 46).unwrap().cells().iter().all(|cell| cell.ch.is_ascii_graphic() || cell.ch == ' '));
}

#[test]
fn matrix_heads_lead_fading_trails_and_leave_lower_cells_transparent() {
    let registry = PresetRegistry::default();
    let mut rain = scene("matrix");
    rain.instances[0].options.insert("matrix-density".into(), OptionValue::Float(1.0));
    rain.instances[0].options.insert("matrix-speed".into(), OptionValue::Float(0.0));
    rain.instances[0].options.insert("matrix-trail".into(), OptionValue::Int(3));
    rain.instances[0].options.insert("matrix-palette".into(), OptionValue::Choice("mono".into()));
    let mut session = SceneSession::new(rain, &registry, 17).unwrap();
    let frame = session.draw(110, 46).unwrap();
    for x in 0..30 {
        let head_y = (0..12).find(|&y| frame.get(x, y).unwrap().ch == '@').unwrap();
        let head = frame.get(x, head_y).unwrap();
        for y in head_y + 1..12 {
            assert_eq!(frame.get(x, y).unwrap().ch, ' ');
        }
        for y in 0..head_y.saturating_sub(2) {
            assert_eq!(frame.get(x, y).unwrap().ch, ' ');
        }
        if head_y > 0 {
            assert!(frame.get(x, head_y - 1).unwrap().color.unwrap().r < head.color.unwrap().r);
        }
    }
}

#[test]
fn starfield_depth_recycles_without_changing_on_same_time_redraw() {
    let registry = PresetRegistry::default();
    let mut flight = scene("starfield");
    flight.instances[0].options.insert("starfield-count".into(), OptionValue::Int(200));
    flight.instances[0].options.insert("starfield-speed".into(), OptionValue::Float(4.0));
    let mut a = SceneSession::new(flight.clone(), &registry, 19).unwrap();
    let mut b = SceneSession::new(flight, &registry, 19).unwrap();
    let initial = a.draw(110, 46).unwrap().to_plain_text();
    advance(&mut a, 3000);
    for _ in 0..30 {
        b.advance(Duration::from_millis(100));
        b.draw(110, 46).unwrap();
    }
    let later = a.draw(110, 46).unwrap().cells().to_vec();
    assert_ne!(initial, a.draw(110, 46).unwrap().to_plain_text());
    assert_eq!(later, b.draw(110, 46).unwrap().cells());
    assert_eq!(later, a.draw(110, 46).unwrap().cells());
    let frame = a.draw(110, 46).unwrap();
    for y in 0..46 {
        for x in 0..110 {
            if x >= 30 || y >= 12 {
                assert_eq!(frame.get(x, y).unwrap().ch, ' ');
            }
        }
    }
}

#[test]
fn fire_retains_heat_and_fixed_steps_ignore_presentation_subdivisions() {
    let registry = PresetRegistry::default();
    let mut fine = SceneSession::new(scene("fire"), &registry, 11).unwrap();
    let mut coarse = SceneSession::new(scene("fire"), &registry, 11).unwrap();
    let initial = fine.draw(110, 46).unwrap().to_plain_text();
    advance(&mut fine, 1000);
    for _ in 0..10 {
        coarse.advance(Duration::from_millis(100));
        coarse.draw(110, 46).unwrap();
    }
    let evolved = fine.draw(110, 46).unwrap().cells().to_vec();
    assert_ne!(initial, fine.draw(110, 46).unwrap().to_plain_text());
    assert_eq!(evolved, coarse.draw(110, 46).unwrap().cells());
    assert_eq!(evolved, fine.draw(110, 46).unwrap().cells());
    fine.set_paused(true);
    fine.advance(Duration::from_secs(20));
    assert_eq!(evolved, fine.draw(110, 46).unwrap().cells());
    fine.set_paused(false);
    fine.advance(Duration::from_millis(100));
    coarse.advance(Duration::from_millis(100));
    assert_eq!(fine.draw(110, 46).unwrap().cells(), coarse.draw(110, 46).unwrap().cells());
}

#[test]
fn cold_fire_leaves_lower_layer_unchanged() {
    let registry = PresetRegistry::default();
    let mut background = scene("plasma");
    background.instances[0].layer = Layer::Background;
    let mut composed = background.clone();
    let mut fire = scene("fire").instances.remove(0);
    fire.options.insert("fire-fuel".into(), OptionValue::Float(0.0));
    composed.instances.push(fire);
    let mut a = SceneSession::new(background, &registry, 3).unwrap();
    let mut b = SceneSession::new(composed, &registry, 3).unwrap();
    advance(&mut a, 1000);
    advance(&mut b, 1000);
    assert_eq!(a.draw(110, 46).unwrap().cells(), b.draw(110, 46).unwrap().cells());
}

#[test]
fn confetti_expires_repeats_and_stays_bounded_when_bursts_overlap() {
    let registry = PresetRegistry::default();
    let mut burst = scene("confetti");
    burst.instances[0].options.insert("confetti-count".into(), OptionValue::Int(400));
    burst.instances[0].options.insert("confetti-repeat".into(), OptionValue::Float(3.0));
    let mut session = SceneSession::new(burst, &registry, 23).unwrap();
    advance(&mut session, 300);
    let first = session.draw(110, 46).unwrap().to_plain_text();
    advance(&mut session, 1700);
    assert!(session.draw(110, 46).unwrap().cells().iter().all(|cell| cell.ch == ' '));
    advance(&mut session, 1300);
    let repeated = session.draw(110, 46).unwrap().cells().to_vec();
    assert_ne!(first, session.draw(110, 46).unwrap().to_plain_text());
    assert!(repeated.iter().any(|cell| cell.ch == '*' || cell.ch == '+'));
    assert_eq!(repeated, session.draw(110, 46).unwrap().cells());
    let mut rapid = session.scene().clone();
    rapid.instances[0].options.insert("confetti-repeat".into(), OptionValue::Float(0.1));
    let ids = session.entry_ids().into_iter().map(Some).collect::<Vec<_>>();
    assert!(session.apply_scene(rapid.clone(), &ids).unwrap().is_empty());
    assert_eq!(repeated, session.draw(110, 46).unwrap().cells());
    rapid.instances[0].placement = Placement::Custom { x: 0, y: 0, width: 80, height: 30 };
    let mut rapid_session = SceneSession::new(rapid, &registry, 23).unwrap();
    advance(&mut rapid_session, 3000);
    assert!(rapid_session.draw(110, 46).unwrap().cells().iter().filter(|cell| cell.ch != ' ').count() <= 400);
}

#[test]
fn frozen_matrix_does_not_wrap_after_shortening_its_trail() {
    let registry = PresetRegistry::default();
    let mut rain = scene("matrix");
    rain.instances[0].options.insert("matrix-density".into(), OptionValue::Float(1.0));
    rain.instances[0].options.insert("matrix-speed".into(), OptionValue::Float(40.0));
    rain.instances[0].options.insert("matrix-trail".into(), OptionValue::Int(40));
    let mut session = SceneSession::new(rain, &registry, 17).unwrap();
    advance(&mut session, 500);
    let mut frozen = session.scene().clone();
    frozen.instances[0].options.insert("matrix-speed".into(), OptionValue::Float(0.0));
    frozen.instances[0].options.insert("matrix-trail".into(), OptionValue::Int(1));
    let ids = session.entry_ids().into_iter().map(Some).collect::<Vec<_>>();
    session.apply_scene(frozen, &ids).unwrap();
    let frame = session.draw(110, 46).unwrap().cells().to_vec();
    advance(&mut session, 200);
    assert_eq!(frame, session.draw(110, 46).unwrap().cells());
}
