use std::collections::BTreeMap;
use std::time::Duration;

use ascii_animation::presets::{text_art, OptionValue, PresetRegistry};
use ascii_animation::runtime::SceneSession;
use ascii_animation::scene::{AnimationInstance, Layer, Placement, Scene};

fn options(effect: &str) -> BTreeMap<String, OptionValue> {
    let mut options = text_art::descriptor().defaults();
    for (key, value) in [
        ("text", OptionValue::Text("OK".into())),
        ("text-font", OptionValue::Choice("Standard".into())),
        ("text-effect", OptionValue::Choice(effect.into())),
        ("text-bg", OptionValue::Choice("none".into())),
        ("text-color-mode", OptionValue::Choice("solid".into())),
        ("text-palette", OptionValue::Choice("mono".into())),
        ("text-speed", OptionValue::Float(1.0)),
        ("text-hold-visible-seconds", OptionValue::Float(0.4)),
        ("text-hold-hidden-seconds", OptionValue::Float(0.3)),
        ("text-glow", OptionValue::Bool(false)),
    ] {
        options.insert(key.into(), value);
    }
    options
}

fn instance(options: BTreeMap<String, OptionValue>) -> AnimationInstance {
    AnimationInstance {
        id: "text".into(), preset: "text-art".into(), options,
        placement: Placement::Custom { x: 35, y: 16, width: 40, height: 14 },
        layer: Layer::Normal, z_index: 0, enabled: true,
    }
}

fn session(options: BTreeMap<String, OptionValue>, seed: u64) -> SceneSession {
    SceneSession::new(Scene { instances: vec![instance(options)], ..Scene::default() }, &PresetRegistry::default(), seed).unwrap()
}

fn advance(session: &mut SceneSession, seconds: f64) {
    let mut remaining = Duration::from_secs_f64(seconds);
    while !remaining.is_zero() {
        let step = remaining.min(Duration::from_millis(50));
        session.advance(step);
        remaining -= step;
    }
}

fn glyphs(session: &mut SceneSession) -> String {
    session.draw(40, 14).unwrap().to_plain_text()
}

#[test]
fn finite_effects_resolve_into_exact_configured_figlet_cells() {
    let expected = glyphs(&mut session(options("none"), 7));
    for effect in ["decrypt", "scattered"] {
        let mut session = session(options(effect), 7);
        let initial = glyphs(&mut session);
        assert_ne!(initial, expected, "{effect} must begin with transient cells");
        advance(&mut session, 2.1);
        assert_eq!(glyphs(&mut session), expected, "{effect} must converge exactly");
        assert_eq!(glyphs(&mut session), expected, "same-time redraw must preserve completion");
    }
}

#[test]
fn finite_effects_hold_visible_then_hidden_and_repeat_without_losing_overshoot() {
    let expected = glyphs(&mut session(options("none"), 7));
    for effect in ["decrypt", "scattered"] {
        let mut playback = session(options(effect), 7);
        advance(&mut playback, 2.05);
        assert_eq!(glyphs(&mut playback), expected);
        advance(&mut playback, 0.3);
        assert_eq!(glyphs(&mut playback), expected);
        advance(&mut playback, 0.1);
        assert!(glyphs(&mut playback).chars().all(|ch| ch == ' ' || ch == '\n'));
        advance(&mut playback, 0.2);
        assert!(glyphs(&mut playback).chars().all(|ch| ch == ' ' || ch == '\n'));
        playback.advance(Duration::from_millis(80));
        let overshot = glyphs(&mut playback);
        assert_ne!(overshot, expected);
        assert!(overshot.chars().any(|ch| ch.is_ascii_graphic()));
        let mut reference = session(options(effect), 7);
        advance(&mut reference, 2.73);
        assert_eq!(overshot, glyphs(&mut reference), "cycle overshoot must carry into the next reveal");
        advance(&mut playback, 2.02);
        assert_eq!(glyphs(&mut playback), expected);
    }
}

#[test]
fn zero_holds_skip_hidden_phase_and_repeat_immediately() {
    for effect in ["decrypt", "scattered"] {
        for (visible, hidden) in [(0.0, 0.0), (0.4, 0.0), (0.0, 0.3)] {
            let mut settings = options(effect);
            settings.insert("text-hold-visible-seconds".into(), OptionValue::Float(visible));
            settings.insert("text-hold-hidden-seconds".into(), OptionValue::Float(hidden));
            let mut playback = session(settings, 7);
            advance(&mut playback, 2.0 + visible + hidden + 0.03);
            let frame = glyphs(&mut playback);
            assert!(frame.chars().any(|ch| ch.is_ascii_graphic()), "{effect} must not insert a blank reset frame");
            assert!(frame.chars().all(|ch| ch == '\n' || ch.is_ascii()), "transient cells must be ASCII");
            assert_eq!(frame, glyphs(&mut playback));
        }
    }
}

#[test]
fn seeded_reveals_are_repeatable_and_visual_updates_keep_their_progress() {
    for effect in ["decrypt", "scattered"] {
        let mut playback = session(options(effect), 7);
        advance(&mut playback, 0.65);
        let before = playback.draw(40, 14).unwrap().clone();
        let mut replay = session(options(effect), 7);
        advance(&mut replay, 0.65);
        assert_eq!(before.cells(), replay.draw(40, 14).unwrap().cells());
        let mut different_seed = session(options(effect), 8);
        advance(&mut different_seed, 0.65);
        assert_ne!(before.to_plain_text(), glyphs(&mut different_seed));

        let ids = playback.entry_ids();
        let mut changed = playback.scene().clone();
        changed.instances[0].options.insert("text-palette".into(), OptionValue::Choice("fire".into()));
        assert!(playback.apply_scene(changed, &[Some(ids[0])]).unwrap().is_empty());
        let recolored = playback.draw(40, 14).unwrap().clone();
        assert_eq!(before.to_plain_text(), recolored.to_plain_text());
        assert_ne!(before.cells(), recolored.cells());

        let mut invalid = playback.scene().clone();
        invalid.instances[0].options.insert("text-speed".into(), OptionValue::Float(0.0));
        assert!(playback.apply_scene(invalid, &[Some(ids[0])]).is_err());
        assert_eq!(recolored.cells(), playback.draw(40, 14).unwrap().cells());
        advance(&mut playback, 1.45);
        let mut expected_options = options("none");
        expected_options.insert("text-palette".into(), OptionValue::Choice("fire".into()));
        assert_eq!(glyphs(&mut playback), glyphs(&mut session(expected_options, 7)));
    }
}

#[test]
fn reveal_spaces_preserve_lower_layers_during_animation_and_hidden_hold() {
    let mut background = options("none");
    background.insert("text".into(), OptionValue::Text(String::new()));
    background.insert("text-bg".into(), OptionValue::Choice("grid".into()));
    let mut lower = instance(background);
    lower.layer = Layer::Background;
    let registry = PresetRegistry::default();
    let mut lower_session = SceneSession::new(Scene {
        instances: vec![lower.clone()], ..Scene::default()
    }, &registry, 7).unwrap();
    let lower_frame = lower_session.draw(40, 14).unwrap().clone();
    assert_eq!(lower_frame.get(0, 0).unwrap().ch, '+');
    for effect in ["decrypt", "scattered"] {
        let mut composed = SceneSession::new(Scene {
            instances: vec![lower.clone(), instance(options(effect))], ..Scene::default()
        }, &registry, 7).unwrap();
        let transient = composed.draw(40, 14).unwrap().clone();
        for (background, visible) in lower_frame.cells().iter().zip(transient.cells()) {
            if background.ch != ' ' {
                assert_ne!(visible.ch, ' ', "transient cells cannot erase a lower layer");
            }
        }
        advance(&mut composed, 2.5);
        assert_eq!(composed.draw(40, 14).unwrap().cells(), lower_frame.cells());
    }
}

#[test]
fn completion_preserves_legacy_decoration_order_and_final_glyphs() {
    for effect in ["decrypt", "scattered"] {
        let mut settings = options(effect);
        for flag in ["text-drop-shadow", "text-border", "text-glow", "text-reflection", "text-particles", "text-mirror"] {
            settings.insert(flag.into(), OptionValue::Bool(true));
        }
        settings.insert("text-bg".into(), OptionValue::Choice("grid".into()));
        let mut baseline = settings.clone();
        baseline.insert("text-effect".into(), OptionValue::Choice("none".into()));
        let mut playback = session(settings, 7);
        let mut reference = session(baseline, 7);
        advance(&mut playback, 2.1);
        advance(&mut reference, 2.1);
        assert_eq!(playback.draw(40, 14).unwrap().cells(), reference.draw(40, 14).unwrap().cells());
    }
}

#[test]
fn speed_updates_keep_reveal_progress_and_holds_use_real_seconds() {
    let expected = glyphs(&mut session(options("none"), 7));
    for effect in ["decrypt", "scattered"] {
        let mut playback = session(options(effect), 7);
        advance(&mut playback, 0.65);
        let before = glyphs(&mut playback);
        let ids = playback.entry_ids();
        let mut faster = playback.scene().clone();
        faster.instances[0].options.insert("text-speed".into(), OptionValue::Float(2.0));
        assert!(playback.apply_scene(faster, &[Some(ids[0])]).unwrap().is_empty());
        assert_eq!(glyphs(&mut playback), before);
        advance(&mut playback, 0.75);
        assert_eq!(glyphs(&mut playback), expected);
        advance(&mut playback, 0.25);
        assert_eq!(glyphs(&mut playback), expected);
        advance(&mut playback, 0.15);
        assert!(glyphs(&mut playback).chars().all(|ch| ch == ' ' || ch == '\n'));
    }
}

#[test]
fn advancing_before_reconfiguration_consumes_the_old_speed_before_edit() {
    for effect in ["decrypt", "scattered"] {
        let mut lazy = session(options(effect), 7);
        let mut reference = session(options(effect), 7);
        advance(&mut lazy, 0.65);
        advance(&mut reference, 0.65);
        let old_frame = glyphs(&mut reference);
        for playback in [&mut lazy, &mut reference] {
            let ids = playback.entry_ids();
            let mut changed = playback.scene().clone();
            changed.instances[0].options.insert("text-speed".into(), OptionValue::Float(2.0));
            assert!(playback.apply_scene(changed, &[Some(ids[0])]).unwrap().is_empty());
        }
        assert_eq!(glyphs(&mut lazy), old_frame, "edit must not apply new speed retroactively");
        assert_eq!(glyphs(&mut lazy), glyphs(&mut reference));
        advance(&mut lazy, 0.5);
        advance(&mut reference, 0.5);
        assert_eq!(lazy.draw(40, 14).unwrap().cells(), reference.draw(40, 14).unwrap().cells());
    }
}

#[test]
fn extreme_instance_priority_does_not_let_decorations_replace_target_glyphs() {
    let mut clean = session(options("none"), 7);
    let target = clean.draw(40, 14).unwrap().clone();
    for effect in ["decrypt", "scattered"] {
        let mut settings = options(effect);
        settings.insert("text-bg".into(), OptionValue::Choice("grid".into()));
        settings.insert("text-drop-shadow".into(), OptionValue::Bool(true));
        let mut text = instance(settings);
        text.z_index = i32::MIN;
        let mut playback = SceneSession::new(Scene {
            instances: vec![text], ..Scene::default()
        }, &PresetRegistry::default(), 7).unwrap();
        advance(&mut playback, 2.1);
        let frame = playback.draw(40, 14).unwrap();
        for (index, target) in target.cells().iter().enumerate().filter(|(_, cell)| cell.ch != ' ') {
            assert_eq!(frame.cells()[index].ch, target.ch);
            assert_eq!(frame.cells()[index].z_index, i32::MIN);
        }
    }
}
